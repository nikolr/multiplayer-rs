use crate::playlist::{Playlist, Track};
use crate::server;
use crate::track::{MultiplayerPlaylist, MultiplayerPlaylistMessage, MultiplayerTrack, MultiplayerTrackMessage};
use iced::alignment::Horizontal;
use iced::widget::{button, center, column, container, row, slider, text, tooltip, vertical_space, Column, Container, Scrollable, Text};
use iced::{Alignment, Element, Fill, FillPortion, Font, Subscription, Task};
use kira::modulator::tweener::{TweenerBuilder, TweenerHandle};
use kira::sound::static_sound::StaticSoundHandle;
use kira::sound::{PlaybackPosition, PlaybackState};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, StartTime, Tween, Value};
use rfd::FileHandle;
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::io;

const HOST_PORT: u16 = 9475;
const CAPTURE_CHUNK_SIZE: usize = 480;
const BIT_RATE: i32 = 64000;
const CHANNELS: u16 = 2;

#[derive(Debug, Clone)]
pub enum Message {
    OpenFiles,
    FilesOpened(Result<Vec<MultiplayerTrack>, Error>),
    ImportPlaylist,
    PlaylistImported(Result<Vec<MultiplayerTrack>, Error>),
    ExportPlaylist,
    PlaylistExported(Result<FileHandle, Error>),
    PlaylistSavedToFile(Result<(), Error>),
    MultiplayerPlaylist(MultiplayerPlaylistMessage),
    UpdatePlaybackPositionSlider(f64),
    SeekToPlaybackPosition,
    TickPlaybackPosition,
    UpdateFadeInDurationSlider(f64),
    UpdateFadeOutDurationSlider(f64),
    Pause,
    Resume,
    Stop,
    Server,
}

#[derive(Clone, Debug)]
enum Signal {
    SendChunk,
}

#[derive(Serialize, Deserialize)]
pub enum ClientMessage {
    AudioRequest(String),
}

#[derive(Serialize, Deserialize)]
pub enum HostMessage {
    //From receiver to sender
    CanStream(bool),
    Chunk(Vec<u8>),
}

#[derive(PartialEq, Debug, Clone)]
enum UsedTrackHandle {
    Primary,
    Secondary,
}

#[derive(Debug, Clone)]
pub enum Error {
    DialogClosed,
    IoError(io::ErrorKind),
}

pub struct Multiplayer {
    is_loading: bool,
    audio_manager: AudioManager,
    primary_track_handle: TrackHandle,
    secondary_track_handle: TrackHandle,
    used_track_handle: UsedTrackHandle,
    currently_playing_static_sound_handle: Option<StaticSoundHandle>,
    primary_volume_tweener: TweenerHandle,
    secondary_volume_tweener: TweenerHandle,
    playlist: MultiplayerPlaylist,
    playback_position: f64,
    fade_in_duration: u64,
    fade_out_duration: u64,
    audio_seek_dragged: bool,
    connected_clients: Arc<Mutex<HashMap<SocketAddr, String>>>,
}
impl Default for Multiplayer {
    fn default() -> Self {
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
        //
        // let ip = match local_ip_address::local_ip() {
        //     Ok(ip_addr) => {
        //         ip_addr.to_string()
        //     }
        //     Err(error) => {
        //         println!("Error getting local IP: {}", error);
        //         String::from("127.0.0.1")
        //     }
        // };
        let mut audio_manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).unwrap();
        let primary_tweener = audio_manager.add_modulator(
            TweenerBuilder {
                initial_value: 0.0,
            }
        ).unwrap();
        let secondary_tweener = audio_manager.add_modulator(
            TweenerBuilder {
                initial_value: 0.0,
            }
        ).unwrap();
        let primary_builder = TrackBuilder::new().volume(Value::FromModulator {
            id: primary_tweener.id(),
            mapping: Mapping {
                input_range: (0.0, 1.0),
                output_range: (Decibels::SILENCE, Decibels::IDENTITY),
                easing: Easing::OutPowi(3),
            },
        });
        let secondary_builder = TrackBuilder::new().volume(Value::FromModulator {
            id: secondary_tweener.id(),
            mapping: Mapping {
                input_range: (0.0, 1.0),
                output_range: (Decibels::SILENCE, Decibels::IDENTITY),
                easing: Easing::OutPowi(3),
            },
        });
        let primary_track = audio_manager.add_sub_track(primary_builder).unwrap();
        let secondary_track = audio_manager.add_sub_track(secondary_builder).unwrap();

