use crossterm::event::{Event, KeyCode};
use parking_lot::Mutex;
use tui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::{
    app::{AppAction, AppContext},
    dickcord,
    interface::View,
};

pub struct SourceSelector {
    context: AppContext,
    selected_index: Mutex<usize>,
}

impl SourceSelector {
    pub fn new(context: AppContext) -> Self {
        Self {
            context,
            selected_index: Default::default(),
        }
    }

    pub fn navigate(&mut self, amount: isize) {
        let mut selected_index = self.selected_index.lock();
        let app_length = self.context.sources().len() as isize;

        let new_index = ((*selected_index) as isize + amount).rem_euclid(app_length);
        *selected_index = new_index as usize;
    }

    pub fn select(&self) {
        let selected_source = self.context.current_source();
        let selected_index = self.selected_index.lock();
        let sources = self.context.sources();

        if let Some(source) = sources.get(*selected_index) {
            let selected_app_index = selected_source
                .as_ref()
                .map(|a| a.index())
                .unwrap_or_default();

            // Stop the stream if pressing play on the same one
            if source.index() == selected_app_index {
                self.context.dispatch_action(AppAction::StopStream);
            } else {
                self.context
                    .dispatch_action(AppAction::SetAudioSource(source.to_owned()));
            }
        }
    }
}

impl View for SourceSelector {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title("─ Sources ")
            .border_style(Style::default().fg(Color::DarkGray))
            .borders(Borders::all());

        // Add margins
        let block_inner = {
            let area = block.inner(area);
            tui::layout::Rect::new(area.left() + 2, area.top() + 1, area.width - 2, area.height)
        };

        block.render(area, buf);

        let sources = self.context.sources();

        let selected_index = self.selected_index.lock();

        let selected_source = self.context.selected_source();
        let current_source = self.context.current_source();

        let discord_state = self.context.discord_state();
        let is_discord_ready = matches!(discord_state, dickcord::State::Connected(_, _));

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

    fn handle_event(&mut self, event: Event) {
        {
            let discord_state = self.context.discord_state();

            // Prevent selecting app before discord connects
            if !matches!(discord_state, dickcord::State::Connected(_, _)) {
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
