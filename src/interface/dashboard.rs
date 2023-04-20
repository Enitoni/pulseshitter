use std::sync::Arc;

use crossbeam::channel::Sender;
use tui::{
    layout::{Constraint, Direction, Layout},
    widgets::Widget,
};

use crate::{
    audio::{CurrentAudioStatus, SelectedApp},
    dickcord::{CurrentDiscordStatus, CurrentDiscordUser},
    pulse::PulseAudio,
    Action,
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

pub struct DashboardViewContext {
    pub pulse: Arc<PulseAudio>,
    pub selected_app: SelectedApp,
    pub actions: Sender<Action>,
    pub audio_status: CurrentAudioStatus,
    pub discord_status: CurrentDiscordStatus,
    pub discord_user: CurrentDiscordUser,
}

impl DashboardView {
    pub fn new(context: DashboardViewContext) -> Self {
        Self {
            app_selector: AppSelector::new(
                context.pulse.clone(),
                context.discord_status.clone(),
                context.selected_app,
                context.actions,
            ),
            audio_module: AudioModule::new(context.audio_status, context.pulse),
            discord_module: DiscordModule::new(context.discord_user, context.discord_status),
        }
    }
}

impl Widget for &DashboardView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(area.width - 38), Constraint::Length(38)])
            .split(area);

        let sidebar_area = chunks[1];
        let sidebar_area = tui::layout::Rect::new(
            sidebar_area.x + 1,
            sidebar_area.y,
            sidebar_area.width - 1,
            sidebar_area.height,
        );

        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Percentage(100)])
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
