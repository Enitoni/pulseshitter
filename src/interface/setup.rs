use crossterm::event::{Event, KeyCode, KeyEvent};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::Color,
    widgets::Widget,
};
use tui_textarea::TextArea;

use super::ViewController;

#[derive(Default, PartialEq, Sequence)]
enum SelectedField {
    #[default]
    BotToken,
    UserId,
}

#[derive(Default)]
pub struct SetupView {
    selected_field: SelectedField,

    bot_token: TextArea<'static>,
    user_id: TextArea<'static>,
}

impl ViewController for SetupView {
    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Tab {
                self.selected_field = next_cycle(&self.selected_field).expect("Never None");
            }

            match self.selected_field {
                SelectedField::BotToken => self.bot_token.input(key),
                SelectedField::UserId => self.user_id.input(key),
            };
        }
    }
}

impl Widget for &SetupView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        for cell in &mut buf.content {
            cell.set_bg(Color::White);

            match self.selected_field {
                SelectedField::BotToken => cell.set_bg(Color::White),
                SelectedField::UserId => cell.set_bg(Color::Black),
            };
        }

        self.user_id.widget().render(chunks[0], buf);
        self.bot_token.widget().render(chunks[1], buf);
    }
}
