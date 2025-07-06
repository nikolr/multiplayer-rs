use crate::track::{MultiplayerPlaylist, MultiplayerPlaylistMessage, MultiplayerTrack, MultiplayerTrackMessage};
use iced::widget::{button, center, column, container, row, slider, text, tooltip, vertical_space, Container};
use iced::{Element, Fill, Font, Subscription, Task};
use kira::modulator::tweener::{TweenerBuilder, TweenerHandle};
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::PlaybackPosition;
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, StartTime, Tween, Value};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;
use std::{error, io, thread};
use rfd::FileHandle;
use sysinfo::{get_current_pid, Pid};
use wasapi::{initialize_mta, AudioClient, Direction, SampleType, StreamMode, WaveFormat};
use crate::playlist::{Playlist, Track};

#[derive(Debug, Clone)]
pub enum Message {
    OpenFiles,
    FilesOpened(Result<Vec<FileHandle>, Error>),
    ImportPlaylist,
    PlaylistImported(Result<FileHandle, Error>),
    ExportPlaylist,
    PlaylistExported(Result<FileHandle, Error>),
    PlaylistSavedToFile(Result<(), Error>),
    MultiplayerPlaylist(MultiplayerPlaylistMessage),
    UpdatePlaybackPositionSlider(f64),
    SeekToPlaybackPosition,
    TickPlaybackPosition,
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
    currently_playing_static_sound_handle: Option<StaticSoundHandle>,
    volume_tweener: TweenerHandle,
    playlist: MultiplayerPlaylist,
    playback_position: f64,
    fade_in_duration: Duration,
    fade_out_duration: Duration,
    volume_fade_in_out_duration: Duration,
    audio_seek_dragged: bool,
}

impl Default for Multiplayer {
    fn default() -> Self {
        let process_id = get_current_pid().unwrap();
        let (tx_capt, rx_capt): (
            std::sync::mpsc::SyncSender<Vec<u8>>,
            std::sync::mpsc::Receiver<Vec<u8>>,
        ) = mpsc::sync_channel(2);
        let chunksize = 4096;

        // Capture
        let _handle = thread::Builder::new()
            .name("Capture".to_string())
            .spawn(move || {
                let result = capture_loop(tx_capt, chunksize, process_id);
                if let Err(err) = result {
                }
            });

        let mut outfile = File::create("recorded_u8.raw").unwrap();

        thread::spawn(move || {
            loop {
                match rx_capt.recv() {
                    Ok(chunk) => {
                        outfile.write_all(&chunk).unwrap();
                    }
                    Err(err) => {

                    }
                }
            }
        });

        let mut audio_manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()).unwrap();
        let mut tweener = audio_manager.add_modulator(
            TweenerBuilder {
                initial_value: 0.0,
            }
        ).unwrap();
        let builder = TrackBuilder::new().volume(Value::FromModulator {
            id: tweener.id(),
            mapping: Mapping {
                input_range: (0.0, 1.0),
                output_range: (Decibels::SILENCE, Decibels::IDENTITY),
                easing: Easing::InOutPowi(1),
            },
        });
        let mut primary_track = audio_manager.add_sub_track(builder).unwrap();
        let mut secondary_track = audio_manager.add_sub_track(TrackBuilder::default()).unwrap();

        Self {
            is_loading: false,
            audio_manager: audio_manager,
            primary_track_handle: primary_track,
            secondary_track_handle: secondary_track,
            currently_playing_static_sound_handle: None,
            volume_tweener: tweener,
            playlist: MultiplayerPlaylist::new(),
            playback_position: 0.0,
            fade_in_duration: Duration::from_millis(600),
            fade_out_duration: Duration::from_millis(600),
            volume_fade_in_out_duration: Duration::from_millis(1000),
            audio_seek_dragged: false,
        }
    }
}

impl Multiplayer {

