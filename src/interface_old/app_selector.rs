use std::sync::Mutex;

use crossterm::event::{Event, KeyCode};
use tui::{
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::AppContext;
use crate::{dickcord_old::DiscordStatus, Action};

use super::ViewController;

pub struct AppSelector {
    context: AppContext,
    selected_index: Mutex<usize>,
}

impl AppSelector {
    pub fn new(context: AppContext) -> Self {
        Self {
            context,
            selected_index: Default::default(),
        }
    }

    pub fn navigate(&mut self, amount: isize) {
        let mut selected_index = self.selected_index.lock().unwrap();
        let app_length = self.context.audio.sources().len() as isize;

        let new_index = ((*selected_index) as isize + amount).rem_euclid(app_length);
        *selected_index = new_index as usize;
    }

    pub fn select(&self) {
        let selected_source = self.context.audio.current_source();
        let selected_index = self.selected_index.lock().unwrap();
        let sources = self.context.audio.sources();

        if let Some(source) = sources.get(*selected_index) {
            let selected_app_index = selected_source
                .as_ref()
                .map(|a| a.index())
                .unwrap_or_default();

            // Stop the stream if pressing play on the same one
            if source.index() == selected_app_index {
                self.context.dispatch_action(Action::StopStream);
            } else {
                self.context
                    .dispatch_action(Action::SetAudioSource(source.to_owned()));
            }
        }
    }
}

impl Widget for &AppSelector {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let audio = &self.context.audio;
        let discord = &self.context.discord;

        let block = Block::default()
            .title("─ Applications ")
            .border_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::all());

        // Add margins
        let block_inner = {
            let area = block.inner(area);
            tui::layout::Rect::new(area.left() + 2, area.top() + 1, area.width - 2, area.height)
        };

        block.render(area, buf);

        let sources = audio.sources();

        let selected_index = self.selected_index.lock().unwrap();

        let selected_source = audio.selected_source();
        let current_source = audio.current_source();

        let discord_status = discord.current_status();
        let is_discord_ready = matches!(discord_status, DiscordStatus::Active(_));

        let top = block_inner.top();

        for (index, source) in sources.iter().enumerate() {
            let is_over = *selected_index == index;

            let is_active = current_source
                .as_ref()
                .map(|f| f.index() == source.index())
                .unwrap_or_default();

            let is_selected = selected_source
                .as_ref()
                .map(|f| f.index() == source.index())
                .unwrap_or_default();

            let paragraph_area = tui::layout::Rect::new(
                block_inner.left(),
                top + index as u16,
                block_inner.width,
                1,
            );

            let symbol = if !is_discord_ready {
                IDLE_SYMBOL
            } else if is_over {
                HOVER_SYMBOL
            } else if is_selected {
                ACTIVE_SYMBOL
            } else {
                IDLE_SYMBOL
            };

            let color = if !is_discord_ready {
                DISABLE_COLOR
            } else if is_active {
                ACTIVE_COLOR
            } else if source.available() {
                IDLE_COLOR
            } else {
                UNAVAILABLE_COLOR
            };

            let paragraph = Paragraph::new(format!("{} {}", symbol, source.name()))
                .style(Style::default().fg(color));

            paragraph.render(paragraph_area, buf);
        }
    }
}

impl ViewController for AppSelector {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        {
            let discord_status = self.context.discord.current_status();

            // Prevent selecting app before discord connects
            if !matches!(discord_status, DiscordStatus::Active(_)) {
                return;
            }
        }

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up => self.navigate(-1),
                KeyCode::Down => self.navigate(1),
                KeyCode::Enter => self.select(),
                _ => {}
            }
        }
    }
}

const IDLE_SYMBOL: &str = "○";
const HOVER_SYMBOL: &str = "●";
const ACTIVE_SYMBOL: &str = "►";

const ACTIVE_COLOR: Color = Color::Green;
const IDLE_COLOR: Color = Color::Reset;
const DISABLE_COLOR: Color = Color::DarkGray;
const UNAVAILABLE_COLOR: Color = Color::Yellow;
