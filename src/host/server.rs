use opus::Bitrate;
use opus::ErrorCode as OpusErrorCode;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::{error, thread};
use sysinfo::{get_current_pid, Pid};
use tokio::io;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use std::thread::JoinHandle;
use wasapi::{initialize_mta, AudioClient, Direction, SampleType, StreamMode, WaveFormat};

const HOST_PORT: u16 = 9475;
const CAPTURE_CHUNK_SIZE: usize = 480;
const BIT_RATE: i32 = 64000;
const CHANNELS: u16 = 2;

pub async fn run(clients: Arc<Mutex<HashMap<SocketAddr, String>>>, tx_capt: tokio::sync::broadcast::Sender<Vec<u8>>) -> io::Result<()> {
    // let gateway_ip = match reqwest::blocking::get("https://api.ipify.org") {
    //     Ok(response) => {
    //         let ip = response.text().unwrap();
    //         ip
    //     },
    //     Err(err) => {
    //         println!("Error getting gateway IP: {}", err);
    //         String::from("127.0.0.1")
    //     }
    // };
    
    let ip = match local_ip_address::local_ip() {
        Ok(ip_addr) => {
            ip_addr.to_string()
        }
        Err(error) => {
            println!("Error getting local IP: {}", error);
            String::from("127.0.0.1")
        }
    };
    let listener = TcpListener::bind(format!("{ip}:{HOST_PORT}")).await?;

    let tx_capt_clone = tx_capt.clone();


    println!("Listening on port {}", HOST_PORT);

    loop {
        let (mut stream, addr) = listener.accept().await?;
        
        let mut rx = tx_capt_clone.subscribe();
        let clients_clone = clients.clone();

        tokio::spawn(async move {
            {
                // Read the first packet to get the client's name
                let mut buf = [0u8; 1024];
                let bytes_read = stream.read(&mut buf).await.unwrap();
                let username = String::from_utf8_lossy(&buf[..bytes_read]).to_string();
                
                let mut clients = clients_clone.lock().unwrap();
                clients.insert(addr, username);
                println!("Client connected: {}", addr);
                println!("Clients: {:?}", clients);
            }
            loop {
                match rx.recv().await {
                    Ok(data) => {
                        match stream.try_write(data.as_slice()) {
                            Ok(_n) => {
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                println!("Error trying to write data as slice: {}", e);
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
                        println!("Server RecvError: {}", err);
                        return;
                    }
                }
            }
        });
    }
}