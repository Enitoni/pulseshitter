use crossterm::event::Event;
use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
};
use tui_textarea::TextArea;

use super::ViewController;

/// A text field that can be focused
pub struct Field {
    label: String,
    area: TextArea<'static>,
}

impl Field {
    pub fn new(label: &str) -> Self {
        let label = label.to_string();
        let area = TextArea::new(vec!["".to_string()]);

        Self { label, area }
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
}

impl ViewController for Field {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        if let Event::Key(key) = event {
            self.area.input(key);
        }
    }
}

impl Widget for &Field {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let text =
            Paragraph::new(self.label.clone()).style(Style::default().add_modifier(Modifier::BOLD));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area);

        text.render(chunks[0], buf);
        self.area.widget().render(chunks[1], buf);
    }
}
