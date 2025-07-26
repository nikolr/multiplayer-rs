use crate::stream;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, Button, Column, Container, Row, Text, TextInput};
use iced::{Alignment, Element, Length, Subscription, Task};
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use rodio::OutputStream;
use serde::{Deserialize, Serialize};

const SERVER_PORT: u16 = 9475;

pub struct Multiplayer {
    username: String,
    server_address: String,
    state: State,
    opus_decoder: opus::Decoder,
    output_stream: OutputStream,
    sink: rodio::Sink,
    ready: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    UsernameChanged(String),
    ServerAddressChanged(String),
    ClearPressed,
    ConnectPressed,
    DisconnectPressed,
    // TODO: Remove these and add a more descriptive one
    Echo(stream::Event),
    Send(stream::Message),
}

enum State {
    Connecting,
    Disconnected,
    Connected(stream::Connection),
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

        let opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
            .expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&stream_handle.mixer());
        
        Self {
            username: String::from("Username"),
            server_address: String::from("192.168.0.31"),
            state: State::Disconnected,
            opus_decoder,
            output_stream: stream_handle,
            sink,
            ready: false,
        }
    }
}

impl Multiplayer {

    pub fn subscription(&self) -> Subscription<Message> {
        match self.ready {
            true => {
                Subscription::run_with_id("main" ,stream::connect(self.server_address.clone(), self.username.clone())).map(Message::Echo)
            }
            false => {
                Subscription::none()
            }
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
                self.state = State::Connecting;
                self.ready = true;

                Task::none()

            },
            Message::DisconnectPressed => {
                self.state = State::Disconnected;
                self.ready = false;
                self.sink.stop();
                
                Task::none()
            },
            Message::Send(message) => match &mut self.state {
                State::Connected(connection) => {
                    self.username.clear();

                    connection.send(message);

                    Task::none()
                }
                State::Disconnected => Task::none(),
                State::Connecting => Task::none(),
            },
            Message::Echo(event) => match event {
                stream::Event::Connected(connection) => {
                    println!("Received Connected Event");
                    self.state = State::Connected(connection);

                    Task::none()
                }
                stream::Event::Disconnected => {
                    println!("Received Disconnected Event");
                    self.state = State::Disconnected;
                    self.ready = false;
                    self.sink.stop();

                    Task::none()
                }
                stream::Event::DataReceived(data) => {
                    let mut opus_decoder_buffer = [0f32; 960];
                    match self.opus_decoder.decode_float(&data, opus_decoder_buffer.as_mut_slice(), false) {
                        Ok(_result) => {
                            let samples_buffer = SamplesBuffer::new(2, 48000, opus_decoder_buffer);
                            self.sink.append(samples_buffer); 
                        }
                        Err(e) => println!("error: {}", e)
                    } 

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
            State::Disconnected | State::Connecting => {
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
                                    Button::new(Text::new(
                                        match self.state {
                                            State::Connecting => "Cancel...",
                                            State::Disconnected => "Connect",
                                            State::Connected(_) => unreachable!(),
                                        }
                                    ).align_x(Horizontal::Center))
                                        .width(Length::Fill)
                                        .on_press(
                                            match self.state {
                                                State::Connecting => {
                                                    Message::DisconnectPressed
                                                }
                                                State::Disconnected => {
                                                    Message::ConnectPressed
                                                }
                                                State::Connected(_) => {
                                                    unreachable!()
                                                }
                                            }
                                        ),
                                )
                        ),
                )
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center)
                    .into();
                content
            },
        }
    }
}