use crossterm::event::{Event, KeyCode};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
};

use crate::interface::{TextField, View};

#[derive(Default, PartialEq, Sequence)]
pub enum SelectedField {
    #[default]
    BotToken,
    UserId,
}

pub struct Form {
    selected_field: SelectedField,
    bot_token: TextField,
    user_id: TextField,
}

impl Form {
    pub fn new() -> Self {
        let mut bot_token = TextField::new("Bot Token");
        let user_id = TextField::new("User ID");

        bot_token.focus();

        Self {
            selected_field: Default::default(),
            bot_token,
            user_id,
        }
    }

    fn cycle_selection(&mut self) {
        self.selected_field = next_cycle(&self.selected_field).expect("Implements sequence");

        let (focus, blur) = match self.selected_field {
            SelectedField::BotToken => (&mut self.bot_token, &mut self.user_id),
            SelectedField::UserId => (&mut self.user_id, &mut self.bot_token),
        };

        focus.focus();
        blur.blur();
    }

    pub fn current_selection(&self) -> &SelectedField {
        &self.selected_field
    }

    pub fn current_values(&self) -> (String, String) {
        let bot_token = self.bot_token.value();
        let user_id = self.user_id.value();

        (bot_token, user_id)
    }
}

impl View for Form {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(4)])
            .split(area);

        self.bot_token.render(chunks[0], buf);
        self.user_id.render(chunks[1], buf);
    }

    fn handle_event(&mut self, event: crossterm::event::Event) {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                self.cycle_selection();
                return;
            }
        }

        match self.selected_field {
            SelectedField::BotToken => self.bot_token.handle_event(event),
            SelectedField::UserId => self.user_id.handle_event(event),
        };
    }
}
