Public Ledger for Auctions - System and Data Security 21/22 FCUP

Implemented from scratch using kademlia-dht with PoW (proof-of-work) consensus and a simple blockchain structure, with an underlying p2p network which communicates via RPC (UDPSocket).

----

To RUN:

use our "fake" make file (./make.sh)

OR

Client side --> IN auctions_cli/
                RUN cargo r <port>              ex: cargo r 1444

Bootstrap side --> IN src/
                   RUN RUST_LOG=info cargo r

----

Notes:
    This project is presented as a simple example, thus deployment isn't scalable and/or stable.
    Auction clients are distinguished by port, although Bootstrap proxy IP (CIDR notation without mask) is needed in order for the 
    client to connect.

    ex:
        1)  ./make.sh -b
        2)  (Terminal_X) ./make.sh -s

        After Bootstrap synchronization:

            3)  (Terminal_Y) ./make.sh 1444,X.X.X.X

----

Sources:
    - Petar Maymounkov and David Mazieres. Kademlia: A peer-to-peer information system based on the xor metric. In International Workshop on Peer-to-Peer Systems, pages 53–65. Springer, 2002
    - Satoshi Nakamoto. Bitcoin: A peer-to-peer electronic cash system. 2008.
    - David Hausheer and Burkhard Stiller. PeerMart: The Technology for a Distributed Auction-based Market for Peer-to-Peer Services.

----