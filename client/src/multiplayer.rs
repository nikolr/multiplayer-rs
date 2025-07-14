use bincode::config::Configuration;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, Button, Column, Container, Row, Text, TextInput};
use iced::{Alignment, Element, Length, Task};
use message_io::network::{NetEvent, SendStatus, Transport};
use message_io::node;
use message_io::node::NodeHandler;
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::thread;

const SERVER_PORT: u16 = 9475;

pub struct Multiplayer {
    username: String,
    server_address: String,
    handler: Option<NodeHandler<()>>,
}

#[derive(Debug, Clone)]
pub enum Message {
    UsernameChanged(String),
    ServerAddressChanged(String),
    ClearPressed,
    ConnectPressed,
    DisconnectPressed,
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
            username: String::new(),
            server_address: String::from("192.168.0.31"),
            handler: None,
        }
    }
}

impl Multiplayer {
    
    fn is_connected(&self) -> bool {
        self.handler.clone().is_some_and(|handler| handler.is_running())
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
                if self.is_connected() {
                    return Task::none();
                }
                match self.server_address.parse::<Ipv4Addr>() {
                    Ok(address) => {
                        
                        let (handler, listener) = node::split();
                        if let Ok((server_id, socket_addr)) = handler.network().connect(Transport::FramedTcp, format!("{address}:{SERVER_PORT}")) {
                            self.handler = Some(handler.clone());
                            
                            let username = self.username.clone();
                            let mut opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
                            let mut opus_decoder_buffer = [0f32; 960];
                            
                            thread::spawn(move || {
                                let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
                                    .expect("open default audio stream");
                                let sink = rodio::Sink::connect_new(&stream_handle.mixer());
                                
                                listener.for_each(move |event| match event.network() {
                                        NetEvent::Connected(endpoint, established) => {
                                            println!("Connected to server: {}", endpoint);
                                            if established {
                                                let audio_request = ClientMessage::AudioRequest(username.clone());
                                                let data = bincode::serde::encode_to_vec::<ClientMessage, Configuration>(audio_request, Configuration::default()).unwrap();
                                                let send_status = handler.network().send(server_id, data.as_slice());
                                                match send_status {
                                                    SendStatus::Sent => {
                                                        println!("Sent audio request");
                                                    }
                                                    SendStatus::MaxPacketSizeExceeded => {}
                                                    SendStatus::ResourceNotFound => {}
                                                    SendStatus::ResourceNotAvailable => {}
                                                }
                                            } else {
                                                println!("Connection failed");
                                            }
                                        }
                                        NetEvent::Accepted(_, _) => {}

                                        NetEvent::Message(_, input_data) => {
                                            let message: (HostMessage, usize) = match bincode::serde::decode_from_slice::<HostMessage, Configuration>(input_data, Configuration::default()) {
                                                Ok(message) => message,
                                                Err(err) => {
                                                    println!("Error decoding message: {}", err);
                                                    return;
                                                }
                                            };
                                            match message.0 {
                                                HostMessage::CanStream(can) => match can {
                                                    true => {
                                                        println!("Host can stream");
                                                    }
                                                    false => {
                                                        println!("Host can't stream");
                                                    }
                                                },
                                                HostMessage::Chunk(chunk) => {
                                                    match opus_decoder.decode_float(chunk.as_slice(), opus_decoder_buffer.as_mut_slice(), false) {
                                                        Ok(result) => {
                                                            let samples_buffer = SamplesBuffer::new(2, 48000, opus_decoder_buffer);
                                                            sink.append(samples_buffer);
                                                        }
                                                        Err(e) => println!("error: {}", e)
                                                    }
                                                }
                                                
                                            }
                                        },
                                        NetEvent::Disconnected(endpoint) => {
                                            println!("Disconnected from host: {}", endpoint);
                                            handler.stop();
                                        }
                                });
                            });
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
                }
                
                Task::none()
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        match self.is_connected() {
            true => {
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
            false => {
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