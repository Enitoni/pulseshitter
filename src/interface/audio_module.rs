use tui::{
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::audio::{AudioStatus, CurrentAudioStatus};

pub struct AudioModule {
    status: CurrentAudioStatus,
}

impl AudioModule {
    pub fn new(status: CurrentAudioStatus) -> Self {
        Self { status }
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

        dbg!(&block_inner, &area);
        block.render(area, buf);

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
            AudioStatus::Connected(_) => "✔",
            AudioStatus::Failed(_) => "⚠",
            _ => "○",
        };

        let status_color = match &*status {
            AudioStatus::Connecting(_) | AudioStatus::Searching(_) => Color::Yellow,
            AudioStatus::Connected(_) => Color::Green,
            AudioStatus::Failed(_) => Color::Red,
            _ => Color::Reset,
        };

        let paragraph = Paragraph::new(format!("{} {}", status_symbol, status_text))
            .style(Style::default().fg(status_color));

        //paragraph.render(block_inner, buf);
    }
}
