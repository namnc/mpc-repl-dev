# TCPDuplex

TCPDuplex provides an abstraction for a TCP duplex connection, enabling asynchronous reading and writing of serialized messages over TCP streams. It uses `serde` for serialization and deserialization of messages, `tokio` for asynchronous operations.

## Features

- Asynchronous TCP stream handling using `tokio`.
- Serialization and deserialization of messages using `serde`.
- Unique UUID assignment for each connection instance.

## Usage

The following code is extracted from `https://github.com/tkmct/mpz/blob/bmr16/garble/mpz-garble/examples/bmr16_demo.rs` 
### Special Example: Interchanging Duplex Connections



This example demonstrates the interchangeability of duplex connections in an application. Initially, we create a `MemoryDuplex` for local, in-memory communication. Then, we replace it with `TCPDuplex` instances to enable network communication, illustrating the ease of transitioning between different duplex types with minimal changes to the application logic.

### Initial Setup with MemoryDuplex

```rust
use tcp_duplex::memory::MemoryDuplex;
use tcp_duplex::GarbleMessage;

let (mut generator_channel, mut evaluator_channel) = MemoryDuplex::<GarbleMessage>::new();

// Example usage with generator_channel and evaluator_channel
```

### Transitioning to TCPDuplex

```rust
use tcp_duplex::protocol::connection::TCPDuplex;
use tokio::net::TcpStream;
use tcp_duplex::GarbleMessage;

#[tokio::main]
async fn main() {
    let proxyaddr = "127.0.0.1:8080";

    // Connecting the generator
    let generator_socket = TcpStream::connect(proxyaddr).await.unwrap();
    let mut generator_conn = TCPDuplex::<GarbleMessage>::new(generator_socket);
    println!("generator_id {:?}", generator_conn.uuid);

    // Connecting the evaluator
    let evaluator_socket = TcpStream::connect(proxyaddr).await.unwrap();
    let mut evaluator_conn = TCPDuplex::<GarbleMessage>::new(evaluator_socket);
    println!("evaluator_id {:?}", evaluator_conn.uuid);

    // Example usage with generator_conn and evaluator_conn
}
```

## Sample Adhoc Proxy Server for Local Testing

The following `adhoc_proxy` server is **an ad-hoc, simplistic proxy server intended for local testing and not suitable for production use**. It listens for incoming connections, pairs them, and forwards messages between each pair. This server is a practical tool for simulating network conditions and testing the interaction between TCPDuplex instances in a controlled, local environment.

```rust
use tokio::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    let connections = Arc::new(Mutex::new(HashMap::new()));
    let mut index = 0;

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let idx = index;
                let conns = connections.clone();
                println!("Got connection from {:?}", socket.peer_addr());

                if idx % 2 == 0 {
                    // Even index, store the connection
                    conns.lock().unwrap().insert(idx, socket);
                } else {
                    // Odd index, retrieve the even-indexed pair and start forwarding
                    if let Some(even_socket) = conns.lock().unwrap().remove(&(idx - 1)) {
                        let odd_socket = socket;
                        tokio::spawn(async move {
                            forward_messages(even_socket, odd_socket).await;
                        });
                    }
                }

                index += 1;
            },
            Err(e) => eprintln!("Failed to accept connection: {}", e),
        }
    }
}

async fn forward_messages(even_socket: TcpStream, odd_socket: TcpStream) {
    let (even_read, even_write) = even_socket.into_split();
    let (odd_read, odd_write) = odd_socket.into_split();

    let even_to_odd = tokio::spawn(async move {
        let mut even_read = even_read;
        let mut odd_write = odd_write;
        io::copy(&mut even_read, &mut odd_write).await.expect("Failed to forward from even to odd");
    });

    let odd_to_even = tokio::spawn(async move {
        let mut odd_read = odd_read;
        let mut even_write = even_write;
        io::copy(&mut odd_read, &mut even_write).await.expect("Failed to forward from odd to even");
    });

    let _ = tokio::join!(even_to_odd, odd_to_even);
    println!("Connection between pair closed");
}

```