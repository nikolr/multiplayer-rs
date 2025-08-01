use bytes::BytesMut;
use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::Stream;
use iced::futures;
use iced::stream;
use tokio::io;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

const HOST_PORT: u16 = 9475;

// TODO: Is this subscription setup even necessary? Just try to recv from Steam messages api in the callback loop
// Let Steam handle joining via the overlay
pub fn connect(addr: String, username: String) -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let mut state = State::Disconnected;
        println!("Attempt connecting to multiplayer server: {}", addr);
        loop {
            match &mut state {
                State::Disconnected => {
                    println!("Connecting to multiplayer server: {}", addr);
                    
                    match TcpStream::connect(format!("{addr}:{HOST_PORT}")).await {
                        Ok(stream) => {
                            let (sender, receiver) = mpsc::channel(100);

                            let _ = output
                                .send(Event::Connected(Connection(sender)))
                                .await;

                            state = State::Connected(MultiplayerConnection::new(stream));
                        }
                        Err(_) => {
                            println!("Failed to connect to multiplayer server");
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            let _ = output.send(Event::Disconnected).await;
                        }
                    }
                }
                State::Connected(multiplayer_connection) => {
                    // First, send the username
                    // Wait for the socket to be writable
                    multiplayer_connection.stream.writable().await.unwrap();

                    // Try to write data, this may still fail with `WouldBlock`
                    // if the readiness event is a false positive.
                    match multiplayer_connection.stream.try_write(username.as_bytes()) {
                        Ok(n) => {
                            println!("wrote username {} bytes", n);
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                            continue;
                        }
                        Err(e) => {
                           println!("error: {}", e);
                        }
                    }

                    // Wait for the socket to be readable
                    match multiplayer_connection.stream.readable().await {
                        Ok(_) => {
                            println!("socket is readable");
                        }
                        Err(e) => {
                            println!("error: {}", e);
                            let _ = output.send(Event::Disconnected).await;
                            break;
                        }
                    }
                    loop {
                        match multiplayer_connection.stream.read_buf(&mut multiplayer_connection.buffer).await {
                            Ok(n) => {
                                if n == 0 {
                                    // The remote closed the connection. For this to be
                                    // a clean shutdown, there should be no data in the
                                    // read buffer. If there is, this means that the
                                    // peer closed the socket while sending a frame.
                                    if multiplayer_connection.buffer.is_empty() {
                                        println!("connection closed with empty buffer");
                                        let _ = output.send(Event::Disconnected).await;
                                        state = State::Disconnected;
                                        break;
                                    } else {
                                        println!("connection reset by peer");
                                        let _ = output.send(Event::Disconnected).await;
                                        state = State::Disconnected;
                                        break;
                                    }
                                }
                                let _ = output.send(Event::DataReceived(multiplayer_connection.buffer.clone())).await;
                                multiplayer_connection.buffer.clear();
                            },
                            Err(e) => {
                                println!("error: {}", e);
                                let _ = output.send(Event::Disconnected).await;
                                state = State::Disconnected;
                                break;
                            }
                        }
                    }
                }
            }
        }
    })
}

#[derive(Debug)]
enum State {
    Disconnected,
    Connected(MultiplayerConnection),
}

#[derive(Debug)]
struct MultiplayerConnection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl MultiplayerConnection {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(80),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    DataReceived(BytesMut),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0
            .try_send(message)
            .expect("Send message to multiplayer server");
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Connected,
    Disconnected,
    User(String),
}