use iced::{Element, Fill};
use iced::widget::{row, column, scrollable, Container, button, text, container, slider, Column, Row};
use kira::sound::static_sound::StaticSoundData;
use crate::multiplayer::Message;

#[derive(Debug, Clone)]
pub enum MultiplayerTrackMessage {
    Play(usize),
    UpdateVolumeSlider(f64),
}

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
    
    pub fn view(&self, index: usize) -> Element<MultiplayerTrackMessage> {
        let audio_slider: Container<MultiplayerTrackMessage> = container(
            slider(
                0.0..=1.0,
                self.volume,
                MultiplayerTrackMessage::UpdateVolumeSlider,
            )
                .height(16)
                .width(Fill)
        )
            .center_x(Fill)
            .padding([10, 40]);

        let top_row: Row<MultiplayerTrackMessage> = row![
                        text(format!("{} - {}", index, self.path)),
                        button("Play")
                            .on_press(MultiplayerTrackMessage::Play(index))
                            .padding(8)
                    ]
            .padding(4)
            .spacing(2);


        column![
                        top_row,
                        audio_slider,
                ]
            .into() 
    }
    
    pub fn update(&mut self, message: MultiplayerTrackMessage) {
        match message {
            MultiplayerTrackMessage::Play(index) => {
                
            }
            MultiplayerTrackMessage::UpdateVolumeSlider(new_volume) => {
                
            }
        }
    }
    
}

#[derive(Debug, Clone)]
pub enum MultiplayerPlaylistMessage {
    Play(usize),
    Pause,
    Stop,
    UpdateVolumeSlider(f64),
    VolumeSliderRelease(usize),
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
                let audio_slider: Container<MultiplayerPlaylistMessage> = container(
                    slider(
                        0.0..=1.0,
                        self.tracks[index].volume,
                        MultiplayerPlaylistMessage::UpdateVolumeSlider,
                    )
                        .on_release(MultiplayerPlaylistMessage::VolumeSliderRelease(index))
                        .height(16)
                        .width(Fill)
                )
                    .center_x(Fill)
                    .padding([10, 40]);
                
                let top_row: Row<MultiplayerPlaylistMessage> = row![
                        text(format!("{} - {}", index, track.path)),
                        button("Play")
                            .on_press(MultiplayerPlaylistMessage::Play(index))
                            .padding(8)
                    ]
                        .padding(4)
                        .spacing(2);


                    column![
                        top_row,
                        audio_slider,
                ]
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