        Self {
            is_loading: false,
            audio_manager,
            primary_track_handle: primary_track,
            secondary_track_handle: secondary_track,
            used_track_handle: UsedTrackHandle::Primary,
            currently_playing_static_sound_handle: None,
            primary_volume_tweener: primary_tweener,
            secondary_volume_tweener: secondary_tweener,
            playlist: MultiplayerPlaylist::new(),
            playback_position: 0.0,
            fade_in_duration: 600,
            fade_out_duration: 600,
            audio_seek_dragged: false,
            connected_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Multiplayer {

    fn get_unused_track_handle(&mut self) -> &mut TrackHandle {
        match self.used_track_handle {
            UsedTrackHandle::Primary => &mut self.secondary_track_handle,
            UsedTrackHandle::Secondary => &mut self.primary_track_handle,
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.audio_seek_dragged {
            return Subscription::none()
        }
        
        iced::time::every(Duration::from_secs_f64(1.0)).map(|_| Message::TickPlaybackPosition)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFiles => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;

                    Task::perform(open_files(), Message::FilesOpened)
                }
            }
            Message::FilesOpened(result) => {
                self.is_loading = false;

                if let Ok(tracks) = result {
                    for track in tracks {
                        self.playlist.add_track(track)
                    }
                }

                Task::none()
            }

            Message::ImportPlaylist => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;

                    Task::perform(open_playlist(), Message::PlaylistImported)
                }
            },

            Message::PlaylistImported(result) => {
                if let Ok(tracks) = result {
                    self.playlist.tracks.clear();
                    self.playlist.current_track = None;
                    self.playback_position = 0.0;
                    if self.currently_playing_static_sound_handle.is_some() {
                        self.currently_playing_static_sound_handle.as_mut().unwrap().stop(Tween {
                            start_time: StartTime::Immediate,
                            duration: Duration::from_secs_f64(0.0),
                            easing: Easing::Linear,
                        });
                        self.currently_playing_static_sound_handle = None;
                    }
                    for track in tracks {
                        self.playlist.add_track(track);
                    }
                }
                self.is_loading = false;

                // TODO Remove this temporary testing server spawning
                Task::perform(server::run(self.connected_clients.clone()), |_| Message::Server)
                // Task::none()
            }

            Message::ExportPlaylist => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;

