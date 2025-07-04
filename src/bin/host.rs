use iced::{Font, Theme};
use multiplayer::multiplayer::Multiplayer;

fn main() -> iced::Result {
    iced::application("Multiplayer", Multiplayer::update, Multiplayer::view)
        .theme(theme)
        .font(include_bytes!("../../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .run()
}

fn theme(state: &Multiplayer) -> Theme {
    Theme::SolarizedDark
}
