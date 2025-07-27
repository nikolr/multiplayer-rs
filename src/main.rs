use iced::{Element, Font, Subscription, Task, Theme};
mod client;
mod entry;
mod host;

fn main() -> iced::Result {
    iced::application("Multiplayer", update, view)
        .theme(theme)
        .font(include_bytes!("../assets/fonts/icons.ttf").as_slice())
        .default_font(Font::MONOSPACE)
        .subscription(subscription)
        .run()
}

fn theme(_state: &State) -> Theme {
    Theme::SolarizedDark
}


struct State {
    screen: Screen,
}

impl Default for State {
    fn default() -> Self {
        State {
            screen: Screen::Entry(entry::entry::Entry::new())
        }
    }
}

enum Screen {
    Entry(entry::entry::Entry),
    Host(host::host::Host),
    Client(client::client::Client),
}

#[derive(Debug, Clone)]
enum Message {
    Entry(entry::entry::Message),
    Host(host::host::Message),
    Client(client::client::Message),
}

fn subscription(state: &State) -> Subscription<Message> {
    match &state.screen {
        Screen::Client(client) => client.subscription().map(Message::Client),
        Screen::Host(host) => host.subscription().map(Message::Host),
        Screen::Entry(_entry) => Subscription::none(),
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    match message {
        Message::Entry(message) => {
            if let Screen::Entry(entry) = &mut state.screen {
                // entry.update(message).map(Message::Entry)
                match message {
                    entry::entry::Message::StartHost => {
                        println!("Starting host");
                        state.screen = Screen::Host(host::host::Host::new());
                        Task::none()
                    }
                    entry::entry::Message::StartClient => {
                        println!("Starting client");
                        state.screen = Screen::Client(client::client::Client::new());
                        Task::none()
                    }
                }
            } else {
                Task::none()
            }

        },
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
        }
    }
}

fn view(state: &State) -> Element<Message> {
    match &state.screen {
        Screen::Entry(entry) => entry.view().map(Message::Entry),
        Screen::Host(host) => host.view().map(Message::Host),
        Screen::Client(client) => client.view().map(Message::Client),
    }
}