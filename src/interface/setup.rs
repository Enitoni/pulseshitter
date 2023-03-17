use crossterm::event::{Event, KeyCode, KeyEvent};
use enum_iterator::{next_cycle, Sequence};
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Text,
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

pub struct SetupView {
    selected_field: SelectedField,

    bot_token: TextArea<'static>,
    user_id: TextArea<'static>,
}

impl SetupView {
    fn cycle_selection(&mut self) {
        deactivate(&mut self.bot_token);
        deactivate(&mut self.user_id);

        self.selected_field = next_cycle(&self.selected_field).expect("Never None");

        match self.selected_field {
            SelectedField::BotToken => activate(&mut self.bot_token),
            SelectedField::UserId => activate(&mut self.user_id),
        };
    }
}

impl Default for SetupView {
    fn default() -> Self {
        let bot_token = create_text_area();

        let mut user_id = create_text_area();
        deactivate(&mut user_id);

        Self {
            selected_field: Default::default(),

            bot_token,
            user_id,
        }
    }
}

fn create_text_area() -> TextArea<'static> {
    TextArea::new(vec!["".to_string()])
}

fn deactivate(area: &mut TextArea<'_>) {
    area.set_cursor_style(Style::reset());
    area.set_cursor_line_style(Style::reset());
}

fn activate(area: &mut TextArea<'_>) {
    area.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
    area.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
}

impl ViewController for SetupView {
    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            if key.code == KeyCode::Tab || key.code == KeyCode::Enter {
                self.cycle_selection();
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
            .margin(1)
            .horizontal_margin(2)
            .split(block_inner);

        self.bot_token.widget().render(chunks[0], buf);
        self.user_id.widget().render(chunks[1], buf);
    }
}
