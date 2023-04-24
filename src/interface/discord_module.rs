use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::dickcord::{DiscordContext, DiscordStatus};

pub struct DiscordModule {
    discord: DiscordContext,
}

impl DiscordModule {
    pub fn new(discord: DiscordContext) -> Self {
        Self { discord }
    }
}

impl Widget for &DiscordModule {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let block = Block::default()
            .border_style(Style::default().fg(Color::DarkGray))
            .title("─ Discord ")
            .borders(Borders::all());

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

        let status = self.discord.current_status();
        let user = self
            .discord
            .current_user()
            .as_ref()
            .map(|u| format!("{}#{}", u.name, u.discriminator))
            .unwrap_or_default();

        if let DiscordStatus::Failed(err) = &status {
            let paragraph = Paragraph::new(format!("⚠  An error occurred! {}", err))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false });

            paragraph.render(block_inner, buf);
        }

        if let DiscordStatus::Connecting = status {
            let paragraph = Paragraph::new("Logging in, please wait...")
                .style(Style::default().fg(Color::Yellow));

            paragraph.render(chunks[0], buf);
        }

        if let DiscordStatus::Connected | DiscordStatus::Active(_) | DiscordStatus::Joining(_) =
            status
        {
            let paragraph =
                Paragraph::new(format!("● {}", user)).style(Style::default().fg(Color::Green));

            paragraph.render(chunks[0], buf);

            let paragraph =
                Paragraph::new("└ Inactive").style(Style::default().fg(Color::DarkGray));

            paragraph.render(chunks[1], buf);
        }

        if let DiscordStatus::Joining(channel) = &status {
            let paragraph = Paragraph::new(format!("└ Joining {}...", channel.name()))
                .style(Style::default().fg(Color::Yellow));

            paragraph.render(chunks[1], buf);
        }

        if let DiscordStatus::Active(channel) = status {
            let paragraph = Paragraph::new(format!("└ 🔊\u{FE0E} {}", channel.name()))
                .style(Style::default().fg(Color::Green));

            paragraph.render(chunks[1], buf);
        }
    }
}
