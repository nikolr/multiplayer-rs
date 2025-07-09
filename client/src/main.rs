use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleRate, SizedSample};
use std::collections::VecDeque;
use std::fmt::Debug;
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;
use opus::Channels::Stereo;

fn main() {
    let socket = UdpSocket::bind("192.168.0.45:9476").unwrap();
    loop {
        if let Ok(()) = socket.connect("192.168.0.31:9475") {
            println!("connected");
            break;
        }
    }

    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("Failed to find a default output device");
    let configs = device.supported_output_configs().unwrap();
    
    // let config = device.default_output_config().unwrap();
    // println!("{:?}", config);
    let viable_configs = configs.filter(|config| {
        (config.sample_format() == cpal::SampleFormat::F32 || config.sample_format() == cpal::SampleFormat::I16) && config.channels() == 2
    }).collect::<Vec<_>>();
    let config_range = match viable_configs.get(0) {
        Some(config) => config,
        None => {
            println!("No suitable config found");
            return;
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
        cpal::SampleFormat::F32 => run::<f32>(device, config.into(), socket).unwrap(),
        cpal::SampleFormat::I16 => run::<i16>(device, config.into(), socket).unwrap(),
        cpal::SampleFormat::U16 => run::<u16>(device, config.into(), socket).unwrap(),
        _ => panic!("Unsupported format"),
    }
    loop {
        thread::sleep(Duration::from_millis(1000));
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
    
    let mut socket_buf = [0u8; 128];
    let mut sample_deque: VecDeque<f32> = VecDeque::new();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut opus_decoder = opus::Decoder::new(48000, Stereo)?;
    let mut opus_decoder_buffer = [0f32; 960];
    thread::spawn(move || {
        loop {
            socket.recv(&mut socket_buf).unwrap();
            // let samples = SampleFormat::Float32.to_float_samples(&socket_buf).unwrap();
            match opus_decoder.decode_float(&socket_buf, opus_decoder_buffer.as_mut_slice(), false) {
                Ok(result) => {
                    let mut samples = Vec::from(opus_decoder_buffer);
                    samples.truncate(result);
                    // for i in 0..960 {
                    //     tx.send(opus_decoder_buffer[i]).unwrap();
                    //     opus_decoder_buffer[i] = 0.0;
                    // }
                    // println!("{:?}", samples);
                    sample_deque.extend(samples);
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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SampleFormat {
    /// Signed 16 bit integer
    Int16,
    /// 32 bit float
    Float32
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
                i16::from_le_bytes((&x[..len]).try_into().unwrap()) as f32 / i16::MAX as f32
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