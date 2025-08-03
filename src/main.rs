use std::net::SocketAddrV4;
use std::str::FromStr;
use std::sync::mpsc;
use std::time::Duration;
use iced::widget::{column, Column, Container, Row, Space, TextInput};
use iced::{Alignment, Element, Font, Length, Subscription, Task, Theme};
use iced_aw::{TabBarPosition, TabLabel, Tabs};
use rodio::buffer::SamplesBuffer;
use steamworks::{AppId, CallbackHandle, CallbackResult, Client, FriendFlags, GameLobbyJoinRequested, GameRichPresenceJoinRequested, LobbyChatMsg, LobbyId, LobbyType, P2PSessionRequest, PersonaStateChange, SendType, SteamId};
use steamworks::networking_messages::{NetworkingMessages, NetworkingMessagesSessionRequest, SessionRequest};
use steamworks::networking_sockets::NetworkingSockets;
use steamworks::networking_types::{ListenSocketEvent, NetworkingConfigEntry, NetworkingIdentity, SendFlags};

mod client;
mod host;
pub mod settings;

fn main() -> iced::Result {
    iced::application("Multiplayer", update, view)
        .theme(theme)
        .font(include_bytes!("../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .subscription(subscription)
        .run()
}

fn theme(_state: &State) -> Theme {
    Theme::SolarizedDark
}


struct State {
    screen: Screen,
    client: Client,
    matchmaking: steamworks::Matchmaking,
    networking: steamworks::Networking,
    messages: NetworkingMessages,
    sockets: NetworkingSockets,
    receiver_create_lobby: std::sync::mpsc::Receiver<steamworks::LobbyId>,
    sender_create_lobby: std::sync::mpsc::Sender<steamworks::LobbyId>,
    receiver_join_lobby: std::sync::mpsc::Receiver<steamworks::LobbyId>,
    sender_join_lobby: std::sync::mpsc::Sender<steamworks::LobbyId>,
    receiver_accept: std::sync::mpsc::Receiver<steamworks::SteamId>,
    receiver_game_lobby_join_accept: std::sync::mpsc::Receiver<GameLobbyJoinRequested>,
    receiver_game_rich_presence_join_accept: std::sync::mpsc::Receiver<GameRichPresenceJoinRequested>,
    lobby_join_id: String,
    lobby_id: Option<steamworks::LobbyId>,
    lobby_host_id: Option<steamworks::SteamId>,
    peers: Vec<steamworks::SteamId>,
    request_callback: CallbackHandle,
    game_lobby_join_requested_callback: CallbackHandle,
}

impl Default for State {
    fn default() -> Self {

        // 480 is Spacewar!, the Steamworks SDK example app.
        let client =
            steamworks::Client::init_app(480).expect("Steam is not running or has not been detected");

        // let _cb = client.register_callback(|p: PersonaStateChange| {
        //     println!("Got callback: {:?}", p);
        // });

        let cloned_client = client.clone();

        let matchmaking = client.matchmaking();
        let networking = client.networking();
        let messages = client.networking_messages();
        let sockets = client.networking_sockets();

        messages.session_request_callback(move |req| {
            println!("Accepting session request from {:?}", req.remote());
            req.accept();
        });
        
        messages.session_failed_callback(|info| {
            eprintln!("Session failed: {info:#?}");
        });

        let friends = client.friends();
        println!("Friends");
        let list = friends.get_friends(FriendFlags::IMMEDIATE);
        println!("{:?}", list);
        for f in &list {
            println!("Friend: {:?} - {}({:?})", f.id(), f.name(), f.state());
            friends.request_user_information(f.id(), true);
        }

        //For getting values from callback
        let (sender_create_lobby, receiver_create_lobby) = mpsc::channel();
        let (sender_join_lobby, receiver_join_lobby) = mpsc::channel();
        let (sender_accept, receiver_accept) = mpsc::channel();
        let (sender_game_lobby_join_accept, receiver_game_lobby_join_accept) = mpsc::channel();
        let (sender_game_rich_presence_join_accept, receiver_game_rich_presence_join_accept) = mpsc::channel();

        //YOU MUST KEEP CALLBACK IN VARIABLE OTHERWISE CALLBACK WILL NOT WORK
        let request_callback = client.register_callback(move |request: P2PSessionRequest| {
            println!("ACCEPTED PEER");
            sender_accept.send(request.remote).unwrap();
        });

        let game_lobby_join_requested_callback = client.register_callback(move |request: GameLobbyJoinRequested| {
            println!("GOT LOBBY JOIN REQUEST");
            sender_game_lobby_join_accept.send(request).unwrap();
        });

        let game_rich_presence_join_requested_callback = client.register_callback(move |request: GameRichPresenceJoinRequested| {
            println!("GOT GAME JOIN REQUEST");
            sender_game_rich_presence_join_accept.send(request).unwrap();
        });
        

        let settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
        match settings.mode {
            settings::Mode::Host => {
                let host = host::host::Host::new(settings);
                State {
                    screen: Screen::Host(host),
                    client: cloned_client,
                    matchmaking,
                    networking,
                    messages,
                    sockets,
                    receiver_create_lobby,
                    sender_create_lobby,
                    receiver_join_lobby,
                    sender_join_lobby,
                    receiver_accept,
                    receiver_game_lobby_join_accept,
                    receiver_game_rich_presence_join_accept,
                    lobby_join_id: String::new(),
                    lobby_id: None,
                    lobby_host_id: None,
                    peers: vec![],
                    request_callback,
                    game_lobby_join_requested_callback,
                }
            },
            settings::Mode::Client => {
                State {
                    screen: Screen::Client(client::client::Client::new(None)),
                    client: cloned_client,
                    matchmaking,
                    networking,
                    messages,
                    sockets,   
                    receiver_create_lobby,
                    sender_create_lobby,
                    receiver_join_lobby,
                    sender_join_lobby,
                    receiver_accept,
                    receiver_game_lobby_join_accept,
                    receiver_game_rich_presence_join_accept,
                    lobby_join_id: String::new(),
                    lobby_id: None,
                    lobby_host_id: None,
                    peers: vec![],
                    request_callback,
                    game_lobby_join_requested_callback,
                }
            }
        }
    }
}

enum Screen {
    Host(host::host::Host),
    Client(client::client::Client),
}

#[derive(Debug, Clone)]
enum Message {
    Host(host::host::Message),
    Client(client::client::Message),
    TabSelected(TabId),
    LobbyJoinIdChanged(String),
    SteamCallback,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
enum TabId {
    #[default]
    Host,
    Client,
}

impl TabId {
    fn from_screen(screen: &Screen) -> Self {
        match screen {
            Screen::Host(_) => TabId::Host,
            Screen::Client(_) => TabId::Client,
        }
    }
}

fn subscription(state: &State) -> Subscription<Message> {

    match &state.screen {
        Screen::Client(client) => {
            Subscription::batch([
                // client.subscription().map(Message::Client),
                iced::time::every(Duration::from_secs_f64(0.01)).map(|_| Message::SteamCallback)
            ])
        },
        Screen::Host(host) => {
            Subscription::batch([
                // host.subscription().map(Message::Host),
                iced::time::every(Duration::from_secs_f64(0.01)).map(|_| Message::SteamCallback)
            ])
        }
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Host(message) => {
            if let Screen::Host(host) = &mut state.screen {
                host.update(message).map(Message::Host)
            } else {
                Task::none()
            }
        },
        Message::Client(message) => {
            if let Screen::Client(client) = &mut state.screen {
                client.update(message).map(Message::Client)
            } else {
                Task::none()
            }
        },
        Message::TabSelected(tab_id) => {
            println!("Tab selected: {:?}", tab_id);
            match tab_id {
                TabId::Host => {
                    let mut settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
                    settings.mode = settings::Mode::Host;
                    confy::store("multiplayer", None, &settings).unwrap();

                    // let host = host::host::Host::new(settings);
                    // state.screen = Screen::Host(host);

                    let local_sender_create_lobby = state.sender_create_lobby.clone();
                    state.matchmaking.create_lobby(LobbyType::FriendsOnly, 4, move |result| match result {
                        Ok(lobby_id) => {
                            local_sender_create_lobby.send(lobby_id).unwrap();
                            println!("Created lobby: [{}]", lobby_id.raw());
                        }
                        Err(err) => panic!("Error: {}", err),
                    });

                    Task::none()
                },
                TabId::Client => {
                    println!("Starting client");
                    if let Screen::Host(host) = &mut state.screen {
                        settings::save(&host).unwrap();

                        match host.capture_thread_handle.take() {
                            Some(capture_thread_handle) => {
                                println!("Joining capture thread");
                                let cancel = host.tx_cancel.take();
                                cancel.unwrap().send(()).unwrap();
                                capture_thread_handle.join().unwrap();
                            }
                            None => {
                                println!("No capture thread handle to join");
                            }
                        }
                    }

                    // state.screen = Screen::Client(client::client::Client::new());
                    //     let local_sender_join_lobby = state.sender_join_lobby.clone();
                    //     state.matchmaking.join_lobby(LobbyId::from_raw(state.lobby_join_id.parse().unwrap()), move |result| match result {
                    //         Ok(lobby) => {
                    //             local_sender_join_lobby.send(lobby).unwrap();
                    //             println!("Joined lobby: [{}]", lobby.raw());
                    //         }
                    //         Err(e) => {
                    //             println!("Ran into error while trying to join lobby: {:?}", e);
                    //         }
                    //     });

                    Task::none()
                },
            }
        },
        Message::SteamCallback => {
            // println!("Steam callback");
            state.client.run_callbacks();

            if let Ok(lobby) = state.receiver_join_lobby.try_recv() {
                println!("JOINED TO LOBBY WITH ID: {}", lobby.raw());
                let host_id = state.matchmaking.lobby_owner(lobby);
                state.lobby_id = Some(lobby);

                state.matchmaking.lobby_members(lobby).iter().for_each(|steam_id| {
                    state.peers.push(*steam_id);
                });
                println!("Peers from client perspective: {:?}", state.peers);
                
                println!("Trying to connect to host: {}", host_id.raw());
                let net_connection = state.sockets.connect_p2p(
                    NetworkingIdentity::from(host_id),
                    0,
                    vec![],
                ).expect("Failed to connect to peer");

                println!("Connection established {:#?}", net_connection.connection_name());
                state.screen = Screen::Client(client::client::Client::new(Some(net_connection)));
                // let _ = state.messages.send_message_to_user(
                //     NetworkingIdentity::new_steam_id(host_id),
                //     SendFlags::RELIABLE,
                //     format!("{} JOINED", state.client.friends().name()).as_bytes(),
                //     0,
                // );
                // // When you connected to lobby you have to send a "ping" message to host
                // // After that host will add you into peer list
                // state.networking.send_p2p_packet(
                //     host_id,
                //     SendType::Reliable,
                //     format!("{} JOINED", state.client.friends().name()).as_bytes(),
                // );
            }

            if let Ok(request) = state.receiver_game_lobby_join_accept.try_recv() {
                println!("Received lobby join request: {:#?}", request);
                let sender_join_lobby_clone = state.sender_join_lobby.clone();
                state.matchmaking.join_lobby(request.lobby_steam_id, move |result| {
                    if let Ok(lobby) = result {
                        sender_join_lobby_clone.send(lobby).unwrap();
                    } else {
                        println!("Error: {:?}", result);
                    }
                });

            }
            
            // TODO: Switch to using steamworks::NetworkingSockets
            match &mut state.screen {
                Screen::Host(host) => {
                    if let Ok(lobby) = state.receiver_create_lobby.try_recv() {
                        println!("CREATED LOBBY WITH ID: {}", lobby.raw());
                        state.lobby_id = Some(lobby);

                        let listen_socket = state.sockets.create_listen_socket_p2p(
                            0,
                            vec![],
                        ).expect("Failed to create listen socket");
                        host.listen_socket = Some(listen_socket);
                    }
                    // if let Ok(user) = state.receiver_accept.try_recv() {
                    //     println!("GET REQUEST FROM {}", user.raw());
                    //     state.peers.push(user);
                    //     state.networking.accept_p2p_session(user);
                    // 
                    // }
                    if let Some(listen_socket) = host.listen_socket.as_mut() {
                        while let Some(event) = listen_socket.try_receive_event() {
                            match event {
                                ListenSocketEvent::Connecting(connection_request) => {
                                    println!("Got connection request from peer: {:?}", connection_request.remote().steam_id().expect("Failed to get steam id from connection"));
                                    let _ = connection_request.accept();
                                },
                                ListenSocketEvent::Connected(connected_event) => {
                                    println!("Connected to peer: {:?}", connected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                    state.peers.push(connected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                },
                                ListenSocketEvent::Disconnected(disconnected_event) => {
                                    println!("Disconnected from peer: {:?}", disconnected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                    state.peers.retain(|peer| *peer != disconnected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                },
                            }
                        }
                    }
                    match host.rx_capt.try_recv() {
                        Ok(data) => {
                            // println!("Got data from capture thread: {:?}", data);
                            //         let _ = state.networking.send_p2p_packet(
                            //             SteamId::from_raw(76561199883301606),
                            //             SendType::UnreliableNoDelay,
                            //             data.as_slice(),
                            //         ); 
                            if state.lobby_id.is_some() && !state.peers.is_empty() {
                                for peer in &state.peers {
                                    // let identity = NetworkingIdentity::new_steam_id(SteamId::from_raw(76561199883301606));
                                    let identity = NetworkingIdentity::new_steam_id(*peer);
                                    let _ = state.messages.send_message_to_user(
                                        identity,
                                        SendFlags::UNRELIABLE_NO_DELAY,
                                        data.as_slice(),
                                        0,
                                    );
                                }
                            }
                        }
                        Err(try_recv_error) => match try_recv_error {
                            std::sync::mpsc::TryRecvError::Empty => {
                                println!("Empty buffer on capture thread");
                            },
                            std::sync::mpsc::TryRecvError::Disconnected => {
                                println!("Capture thread disconnected");
                            },
                        }, 
                    }
                },
                Screen::Client(client) => {
                    // while let Some(size) = state.networking.is_p2p_packet_available() {
                    //     let mut empty_array = vec![0; size];
                    //     let buffer = empty_array.as_mut_slice();
                    //     if let Some((sender, n)) = state.networking.read_p2p_packet(buffer) {
                    //         let mut opus_decoder_buffer = [0f32; 960];
                    //         match client.opus_decoder.decode_float(&buffer, opus_decoder_buffer.as_mut_slice(), false) {
                    //             Ok(_result) => {
                    //                 let samples_buffer = SamplesBuffer::new(2, 48000, opus_decoder_buffer);
                    //                 client.sink.append(samples_buffer);
                    //             }
                    //             Err(e) => println!("error: {}", e)
                    //         }
                    //     }
                    if let Some(net_connection) = &mut client.net_connection {
                        match net_connection.receive_messages(100) {
                            Ok(messages) => {
                                for message in messages {
                                    println!("Got message!");
                                    let peer = message.identity_peer();
                                    let data = message.data();
                                    match client.opus_decoder.decode_float(&data, client.opus_decoder_buffer.as_mut_slice(), false) {
                                        Ok(_result) => {
                                            let samples_buffer = SamplesBuffer::new(2, 48000, client.opus_decoder_buffer);
                                            client.sink.append(samples_buffer);
                                            client.opus_decoder_buffer.fill(0.0);
                                        }
                                        Err(e) => println!("error: {}", e)
                                    }
                                }
                                
                            },
                            Err(invalid_handle_error) => {
                                println!("Invalid handle error: {:?}", invalid_handle_error);
                            },
                        }

                    }
                    // for message in state.messages.receive_messages_on_channel(0, 100) {
                    //     println!("Got message!");
                    //     let peer = message.identity_peer();
                    //     let data = message.data();
                    //     match client.opus_decoder.decode_float(&data, client.opus_decoder_buffer.as_mut_slice(), false) {
                    //         Ok(_result) => {
                    //             let samples_buffer = SamplesBuffer::new(2, 48000, client.opus_decoder_buffer);
                    //             client.sink.append(samples_buffer);
                    //             client.opus_decoder_buffer.fill(0.0);
                    //         }
                    //         Err(e) => println!("error: {}", e)
                    //     }
                    // }
                }
            }

            
            Task::none()       
        },
        Message::LobbyJoinIdChanged(new_lobby_join_id) => {
            state.lobby_join_id = new_lobby_join_id;

            Task::none()       
        },
    }
}

fn view(state: &State) -> Element<Message> {
    let tab_bar: Element<Message> = Tabs::new(Message::TabSelected)
        .tab_bar_position(TabBarPosition::Top)
        .push(
            TabId::Host,
            TabLabel::Text("Host".to_string()),
            Space::with_width(0.0)
        )
        .push(
            TabId::Client,
            TabLabel::Text("Client".to_string()),
            Space::with_width(0.0)
        )
        .set_active_tab(&TabId::from_screen(&state.screen))
        .tab_bar_height(Length::Shrink)
        .into();
    
    let lobby_text: Element<Message> = Container::new(Row::new()
        .align_y(Alignment::Center)
        .padding(20)
        .spacing(16)
        .push(
            TextInput::new("Lobby id", &state.lobby_join_id)
                .on_input(Message::LobbyJoinIdChanged)
                .padding(10)
                .size(32),
        )
    ).into();
    
    match &state.screen {
        Screen::Host(host) => {
            column![
                tab_bar,
                lobby_text,
                host.view().map(Message::Host)
            ].into()
        },
        Screen::Client(client) => {
            column![
                tab_bar,
                client.view().map(Message::Client)
            ].into()
        },
    }
}