use std::io;
use std::io::Cursor;
use std::net::UdpSocket;
use std::ops::Mul;
use std::path::PathBuf;
use std::sync::Arc;
use iced::{Application, Element, Fill, Font, Task, Theme};
use iced::widget::{button, center, container, row, column, text, tooltip, vertical_space};
use rodio::Sink;
use multiplayer::track::Track;

fn main() -> iced::Result {
    iced::application("Multiplayer", Multiplayer::update, Multiplayer::view)
        .theme(theme)
        .font(include_bytes!("../../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .run()
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFile,
    FileOpened(Result<(PathBuf, Arc<Vec<u8>>), Error>),
    Play,
    SwitchTrack(usize),
}

#[derive(Debug, Clone)]
pub enum Error {
    DialogClosed,
    IoError(io::ErrorKind),
}

struct Multiplayer {
    is_loading: bool,
    sink: rodio::Sink,
    output: rodio::OutputStream,
    udp_socket: UdpSocket,
    sound_data: Arc<Vec<u8>>,
    playback_position: usize
}

impl Default for Multiplayer {
    fn default() -> Self {
        let (stream, handle) = rodio::OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&handle).unwrap();
        Self {
            is_loading: false,
            sink: sink,
            output: stream,
            udp_socket: UdpSocket::bind("0.0.0.0:9475").unwrap(),
            sound_data: Arc::new(Vec::new()),
            playback_position: 0
        }
    }
}

impl Multiplayer {
    fn update(&mut self, message: Message) -> Task<Message> {
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
                    self.sound_data = contents;
                }

                Task::none()
            }
            Message::Play => {
                let cursor = Cursor::new(AsRefArcVec(self.sound_data.clone()));
                let source = rodio::Decoder::new(cursor).unwrap();
                self.sink.append(source);
                self.sink.play();

                Task::none()
            }

            Message::SwitchTrack(track) => {
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
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
        ]
            .into()
    }
}

struct AsRefArcVec(Arc<Vec<u8>>);

impl AsRef<[u8]> for AsRefArcVec {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

async fn open_file() -> Result<(PathBuf, Arc<Vec<u8>>), Error> {
    let path = rfd::AsyncFileDialog::new()
        .set_title("Choose an audio file...")
        .add_filter("Audio files", &["wav", "mp3", "flac", "ogg"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_file(path.path().to_owned()).await
}

async fn load_file(path: PathBuf) -> Result<(PathBuf, Arc<Vec<u8>>), Error> {
    let contents = smol::fs::read(path.clone())
        .await
        .map(Arc::new)
        .map_err(|error| error.kind())
        .map_err(Error::IoError)?;

    Ok((path, contents))
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

fn theme(state: &Multiplayer) -> Theme {
    Theme::SolarizedDark
}
