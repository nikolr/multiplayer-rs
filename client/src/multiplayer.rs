use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use iced::{Alignment, Element, Length, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Button, Column, Container, Row, Text, TextInput};
use message_io::network::{NetEvent, Transport};
use message_io::node;
use message_io::node::{NodeEvent, NodeHandler, NodeListener};

const SERVER_PORT: u16 = 9475;

pub struct Multiplayer {
    username: String,
    server_address: String,
    join_handle: Option<JoinHandle<()>>,
    // tx: Option<Sender<NodeMessage>>,
    // rx: Receiver<NodeMessage>,
    handler: Option<NodeHandler<Signal>>,
    // listener: NodeListener<()>,
}

#[derive(Debug, Clone)]
pub enum Message {
    UsernameChanged(String),
    ServerAddressChanged(String),
    ClearPressed,
    ConnectPressed,
    DisconnectPressed,
}

enum NodeMessage {
    Disconnect,
}

#[derive(Debug, Clone)]
pub enum Signal {
    Greet(String),
    Ping,
}

impl Default for Multiplayer {
    fn default() -> Self {
        // let (handler, listener) = node::split::<()>();
        // let (tx, rx): (Sender<NodeMessage>, Receiver<NodeMessage>) = mpsc::channel();

        Self {
            username: String::new(),
            server_address: String::new(),
            join_handle: None,
            handler: None,
        }
    }
}

impl Multiplayer {
    pub fn update(&mut self, message: Message) -> Task<Message>{
        match message {
            Message::UsernameChanged(username) => {
                self.username = username;

                Task::none()
            },
            Message::ServerAddressChanged(server_address) => {
                self.server_address = server_address;

                Task::none()
            },
            Message::ClearPressed => {
                Task::none()
            },
            Message::ConnectPressed => {
                match self.server_address.parse::<Ipv4Addr>() {
                    Ok(address) => {
                        
                        let (handler, listener): (NodeHandler<Signal>, NodeListener<Signal>) = node::split();
                        self.handler = Some(handler.clone());
                        
                        if let Ok((server, socket_addr)) = handler.network().connect(Transport::Udp, format!("{address}:{SERVER_PORT}")) {
                            println!("Connected to server");
                            let username = self.username.clone();
                            self.join_handle = Some(thread::spawn(move || {
                                listener.for_each(move |event| match event {
                                    NodeEvent::Network(net_event) => match net_event {
                                        NetEvent::Connected(_endpoint, _ok) => {
                                            handler.signals().send(Signal::Greet(format!("Hello: {username}!")));
                                        }
                                        NetEvent::Accepted(_, _) => {}
                                        NetEvent::Message(endpoint, data) => {
                                            println!("Received message from {}: {:#?}", endpoint, data);
                                        }
                                        NetEvent::Disconnected(_) => {}
                                    }
                                    NodeEvent::Signal(signal) => match signal {
                                        Signal::Greet(greeting) => {
                                            handler.network().send(server, greeting.as_bytes());
                                        }
                                        Signal::Ping => {}
                                    }
                                })
                            }));
                        }
                    }
                    Err(_) => {
                        println!("Invalid IP address")
                    },
                }

                Task::none()

            },
            Message::DisconnectPressed => {
                if let Some(handler) = self.handler.take() {
                    handler.stop();
                    println!("Handler stop called");
                    if let Some(join_handle) = self.join_handle.take() {
                        match join_handle.join() {
                            Ok(_) => println!("Disconnected from server"),
                            Err(_) => println!("Error disconnecting from server"),
                        }
                    }
                }
                
                Task::none()
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        let content: Element<Message> = Container::new(
            Column::new()
                .align_x(Alignment::Center)
                .max_width(600)
                .padding(20)
                .spacing(16)
                .push(
                    TextInput::new("Username", &self.username)
                        .on_input(Message::UsernameChanged)
                        .padding(10)
                        .size(32),
                )
                .push(
                    TextInput::new("Server IP", &self.server_address)
                        .on_input(Message::ServerAddressChanged)
                        .padding(10)
                        .size(32)
                )
                .push(
                    Row::new()
                        .spacing(10)
                        .push(
                            Button::new(Text::new("Clear").align_x(Horizontal::Center))
                                .width(Length::Fill)
                                .on_press(Message::ClearPressed),
                        )
                        .push(
                            Button::new(Text::new("Connect").align_x(Horizontal::Center))
                                .width(Length::Fill)
                                .on_press(Message::ConnectPressed),
                        )
                        .push(
                            Button::new(Text::new("Disconnect").align_x(Horizontal::Center))
                                .width(Length::Fill)
                                .on_press(Message::DisconnectPressed),
                        ),
                ),
        )
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into();
        content
    }
}