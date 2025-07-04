use std::collections::{BTreeMap, HashMap};
use iced::Element;
use iced::widget::{button, container, row, scrollable, text, Column, Container};
use kira::sound::PlaybackPosition;
use kira::sound::static_sound::StaticSoundData;
use crate::multiplayer::Message;

pub struct MultiplayerTrack {
    pub path: String,
    pub data: StaticSoundData,
    // TODO Maybe use Decibels here?
    pub volume: f64,
}

impl MultiplayerTrack {
    pub fn new(path: String, data: StaticSoundData) -> Self {
        Self {
            path,
            data,
            volume: 1.0,
        }
    }
    
}

#[derive(Debug, Clone)]
pub enum MultiplayerPlaylistMessage {
    Play(usize),
    Pause,
    Stop,
}

pub struct MultiplayerPlaylist {
    pub tracks: Vec<MultiplayerTrack>,
    pub current_track: Option<usize>,
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
    
    pub fn get_track(&self, index: usize) -> &MultiplayerTrack {
        &self.tracks[index]
    }
    
    pub fn get_current_track(&self) -> Option<&MultiplayerTrack> {
        self.current_track.and_then(|index| self.tracks.get(index))
    }
    
    pub fn view(&self) -> Element<'_, Message> {
        let multiplayer_track_views: Vec<Element<MultiplayerPlaylistMessage>> = self.tracks.iter().enumerate()
            .map(|(index, track)| {
                row![
                    text(format!("{} - {}", index, track.path)),
                    button("Play")
                        .on_press(MultiplayerPlaylistMessage::Play(index))
                        .padding(8)
                ]
                    .padding(4)
                    .spacing(2)
                    .into()
            })
            .collect();
        
        let container: Element<'_, MultiplayerPlaylistMessage> = Container::new(scrollable(
            Column::with_children(multiplayer_track_views)
        ))
            .into();
        container.map(Message::MultiplayerPlaylist)
    }
}

