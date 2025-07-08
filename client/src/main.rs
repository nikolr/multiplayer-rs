use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SizedSample};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::net::UdpSocket;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    println!("Hello, world!");
    let socket = UdpSocket::bind("127.0.0.1:9476").unwrap();
    socket.connect("127.0.0.1:9475").unwrap();

    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("Failed to find a default output device");
    let config = device.default_output_config().unwrap();

    match config.sample_format() {
        cpal::SampleFormat::F32 => run::<f32>(device, config.into(), socket).unwrap(),
        cpal::SampleFormat::I16 => run::<i16>(device, config.into(), socket).unwrap(),
        cpal::SampleFormat::U16 => run::<u16>(device, config.into(), socket).unwrap(),
        _ => panic!("Unsupported format"),
    }
    loop {
        sleep(Duration::from_millis(1000));
    }
}

fn run<T>(device: cpal::Device, config: cpal::StreamConfig, socket: UdpSocket) -> Result<(), anyhow::Error>
where
    T: SizedSample + FromSample<f32> + std::fmt::Debug,
{
    // let contents = std::fs::read("test.raw")?;
    // let sample_format = SampleFormat::Float32;
    // let mut samples = sample_format.to_float_samples(contents.as_slice())?;
    // let mut sample_deque = VecDeque::from(samples);

    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;
    println!("sample rate: {}", sample_rate);
    println!("channels: {}", channels);
    
    let mut socket_buf = [0u8; 512];
    let mut sample_deque: VecDeque<f32> = VecDeque::new();

    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        loop {
            socket.recv(&mut socket_buf).unwrap();
            let samples = SampleFormat::Float32.to_float_samples(&socket_buf).unwrap();
            sample_deque.extend(samples);
            while let Some(value) = sample_deque.pop_front() {
                tx.send(value).unwrap();
            }
        }
    });

    thread::spawn(move || {
        let mut next_value = move || rx.try_recv().unwrap_or(0.0);
        println!("next value: {}", next_value());
        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);
        let stream = device.build_output_stream(
            &config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, &mut next_value)
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

fn next_value(buf: &mut [u8], udp_socket: &UdpSocket) -> f32 {
    udp_socket.recv(buf).unwrap();
    println!("next value: {:#?}", buf);
    return 0.0;
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32> + Debug,
{
    // for frame in output.chunks_mut(channels) {
    //     let value: T = T::from_sample(next_sample());
    //     for sample in frame.iter_mut() {
    //         *sample = value;
    //     }
    // }
    for sample in output.iter_mut() {
        *sample = T::from_sample(next_sample());
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AudioFormat {
    /// Number of channels in the audio
    pub channels: usize,
    /// Sample rate of the audio
    pub sample_rate: usize,
    /// Number of bits per sample
    pub bit_depth: u16,
    /// Whether audio uses floating point samples
    pub is_float: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SampleFormat {
    /// Signed 16 bit integer
    Int16,
    /// 32 bit float
    Float32
}

impl AudioFormat {
    fn sample_format(&self) -> anyhow::Result<SampleFormat> {
        match (self.bit_depth, self.is_float) {
            (16, false) => Ok(SampleFormat::Int16),
            (32, true) => Ok(SampleFormat::Float32),
            (bd, float) => {
                anyhow::bail!("Unsupported format bit_depth: {} is_float: {}", bd, float)
            }
        }
    }
}

impl SampleFormat {
    const fn bytes_per_sample(&self) -> usize {
        match self {
            Self::Int16 => 2,
            Self::Float32 => 4,
        }
    }
    
    fn to_float_fn(&self) -> Box<dyn Fn(&[u8]) -> f32> {
        let len = self.bytes_per_sample();
        match self {
            Self::Int16 => Box::new(move |x: &[u8]| {
                i16::from_be_bytes(x[..len].try_into().unwrap()) as f32 / i16::MAX as f32
            }),
            Self::Float32 => Box::new(move |x: &[u8]| f32::from_le_bytes(x[..len].try_into().unwrap())),
        }
    }
    
    fn to_float_samples(&self, samples: &[u8]) -> anyhow::Result<Vec<f32>> {
        let len = self.bytes_per_sample();
        if samples.len() % len != 0 {
            anyhow::bail!("Invalid number of samples {}", samples.len());
        }
        
        let conversion = self.to_float_fn();
        
        let samples = samples.chunks(len).map(conversion).collect();
        Ok(samples)
    }
}