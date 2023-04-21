use std::sync::{Arc, Mutex};

use crossbeam::channel::Sender;
use crossterm::event::{Event, KeyCode};
use tui::{
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    audio::SelectedApp,
    dickcord::{CurrentDiscordStatus, DiscordStatus},
    pulse::PulseAudio,
    Action,
};

use super::ViewController;

pub struct AppSelector {
    pulse: Arc<PulseAudio>,
    discord_status: CurrentDiscordStatus,
    actions: Sender<Action>,

    selected_app: SelectedApp,
    selected_index: Mutex<usize>,
}

impl AppSelector {
    pub fn new(
        pulse: Arc<PulseAudio>,
        discord_status: CurrentDiscordStatus,
        selected_app: SelectedApp,
        actions: Sender<Action>,
    ) -> Self {
        Self {
            pulse,
            actions,
            selected_app,
            discord_status,
            selected_index: Default::default(),
        }
    }

    pub fn navigate(&mut self, amount: isize) {
        let mut selected_index = self.selected_index.lock().unwrap();
        let app_length = self.pulse.applications().len() as isize;

        let new_index = ((*selected_index) as isize + amount).rem_euclid(app_length);
        *selected_index = new_index as usize;
    }

    pub fn select(&self) {
        let selected_index = self.selected_index.lock().unwrap();

        if let Some(app) = self.pulse.applications().get(*selected_index) {
            self.actions
                .send(Action::SetApplication(app.to_owned()))
                .unwrap();
        }
    }
}

impl Widget for &AppSelector {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let apps = self.pulse.applications();

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

        let selected_index = self.selected_index.lock().unwrap();
        let selected_app = self.selected_app.lock().unwrap();

        let discord_status = self.discord_status.lock().unwrap();
        let is_discord_ready = matches!(*discord_status, DiscordStatus::Active(_));

        let top = block_inner.top();

        for (index, app) in apps.iter().enumerate() {
            let is_over = *selected_index == index;

            let is_active = selected_app
                .as_ref()
                .map(|f| f.sink_input_index == app.sink_input_index)
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
            } else if is_active {
                ACTIVE_SYMBOL
            } else {
                IDLE_SYMBOL
            };

            let color = if !is_discord_ready {
                DISABLE_COLOR
            } else if is_active {
                ACTIVE_COLOR
            } else {
                IDLE_COLOR
            };

            let paragraph = Paragraph::new(format!("{} {}", symbol, app.name.clone()))
                .style(Style::default().fg(color));

            paragraph.render(paragraph_area, buf);
        }
    }
}

impl ViewController for AppSelector {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        {
            let discord_status = self.discord_status.lock().unwrap();

            // Prevent selecting app before discord connects
            if !matches!(*discord_status, DiscordStatus::Active(_)) {
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
