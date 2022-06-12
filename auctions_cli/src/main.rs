use chrono::prelude::*;
use crossterm::{
    event::{self, Event, KeyCode, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen,
        LeaveAlternateScreen,},
};

use rand::{Rng, distributions::Alphanumeric, prelude::*};
use serde::{Deserialize, Serialize};
//use serde_json::json;
use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use std::{error::Error};
use thiserror::Error;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
    },
    Frame, Terminal,
};
use tui_input::backend::crossterm as input_backend;
use tui_input::Input;
use std::env;


#[path = "../../src/aux.rs"]
mod aux;
#[path = "../../src/blockchain.rs"]
mod blockchain;
#[path = "../../src/bootstrap.rs"]
mod bootstrap;
#[path = "../../src/kademlia.rs"]
mod kademlia;
#[path = "../../src/lib.rs"]
mod lib;
#[path = "../../src/node.rs"]
mod node;
#[path = "../../src/pubsub.rs"]
mod pubsub;
#[path = "../../src/rpc.rs"]
mod rpc;
use crate::lib::{NODETIMEOUT, K_PARAM, N_KBUCKETS, KEY_LEN, ALPHA, TREPLICATE};
use crate::bootstrap::{App, Bootstrap};
use crate::node::{Node};

#[derive(Serialize, Deserialize, Clone)]
struct Topic {
    ttl: String,
    highest_bid: usize,
    highest_bidder: String,
    id: String,
    name: String,
    num_subs: usize,
    subscribed: bool
}

enum InputMode {
    Home,
    Topics,
    Editing,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
    Topics,
    //Input,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Topics => 1,
            //MenuItem::Input => 2,
        }
    }
}

/// App holds the state of the application
struct AppCli {
    /// Current value of the input box
    input: Input,
    /// Current input mode
    input_mode: InputMode,
    /// History of recorded messages
    messages: Vec<String>,
}

