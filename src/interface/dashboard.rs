


use tui::{
    layout::{Constraint, Direction, Layout},
    widgets::Widget,
};

use crate::{
    AppContext,
};

use super::{
    app_selector::AppSelector, audio_module::AudioModule, discord_module::DiscordModule,
    ViewController,
};

pub struct DashboardView {
    app_selector: AppSelector,
    audio_module: AudioModule,
    discord_module: DiscordModule,
}

impl DashboardView {
    pub fn new(context: AppContext) -> Self {
        Self {
            app_selector: AppSelector::new(context.clone()),
            audio_module: AudioModule::new(context.audio.clone()),
            discord_module: DiscordModule::new(context.discord),
        }
    }
}

impl Widget for &DashboardView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(area.width.saturating_sub(38)),
                Constraint::Length(38),
            ])
            .split(area);

        let sidebar_area = chunks[1];
        let sidebar_area = tui::layout::Rect::new(
            sidebar_area.x + 1,
            sidebar_area.y,
            sidebar_area.width.saturating_sub(1),
            sidebar_area.height,
        );

        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6),
                Constraint::Length(sidebar_area.height.saturating_sub(7)),
            ])
            .split(sidebar_area);

        self.app_selector.render(chunks[0], buf);
        self.audio_module.render(sidebar_chunks[1], buf);
        self.discord_module.render(sidebar_chunks[0], buf);
    }
}

impl ViewController for DashboardView {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        self.app_selector.handle_event(event)
    }
}
