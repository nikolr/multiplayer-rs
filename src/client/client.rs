use iced::Task;
use opus::Channels::Stereo;
use rodio::buffer::SamplesBuffer;
use rodio::OutputStream;
use steamworks::networking_sockets::NetConnection;
use steamworks::networking_types::{NetConnectionEnd, NetworkingConnectionState};
use crate::{host, settings, Message, Screen, State};

const SERVER_PORT: u16 = 9475;

pub struct Client {
    pub opus_decoder: opus::Decoder,
    pub opus_decoder_buffer: [f32; 960],
    output_stream: OutputStream,
    pub sink: rodio::Sink,
    pub net_connection: Option<NetConnection>,
}   

impl Client {
    pub fn new(net_connection: NetConnection) -> Self {
        let opus_decoder = opus::Decoder::new(48000, Stereo).unwrap();
        let opus_decoder_buffer = [0f32; 960];
        let output_stream = rodio::OutputStreamBuilder::open_default_stream()
            .expect("open default audio stream");
        let sink = rodio::Sink::connect_new(&output_stream.mixer());

        Self {
            opus_decoder,
            opus_decoder_buffer,
            output_stream,
            sink,
            net_connection: Some(net_connection),
        }
    }
}