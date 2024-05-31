use crossterm::event::Event;
use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
};
use tui_textarea::TextArea;

use crate::interface::View;

pub struct TextField {
    label: String,
    area: TextArea<'static>,
}

impl TextField {
    pub fn new(label: &str) -> Self {
        let label = label.to_string();
        let area = TextArea::new(vec!["".to_string()]);

        let mut result = Self { label, area };
        result.blur();

        result
    }

    pub fn focus(&mut self) {
        self.area
            .set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));

        self.area
            .set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
    }

    pub fn blur(&mut self) {
        self.area.set_cursor_style(Style::reset());
        self.area.set_cursor_line_style(Style::reset());
    }

    pub fn value(&self) -> String {
        self.area.lines()[0].to_string()
    }
}

impl View for TextField {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let text =
            Paragraph::new(self.label.clone()).style(Style::default().add_modifier(Modifier::BOLD));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        text.render(chunks[0], buf);
        self.area.widget().render(chunks[1], buf);
    }

    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            self.area.input(key);
        }
    }
}
