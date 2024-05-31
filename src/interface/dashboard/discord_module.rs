use tui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{
    app::AppContext,
    dickcord::{State, VoiceState},
    interface::View,
};

pub struct DiscordModule {
    context: AppContext,
}

impl DiscordModule {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }
}

impl View for DiscordModule {
    fn render(&self, area: Rect, buf: &mut Buffer) {
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

        let state = self.context.discord_state();

        if let State::Error(err) = &state {
            let paragraph = Paragraph::new(format!("⚠  An error occurred! {}", err))
                .style(Style::default().fg(Color::Red))
                .wrap(Wrap { trim: false });

            paragraph.render(block_inner, buf);
        }

        if let State::Connecting = &state {
            let paragraph = Paragraph::new("Logging in, please wait...")
                .style(Style::default().fg(Color::Yellow));

            paragraph.render(chunks[0], buf);
        }

        if let State::Connected(user, voice_state) = state {
            let paragraph =
                Paragraph::new(format!("● {}", user.name)).style(Style::default().fg(Color::Green));

            paragraph.render(chunks[0], buf);

            if let VoiceState::Idle = &voice_state {
                let paragraph =
                    Paragraph::new("└ Inactive").style(Style::default().fg(Color::DarkGray));

                paragraph.render(chunks[1], buf);
            }

            if let VoiceState::Joining(channel) = &voice_state {
                let paragraph = Paragraph::new(format!("└ Joining {}...", channel.name()))
                    .style(Style::default().fg(Color::Yellow));

                paragraph.render(chunks[1], buf);
            }

            if let VoiceState::Active(channel) = &voice_state {
                let paragraph = Paragraph::new(format!("└ 🔊\u{FE0E} {}", channel.name()))
                    .style(Style::default().fg(Color::Green));

                paragraph.render(chunks[1], buf);
            }
        }
    }
}
