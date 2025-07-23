use std::{fmt, thread};
use iced::futures;
use iced::stream;
use iced::widget::text;

use futures::channel::mpsc;
use futures::sink::SinkExt;
use futures::stream::{Stream, StreamExt};

use async_tungstenite::tungstenite;
use async_tungstenite::tungstenite::Utf8Bytes;
use bincode::config::Configuration;
use message_io::network::{NetEvent, SendStatus, Transport};
use message_io::node;
use message_io::node::{NodeEvent, NodeHandler, NodeListener, StoredNetEvent};
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use crate::multiplayer::{ClientMessage, HostMessage};

pub fn connect() -> impl Stream<Item = Event> {
    stream::channel(100, |mut output| async move {
        let mut state = State::Disconnected;

        loop {
            match &mut state {
                State::Disconnected => {

                    let (handler, listener): (NodeHandler<()>, NodeListener<()>) = node::split();
                    let address = "192.168.0.31";
                    let server_port = 9475;
                    match handler.network().connect(Transport::FramedTcp, format!("{address}:{server_port}")) {
                        Ok((server_endpoint, socket_addr)) => {
                            let (sender, receiver) = mpsc::channel(100);
                            println!("Sent connection");
                            let _ = output.send(Event::Connected(Connection(sender))).await;
                            println!("Sent connection");
                            state = State::Connected(handler.clone(), receiver);
                            
                            let mut output_clone = output.clone();
                            thread::spawn(move || {

                                let mut opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
                                let mut opus_decoder_buffer = [0f32; 960];
                                let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
                                    .expect("open default audio stream");
                                let sink = rodio::Sink::connect_new(&stream_handle.mixer());
                                let task = listener.for_each(move |event| match event.network() {
                                    NetEvent::Connected(endpoint, established) => {
                                        println!("Connected to server: {}", endpoint);
                                        if established {
                                            let audio_request = ClientMessage::AudioRequest(String::from("TESTER"));
                                            let data = bincode::serde::encode_to_vec::<ClientMessage, Configuration>(audio_request, Configuration::default()).unwrap();
                                            let send_status = handler.network().send(server_endpoint, data.as_slice());
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
                                                    Ok(_result) => {
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
                                        let _ = output_clone.send(Event::Disconnected);
                                    }
                                });
                            });
                        }
                        Err(_) => {}
                }
            }
                State::Connected(handler, receiver) => {
                    if let Some(message) = receiver.next().await {
                        match message {
                            Message::Connected => {}
                            Message::Disconnected => {
                                println!("Disconnected from server");
                                handler.stop();
                                state = State::Disconnected;
                            }
                            Message::User(_) => {}
                            Message::Data(_) => {}
                        }
                    }
                }
            }
        }
    })
}

enum State {
    Disconnected,
    Connected(
        NodeHandler<()>,
        mpsc::Receiver<Message>,
    ),
}

#[derive(Debug, Clone)]
pub enum Event {
    Connected(Connection),
    Disconnected,
    MessageReceived(Message),
}

#[derive(Debug, Clone)]
pub struct Connection(pub(crate) mpsc::Sender<Message>);

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