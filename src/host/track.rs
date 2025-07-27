use std::io::ErrorKind;
use crate::host::playlist::Track;
use iced::alignment::Horizontal;
use iced::widget::{button, column, container, row, scrollable, slider, text, Column, Container};
use iced::{Element, Fill};
use iced::Length;
use kira::sound::static_sound::StaticSoundData;
use crate::host::host::{Error, Message};
use crate::host::host::Error::IoError;

#[derive(Debug, Clone)]
pub enum MultiplayerTrackMessage {
    Play(bool),
    UpdateVolumeSlider(f64),
    Remove,
    MoveTrackUp,
    MoveTrackDown,
}


#[derive(Debug, Clone)]
pub struct MultiplayerTrack {
    pub path: String,
    pub data: StaticSoundData,
    pub volume: f64,
}

impl MultiplayerTrack {
    pub fn new(path: String) -> Result<Self, Error> {
        let static_sound_data = StaticSoundData::from_file(path.clone());
        match static_sound_data {
            Ok(data) => Ok(Self {
                path,
                data,
                volume: 1.0,
            }),
            Err(_) => Err(IoError(ErrorKind::InvalidData)),
        }
    }
    
    pub fn from(track: &Track) -> Result<Self, Error> {
        let static_sound_data = StaticSoundData::from_file(track.path.clone());
        match static_sound_data {
            Ok(data) => Ok(Self {
                path: track.path.clone(),
                data,
                volume: track.volume,
            }),
            Err(_) => Err(IoError(ErrorKind::InvalidData)),
        }
    }
    
    pub fn view(&self, currently_playing: bool) -> Element<MultiplayerTrackMessage> {
        let audio_slider: Container<MultiplayerTrackMessage> = container(
            slider(
                0.0..=1.0,
                self.volume,
                MultiplayerTrackMessage::UpdateVolumeSlider,
            )
                .height(16)
                .step(0.01)
                .width(Fill)
        )
            .center_x(Fill)
            .padding([10, 40]);

        let top_row: Container<MultiplayerTrackMessage> = container(
            row![
                container(
                    button("Play").on_press(MultiplayerTrackMessage::Play(false)).height(32)
                ).align_x(Horizontal::Left)
                .padding([2, 4]),
                container(
                    button("Reset").on_press(MultiplayerTrackMessage::Play(true)).height(32)
                ).align_x(Horizontal::Left)
                .padding([2, 4]),
                text(self.path.to_string()).align_x(Horizontal::Center).width(Fill),
                column![
                    button("Remove").on_press(MultiplayerTrackMessage::Remove).height(32),
                ].align_x(Horizontal::Right),
                column![
                    button("UP").on_press(MultiplayerTrackMessage::MoveTrackUp).height(16),
                    button("DOWN").on_press(MultiplayerTrackMessage::MoveTrackDown).height(16),
                ].align_x(Horizontal::Right),
            ]
                .spacing(4)
        )
            .center_x(Fill)
            .width(Fill)
            .padding([2, 20]);

        container(
            column![
                        top_row,
                        audio_slider,
                ]
        ).style(if currently_playing {container::rounded_box} else {container::dark})
            .into()
    }
}

#[derive(Debug, Clone)]
pub enum MultiplayerPlaylistMessage {
    MultiplayerTrack(usize, MultiplayerTrackMessage),
}

pub struct MultiplayerPlaylist {
    pub tracks: Vec<MultiplayerTrack>,
    pub current_track: Option<usize>,
}

impl Default for MultiplayerPlaylist {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiplayerPlaylist {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_track: None,
        }
    }
    
    pub fn add_track(&mut self, track: MultiplayerTrack) {
        self.tracks.push(track);
    }
    
    pub fn remove_track(&mut self, index: usize) {
        self.tracks.remove(index);
    }
    
    pub fn get_track(&self, index: usize) -> Option<&MultiplayerTrack> {
        if index >= self.tracks.len() {
            return None;
        }
        Some(&self.tracks[index])
    }
    
    pub fn swap_tracks(&mut self, index1: usize, index2: usize) {
        self.tracks.swap(index1, index2);
    }
    
    pub fn set_current_track(&mut self, index: Option<usize>) {
        self.current_track = index;
    }
    
    pub fn get_current_track(&self) -> Option<&MultiplayerTrack> {
        self.current_track.and_then(|index| self.tracks.get(index))
    }
    
    pub fn view(&self) -> Element<'_, Message> {
        let multiplayer_track_views: Vec<Element<MultiplayerPlaylistMessage>> = self.tracks.iter()
            .enumerate()
            .map(|index| MultiplayerTrack::view(index.1, self.current_track.is_some_and(|_| index.0 == self.current_track.unwrap())))
            .enumerate()
            .map(|(index, track)| {
                track.map(move |message| MultiplayerPlaylistMessage::MultiplayerTrack(index, message))
            })
            .collect();
        
        let container: Element<'_, MultiplayerPlaylistMessage> = Container::new(
            scrollable(
                Column::with_children(multiplayer_track_views)
            )
        )
            .height(Length::FillPortion(3))
            .padding(10)
            .center_x(Fill)
            .into();
        container.map(Message::MultiplayerPlaylist)
    }
}

