use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, Button, Column, Container, Row, Text, TextInput};
use iced::{Alignment, Element, Length, Subscription, Task};
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::thread;
use rodio::OutputStream;
use crate::client_logic;

const SERVER_PORT: u16 = 9475;

pub struct Multiplayer {
    username: String,
    server_address: String,
    state: State,
    opus_decoder: opus::Decoder,
    output_stream: OutputStream,
    sink: rodio::Sink,
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
    Connecting,
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

        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
            .expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&stream_handle.mixer());
        let opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
        Self {
            username: String::from("Username"),
            server_address: String::from("192.168.0.31"),
            state: State::Disconnected,
            output_stream: stream_handle,
            opus_decoder,
            sink
        }
    }
}

impl Multiplayer {

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(client_logic::connect).map(Message::Echo)
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
                // if self.handler.clone().is_some_and(|handler| handler.is_running()) {
                //     return Task::none();
                // }
                // match self.server_address.parse::<Ipv4Addr>() {
                //     Ok(address) => {
                //         let (handler, listener): (NodeHandler<()>, NodeListener<()>) = node::split();
                //         self.handler = Some(handler.clone());
                //         
                //         if let Ok((server_id, socket_addr)) = handler.network().connect(Transport::FramedTcp, format!("{address}:{SERVER_PORT}")) {
                //             // if socket_addr.ip() == Ipv4Addr::new(0, 0, 0, 0) {
                //             //     println!("Connection failed");
                //             //     handler.stop();
                //             //     return Task::none();
                //             // }
                //             let username = self.username.clone();
                //             let mut opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
                //             let mut opus_decoder_buffer = [0f32; 960];
                // 
                //             thread::spawn(move || {
                //                 let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
                //                     .expect("open default audio stream");
                //                 let sink = rodio::Sink::connect_new(&stream_handle.mixer());
                // 
                //                 listener.for_each(move |event| match event.network() {
                //                         NetEvent::Connected(endpoint, established) => {
                //                             println!("Connected to server: {}", endpoint);
                //                             if established {
                //                                 let audio_request = ClientMessage::AudioRequest(username.clone());
                //                                 let data = bincode::serde::encode_to_vec::<ClientMessage, Configuration>(audio_request, Configuration::default()).unwrap();
                //                                 let send_status = handler.network().send(server_id, data.as_slice());
                //                                 match send_status {
                //                                     SendStatus::Sent => {
                //                                         println!("Sent audio request");
                //                                     }
                //                                     SendStatus::MaxPacketSizeExceeded => {}
                //                                     SendStatus::ResourceNotFound => {}
                //                                     SendStatus::ResourceNotAvailable => {}
                //                                 }
                //                             } else {
                //                                 println!("Connection failed");
                //                             }
                //                         }
                //                         NetEvent::Accepted(_, _) => {}
                // 
                //                         NetEvent::Message(_, input_data) => {
                //                             let message: (HostMessage, usize) = match bincode::serde::decode_from_slice::<HostMessage, Configuration>(input_data, Configuration::default()) {
                //                                 Ok(message) => message,
                //                                 Err(err) => {
                //                                     println!("Error decoding message: {}", err);
                //                                     return;
                //                                 }
                //                             };
                //                             match message.0 {
                //                                 HostMessage::CanStream(can) => match can {
                //                                     true => {
                //                                         println!("Host can stream");
                //                                     }
                //                                     false => {
                //                                         println!("Host can't stream");
                //                                     }
                //                                 },
                //                                 HostMessage::Chunk(chunk) => {
                //                                     match opus_decoder.decode_float(chunk.as_slice(), opus_decoder_buffer.as_mut_slice(), false) {
                //                                         Ok(_result) => {
                //                                             let samples_buffer = SamplesBuffer::new(2, 48000, opus_decoder_buffer);
                //                                             sink.append(samples_buffer);
                //                                         }
                //                                         Err(e) => println!("error: {}", e)
                //                                     }
                //                                 }
                //                             }
                //                         },
                //                         NetEvent::Disconnected(endpoint) => {
                //                             println!("Disconnected from host: {}", endpoint);
                //                             handler.stop();
                //                         }
                //                 });
                //             });
                //         }
                //     }
                //     Err(_) => {
                //         println!("Invalid IP address")
                //     },
                // }

                Task::none()

            },
            Message::DisconnectPressed => {
                
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
                client_logic::Event::Connected(connection) => {
                    println!("Received Connected Event");
                    self.state = State::Connected(connection);

                    Task::none()
                }
                client_logic::Event::Disconnected => {
                    println!("Received Disconnected Event");
                    self.state = State::Disconnected;

                    Task::none()
                }
                client_logic::Event::DataReceived(data) => {
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
                                    Button::new(Text::new("Connect").align_x(Horizontal::Center))
                                        .width(Length::Fill)
                                        .on_press(Message::ConnectPressed),
                                )
                                .push(
                                    Button::new(Text::new("TEST SEND").align_x(Horizontal::Center))
                                        .width(Length::Fill)
                                        .on_press(Message::Send(
                                            client_logic::Message::new(&self.username.clone()).unwrap_or_else(|| client_logic::Message::new("test").unwrap()))
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