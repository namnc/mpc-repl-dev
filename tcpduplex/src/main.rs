
mod roommanager;
mod tmp;
mod ws;
mod protocol;
use protocol::{connection::TCPDuplex, *};
use futures::prelude::*;
use roommanager::{handle_connection, Rooms};
use warp::Filter;

use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let client_socket = TcpStream::connect("127.0.0.1:8080").await.unwrap();
    let mut client_conn = TCPDuplex::<String>::new(client_socket);

    // Send a message from the client to the server
    let message = "Hello from client!".to_string() ;
    client_conn.send(message.clone()).await.unwrap();
    client_conn.flush().await.unwrap();

    // Receive the echo and assert it matches the sent message
    if let Some(Ok(received_message)) = client_conn.next().await {
        assert_eq!(received_message, message);
        println!("Client received: {:?}", received_message);
    } else {
        panic!("Client did not receive the expected message");
    }
}
fn with_clients(rooms: Rooms) -> impl Filter<Extract = (Rooms,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || rooms.clone())
}