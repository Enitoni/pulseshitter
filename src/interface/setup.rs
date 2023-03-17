use crossterm::event::{Event, KeyCode, KeyEvent};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::Color,
    widgets::{Block, Borders, Widget},
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
            if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                self.selected_field = next_cycle(&self.selected_field).expect("Never None");
                return;
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
        let block = Block::default().title("Setup").borders(Borders::all());
        let block_inner = block.inner(area);

        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(block_inner);

        self.user_id.widget().render(chunks[0], buf);
        self.bot_token.widget().render(chunks[1], buf);
    }
}
