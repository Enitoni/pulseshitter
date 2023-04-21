use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::Widget,
};

use crate::audio::AudioContext;

pub struct Meter {
    audio: AudioContext,
}

impl Meter {
    pub fn new(context: AudioContext) -> Self {
        Self { audio: context }
    }

    fn render_meter(&self, value: f32, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let bar_width = area.width as f32 * value;

        let amount_of_full_characters = bar_width.floor() as usize;

        let partial = bar_width - amount_of_full_characters as f32;
        let partial_index = (BAR_PARTIALS.len() - 1) as f32 * partial;
        let partial_symbol = BAR_PARTIALS[partial_index as usize];

        buf.set_string(
            area.x + amount_of_full_characters as u16,
            area.y,
            partial_symbol,
            Style::default(),
        );

        buf.set_string(
            area.x,
            area.y,
            BAR_PARTIALS[8].repeat(amount_of_full_characters),
            Style::default(),
        );

        buf.set_style(
            Rect::new(area.x, area.y, area.width, 1),
            Style::default().bg(Color::Black),
        );
    }
}

impl Widget for &Meter {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let (left, right) = self.audio.meter.value_ranged();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Length(1)])
            .split(area);

        self.render_meter(left, chunks[0], buf);
        self.render_meter(right, chunks[1], buf);
    }
}

const BAR_PARTIALS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
