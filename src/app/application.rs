use iced::widget::{column, row, Button, TextInput};
use iced::{Alignment, Element};

#[derive(Default)]
pub struct Browser {
    url: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    UrlChanged(String),
    Back,
    Forward,
}

impl Browser {
    fn new(_flags: ()) -> Self {
        Self::default()
    }

    pub fn title(&self) -> String {
        "Minimalist Browser".to_string()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::UrlChanged(new_url) => {
                self.url = new_url;
            }
            Message::Back => {
                // Logic for "Back" can be implemented here
                println!("Back button clicked");
            }
            Message::Forward => {
                // Logic for "Forward" can be implemented here
                println!("Forward button clicked");
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let back_button = Button::new("←").on_press(Message::Back).padding(10);

        let forward_button = Button::new("→").on_press(Message::Forward).padding(10);

        let url_input = TextInput::new("Enter URL...", &self.url)
            .on_input(Message::UrlChanged)
            .padding(10)
            .size(20);

        let content = row![back_button, forward_button, url_input]
            .spacing(10)
            .align_y(Alignment::Center);

        column![content]
            .padding(20)
            .align_x(Alignment::Center)
            .into()
    }
}
