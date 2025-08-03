use iced::alignment::{Horizontal, Vertical};
use iced::widget::{column, container, Button, Column, Container, Row, Text, TextInput};
use iced::{Alignment, Element, Event, Length, Subscription, Task};
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use rodio::OutputStream;
use serde::{Deserialize, Serialize};
use steamworks::networking_sockets::NetConnection;

const SERVER_PORT: u16 = 9475;

pub struct Client {
    pub(crate) opus_decoder: opus::Decoder,
    pub(crate) opus_decoder_buffer: [f32; 960],
    output_stream: OutputStream,
    pub(crate) sink: rodio::Sink,
    pub(crate) net_connection: Option<NetConnection>,
}

#[derive(Debug, Clone)]
pub enum Message {
    DisconnectPressed,
}

impl Default for Client {
    fn default() -> Self {

        let opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
        let opus_decoder_buffer = [0f32; 960];
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
            .expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&stream_handle.mixer());

        Self {
            opus_decoder,
            opus_decoder_buffer,
            output_stream: stream_handle,
            sink,
            net_connection: None,       
        }
    }
}

impl Client {
    
    pub fn new(net_connection: Option<NetConnection>) -> Self {
        let mut client = Self::default();
        client.net_connection = net_connection;
        client   
        
    }
    pub fn update(&mut self, message: Message) -> Task<Message>{
        match message {
            Message::DisconnectPressed => {
                self.net_connection.take();
                self.sink.stop();
                
                Task::none()
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
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
}