impl Default for AppCli {
    fn default() -> AppCli {
        AppCli {
            input: Input::default(),
            input_mode: InputMode::Home,
            messages: Vec::new(),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {//Box<dynapp.input_mode = InputMode::Topics; Error>> {
    let cmd_args: Vec<String> = env::args().collect();
    let port: u16 = cmd_args[1].parse::<u16>().unwrap();
    let boot_ip: String = cmd_args[2].clone();
    let mut rng = rand::thread_rng();
    let rand_n = rng.gen_range(0..4);
    let boot_port: u16 = format!("133{}", rand_n.clone()).parse::<u16>().unwrap();
    let boot_node: Node = Node::new(boot_ip, boot_port);
    // setup terminal
    enable_raw_mode()?;

    let app = App::new(aux::get_ip().unwrap(), port, boot_node);
    //let test_id = format!("test-{}", rand_n);
    //app.publish(test_id);


    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let appcli = AppCli::default();
    let res = run_app(&mut terminal, appcli, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut appcli: AppCli,
    mut app: App,
) -> io::Result<()> {

    let menu_titles = vec!["Home", "Topics", "Publish", "Subscribe", "Bid", "Quit"];
    let mut active_menu_item = MenuItem::Home;
    let mut topic_list_state = ListState::default();
    topic_list_state.select(Some(0));

    let mut flag = 0; // 0=none, 1=bid, 2=publish

    loop {
        //terminal.draw(|f| ui(f, &mut app))?;
        
        terminal.draw(|f| {            
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Min(2),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    Spans::from(vec![
                        Span::styled(
                            first,
                            Style::default()
                                .fg(Color::LightBlue)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(rest, Style::default().fg(Color::White)),
                        ])
                })
                .collect();

            let tabs = Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::LightBlue))
                .divider(Span::raw("|"));

            f.render_widget(tabs, chunks[0]);
            
            match active_menu_item {
                MenuItem::Home => f.render_widget(render_home(), chunks[1]),
                MenuItem::Topics => {
                    let topic_list = get_topics(&app).expect("can fetch topic list");
                    
                    if topic_list.len() != 0 {
                        let topics_chunk = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .split(chunks[1]);
                        let (left, right) = render_topics(&topic_list_state, &app, topic_list);
                        f.render_stateful_widget(left, topics_chunk[0], &mut topic_list_state);
                        f.render_widget(right, topics_chunk[1]);
                    } else {
                        let empty = Paragraph::new(vec![
                            Spans::from(vec![Span::raw("")]),
                            Spans::from(vec![Span::raw("")]),
                            Spans::from(vec![Span::raw("")]),
                            Spans::from(vec![Span::raw("There is no auction running at the moment")]),
                        ])
                        .alignment(Alignment::Center)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .style(Style::default().fg(Color::White))
                                .border_type(BorderType::Plain),
                        );

                        f.render_widget(empty, chunks[1]);
                    }
                },
                //MenuItem::Input => f.render_widget(render_home(), chunks[1]),
            }

            let width = chunks[0].width.max(3) - 3; // keep 2 for borders and 1 for cursor
            let scroll = (appcli.input.cursor() as u16).max(width) - width;
            
            
            match appcli.input_mode {
                InputMode::Home =>
                    // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
                    {}
                InputMode::Topics =>
                    // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
                    {}

                InputMode::Editing => {
                    let mut title = "Input";
                    if flag == 1 {
                        title = "Bid (ex: <value>)";
                    } else if flag == 2 {
                        title = "Publish (ex: <topic_name>:<initial_value>)";
                    }

                    let input = Paragraph::new(appcli.input.value())
                        .style(match appcli.input_mode {
                            InputMode::Home => Style::default(),
                            InputMode::Topics => Style::default(),
                            InputMode::Editing => Style::default().fg(Color::Green),
                        })
                        .scroll((0, scroll))
                        .block(Block::default().borders(Borders::ALL).title(title));
                    f.render_widget(input, chunks[2]);
                    // Make the cursor visible and ask tui-rs to put it at the specified coordinates after rendering
                    f.set_cursor(
                        // Put cursor past the end of the input text
                        chunks[2].x + (appcli.input.cursor() as u16).min(20) + 1,
                        // Move one line down, from the border to the input line
                        chunks[2].y + 1,
                    )
                }
            }

        
        });
        
        
        //////////////////////////////////////////////////////////////////////////////////////
        //////////////////////////////////////////////////////////////////////////////////////
        //////////////////////////////////////////////////////////////////////////////////////
        //////////////////////////////////////////////////////////////////////////////////////
        
        
        
        if let Event::Key(key) = event::read()? {
            match appcli.input_mode {
                InputMode::Home => match key.code {
                    KeyCode::Char('t') => { // show topics menu
                        active_menu_item = MenuItem::Topics;
                        appcli.input_mode = InputMode::Topics;
                    }
                    KeyCode::Char('q') => { // quit cli app
                        return Ok(());
                    }
                    _ => {}
                },
                InputMode::Topics => match key.code {
                    KeyCode::Char('h') => { // show home menu
                        active_menu_item = MenuItem::Home;
                        appcli.input_mode = InputMode::Home;
                    }
                    KeyCode::Char('s') => { // subscribe selected topic
                        let result = subscribe_topic(&mut topic_list_state, &app);
                    }
                    KeyCode::Char('p') => { // publish new topic
                        flag = 2;
                        appcli.input_mode = InputMode::Editing;
                    }
                    KeyCode::Char('b') => { // bid on selected topic
                        if let Some(selected) = topic_list_state.selected() {
                            let topic_list = get_topics(&app).expect("can fetch topic list");
                            let selected_topic = topic_list
                                .get(
                                    topic_list_state
                                        .selected()
                                        .expect("there is always a selected topic"),
                                )
                                .expect("exists")
                                .clone();

                            if selected_topic.subscribed {
                                flag = 1;
                                appcli.input_mode = InputMode::Editing;
                            }
                        }
                        
                    }
                    KeyCode::Char('q') => { // quit cli app
                        return Ok(());
                    }
                    KeyCode::Down => { // navigate in topics menu
                        if let Some(selected) = topic_list_state.selected() {
                            let amount_topics = get_topics(&app).expect("can fetch topic list").len();
                            if selected >= amount_topics - 1 {
                                topic_list_state.select(Some(0));
                            } else {
                                topic_list_state.select(Some(selected + 1));
                            }
                            use serde_json::json;  }
                    }
                    KeyCode::Up => {
                        if let Some(selected) = topic_list_state.selected() {
                            let amount_topics = get_topics(&app).expect("can fetch topic list").len();//get_topics().expect("can fetch topic list").len();
                            if selected > 0 {
                                topic_list_state.select(Some(selected - 1));
                            } else {
                                topic_list_state.select(Some(amount_topics - 1));
                            }
                        }
                    }
                    _ => {}
                },
                InputMode::Editing => match key.code {
                    KeyCode::Enter => {
                        appcli.messages.push(appcli.input.value().into());
                        appcli.input.reset();

                        if flag == 1 {
                            let result = bid_topic(&mut topic_list_state, &appcli.messages, &app).expect("can bid on topic");
                        } 
                        else if flag == 2 {
                            let result = publish_topic(&appcli.messages, &app);
                        } 
                        
                        appcli.messages.clear();
                        appcli.input_mode = InputMode::Topics;
                    }
                    KeyCode::Esc => {
                        appcli.input_mode = InputMode::Topics;
                    }
                    _ => {
                        input_backend::to_input_request(Event::Key(key))
                            .and_then(|req| appcli.input.handle(req));
                    }
                },
            }
        }
    }
}