                    Task::perform(save_playlist(), Message::PlaylistExported)
                }
            },

            Message::PlaylistExported(result) => {
                match result {
                    Ok(path) => {
                        let playlist = self.playlist.tracks.iter()
                            .map(|track| {
                                Track {
                                    path: track.path.clone(),
                                    volume: track.volume,
                                }
                            })
                            .collect::<Vec<Track>>();
                        let playlist = Playlist {
                            tracks: playlist
                        };
                        let playlist_json = serde_json::to_string(&playlist).unwrap();

                        Task::perform(save_playlist_to_file(path, playlist_json), Message::PlaylistSavedToFile)
                    }
                    Err(_) => {
                        self.is_loading = false;
                        Task::none()
                    }
                }
            },

            Message::PlaylistSavedToFile(_) => {
                self.is_loading = false;
                Task::none()
            }

            Message::MultiplayerPlaylist(message) => {
                match message {
                    MultiplayerPlaylistMessage::MultiplayerTrack(index, message) => {
                        match message {
                            MultiplayerTrackMessage::Play(reset) => {
                                if self.playlist.current_track.is_some_and(|current_track| current_track == index) && !reset {
                                    return Task::none();
                                }
                                self.playlist.current_track = Some(index);
                                let new_volume = match self.playlist.get_track(index) {
                                    None => 1.0,
                                    Some(track) => track.volume,
                                };
                                if reset {
                                    self.playback_position = 0.0;
                                }
                                else {
                                    self.playback_position = match self.playlist.get_current_track() {
                                        None => 0.0,
                                        Some(track) => {
                                            if track.data.duration() < Duration::from_secs_f64(self.playback_position) {
                                                0.0
                                            } else {
                                                self.playback_position
                                            }
                                        }
                                    };
                                }

                                if self.currently_playing_static_sound_handle.is_some() {
                                    self.currently_playing_static_sound_handle.take().unwrap().stop(Tween {
                                        start_time: StartTime::Immediate,
                                        duration: Duration::from_millis(self.fade_out_duration),
                                        easing: Easing::Linear,
                                    });
                                }
                                let static_sound_data = match self.playlist.get_track(index) {
                                    None => return Task::none(),
                                    Some(track) => {
                                        track.data
                                            .start_position(PlaybackPosition::Seconds(self.playback_position))
                                            .loop_region(..)

                                    },
                                };
                                if self.used_track_handle == UsedTrackHandle::Primary {
                                    self.primary_volume_tweener.set(
                                        0.0,
                                        Tween {
                                            start_time: StartTime::Immediate,
                                            duration: Duration::from_millis(self.fade_out_duration),
                                            easing: Easing::Linear,
                                        });
                                }
                                else {
                                    self.secondary_volume_tweener.set(
                                        0.0,
                                        Tween {
                                            start_time: StartTime::Immediate,
                                            duration: Duration::from_millis(self.fade_out_duration),
                                            easing: Easing::Linear,
                                        });
                                }
                                self.currently_playing_static_sound_handle = Option::from(self.get_unused_track_handle().play(static_sound_data).unwrap());
                                if self.used_track_handle == UsedTrackHandle::Primary {
                                    self.secondary_volume_tweener.set(
                                        new_volume,
                                        Tween {
                                            start_time: StartTime::Immediate,
                                            duration: Duration::from_millis(self.fade_in_duration),
                                            easing: Easing::Linear,
                                        });
                                }
                                else {
                                    self.primary_volume_tweener.set(
                                        new_volume,
                                        Tween {
                                            start_time: StartTime::Immediate,
                                            duration: Duration::from_millis(self.fade_in_duration),
                                            easing: Easing::Linear,
                                        });
                                }
                                self.used_track_handle = if self.used_track_handle == UsedTrackHandle::Primary { UsedTrackHandle::Secondary } else { UsedTrackHandle::Primary };
                            }
                            MultiplayerTrackMessage::UpdateVolumeSlider(new_volume) => {
                                self.playlist.tracks[index].volume = new_volume;
                                if self.playlist.current_track.is_some_and(|current_track| current_track == index) {
                                    if self.used_track_handle == UsedTrackHandle::Primary {
                                        self.primary_volume_tweener.set(
                                            new_volume,
                                            Tween {
                                                start_time: StartTime::Immediate,
                                                duration: Duration::from_millis(0),
                                                easing: Easing::Linear,
                                            });
                                    }
                                    else {
                                        self.secondary_volume_tweener.set(
                                            new_volume,
                                            Tween {
                                                start_time: StartTime::Immediate,
                                                duration: Duration::from_millis(0),
                                                easing: Easing::Linear,
                                            });
                                    }
                                }
                            },
                            MultiplayerTrackMessage::Remove => {
                                if self.playlist.current_track.is_some_and(|current_track| current_track == index ) {
                                    self.playlist.current_track = None;
                                    self.currently_playing_static_sound_handle.as_mut().unwrap().stop(Tween {
                                        start_time: StartTime::Immediate,
                                        duration: Duration::from_millis(0),
                                        easing: Easing::Linear,
                                    });
                                    self.currently_playing_static_sound_handle = None;
                                    self.playback_position = 0.0;
                                }
                                else if self.playlist.current_track.is_some() && index < self.playlist.current_track.unwrap() {
                                    self.playlist.current_track = Some(self.playlist.current_track.unwrap() - 1);
                                }
                                self.playlist.remove_track(index);
                            },
                            MultiplayerTrackMessage::MoveTrackUp => {
                                if index != 0 {
                                    self.playlist.swap_tracks(index, index - 1);
                                    if let Some(current_track) = self.playlist.current_track {
                                        if current_track == index {
                                            self.playlist.current_track = Some(index - 1);
                                        }
                                        else if current_track == index - 1 {
                                            self.playlist.current_track = Some(index);
                                        }
                                    }
                                }
                            },
                            MultiplayerTrackMessage::MoveTrackDown => {
                                if index != self.playlist.tracks.len() - 1 {
                                    self.playlist.swap_tracks(index, index + 1);
                                    if let Some(current_track) = self.playlist.current_track {
                                        if current_track == index {
                                            self.playlist.current_track = Some(index + 1);
                                        }
                                        else if current_track == index + 1 {
                                            self.playlist.current_track = Some(index);
                                        }
                                    }
                                }
                            },
                        }
                    }
                }

                Task::none()
            },
            Message::UpdatePlaybackPositionSlider(slider_position) => {
                self.audio_seek_dragged = true;
                self.playback_position = slider_position;

                Task::none()
            },
            Message::SeekToPlaybackPosition => {
                if let Some(handle) = self.currently_playing_static_sound_handle.as_mut() {
                    handle.seek_to(self.playback_position);
                }
                self.audio_seek_dragged = false;

                Task::none()
            },
            Message::TickPlaybackPosition => {
                if let Some(handle) = &self.currently_playing_static_sound_handle {
                    self.playback_position = handle.position();
                }

                Task::none()
            },
            Message::UpdateFadeInDurationSlider(fade_in) => {
                self.fade_in_duration = fade_in as u64;

                Task::none()
            },
            Message::UpdateFadeOutDurationSlider(fade_out) => {
                self.fade_out_duration = fade_out as u64;

                Task::none()
            },

            Message::Pause => {
                if self.currently_playing_static_sound_handle.is_some() {
                    self.currently_playing_static_sound_handle.as_mut().unwrap().pause(Tween {
                        start_time: StartTime::Immediate,
                        duration: Duration::from_millis(self.fade_out_duration),
                        easing: Easing::Linear,
                    })
                }
                Task::none()
            },

            Message::Resume => {
                if self.currently_playing_static_sound_handle.is_some() {
                    let handle = self.currently_playing_static_sound_handle.as_mut().unwrap();
                    if handle.state() == PlaybackState::Paused {
                        handle.resume(Tween {
                            start_time: StartTime::Immediate,
                            duration: Duration::from_millis(self.fade_in_duration),
                            easing: Easing::Linear,
                        })
                    }
                }

                Task::none()
            }

            Message::Stop => {
                if self.currently_playing_static_sound_handle.is_some() {
                    self.currently_playing_static_sound_handle.as_mut().unwrap().stop(Tween {
                        start_time: StartTime::Immediate,
                        duration: Duration::from_millis(self.fade_out_duration),
                        easing: Easing::Linear,
                    });
                    self.currently_playing_static_sound_handle = None;
                    self.playlist.current_track = None;
                }
                self.playback_position = 0.0;

                Task::none()
            }
            Message::Server => {
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let fade_in_slider: Container<'_, Message> = container(
            row![
                slider(
                    0.0..=5000.0,
                    self.fade_in_duration as f64,
                    Message::UpdateFadeInDurationSlider,
                )
                    .height(8)
                    .width(FillPortion(4)),
                text(format!("{} ms", self.fade_in_duration)).width(FillPortion(1)),
            ]
                .spacing(4)
        )
            .center_x(Fill)
            .padding([6, 40]);
        let fade_out_slider: Container<'_, Message> = container(
            row![
                slider(
                    0.0..=5000.0,
                    self.fade_out_duration as f64,
                    Message::UpdateFadeOutDurationSlider,
                )
                    .height(8)
                    .width(FillPortion(4)),
                text(format!("{} ms", self.fade_out_duration)).width(FillPortion(1)),
            ]
                .spacing(4)
        )
            .center_x(Fill)
            .padding([6, 40]);

        let connected_clients = Arc::clone(&self.connected_clients);
        let clients = connected_clients.lock().unwrap();
        let client_views = clients.iter().map(|client| {
            Text::new(format!("{}", client.0))
                .size(18)
                .into()
        }).collect::<Vec<Element<Message>>>();
        let client_container = Scrollable::new(
            Column::from_vec(client_views)
        )
            .spacing(2);
        
        let controls = row![
            action(
                open_file_icon(),
                "Open file",
                (!self.is_loading).then_some(Message::OpenFiles)
            ),
            action(
                save_icon(),
                "Save playlist",
                (!self.is_loading).then_some(Message::ExportPlaylist)
            ),
            action(
                open_icon(),
                "Open playlist",
                (!self.is_loading).then_some(Message::ImportPlaylist)
            ),
            column![
                fade_in_slider.align_y(Alignment::Start),
                vertical_space(),
                fade_out_slider.align_y(Alignment::End),
            ].width(FillPortion(6)),
            text("Connected clients:")
                .align_x(Horizontal::Left)
                .width(FillPortion(2)),
            client_container.width(FillPortion(3)),
        ]
            .height(84)
            .padding(8)
            .spacing(4);

        let total_duration = match self.playlist.get_current_track() {
            Some(track) => track.data.duration(),
            None => Duration::from_secs(0)
        };
        let seeker_slider: Container<'_, Message> = container(
            slider(
                0.0..=total_duration.as_secs_f64(),
                self.playback_position,
                Message::UpdatePlaybackPositionSlider,
            )
                .on_release(Message::SeekToPlaybackPosition)
                .height(16)
                .width(Fill)
        )
            .center_x(Fill)
            .padding([10, 40]);

        let playback_controls = row![
            action(
                open_icon(),
                "Pause",
                (self.currently_playing_static_sound_handle.is_some() && self.currently_playing_static_sound_handle.as_ref().unwrap().state() == PlaybackState::Playing).then_some(Message::Pause)
            ),
            action(
                save_icon(),
                "Resume",
                (self.currently_playing_static_sound_handle.is_some() && self.currently_playing_static_sound_handle.as_ref().unwrap().state() == PlaybackState::Paused).then_some(Message::Resume)
            ),
            action(
                open_icon(),
                "Stop",
                (self.currently_playing_static_sound_handle.is_some() && self.currently_playing_static_sound_handle.as_ref().unwrap().state() != PlaybackState::Stopped).then_some(Message::Stop)
            ),
        ]
            .height(36)
            .padding(8)
            .spacing(8);


        column![
            controls,
            self.playlist.view(),
            vertical_space(),
            seeker_slider,
            container(playback_controls).center_x(Fill),
        ]
            .into()
    }
}