    pub fn subscription(&self) -> Subscription<Message> {
        if self.audio_seek_dragged {
            return Subscription::none()
        }
        let time = iced::time::every(Duration::from_secs_f64(1.0)).map(|_| Message::TickPlaybackPosition);
        Subscription::from(time)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFiles => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;

                    Task::perform(open_file(), Message::FilesOpened)
                }
            }
            Message::FilesOpened(result) => {
                self.is_loading = false;

                if let Ok(paths) = result {
                    for path in paths {
                        self.playlist.add_track(MultiplayerTrack::new(String::from(path.path().to_str().unwrap())).unwrap())
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
                if let Ok(file) = result {
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
                    let playlist_json = std::fs::read_to_string(file.path().to_str().unwrap()).unwrap();
                    let playlist: Playlist = serde_json::from_str(&playlist_json).unwrap();
                    for track in playlist.tracks {
                        self.playlist.add_track(MultiplayerTrack::from(&track).unwrap())
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
                println!("Playlist Saved");
                self.is_loading = false;
                Task::none()
            }

            Message::MultiplayerPlaylist(message) => {
                match message {
                    MultiplayerPlaylistMessage::MultiplayerTrack(index, message) => {
                        match message {
                            MultiplayerTrackMessage::Play => {
                                self.playlist.current_track = Some(index);
                                let new_volume = match self.playlist.get_current_track() {
                                    None => 1.0,
                                    Some(track) => track.volume,
                                };
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

                                if self.currently_playing_static_sound_handle.is_some() {
                                    self.currently_playing_static_sound_handle.take().unwrap().stop(Tween {
                                        start_time: StartTime::Immediate,
                                        duration: self.fade_out_duration,
                                        easing: Easing::Linear,
                                    });
                                }
                                let static_sound_data = self.playlist.get_track(index).data
                                    .start_position(PlaybackPosition::Seconds(self.playback_position))
                                    .loop_region(..)
                                    .fade_in_tween(Tween {
                                        start_time: StartTime::Immediate,
                                        duration: self.fade_in_duration,
                                        easing: Easing::Linear,
                                    });

                                self.currently_playing_static_sound_handle = Option::from(self.primary_track_handle.play(static_sound_data).unwrap());
                                self.volume_tweener.set(
                                    new_volume,
                                    Tween {
                                        start_time: StartTime::Immediate,
                                        duration: self.volume_fade_in_out_duration,
                                        easing: Easing::Linear,
                                    });
                            }
                            MultiplayerTrackMessage::UpdateVolumeSlider(new_volume) => {
                                self.playlist.tracks[index].volume = new_volume;
                                if self.playlist.current_track.is_some_and(|current_track| current_track == index) {
                                    self.volume_tweener.set(
                                        new_volume,
                                        Tween {
                                            start_time: StartTime::Immediate,
                                            duration: self.fade_out_duration,
                                            easing: Easing::Linear,
                                        });
                                }
                            },
                            MultiplayerTrackMessage::Remove => {
                                if self.playlist.current_track.is_some_and(|current_track| current_track == index ) {
                                    self.playlist.current_track = None;
                                    self.currently_playing_static_sound_handle.as_mut().unwrap().stop(Tween {
                                        start_time: StartTime::Immediate,
                                        duration: self.fade_out_duration,
                                        easing: Easing::Linear,
                                    });
                                    self.currently_playing_static_sound_handle = None;
                                    self.playback_position = 0.0;
                                }
                                self.playlist.remove_track(index);
                            },
                            MultiplayerTrackMessage::MoveTrackUp => {
                                if index != 0 {
                                    self.playlist.swap_tracks(index, index - 1);
                                    println!("Move Track Up");
                                }
                            },
                            MultiplayerTrackMessage::MoveTrackDown => {
                                if index != self.playlist.tracks.len() - 1 {
                                    self.playlist.swap_tracks(index, index + 1);
                                    println!("Move Track Down");
                                }
                            },
                        }
                    }
                }

                Task::none()
            },
            Message::UpdatePlaybackPositionSlider(slider_position) => {
                self.audio_seek_dragged = true;
                println!("Update Slider: {:?}", slider_position);
                self.playback_position = slider_position;

                Task::none()
            },
            Message::SeekToPlaybackPosition => {
                if let Some(handle) = self.currently_playing_static_sound_handle.as_mut() {
                    println!("Seek Audio: {:?}", self.playback_position);
                    handle.seek_to(self.playback_position);
                }
                self.audio_seek_dragged = false;

                Task::none()
            },
            Message::TickPlaybackPosition => {
                if let Some(handle) = &self.currently_playing_static_sound_handle {
                    println!("Update Time: {:?}", handle.position());
                    self.playback_position = handle.position();
                }

                Task::none()
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        let controls = row![
            action(
                open_icon(),
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
        ]
            .height(42)
            .padding(2)
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
        
        column![
            controls,
            vertical_space(),
            self.playlist.view(),
            vertical_space(),
            seeker_slider,
        ]
            .into()
    }
}

async fn open_file() -> Result<Vec<FileHandle>, Error> {
    let paths = rfd::AsyncFileDialog::new()
        .set_title("Choose an audio file...")
        .add_filter("Audio files", &["wav", "mp3", "flac", "ogg"])
        .pick_files()
        .await
        .ok_or(Error::DialogClosed)?;

    Ok(paths)
}

async fn open_playlist() -> Result<FileHandle, Error> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Choose a playlist file...")
        .add_filter("Playlist files", &["json"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    Ok(path)
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


// Capture loop, capture samples and send in chunks of "chunksize" frames to channel
fn capture_loop(
    tx_capt: std::sync::mpsc::SyncSender<Vec<u8>>,
    chunksize: usize,
    process_id: Pid,
) -> Result<(), Box<dyn error::Error>> {
    initialize_mta().ok().unwrap();

    let desired_format = WaveFormat::new(8, 8, &SampleType::Int, 44100, 2, None);
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

    // just eat the reallocation because querying the buffer size gives massive values.
    let mut sample_queue: VecDeque<u8> = VecDeque::new();

    audio_client.start_stream().unwrap();

    loop {
        while sample_queue.len() > (blockalign as usize * chunksize) {
            let mut chunk = vec![0u8; blockalign as usize * chunksize];
            for element in chunk.iter_mut() {
                *element = sample_queue.pop_front().unwrap();
            }
            tx_capt.send(chunk).unwrap();
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
