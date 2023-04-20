use std::sync::Arc;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{
    audio::{AudioStatus, CurrentAudioStatus},
    pulse::PulseAudio,
};

pub struct AudioModule {
    status: CurrentAudioStatus,
    pulse: Arc<PulseAudio>,
}

impl AudioModule {
    pub fn new(status: CurrentAudioStatus, pulse: Arc<PulseAudio>) -> Self {
        Self { status, pulse }
    }
}

impl Widget for &AudioModule {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let block = Block::default().title("─ Audio ").borders(Borders::all());

        let block_inner = {
            let area = block.inner(area);
            tui::layout::Rect::new(
                area.left() + 2,
                area.top() + 1,
                area.width - 2,
                area.height - 1,
            )
        };

        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(block_inner);

        let status = self.status.lock().unwrap();

        let status_text = match &*status {
            AudioStatus::Idle => "Idle".to_string(),
            AudioStatus::Connecting(app) => format!("Connecting to {}...", app.name),
            AudioStatus::Connected(app) => format!("Streaming {}", app.name),
            AudioStatus::Searching(app) => format!("Reconnecting to {}...", app.name),
            AudioStatus::Failed(err) => format!("Failed to connect! {}", err),
        };

        let status_symbol = match &*status {
            AudioStatus::Connecting(_) | AudioStatus::Searching(_) => "⭮",
            AudioStatus::Connected(_) => "►",
            AudioStatus::Failed(_) => "⚠",
            _ => "○",
        };

        let status_color = match &*status {
            AudioStatus::Connecting(_) | AudioStatus::Searching(_) => Color::Yellow,
            AudioStatus::Connected(_) => Color::Green,
            AudioStatus::Failed(_) => Color::Red,
            _ => Color::Reset,
        };

        let status_paragraph = Paragraph::new(format!("{}  {}", status_symbol, status_text))
            .style(Style::default().fg(status_color));

        let info_paragraph = Paragraph::new(Spans::from(vec![
            Span::styled("Device\n", Style::default().fg(Color::Gray)),
            Span::raw(self.pulse.device_name()),
            Span::styled("\n\nLatency", Style::default().fg(Color::Gray)),
            Span::raw("0ms"),
        ]))
        .wrap(Wrap { trim: false });

        status_paragraph.render(chunks[0], buf);
        info_paragraph.render(chunks[1], buf);
    }
}
