use tui::{
    layout::{Constraint, Direction, Layout},
    widgets::Widget,
};

use crate::AppContext;

use super::{
    app_selector::AppSelector, audio_module::AudioModule, discord_module::DiscordModule,
    meter::Meter, ViewController,
};

pub struct DashboardView {
    app_selector: AppSelector,
    audio_module: AudioModule,
    discord_module: DiscordModule,
    meter: Meter,
}

impl DashboardView {
    pub fn new(context: AppContext) -> Self {
        Self {
            meter: Meter::new(context.audio.clone()),
            app_selector: AppSelector::new(context.clone()),
            audio_module: AudioModule::new(context.audio.clone()),
            discord_module: DiscordModule::new(context.discord),
        }
    }
}

impl Widget for &DashboardView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(area.height.saturating_sub(5)),
                Constraint::Length(4),
            ])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(area.width.saturating_sub(38)),
                Constraint::Length(38),
            ])
            .split(chunks[0]);

        let sidebar_area = main_chunks[1];
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

        self.app_selector.render(main_chunks[0], buf);
        self.audio_module.render(sidebar_chunks[1], buf);
        self.discord_module.render(sidebar_chunks[0], buf);

        let mut meter_area = chunks[1];
        meter_area.x += 1;
        meter_area.y += 1;
        meter_area.width -= 1;

        self.meter.render(meter_area, buf);
    }
}

impl ViewController for DashboardView {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        self.app_selector.handle_event(event)
    }
}
