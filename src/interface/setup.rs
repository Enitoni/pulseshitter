use crossterm::event::{Event, KeyCode, KeyEvent};
use tui::{style::Color, widgets::Widget};

use super::ViewController;

#[derive(Default)]
enum SelectedField {
    #[default]
    BotToken,
    UserId,
}

#[derive(Default)]
pub struct SetupView {
    selected_field: SelectedField,

    bot_token: String,
    user_id: String,
}

impl ViewController for SetupView {
    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up => self.selected_field = SelectedField::BotToken,
                KeyCode::Down => self.selected_field = SelectedField::UserId,
                _ => {}
            }
        }
    }
}

impl Widget for SetupView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        for cell in &mut buf.content {
            cell.set_bg(Color::White);

            match self.selected_field {
                SelectedField::BotToken => cell.set_bg(Color::White),
                SelectedField::UserId => cell.set_bg(Color::Black),
            };
        }
    }
}