async fn open_files() -> Result<Vec<MultiplayerTrack>, Error> {
    let paths = rfd::AsyncFileDialog::new()
        .set_title("Choose an audio file...")
        .add_filter("Audio files", &["wav", "mp3", "flac", "ogg"])
        .pick_files()
        .await
        .ok_or(Error::DialogClosed)?;

    paths.iter().map(|path| {
        MultiplayerTrack::new(String::from(path.path().to_str().unwrap()))
    })
    .collect::<Result<Vec<MultiplayerTrack>, Error>>()

}

async fn open_playlist() -> Result<Vec<MultiplayerTrack>, Error> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Choose a playlist file...")
        .add_filter("Playlist files", &["json"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    parse_playlist(path).await
}

async fn parse_playlist(file_handle: FileHandle) -> Result<Vec<MultiplayerTrack>, Error> {
    let playlist_json = std::fs::read_to_string(file_handle.path().to_str().unwrap()).unwrap();
    let playlist: Playlist = serde_json::from_str(&playlist_json).unwrap();
    
    playlist.tracks.iter()
        .map(|track| {
            MultiplayerTrack::from(track)
        })
        .collect::<Result<Vec<MultiplayerTrack>, Error>>()
}

async fn save_playlist() -> Result<FileHandle, Error> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Choose a playlist file...")
        .add_filter("Playlist files", &["json"])
        .save_file()
        .await
        .ok_or(Error::DialogClosed)?;

    Ok(path)
}

async fn save_playlist_to_file(path: FileHandle, playlist_json: String) -> Result<(), Error> {
    let res = path.write(playlist_json.as_bytes()).await;
    match res {
        Ok(_) => {
            Ok(())
        }
        Err(_) => {
            Err(Error::IoError(io::ErrorKind::Other))
        }
    }
}

fn action<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    label: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let action = button(center(content).width(30));

    if let Some(on_press) = on_press {
        tooltip(
            action.on_press(on_press),
            label,
            tooltip::Position::FollowCursor,
        )
            .style(container::rounded_box)
            .into()
    } else {
        action.style(button::secondary).into()
    }
}

fn open_file_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0e800}')
}

fn save_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0e801}')
}

fn open_icon<'a, Message>() -> Element<'a, Message> {
    icon('\u{0f115}')
}

fn icon<'a, Message>(codepoint: char) -> Element<'a, Message> {
    const ICON_FONT: Font = Font::with_name("editor-icons");

    text(codepoint).font(ICON_FONT).into()
}
