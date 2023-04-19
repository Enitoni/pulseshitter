use crossterm::event::{Event, KeyCode, KeyEvent};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Widget},
};
use tui_textarea::TextArea;

use super::{field::Field, ViewController};

#[derive(Default, PartialEq, Sequence)]
enum SelectedField {
    #[default]
    BotToken,
    UserId,
}

pub struct SetupView {
    selected_field: SelectedField,

    bot_token: Field,
    user_id: Field,
}

impl SetupView {
    fn cycle_selection(&mut self) {
        self.selected_field = next_cycle(&self.selected_field).expect("Implements sequence");

        let (focus, blur) = match self.selected_field {
            SelectedField::BotToken => (&mut self.bot_token, &mut self.user_id),
            SelectedField::UserId => (&mut self.user_id, &mut self.bot_token),
        };

        focus.focus();
        blur.blur();
    }
}

impl Default for SetupView {
    fn default() -> Self {
        let bot_token = Field::new("Bot token");
        let user_id = Field::new("User id");

        Self {
            selected_field: Default::default(),

            bot_token,
            user_id,
        }
    }
}

impl ViewController for SetupView {
    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                self.cycle_selection();
                return;
            }

            match self.selected_field {
                SelectedField::BotToken => self.bot_token.handle_event(event),
                SelectedField::UserId => self.user_id.handle_event(event),
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
            .constraints([Constraint::Length(2), Constraint::Length(2)])
            .margin(1)
            .horizontal_margin(2)
            .split(block_inner);

        self.bot_token.render(chunks[0], buf);
        self.user_id.render(chunks[1], buf);
    }
}
