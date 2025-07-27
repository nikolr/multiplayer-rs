use iced::widget::{button, column, text};
use iced::{Element, Task};

pub struct Entry {

}

#[derive(Debug, Clone)]
pub enum Message {
    StartHost,
    StartClient,
}

impl Entry {
    
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn update(&mut self, message: Message) -> Task<Message>{
        match message {
            Message::StartHost => {
                println!("Starting host");
                
                Task::none()
            },
            Message::StartClient => {
                println!("Starting client");
                
                Task::none()
            },
        }
    }

    pub fn view(&self) -> Element<Message> {
        column![
            button(text("Start Host")).on_press(Message::StartHost),
            button(text("Start Client")).on_press(Message::StartClient),
        ].into()
    }
}