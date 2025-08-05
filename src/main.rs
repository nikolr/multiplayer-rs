use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, pick_list, row, scrollable, text, Button, Column, Container, Row, Scrollable, Text};
use iced::{Element, FillPortion, Font, Length, Subscription, Task, Theme};
use rodio::buffer::SamplesBuffer;
use std::collections::HashMap;
use std::sync::mpsc;
use std::time::Duration;
use iced::widget::scrollable::Scrollbar;
use steamworks::networking_sockets::{NetConnection, NetworkingSockets};
use steamworks::networking_types::{AppNetConnectionEnd, ListenSocketEvent, NetConnectionEnd, NetworkingConnectionState, NetworkingIdentity, SendFlags};
use steamworks::{CallbackHandle, Client, Friends, GameLobbyJoinRequested, LobbyType, SteamId};
use crate::settings::save_state_settings;

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

pub struct State {
    screen: Screen,
    client: Client,
    matchmaking: steamworks::Matchmaking,
    sockets: NetworkingSockets,
    friends: Friends,
    receiver_create_lobby: mpsc::Receiver<steamworks::LobbyId>,
    sender_create_lobby: mpsc::Sender<steamworks::LobbyId>,
    receiver_join_lobby: mpsc::Receiver<steamworks::LobbyId>,
    sender_join_lobby: mpsc::Sender<steamworks::LobbyId>,
    receiver_game_lobby_join_accept: mpsc::Receiver<GameLobbyJoinRequested>,
    lobby_id: Option<steamworks::LobbyId>,
    networking_identity: NetworkingIdentity,
    peers: HashMap<SteamId, Peer>,
    game_lobby_join_requested_callback: CallbackHandle,
    lobby_type: settings::LobbyType,
    max_members: u32,
}

struct Peer {
    name: String,
    net_connection: NetConnection,
}

impl Peer {
    fn new(name: String, net_connection: NetConnection) -> Self {
        Self {
            name,
            net_connection,
        }
    }
}

