use iced::widget::{column, Space};
use iced::{Element, Font, Length, Subscription, Task, Theme};
use iced_aw::{TabBarPosition, TabLabel, Tabs};

mod client;
mod host;
pub mod settings;

fn main() -> iced::Result {
    iced::application("Multiplayer", update, view)
        .theme(theme)
        .font(include_bytes!("../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .subscription(subscription)
        .run_with(State::new)
}

fn theme(_state: &State) -> Theme {
    Theme::SolarizedDark
}


struct State {
    screen: Screen,
}

impl State {
    fn new() -> (Self, Task<Message>) {
        // TODO: Read a config file here and determine which screen to start on
        let settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
        match settings.mode {
            settings::Mode::Host => {
                let (host, task) = host::host::Host::new(settings);
                let state = State { screen: Screen::Host(host) };
                (state, task.map(Message::Host))
            },
            settings::Mode::Client => {
                let state = State { screen: Screen::Client(client::client::Client::new()) };
                (state, iced::widget::focus_next())
            }
        }
    }
}

enum Screen {
    Host(host::host::Host),
    Client(client::client::Client),
}

#[derive(Debug, Clone)]
enum Message {
    Host(host::host::Message),
    Client(client::client::Message),
    TabSelected(TabId),
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
enum TabId {
    #[default]
    Host,
    Client,
}

impl TabId {
    fn from_screen(screen: &Screen) -> Self {
        match screen {
            Screen::Host(_) => TabId::Host,
            Screen::Client(_) => TabId::Client,
        }
    }
}

fn subscription(state: &State) -> Subscription<Message> {
    match &state.screen {
        Screen::Client(client) => client.subscription().map(Message::Client),
        Screen::Host(host) => host.subscription().map(Message::Host),
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Host(message) => {
            if let Screen::Host(host) = &mut state.screen {
                host.update(message).map(Message::Host)
            } else {
                Task::none()
            }
        },
        Message::Client(message) => {
            if let Screen::Client(client) = &mut state.screen {
                client.update(message).map(Message::Client)
            } else {
                Task::none()
            }
        },
        Message::TabSelected(tab_id) => {
            println!("Tab selected: {:?}", tab_id);
            match tab_id {
                TabId::Host => {
                    println!("Starting host");
                    let mut settings: settings::Settings = confy::load("multiplayer", None).unwrap_or_default();
                    settings.mode = settings::Mode::Host;
                    confy::store("multiplayer", None, &settings).unwrap();
                    let (host, task) = host::host::Host::new(settings);
                    state.screen = Screen::Host(host);
                    Task::batch([
                        task.map(Message::Host)
                    ])
                },
                TabId::Client => {
                    println!("Starting client");
                    if let Screen::Host(host) = &mut state.screen {
                        settings::save(&host).unwrap();
                        let task_handle = host.task_handle.take();
                        println!("Aborting server task");
                        match task_handle {
                            Some(task_handle) => task_handle.abort(),
                            None => {
                                println!("No task handle to abort");
                            }
                        }
                        match host.capture_thread_handle.take() {
                            Some(capture_thread_handle) => {
                                println!("Joining capture thread");
                                let cancel = host.tx_cancel.take();
                                cancel.unwrap().send(()).unwrap();
                                host.rx_capt.take();
                                capture_thread_handle.join().unwrap();

                            }
                            None => {
                                println!("No capture thread handle to join");
                            }       
                        }
                        state.screen = Screen::Client(client::client::Client::new());
                    }
                    Task::batch([
                        iced::widget::focus_next()
                    ])
                },
            }
        },
    }
}

fn view(state: &State) -> Element<Message> {
    let tab_bar: Element<Message> = Tabs::new(Message::TabSelected)
        .tab_bar_position(TabBarPosition::Top)
        .push(
            TabId::Host,
            TabLabel::Text("Host".to_string()),
            Space::with_width(0.0)
        )
        .push(
            TabId::Client,
            TabLabel::Text("Client".to_string()),
            Space::with_width(0.0)
        )
        .set_active_tab(&TabId::from_screen(&state.screen))
        .tab_bar_height(Length::Shrink)
        .into();
    match &state.screen {
        Screen::Host(host) => {
            column![
                tab_bar,
                host.view().map(Message::Host)
            ].into()
        },
        Screen::Client(client) => {

            column![
                tab_bar,
                client.view().map(Message::Client)
            ].into()
        },
    }
}