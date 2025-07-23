use bincode::config::Configuration;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, Button, Column, Container, Row, Text, TextInput};
use iced::{Alignment, Element, Length, Subscription, Task};
use message_io::network::{NetEvent, SendStatus, Transport};
use message_io::node;
use message_io::node::{NodeHandler, NodeListener};
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::thread;
use async_tungstenite::tungstenite::client;
use iced::futures::SinkExt;
use crate::client_logic;
use crate::client_logic::Event;

const SERVER_PORT: u16 = 9475;

pub struct Multiplayer {
    username: String,
    server_address: String,
    handler: Option<NodeHandler<()>>,
    state: State,
    messages: Vec<client_logic::Message>,
    connecting: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    UsernameChanged(String),
    ServerAddressChanged(String),
    ClearPressed,
    ConnectPressed,
    DisconnectPressed,
    // TODO: Remove these and add a more descriptive one
    Echo(client_logic::Event),
    Send(client_logic::Message),
}

enum State {
    Disconnected,
    Connected(client_logic::Connection),
}

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    AudioRequest(String),
}

#[derive(Serialize, Deserialize)]
pub enum HostMessage {
    CanStream(bool),
    Chunk(Vec<u8>),
}

impl Default for Multiplayer {
    fn default() -> Self {

        Self {
            username: String::from("Username"),
            server_address: String::from("192.168.0.31"),
            handler: None,
            state: State::Disconnected,
            messages: Vec::new(),
            connecting: false,       
        }
    }
}

impl Multiplayer {

    pub fn subscription(&self) -> Subscription<Message> {
        if self.connecting {
            Subscription::run(client_logic::connect).map(Message::Echo)
        }
        else {
            Subscription::none()
        }
    }

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
                // TODO: Here flip some state for subscription
                self.connecting = true;
                if self.handler.clone().is_some_and(|handler| handler.is_running()) {
                    return Task::none();
                }
                
                Task::none()

            },
            Message::DisconnectPressed => {
                if let Some(handler) = self.handler.take() {
                    handler.stop();
                }
                match &mut self.state {
                    State::Disconnected => {
                        println!("Already disconnected");
                    }
                    State::Connected(connection) => {
                        println!("Sending disconnect message");
                        
                        connection.send(client_logic::Message::Disconnected)
                    }
                }
                self.state = State::Disconnected;
                self.connecting = false;
                
                Task::none()
            },
            Message::Send(message) => match &mut self.state {
                State::Connected(connection) => {
                    self.username.clear();

                    connection.send(message);

                    Task::none()
                }
                State::Disconnected => Task::none(),
            },
            Message::Echo(event) => match event {
                client_logic::Event::Connected(connection) => {
                    println!("Connected");
                    self.state = State::Connected(connection);

                    self.messages.push(client_logic::Message::connected());

                    Task::none()
                }
                client_logic::Event::Disconnected => {
                    println!("Disconnected");
                    if let State::Connected(connection) = &mut self.state {
                        connection.0.send(client_logic::Message::Disconnected);
                    }
                    self.state = State::Disconnected;

                    self.messages.push(client_logic::Message::disconnected());

                    Task::none()
                }
                client_logic::Event::MessageReceived(message) => {
                    self.messages.push(message);
                    println!("{:?}", self.messages);

                    Task::none()
                }
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        match self.state {
            State::Connected(_) => {
                container(
                    column![
                        Container::new(Text::new("Connected").center().align_x(Horizontal::Center)),
                        Button::new(Text::new("Disconnect").center().align_x(Horizontal::Center))
                            .on_press(Message::DisconnectPressed),
                    ]
                )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center)
                    .into()
            }
            State::Disconnected => {
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
                                    Button::new(Text::new("TEST SEND").align_x(Horizontal::Center))
                                        .width(Length::Fill)
                                        // .on_press(Message::Send(
                                        //     client_logic::Message::new(&self.username.clone()).unwrap_or_else(|| client_logic::Message::new("test").unwrap()))
                                        // ),
                                    .on_press(Message::Send(
                                        client_logic::Message::Disconnected)
                                    ),
                                )
                        ),
                )
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center)
                    .into();
                content
            }
        }
    }
}