use std::sync::Arc;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{
    audio::{AudioLatency, AudioStatus, CurrentAudioStatus},
    pulse::PulseAudio,
};

pub struct AudioModule {
    status: CurrentAudioStatus,
    latency: AudioLatency,
    pulse: Arc<PulseAudio>,
}

impl AudioModule {
    pub fn new(status: CurrentAudioStatus, pulse: Arc<PulseAudio>, latency: AudioLatency) -> Self {
        Self {
            status,
            pulse,
            latency,
        }
    }
}

impl Widget for &AudioModule {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let block = Block::default()
            .border_style(Style::default().fg(Color::DarkGray))
            .title("─ Audio ")
            .borders(Borders::all());

        let block_inner = {
            let area = block.inner(area);
            tui::layout::Rect::new(
                area.left() + 2,
                area.top() + 1,
                area.width.saturating_sub(3),
                area.height.saturating_sub(1),
            )
        };

        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(block_inner);

        let status = self.status.lock().unwrap();

        if let AudioStatus::Failed(err) = &*status {
            let paragraph = Paragraph::new(format!("⚠  An error occured! {}", err))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false });

            paragraph.render(block_inner, buf);
            return;
        }

        let status_text = match &*status {
            AudioStatus::Idle => "Idle".to_string(),
            AudioStatus::Connecting(app) => format!("Connecting to {}...", app.name),
            AudioStatus::Connected(app) => format!("Streaming {}", app.name),
            AudioStatus::Searching(app) => format!("Reconnecting to {}...", app.name),
            _ => unreachable!(),
        };

        let status_symbol = match &*status {
            AudioStatus::Connecting(_) | AudioStatus::Searching(_) => "⭮",
            AudioStatus::Connected(_) => "►",
            _ => "○",
        };

        let status_color = match &*status {
            AudioStatus::Connecting(_) | AudioStatus::Searching(_) => Color::Yellow,
            AudioStatus::Connected(_) => Color::Green,
            _ => Color::Reset,
        };

        let status_paragraph = Paragraph::new(format!("{}  {}", status_symbol, status_text))
            .style(Style::default().fg(status_color));

        let info_lines = vec![
            Spans::from(Span::styled("Device:", Style::default().fg(Color::Gray))),
            Spans::from(Span::raw(self.pulse.device_name())),
            Spans::default(),
            Spans::from(Span::styled("Latency:", Style::default().fg(Color::Gray))),
            Spans::from(Span::raw(format!(
                "{:.4}ms",
                self.latency.load() as f32 / 1000.
            ))),
        ];

        let info_paragraph = Paragraph::new(info_lines).wrap(Wrap { trim: false });

        status_paragraph.render(chunks[0], buf);
        info_paragraph.render(chunks[1], buf);
    }
}