impl Default for State {
    fn default() -> Self {

        // 480 is Spacewar!, the Steamworks SDK example app.
        let client =
            steamworks::Client::init_app(480).expect("Steam is not running or has not been detected");

        let cloned_client = client.clone();

        let networking_identity = NetworkingIdentity::new_steam_id(client.user().steam_id());
        
        let matchmaking = client.matchmaking();
        let sockets = client.networking_sockets();
        let friends = client.friends();

        let (sender_create_lobby, receiver_create_lobby) = mpsc::channel();
        let (sender_join_lobby, receiver_join_lobby) = mpsc::channel();
        let (sender_game_lobby_join_accept, receiver_game_lobby_join_accept) = mpsc::channel();

        let game_lobby_join_requested_callback = client.register_callback(move |request: GameLobbyJoinRequested| {
            println!("GOT LOBBY JOIN REQUEST");
            sender_game_lobby_join_accept.send(request).unwrap();
        });

        let settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
        let host = host::host::Host::new(settings.clone());
        State {
            screen: Screen::Host(host),
            client: cloned_client,
            matchmaking,
            sockets,
            friends,
            receiver_create_lobby,
            sender_create_lobby,
            receiver_join_lobby,
            sender_join_lobby,
            receiver_game_lobby_join_accept,
            lobby_id: None,
            networking_identity,
            peers: HashMap::new(),
            game_lobby_join_requested_callback,
            lobby_type: settings.lobby_type,
            max_members: settings.max_members,
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
    CreateLobbyButtonPressed,
    LeaveLobbyButtonPressed,
    MaxPlayersChanged(u32),
    LobbyTypeChanged(settings::LobbyType),
    SteamCallback,
    DisconnectPressed,
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

        Message::CreateLobbyButtonPressed => {
            if let Screen::Host(host) = &mut state.screen {
                if host.listen_socket.is_some() {
                    return Task::none();
                }
            }
            let settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
            confy::store("multiplayer", None, &settings).unwrap();

            let local_sender_create_lobby = state.sender_create_lobby.clone();
            println!("Creating lobby with max_members: {} and lobby_type: {}", settings.max_members, settings.lobby_type);
            state.matchmaking.create_lobby(
                state.lobby_type.into(), 
                state.max_members,
                move |result| match result {
                    Ok(lobby_id) => {
                        local_sender_create_lobby.send(lobby_id).unwrap();
                        println!("Created lobby: [{}]", lobby_id.raw());
                    }
                    Err(err) => panic!("SteamError: {}", err),
            });

            Task::none()
        },
        Message::SteamCallback => {
            state.client.run_callbacks();

            if let Ok(lobby) = state.receiver_join_lobby.try_recv() {
                println!("JOINED TO LOBBY WITH ID: {}", lobby.raw());
                let host_id = state.matchmaking.lobby_owner(lobby);
                state.lobby_id = Some(lobby);
                println!("Trying to connect to host: {}", host_id.raw());
                let net_connection = state.sockets.connect_p2p(
                    NetworkingIdentity::from(host_id),
                    0,
                    vec![],
                ).expect("Failed to connect to peer");

                println!("Connection established");
                if let Screen::Host(host) = &mut state.screen {
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
                state.screen = Screen::Client(client::client::Client::new(net_connection));
            }

            if let Ok(request) = state.receiver_game_lobby_join_accept.try_recv() {
                println!("Received lobby join request: {:#?}", request);
                leave_lobby(state);

                let sender_join_lobby_clone = state.sender_join_lobby.clone();
                state.matchmaking.join_lobby(request.lobby_steam_id, move |result| {
                    if let Ok(lobby) = result {
                        sender_join_lobby_clone.send(lobby).unwrap();
                    } else {
                        println!("Error: {:?}", result);
                    }
                });
            }
            
            // TODO: Show connected peers
            // TODO: Try to pass peer data to all peers
            match &mut state.screen {
                Screen::Host(host) => {
                    if let Ok(lobby) = state.receiver_create_lobby.try_recv() {
                        println!("CREATED LOBBY WITH ID: {}", lobby.raw());
                        state.lobby_id = Some(lobby);

                        println!("Creating listen socket...");
                        let listen_socket = state.sockets.create_listen_socket_p2p(
                            0,
                            vec![],
                        ).expect("Failed to create listen socket");
                        println!("Created listen socket");
                        host.listen_socket = Some(listen_socket);
                    }
                    if let Some(listen_socket) = host.listen_socket.as_mut() {
                        while let Some(event) = listen_socket.try_receive_event() {
                            match event {
                                ListenSocketEvent::Connecting(connection_request) => {
                                    println!("Got connection request from peer: {:?}", connection_request.remote().steam_id().expect("Failed to get steam id from connection"));
                                    let _ = connection_request.accept();
                                },
                                ListenSocketEvent::Connected(connected_event) => {
                                    println!("Connected to peer: {:?}", connected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                    state.peers.insert(connected_event.remote().steam_id().expect("Failed to get steam id from connection"), Peer::new(state.friends.get_friend(connected_event.remote().steam_id().expect("Should be able to get PersonaName if they are in the same lobby")).name() ,connected_event.take_connection()));
                                },
                                ListenSocketEvent::Disconnected(disconnected_event) => {
                                    println!("Disconnected from peer: {:?}", disconnected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                    state.peers.remove(&disconnected_event.remote().steam_id().expect("Failed to get steam id from connection"));
                                },
                            }
                        }
                    }
                    match host.rx_capt.try_recv() {
                        Ok(data) => {
                            if state.lobby_id.is_some() && !state.peers.is_empty() {
                                let mut networking_messages = Vec::with_capacity(state.peers.len());
                                for peer in &state.peers {
                                    let identity = NetworkingIdentity::new_steam_id(*peer.0);
                                    let mut networking_message = state.client.networking_utils().allocate_message(80);
                                    // networking_message.set_data(data.clone()).expect("Unable to set data to netwoking_message");
                                    networking_message.set_identity_peer(state.networking_identity.clone());
                                    networking_message.copy_data_into_buffer(data.as_slice()).unwrap();
                                    networking_message.set_send_flags(SendFlags::UNRELIABLE_NO_DELAY);
                                    networking_message.set_connection(&peer.1.net_connection);
                                    networking_messages.push(networking_message);
                                }
                                host.listen_socket.as_mut()
                                    .expect("Host should have a listen socket if it has connected peers")
                                    .send_messages(networking_messages);
                            }
                        }
                        Err(try_recv_error) => match try_recv_error {
                            mpsc::TryRecvError::Empty => {
                                println!("Empty buffer on capture thread");
                            },
                            mpsc::TryRecvError::Disconnected => {
                                println!("Capture thread disconnected");
                            },
                        },
                    }
                },
                Screen::Client(client) => {
                    if let Some(net_connection) = &mut client.net_connection {
                        let connection_info = state.sockets.get_connection_info(net_connection);
                        match connection_info {
                            Ok(connection_info) => match connection_info.state() {
                                Ok(networking_connection_state) => match networking_connection_state {
                                    NetworkingConnectionState::None => {}
                                    NetworkingConnectionState::Connecting => {}
                                    NetworkingConnectionState::FindingRoute => {}
                                    NetworkingConnectionState::Connected => {}
                                    NetworkingConnectionState::ClosedByPeer => {
                                        disconnect(state, NetConnectionEnd::RemoteTimeout);

                                        return Task::none();
                                    }
                                    NetworkingConnectionState::ProblemDetectedLocally => {
                                        disconnect(state, NetConnectionEnd::LocalOfflineMode);

                                        return Task::none();
                                    }
                                }
                                Err(_invalid_connection_state) => {
                                    println!("Invalid connection handle");
                                },
                            },
                            Err(b) => {
                                println!("Error retrieving connection info: {:?}", b);
                            }
                        }
                        match net_connection.receive_messages(100) {
                            Ok(messages) => {
                                for message in messages {
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
                }
            }

            Task::none()       
        },
        Message::DisconnectPressed => {
            disconnect(state, NetConnectionEnd::App(AppNetConnectionEnd::normal(0)));
            
            Task::none()       
        },
        Message::MaxPlayersChanged(value) => {
            state.max_members = value;
            save_state_settings(state).unwrap();
            
            Task::none()
        },
        Message::LobbyTypeChanged(lobby_type) => {
            state.lobby_type = lobby_type;
            save_state_settings(state).unwrap();
            
            Task::none()       
        },
        Message::LeaveLobbyButtonPressed => {
            leave_lobby(state);
            
            Task::none()
        },
    }
}

fn view(state: &State) -> Element<Message> {
    let max_member_possible_values = [1, 2, 3, 4, 5, 6, 7, 8];
    let lobby_type_possible_values = [settings::LobbyType::FriendsOnly, settings::LobbyType::Private];
    let lobby_creation_view: Element<Message> = 
    container(
        row![
            pick_list(
                max_member_possible_values,
                Some(state.max_members),
                Message::MaxPlayersChanged,
            ),
            pick_list(
                lobby_type_possible_values,
                Some(state.lobby_type),
                Message::LobbyTypeChanged,
            ),
            Button::new(Text::new("Create Lobby").center().align_x(Horizontal::Center))
                .on_press(Message::CreateLobbyButtonPressed),
        ]
    )
        .into();
    
    let client_view: Element<Message> =
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
        .into();
    
    match &state.screen {
        Screen::Host(host) => {
            if host.listen_socket.is_some() {
                return column![
                    row![
                        button("Close Lobby").on_press(Message::LeaveLobbyButtonPressed).padding(8),
                        container(text("Connected clients:").size(16)).padding(8),
                        Scrollable::new(Column::from_vec(state.peers.iter()
                            .map(|peer| {
                                Text::new(format!("{}", peer.1.name)).size(16).into()
                            }).collect::<Vec<Element<Message>>>()).spacing(12).padding(8)
                        ).direction(scrollable::Direction::Horizontal(Scrollbar::new())).anchor_bottom().width(Length::Fill),
                    ],
                    host.view().map(Message::Host)
            ].into();
            }
            column![
                lobby_creation_view,
                host.view().map(Message::Host)
            ].into()
        },
        Screen::Client(client) => {
            column![
                client_view,
            ].into()
        },
    }
}

fn disconnect(state: &mut State, net_connection_end: NetConnectionEnd) {
    if let Screen::Client(client) = &mut state.screen {
        let net_connection = client.net_connection.take();
        net_connection.expect("net_connection should exist when trying to close it").close(net_connection_end, None, false);
        client.net_connection.take();
        client.sink.stop();
    }
    let settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
    confy::store("multiplayer", None, &settings).unwrap();
    let host = host::host::Host::new(settings);
    state.lobby_id = None;
    state.screen = Screen::Host(host);
}

fn leave_lobby(state: &mut State) {
    if let Some(lobby_id) = state.lobby_id {
        state.matchmaking.leave_lobby(lobby_id);
        state.lobby_id = None;
        state.peers.clear();
        if let Screen::Host(host) = &mut state.screen {
            host.listen_socket = None;
        }
    }
}