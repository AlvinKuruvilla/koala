use koala::app::application::Browser;

pub fn main() -> iced::Result {
    let _ = iced::application::application(Browser::title, Browser::update, Browser::view).run();
    Ok(())
}
