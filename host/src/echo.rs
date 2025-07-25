use std::{error, thread};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use bincode::config::Configuration;
use iced::futures;
use sysinfo::{get_current_pid, Pid};
use tokio::io;
use tokio::net::{TcpListener, UdpSocket};
use wasapi::{initialize_mta, AudioClient, Direction, SampleType, StreamMode, WaveFormat};
use opus::Bitrate;
use opus::ErrorCode as OpusErrorCode;
use tokio::io::AsyncWriteExt;

const HOST_PORT: u16 = 9475;
const CAPTURE_CHUNK_SIZE: usize = 480;
const BIT_RATE: i32 = 64000;
const CHANNELS: u16 = 2;

pub async fn run(clients: Arc<Mutex<HashMap<SocketAddr, String>>>) -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:9475").await?;
    
    let process_id = get_current_pid().unwrap();
    let (tx_capt, rx_capt): (
        tokio::sync::broadcast::Sender<Vec<u8>>,
        tokio::sync::broadcast::Receiver<Vec<u8>>,
    ) = tokio::sync::broadcast::channel(16);
    let tx_capt_clone = tx_capt.clone();

    let _handle = thread::Builder::new()
        .name("Capture".to_string())
        .spawn(move || {
            let result = capture_loop(tx_capt, CAPTURE_CHUNK_SIZE, process_id);
            if let Err(_err) = result {
                println!("Capture thread exited with error: {}", _err);
            }
        });

    println!("Listening on port {}", HOST_PORT);

    loop {
        let (mut stream, addr) = listener.accept().await?;
        let mut rx = tx_capt_clone.subscribe();
        let clients_clone = clients.clone();
        tokio::spawn(async move {
            {
                let mut clients = clients_clone.lock().unwrap();
                clients.insert(addr, "test".to_string());
                println!("Client connected: {}", addr);
                println!("Clients: {:?}", clients);
            }
            loop {
                match rx.recv().await {
                    Ok(data) => {
                        // TODO: Implement framing logic here
                        // let _ = stream.write_all(&data).await;
                        match stream.try_write(data.as_slice()) {
                            Ok(_n) => {
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                println!("Error: {}", e);
                                {
                                    let mut clients = clients_clone.lock().unwrap();
                                    clients.remove(&addr);
                                    println!("Client disconnected: {}", addr);
                                    println!("Clients: {:?}", clients);
                                }
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        println!("Error: {}", err);
                    }
                }
            }
        });
    }
}

fn capture_loop(
    tx_capt: tokio::sync::broadcast::Sender<Vec<u8>>,
    chunksize: usize,
    process_id: Pid,
) -> Result<(), Box<dyn error::Error>> {
    initialize_mta().ok().unwrap();

    let desired_format = WaveFormat::new(32, 32, &SampleType::Float, 48000, 2, None);
    let blockalign = desired_format.get_blockalign();
    let autoconvert = true;
    let include_tree = true;

    let mut audio_client = AudioClient::new_application_loopback_client(process_id.as_u32(), include_tree)?;
    let mode = StreamMode::EventsShared {
        autoconvert,
        buffer_duration_hns: 0,
    };
    audio_client.initialize_client(&desired_format, &Direction::Capture, &mode)?;

    let h_event = audio_client.set_get_eventhandle().unwrap();

    let capture_client = audio_client.get_audiocaptureclient().unwrap();

    let mut sample_queue: VecDeque<u8> = VecDeque::new();

    audio_client.start_stream().unwrap();

    let mut opus_encoder = opus::Encoder::new(48000, opus::Channels::Stereo, opus::Application::Audio).unwrap();
    opus_encoder.set_bitrate(Bitrate::Bits(BIT_RATE)).unwrap();
    // let frame_size = (48000 / 1000 * 20) as usize;

    loop {
        while sample_queue.len() > (blockalign as usize * chunksize) {
            let mut chunk = vec![0u8; blockalign as usize * chunksize];
            for element in chunk.iter_mut() {
                *element = sample_queue.pop_front().unwrap();
            }
            let opus_frame = SampleFormat::Float32.to_float_samples(chunk.as_mut_slice())?;
            match opus_encoder.encode_vec_float(opus_frame.as_slice(), 80) {
                Ok(buf) => {
                    tx_capt.send(buf).unwrap();
                }
                Err(error) => {
                    match error.code() {
                        OpusErrorCode::BufferTooSmall => {
                            println!("Buffer too small");
                        }
                        OpusErrorCode::BadArg => {
                            println!("Bad arg");
                        }
                        OpusErrorCode::InternalError => {
                            println!("Internal error");
                        }
                        OpusErrorCode::InvalidState => {
                            println!("Invalid state");
                        },
                        _ => todo!()
                    }
                }
            };
        }

        let new_frames = capture_client.get_next_packet_size()?.unwrap_or(0);
        let additional = (new_frames as usize * blockalign as usize)
            .saturating_sub(sample_queue.capacity() - sample_queue.len());
        sample_queue.reserve(additional);
        if new_frames > 0 {
            capture_client
                .read_from_device_to_deque(&mut sample_queue)
                .unwrap();
        }
        if h_event.wait_for_event(3000).is_err() {
            audio_client.stop_stream().unwrap();
            break;
        }
    }
    Ok(())
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SampleFormat {
    // Int16,
    Float32
}

impl SampleFormat {
    const fn bytes_per_sample(&self) -> usize {
        match self {
            // Self::Int16 => 2,
            Self::Float32 => 4,
        }
    }

    fn to_float_fn(&self) -> Box<dyn Fn(&[u8]) -> f32> {
        let len = self.bytes_per_sample();
        match self {
            // Self::Int16 => Box::new(move |x: &[u8]| {
            //     i16::from_le_bytes((&x[..len]).try_into().unwrap()) as f32 / i16::MAX as f32
            // }),
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

