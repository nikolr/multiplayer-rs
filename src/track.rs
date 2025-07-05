use iced::Length;
use crate::multiplayer::Message;
use iced::widget::{button, column, container, horizontal_space, row, scrollable, slider, text, Column, Container, Row};
use iced::{Element, Fill};
use iced::alignment::Horizontal;
use kira::sound::static_sound::StaticSoundData;

#[derive(Debug, Clone)]
pub enum MultiplayerTrackMessage {
    Play,
    UpdateVolumeSlider(f64),
}

pub struct MultiplayerTrack {
    pub path: String,
    pub data: StaticSoundData,
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
    
    pub fn view(&self) -> Element<MultiplayerTrackMessage> {
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
                text(format!("{}", self.path)).align_x(Horizontal::Left),
                horizontal_space(),
                container(
                    button("Play").on_press(MultiplayerTrackMessage::Play)
                ).align_x(Horizontal::Right)
                .padding([2, 20]),
            ]
                .spacing(10)
        )
            .center_x(Fill)
            .width(Fill)
            .padding([2, 20]);

        column![
                        top_row,
                        audio_slider,
                ]
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
        let multiplayer_track_views: Vec<Element<MultiplayerPlaylistMessage>> = self.tracks.iter()
            .map(MultiplayerTrack::view)
            .enumerate()
            .map(|(index, track)| {
                track.map(move |message| MultiplayerPlaylistMessage::MultiplayerTrack(index, message))
                    .into()
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

