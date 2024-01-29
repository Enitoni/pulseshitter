use tui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::app::AppContext;

use super::{View, LOGO};

pub struct Dashboard {
    context: AppContext,
}

impl Dashboard {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }
}

impl View for Dashboard {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let is_big_enough = area.width >= 70 && area.height >= 29;

        if !is_big_enough {
            let text = "Please resize your terminal window.";

            let centered_y = area.height / 2;
            let centered_area = Rect::new(area.x, centered_y, area.width, area.height - centered_y);

            let notice = Paragraph::new(text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });

            notice.render(centered_area, buf);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(area.height.saturating_sub(6)),
                Constraint::Length(2),
            ])
            .horizontal_margin(1)
            .split(area);

        let logo = Paragraph::new(LOGO).alignment(Alignment::Center);

        let footer_style = Style::default().fg(Color::DarkGray);
        let copyright = Paragraph::new("© 2023 Enitoni, Some rights reserved.")
            .alignment(Alignment::Left)
            .style(footer_style);

        let version = Paragraph::new(format!("v{}", env!("CARGO_PKG_VERSION")))
            .alignment(Alignment::Right)
            .style(footer_style);

        let footer_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .horizontal_margin(1)
            .split(chunks[2]);

        logo.render(chunks[0], buf);
        copyright.render(footer_chunks[0], buf);
        version.render(footer_chunks[1], buf);
    }
}