fn render_home<'a>() -> Paragraph<'a> {
    let home = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "Auctions CLI",
            Style::default().fg(Color::Blue),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "Commands",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("       Press 't' to access topics       ")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 'p' to publish topics (Input mode)")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("       Press 's' to subscribe topic      ")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw(" Press 'b' to place new bid (Input mode)  ")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Input mode -> (Press 'Enter' to submit or 'Esc' to exit mode)")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "Developed by:",
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::BOLD),
        )]),
        Spans::from(vec![Span::styled(
            "Guilherme Pereira, up201809622",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::styled(
            "José Simões, up201804931",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::styled(
            "Tiago Garcia, up202103778",
            Style::default().fg(Color::LightBlue),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Home")
            .border_type(BorderType::Plain),
    );
    home
}

fn render_topics<'a>(topic_list_state: &ListState, app: &App, topic_list: Vec<Topic>) -> (List<'a>, Table<'a>) {
    let topics = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Topics")
        .border_type(BorderType::Plain);

    // get topics list
    //let topic_list = get_topics(&app).expect("can fetch topic list");

    let items: Vec<_> = topic_list
    .iter()
    .map(|topic| {
        let mut subscribed = "";
        if topic.subscribed.clone() == true
        {
            subscribed = " [Subscribed]";
        }

        ListItem::new(Spans::from(vec![Span::styled(
            topic.name.clone() + subscribed,
            Style::default(),
        )]))
    })
    .collect();

    let selected_topic = topic_list
        .get(
            topic_list_state
                .selected()
                .expect("there is always a selected topic"),
        )
        .expect("exists")
        .clone();

    let list = List::new(items).block(topics).highlight_style(
        Style::default()
            .bg(Color::LightBlue)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let topic_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(selected_topic.ttl.to_string())),
        Cell::from(Span::raw(selected_topic.highest_bid.to_string())),
        Cell::from(Span::raw(selected_topic.highest_bidder)),
        Cell::from(Span::raw(selected_topic.num_subs.to_string())),
        Cell::from(Span::raw(selected_topic.subscribed.to_string())),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "TTL",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Highest Bid",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Bidder",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Subscribers",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Subscribed",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Detail")
            .border_type(BorderType::Plain),
    )
    .widths(&[
        Constraint::Percentage(25),
        Constraint::Percentage(10),
        Constraint::Percentage(15),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
    ]);

    (list, topic_detail)
}

fn get_topics(app: &App) -> Result<Vec<Topic>, Box<dyn Error>> {
    //get list of topics
    let res: Vec<(String, String, String)> = app.get_topics();

    //for each topic on the list -> get info
    let mut topics_list = Vec::new();
    for (topic,tll,_) in res
    {
        let json = app.get_json(topic);
        let t: Topic = serde_json::from_str(&json.to_string())?;
        topics_list.push(t);
    }
    
    // return topics list
    Ok(topics_list)
} 

fn bid_topic(topic_list_state: &mut ListState, value: &Vec<String>, app: &App) -> Result<bool, Box<dyn Error>> {
    let mut result = false;
    
    if let Some(selected) = topic_list_state.selected() {

        match value[0].parse::<i32>() {
            Ok(n) => {
                // format input
                let msg = format!("bid {}", value[0]);

                // based on selected topic -> get topic name
                let topic_list = get_topics(&app).expect("can fetch topic list");
                let selected_topic = topic_list
                    .get(
                        topic_list_state
                            .selected()
                            .expect("there is always a selected topic"),
                    )
                    .expect("exists")
                    .clone();
                
                // add new bid to topic
                result = app.add_msg(selected_topic.name, msg);
            },
            Err(e) => {
                result = false;
            },
        }

        
    }
    Ok(result)
}

fn publish_topic(value: &Vec<String>, app: &App) -> Result<bool, Box<dyn Error>> {
    let mut result = false;
    
    // split input (ex: topic_name:bid)
    let msg_split: Vec<&str> = value[0].split(":").collect();

    if msg_split.len() >= 2 {
        match msg_split[1].parse::<i32>() {
            Ok(n) => {
                let topic_name = msg_split[0];
                let topic_bid = msg_split[1];

                // publish topic
                let status = app.publish(topic_name.to_string());

                if status {
                    // add new bid to topic
                    let msg = format!("bid {}", topic_bid);
                    result = app.add_msg(topic_name.to_string(), msg);
                } 
            },
            Err(e) => {
                result = false;
            },
        }
    }
    Ok(result)
}

fn subscribe_topic(topic_list_state: &mut ListState, app: &App) -> Result<bool, Box<dyn Error>> {
    let mut result = false;
    
    if let Some(selected) = topic_list_state.selected() {

        // based on selected topic -> get topic name
        let topic_list = get_topics(&app).expect("can fetch topic list");
        let selected_topic = topic_list
            .get(
                topic_list_state
                    .selected()
                    .expect("there is always a selected topic"),
            )
            .expect("exists")
            .clone();
        
        if(!selected_topic.subscribed) {
            // subscribe topic
            result = app.subscribe(selected_topic.name);
        }
    }
    Ok(result)
}
