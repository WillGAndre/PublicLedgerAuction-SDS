use chrono::prelude::*;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use rand::{distributions::Alphanumeric, prelude::*};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
    },
    Terminal,
};


const DB_PATH: &str = "./data/db.json";

#[derive(Error, Debug)]
pub enum Error {
    #[error("error reading the DB file: {0}")]
    ReadDBError(#[from] io::Error),
    #[error("error parsing the DB file: {0}")]
    ParseDBError(#[from] serde_json::Error),
}

// input event
enum Event<I> {
    Input(I),
    Tick,
}

#[derive(Serialize, Deserialize, Clone)]
struct Topic {
    id: usize,
    name: String,
    //item: String,
    num_subs: usize,
    highest_bid: usize,
    highest_bidder: String,
    created_at: DateTime<Utc>,
    ends_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug)]
enum MenuItem {
    Home,
    Topics,
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Topics => 1,
        }
    }
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode"); // no need to wait for an 'Enter' from user

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["Home", "Topics", "Refresh", "Subscribe", "Bid", "Quit"];
    let mut active_menu_item = MenuItem::Home;
    let mut topic_list_state = ListState::default();
    topic_list_state.select(Some(0));

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
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
                .split(size);

            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    Spans::from(vec![
                        Span::styled(
                            first,
                            Style::default()
                                .fg(Color::Yellow)
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
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            rect.render_widget(tabs, chunks[0]);
            match active_menu_item {
                MenuItem::Home => rect.render_widget(render_home(), chunks[1]),
                MenuItem::Topics => {
                    let topics_chunk = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .split(chunks[1]);
                    let (left, right) = render_topics(&topic_list_state);
                    rect.render_stateful_widget(left, topics_chunk[0], &mut topic_list_state);
                    rect.render_widget(right, topics_chunk[1]);
                }
            }
        })?;

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('q') => {
                    disable_raw_mode()?;
                    terminal.show_cursor()?;
                    break;
                }
                KeyCode::Char('h') => active_menu_item = MenuItem::Home,
                KeyCode::Char('t') => active_menu_item = MenuItem::Topics,
                KeyCode::Char('r') => {
                    //refresh_topic_info().expect("refresh topic info");
                }
                KeyCode::Char('s') => {
                    //subscribe_topic(&mut topic_list_state).expect("can subscribe on topic");
                }
                KeyCode::Char('b') => {
                    bid_topic(&mut topic_list_state).expect("can bid on topic");
                }
                KeyCode::Down => {
                    if let Some(selected) = topic_list_state.selected() {
                        let amount_topics = read_db().expect("can fetch topic list").len();
                        if selected >= amount_topics - 1 {
                            topic_list_state.select(Some(0));
                        } else {
                            topic_list_state.select(Some(selected + 1));
                        }
                    }
                }
                KeyCode::Up => {
                    if let Some(selected) = topic_list_state.selected() {
                        let amount_topics = read_db().expect("can fetch topic list").len();
                        if selected > 0 {
                            topic_list_state.select(Some(selected - 1));
                        } else {
                            topic_list_state.select(Some(amount_topics - 1));
                        }
                    }
                }
                _ => {}
            },
            Event::Tick => {}
        }
    }

    Ok(())
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
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 't' to access topics")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 'r' to refresh topics")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 's' to subscribe topic")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 'b' to place new bid")]),
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

fn render_topics<'a>(topic_list_state: &ListState) -> (List<'a>, Table<'a>) {
    let topics = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Topics")
        .border_type(BorderType::Plain);

    let topic_list = read_db().expect("can fetch topic list");
    let items: Vec<_> = topic_list
        .iter()
        .map(|topic| {
            ListItem::new(Spans::from(vec![Span::styled(
                topic.name.clone(),
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
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    let topic_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(selected_topic.num_subs.to_string())),
        Cell::from(Span::raw(selected_topic.highest_bid.to_string())),
        Cell::from(Span::raw(selected_topic.highest_bidder)),
        Cell::from(Span::raw(selected_topic.created_at.to_string())),
        Cell::from(Span::raw(selected_topic.ends_at.to_string())),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "Subs",
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
            "Created at",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Ends at",
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
        Constraint::Percentage(5),
        Constraint::Percentage(10),
        Constraint::Percentage(10),
        Constraint::Percentage(20),
        Constraint::Percentage(20),
    ]);

    (list, topic_detail)
}

fn read_db() -> Result<Vec<Topic>, Error> {
    let db_content = fs::read_to_string(DB_PATH)?;
    let parsed: Vec<Topic> = serde_json::from_str(&db_content)?;
    Ok(parsed)
}

fn refresh_topic_info() -> Result<(), Error> {
    // let mut rng = rand::thread_rng();
    // let db_content = fs::read_to_string(DB_PATH)?;
    // let mut parsed: Vec<Topic> = serde_json::from_str(&db_content)?;
    // let catsdogs = match rng.gen_range(0, 1) {
    //     0 => "cats",
    //     _ => "dogs",
    // };

    // let random_pet = Pet {
    //     id: rng.gen_range(0, 9999999),
    //     name: rng.sample_iter(Alphanumeric).take(10).collect(),
    //     category: catsdogs.to_owned(),
    //     age: rng.gen_range(1, 15),
    //     created_at: Utc::now(),
    // };

    // parsed.push(random_pet);
    // fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
    //Ok(parsed)



    // call render topics method

    Ok(())

}


fn subscribe_topic(topic_list_state: &mut ListState) -> Result<(), Error> {
    if let Some(selected) = topic_list_state.selected() {
        // call subscribe function

        // if subscribe successful -> add ("// Topic Subscribed") in Details window
    }
    Ok(())
}

fn bid_topic(topic_list_state: &mut ListState) -> Result<(), Error> {
    if let Some(selected) = topic_list_state.selected() {
        // let db_content = fs::read_to_string(DB_PATH)?;
        // let mut parsed: Vec<Pet> = serde_json::from_str(&db_content)?;
        // parsed.remove(selected);
        // fs::write(DB_PATH, &serde_json::to_vec(&parsed)?)?;
        // pet_list_state.select(Some(selected - 1));


        // pop new window or allow input somewhere to place bid
        let bid = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Bid")
            .border_type(BorderType::Plain);

        // call functions to add new bid/message in App

        // if successful refresh Topics list 

    }
    Ok(())
}