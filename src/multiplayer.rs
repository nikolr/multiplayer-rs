use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::{error, io, thread};
use std::ffi::OsStr;
use std::io::{Cursor, Write};
use std::net::UdpSocket;
use std::ops::Mul;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;
use iced::{Application, Element, Fill, Font, Task, Theme};
use iced::application::Update;
use iced::widget::{button, center, container, row, column, text, tooltip, vertical_space};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, StartTime, Tween, Value};
use kira::modulator::tweener::{TweenerBuilder, TweenerHandle};
use kira::sound::PlaybackPosition;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use wasapi::{initialize_mta, AudioClient, Direction, SampleType, StreamMode, WaveFormat};
use sysinfo::{get_current_pid, Pid, ProcessRefreshKind, RefreshKind, System};
use crate::track::{MultiplayerPlaylist, MultiplayerPlaylistMessage, MultiplayerTrack};

#[derive(Debug, Clone)]
pub enum Message {
    OpenFile,
    FileOpened(Result<(PathBuf, StaticSoundData), Error>),
    Play,
    SwitchTrack(usize),
    MultiplayerPlaylist(MultiplayerPlaylistMessage)
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
                easing: Easing::Linear,
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
        }
    }
}

impl Multiplayer {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFile => {
                if self.is_loading {
                    Task::none()
                } else {
                    self.is_loading = true;

                    Task::perform(open_file(), Message::FileOpened)
                }
            }
            Message::FileOpened(result) => {
                self.is_loading = false;

                if let Ok((path, contents)) = result {
                    self.playlist.add_track(
                        MultiplayerTrack::new(String::from(path.to_str().unwrap()), contents)
                    )
                }

                Task::none()
            }
            Message::Play => {
                Task::none()
            }

            Message::SwitchTrack(track) => {
                Task::none()
            }

            Message::MultiplayerPlaylist(message) => {
                match message {
                    MultiplayerPlaylistMessage::Play(id) => {
                        let new_volume = self.playlist.get_track(id).volume;
                        self.playback_position = match &self.currently_playing_static_sound_handle {
                            None => {
                                0.0
                            }
                            Some(handle) => {
                                handle.position()
                            }
                        };

                        let static_sound_data = self.playlist.get_track(id).data
                            .start_position(PlaybackPosition::Seconds(self.playback_position))
                            .loop_region(..);
                        
                        let handle = self.primary_track_handle.play(static_sound_data).unwrap();
                        self.volume_tweener.set(
                            new_volume,
                            Tween {
                                start_time: StartTime::Immediate,
                                duration: Duration::from_secs(1),
                                easing: Easing::Linear,
                        });
                    },
                    MultiplayerPlaylistMessage::Pause | MultiplayerPlaylistMessage::Stop => todo!()
                }

                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let controls = row![
            action(
                open_icon(),
                "Open file",
                (!self.is_loading).then_some(Message::OpenFile)
            ),
            action(
                icon('\u{0f115}'),
                "Play",
                Some(Message::Play)
            )
        ]
            .height(42)
            .padding(2)
            .spacing(4);

        column![
            controls,
            vertical_space(),
            // TODO Figure out how to shove multiplayerplaylist view here
            self.playlist.view(),
        ]
            .into()
    }
}

async fn open_file() -> Result<(PathBuf, StaticSoundData), Error> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Choose an audio file...")
        .add_filter("Audio files", &["wav", "mp3", "flac", "ogg"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    let static_soud_data = StaticSoundData::from_file(path.path()).unwrap();
    Ok((path.path().to_owned(), static_soud_data))
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
