use std::fmt;
use iced::futures;
use iced::stream;
use iced::widget::text;

use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::{Stream, StreamExt};
use opus::Channels::Stereo;
use tokio::io;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpStream};

pub fn connect() -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let mut state = State::Disconnected;
        loop {
            match &mut state {
                State::Disconnected => {
                    const MULTIPLAYER_SERVER: &str = "127.0.0.1:9475";
                    println!("Connecting to multiplayer server: {}", MULTIPLAYER_SERVER);
                    
                    match TcpStream::connect(MULTIPLAYER_SERVER).await {
                        Ok(stream) => {
                            let (sender, receiver) = mpsc::channel(100);

                            let _ = output
                                .send(Event::Connected(Connection(sender)))
                                .await;

                            state = State::Connected(stream, receiver);
                        }
                        Err(_) => {
                            println!("Failed to connect to multiplayer server");
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            let _ = output.send(Event::Disconnected).await;
                        }
                    }
                }
                State::Connected(stream, rx) => {
                    loop {
                        // Wait for the socket to be readable
                        match stream.readable().await {
                            Ok(_) => {
                                println!("socket is readable");
                            }
                            Err(e) => {
                                println!("error: {}", e);
                                let _ = output.send(Event::Disconnected).await;
                                break;
                            }
                        }

                        // Creating the buffer **after** the `await` prevents it from
                        // being stored in the async task.
                        let mut buf = [0; 80];

                        // Try to read data, this may still fail with `WouldBlock`
                        // if the readiness event is a false positive.
                        match stream.try_read(&mut buf) {
                            Ok(0) => {
                                println!("connection closed");
                                let _ = output.send(Event::Disconnected).await;
                                break;
                            },
                            Ok(n) => {
                                println!("read {} bytes", n);
                                println!("{:?}", buf);
                                let _ = output.send(Event::DataReceived(buf)).await;
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                println!("error: {}", e);
                                let _ = output.send(Event::Disconnected).await;
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
    Connected(
        TcpStream,
        mpsc::Receiver<Message>,
    ),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    DataReceived([u8; 80]),
}

#[derive(Debug, Clone)]
pub struct Connection(mpsc::Sender<Message>);

impl Connection {
    pub fn send(&mut self, message: Message) {
        self.0
            .try_send(message)
            .expect("Send message to echo server");
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Connected,
    Disconnected,
    User(String),
    Data(Vec<u8>),
}

impl Message {
    pub fn new(message: &str) -> Option<Self> {
        if message.is_empty() {
            None
        } else {
            Some(Self::User(message.to_string()))
        }
    }

    pub fn connected() -> Self {
        Message::Connected
    }

    pub fn disconnected() -> Self {
        Message::Disconnected
    }

    pub fn as_str(&self) -> &str {
        match self {
            Message::Connected => "Connected successfully!",
            Message::Disconnected => "Connection lost... Retrying...",
            Message::User(message) => message.as_str(),
            // TODO Make this actually pass the audio buffer
            Message::Data(data) => std::str::from_utf8(data).unwrap(),
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> text::IntoFragment<'a> for &'a Message {
    fn into_fragment(self) -> text::Fragment<'a> {
        text::Fragment::Borrowed(self.as_str())
    }
}