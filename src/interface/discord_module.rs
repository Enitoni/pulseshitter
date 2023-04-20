use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::dickcord::{CurrentDiscordStatus, CurrentDiscordUser, DiscordStatus};

pub struct DiscordModule {
    user: CurrentDiscordUser,
    status: CurrentDiscordStatus,
}

impl DiscordModule {
    pub fn new(user: CurrentDiscordUser, status: CurrentDiscordStatus) -> Self {
        Self { user, status }
    }
}

impl Widget for &DiscordModule {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let block = Block::default().title("─ Discord ").borders(Borders::all());

        let block_inner = {
            let area = block.inner(area);
            tui::layout::Rect::new(
                area.left() + 2,
                area.top() + 1,
                area.width - 2,
                area.height - 1,
            )
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(block_inner);

        block.render(area, buf);

        let status = self.status.lock().unwrap();
        let user = self
            .user
            .lock()
            .unwrap()
            .as_ref()
            .map(|u| format!("{}#{}", u.name, u.discriminator))
            .unwrap_or_default();

        if let DiscordStatus::Failed(err) = &*status {
            let paragraph = Paragraph::new(format!("⚠  Oops! {}", err))
                .style(Style::default().fg(Color::Yellow));

            paragraph.render(chunks[0], buf);
        }

        if let DiscordStatus::Connecting = *status {
            let paragraph =
                Paragraph::new("⭮  Logging in...").style(Style::default().fg(Color::Yellow));

            paragraph.render(chunks[0], buf);
        }

        if let DiscordStatus::Connected | DiscordStatus::Active(_) | DiscordStatus::Joining(_) =
            &*status
        {
            let paragraph =
                Paragraph::new(format!("● {}", user)).style(Style::default().fg(Color::Green));

            paragraph.render(chunks[0], buf);
        }

        if let DiscordStatus::Joining(channel) = &*status {
            let paragraph = Paragraph::new(format!("└ Joining {}...", channel.name()))
                .style(Style::default().fg(Color::Yellow));

            paragraph.render(chunks[1], buf);
        }

        if let DiscordStatus::Active(channel) = &*status {
            let paragraph = Paragraph::new(format!("└ In {}", channel.name()))
                .style(Style::default().fg(Color::Green));

            paragraph.render(chunks[1], buf);
        }
    }
}
