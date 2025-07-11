use std::collections::VecDeque;
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use bincode::config::Configuration;
use cpal::{FromSample, Sample, SampleRate, SizedSample};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use iced::{Alignment, Element, Length, Task};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Button, Column, Container, Row, Text, TextInput};
use message_io::network::{NetEvent, SendStatus, Transport};
use message_io::node;
use message_io::node::{NodeEvent, NodeHandler, NodeListener};
use opus::Channels::Stereo;
use serde::{Deserialize, Serialize};

const SERVER_PORT: u16 = 9475;

pub struct Multiplayer {
    username: String,
    server_address: String,
    join_handle: Option<JoinHandle<()>>,
    // tx: Sender<Vec<u8>>,
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

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    //From sender to receiver
    AudioRequest,
}

#[derive(Serialize, Deserialize)]
pub enum HostMessage {
    //From receiver to sender
    CanStream(bool),
    Chunk(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum Signal {
    Greet(String),
    Ping,
}

impl Default for Multiplayer {
    fn default() -> Self {
        // let (handler, listener) = node::split::<()>();


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

                        let (tx, rx) = mpsc::channel();

                        let host = cpal::default_host();

                        let device = host
                            .default_output_device()
                            .expect("Failed to find a default output device");
                        let configs = device.supported_output_configs().unwrap();

                        let viable_configs = configs.filter(|config| {
                            (config.sample_format() == cpal::SampleFormat::F32 || config.sample_format() == cpal::SampleFormat::I16) && config.channels() == 2
                        }).collect::<Vec<_>>();
                        let config_range = match viable_configs.get(0) {
                            Some(config) => config,
                            None => {
                                panic!("No suitable config found");
                            }
                        };
                        let config = match config_range.try_with_sample_rate(SampleRate(48000)) {
                            Some(c) => c,
                            None => {
                                panic!("System does not support sample rate");
                            }
                        };
                        println!("{:?}", config);

                        match config.sample_format() {
                            cpal::SampleFormat::F32 => run::<f32>(device, config.into(), rx).unwrap(),
                            cpal::SampleFormat::I16 => run::<i16>(device, config.into(), rx).unwrap(),
                            cpal::SampleFormat::U16 => run::<u16>(device, config.into(), rx).unwrap(),
                            _ => panic!("Unsupported format"),
                        }
                        
                        println!("I think this is the host address:");
                        println!("{address}:{SERVER_PORT}");
                        if let Ok((server_id, socket_addr)) = handler.network().connect(Transport::FramedTcp, format!("{address}:{SERVER_PORT}")) {
                            let username = self.username.clone();
                            self.join_handle = Some(thread::spawn(move || {
                                listener.for_each(move |event| match event.network() {
                                        NetEvent::Connected(endpoint, established) => {
                                            println!("Connected to server: {}", endpoint);
                                            if established {
                                                let audio_request = ClientMessage::AudioRequest;
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
                                                    println!("Received chunk");
                                                    println!("Chunk size: {}", chunk.len());
                                                    tx.send(chunk).unwrap();
                                                }
                                                
                                            }
                                        },
                                        NetEvent::Disconnected(_) => {}
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

fn run<T>(device: cpal::Device, config: cpal::StreamConfig, rx_chunk: Receiver<Vec<u8>>) -> Result<(), anyhow::Error>
where
    T: SizedSample + FromSample<f32>,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut sample_deque: VecDeque<f32> = VecDeque::new();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut opus_decoder = opus::Decoder::new(48000, Stereo)?;
    let mut opus_decoder_buffer = [0f32; 960];
    thread::spawn(move || {
        loop {
            let chunk = rx_chunk.recv().unwrap();
            match opus_decoder.decode_float(chunk.as_slice(), opus_decoder_buffer.as_mut_slice(), false) {
                Ok(result) => {
                    for i in 0..(result * channels) {
                        sample_deque.push_back(opus_decoder_buffer[i]);
                    }
                    while let Some(value) = sample_deque.pop_front() {
                        tx.send(value).unwrap();
                    }
                },

                Err(e) => println!("error: {}", e)
            };
        }
    });

    thread::spawn(move || {
        let mut next_value = move || rx.try_recv().unwrap_or(0.0);
        println!("next value: {}", next_value());
        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                write_data(data, &mut next_value)
            },
            err_fn,
            None,
        ).unwrap();
        stream.play().unwrap();
        loop {
            thread::sleep(Duration::from_millis(1));
        }
    });
    Ok(())
}

fn write_data<T>(output: &mut [T], next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    for sample in output.iter_mut() {
        *sample = T::from_sample(next_sample());
    }
}