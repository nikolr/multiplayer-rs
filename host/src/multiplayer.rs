use crate::playlist::{Playlist, Track};
use crate::track::{MultiplayerPlaylist, MultiplayerPlaylistMessage, MultiplayerTrack, MultiplayerTrackMessage};
use iced::widget::{button, center, column, container, row, slider, text, tooltip, vertical_space, Container};
use iced::{Alignment, Element, Fill, Font, Subscription, Task};
use kira::modulator::tweener::{TweenerBuilder, TweenerHandle};
use kira::sound::static_sound::StaticSoundHandle;
use kira::sound::{PlaybackPosition, PlaybackState};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, StartTime, Tween, Value};
use rfd::FileHandle;
use std::cmp::PartialEq;
use std::collections::VecDeque;
use std::net::UdpSocket;
use std::time::Duration;
use std::{error, io, thread};
use opus::Bitrate;
use opus::ErrorCode as OpusErrorCode;
use sysinfo::{get_current_pid, Pid};
use wasapi::{initialize_mta, AudioClient, Direction, SampleType, StreamMode, WaveFormat};

const CAPTURE_CHUNK_SIZE: usize = 480;
const BIT_RATE: i32 = 64000;

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
}

impl Default for Multiplayer {
    fn default() -> Self {
        let process_id = get_current_pid().unwrap();
        let (tx_capt, rx_capt): (
            std::sync::mpsc::SyncSender<Vec<u8>>,
            std::sync::mpsc::Receiver<Vec<u8>>,
        ) = std::sync::mpsc::sync_channel(2);

        let _handle = thread::Builder::new()
            .name("Capture".to_string())
            .spawn(move || {
                let result = capture_loop(tx_capt, CAPTURE_CHUNK_SIZE, process_id);
                if let Err(_err) = result {
                }
            });

        let udp_socket = UdpSocket::bind("192.168.0.31:9475").unwrap();

        thread::spawn(move || {
            loop {
                match rx_capt.recv() {
                    Ok(chunk) => {
                            if let Ok(_length) = udp_socket.send_to(&chunk, "192.168.0.45:9476") {
                            }
                        }
                    Err(_err) => {}
                }
            }
        });

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

                Task::none()
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
                    .width(Fill),
                text(format!("{} ms", self.fade_in_duration)).width(Fill),
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
                    .width(Fill),
                text(format!("{} ms", self.fade_out_duration)).width(Fill),
            ]
                .spacing(4)
        )
            .center_x(Fill)
            .padding([6, 40]);

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
            ]
        ]
            .height(84)
            .padding(8)
            .spacing(8);

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

fn capture_loop(
    tx_capt: std::sync::mpsc::SyncSender<Vec<u8>>,
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