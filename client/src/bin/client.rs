use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleRate, SizedSample};
use std::collections::VecDeque;
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;
use iced::{Font, Theme};
use opus::Channels::Stereo;
use multiplayer_client::multiplayer::Multiplayer;

fn main() -> iced::Result {
    iced::application("Multiplayer", Multiplayer::update, Multiplayer::view)
        .theme(theme)
        .font(include_bytes!("../../../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .run()
}

fn theme(_state: &Multiplayer) -> Theme {
    Theme::SolarizedDark
}


// fn main() {
//     let socket = UdpSocket::bind("192.168.0.45:9476").unwrap();
//     loop {
//         if let Ok(()) = socket.connect("192.168.0.31:9475") {
//             println!("connected");
//             break;
//         }
//     }
// 
//     let host = cpal::default_host();
// 
//     let device = host
//         .default_output_device()
//         .expect("Failed to find a default output device");
//     let configs = device.supported_output_configs().unwrap();
//     
//     let viable_configs = configs.filter(|config| {
//         (config.sample_format() == cpal::SampleFormat::F32 || config.sample_format() == cpal::SampleFormat::I16) && config.channels() == 2
//     }).collect::<Vec<_>>();
//     let config_range = match viable_configs.first() {
//         Some(config) => config,
//         None => {
//             println!("No suitable config found");
//             return;
//         }   
//     };
//     let config = match config_range.try_with_sample_rate(SampleRate(48000)) {
//         Some(c) => c,
//         None => {
//             panic!("System does not support sample rate");
//         }
//     };
//     println!("{config:?}");
// 
//     match config.sample_format() {
//         cpal::SampleFormat::F32 => run::<f32>(device, config.into(), socket).unwrap(),
//         cpal::SampleFormat::I16 => run::<i16>(device, config.into(), socket).unwrap(),
//         cpal::SampleFormat::U16 => run::<u16>(device, config.into(), socket).unwrap(),
//         _ => panic!("Unsupported format"),
//     }
//     loop {
//         thread::sleep(Duration::from_millis(1000));
//     }
// }
// 
// fn run<T>(device: cpal::Device, config: cpal::StreamConfig, socket: UdpSocket) -> Result<(), anyhow::Error>
// where
//     T: SizedSample + FromSample<f32>,
// {
//     let sample_rate = config.sample_rate.0 as f32;
//     let channels = config.channels as usize;
//     
//     let mut socket_buf = [0u8; 80];
//     let mut sample_deque: VecDeque<f32> = VecDeque::new();
// 
//     let (tx, rx) = std::sync::mpsc::channel();
//     let mut opus_decoder = opus::Decoder::new(48000, Stereo)?;
//     let mut opus_decoder_buffer = [0f32; 960];
//     thread::spawn(move || {
//         loop {
//             socket.recv(&mut socket_buf).unwrap();
//             match opus_decoder.decode_float(&socket_buf, opus_decoder_buffer.as_mut_slice(), false) {
//                 Ok(result) => {
//                     for sample in opus_decoder_buffer.iter().take((result * channels)) {
//                         sample_deque.push_back(*sample);
//                     }
//                     while let Some(value) = sample_deque.pop_front() {
//                         tx.send(value).unwrap();
//                     }
//                 },
//                 
//                 Err(e) => println!("error: {e}")
//             };
//         }
//     });
// 
//     thread::spawn(move || {
//         let mut next_value = move || rx.try_recv().unwrap_or(0.0);
//         println!("next value: {}", next_value());
//         let err_fn = |err| eprintln!("an error occurred on stream: {err}");
//         let stream = device.build_output_stream(
//             &config,
//             move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
//                 write_data(data, &mut next_value)
//             },
//             err_fn,
//             None,
//         ).unwrap();
//         stream.play().unwrap();
//         loop {
//             thread::sleep(Duration::from_millis(1));
//         }
//     });
//     Ok(())
// }
// 
// fn write_data<T>(output: &mut [T], next_sample: &mut dyn FnMut() -> f32)
// where
//     T: Sample + FromSample<f32>,
// {
//     for sample in output.iter_mut() {
//         *sample = T::from_sample(next_sample());
//     }
// }
