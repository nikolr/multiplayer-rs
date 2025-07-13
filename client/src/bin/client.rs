#![windows_subsystem = "windows"]

use iced::{Font, Theme};
use multiplayer_client::multiplayer::Multiplayer;

fn main() -> iced::Result {
    iced::application("Multiplayer", Multiplayer::update, Multiplayer::view)
        .theme(theme)
        .font(include_bytes!("../../../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .run()
}

fn theme(_state: &Multiplayer) -> Theme {
    Theme::SolarizedDark
}
