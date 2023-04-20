use std::sync::Arc;

use crossbeam::channel::Sender;
use tui::{
    layout::{Constraint, Direction, Layout},
    widgets::Widget,
};

use crate::{
    audio::{CurrentAudioStatus, SelectedApp},
    pulse::PulseAudio,
    Action,
};

use super::{app_selector::AppSelector, audio_module::AudioModule, ViewController};

pub struct DashboardView {
    app_selector: AppSelector,
    audio_module: AudioModule,
}

pub struct DashboardViewContext {
    pub pulse: Arc<PulseAudio>,
    pub selected_app: SelectedApp,
    pub actions: Sender<Action>,
    pub audio_status: CurrentAudioStatus,
}

impl DashboardView {
    pub fn new(context: DashboardViewContext) -> Self {
        Self {
            app_selector: AppSelector::new(context.pulse, context.selected_app, context.actions),
            audio_module: AudioModule::new(context.audio_status),
        }
    }
}

impl Widget for &DashboardView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(area.width - 34), Constraint::Length(32)])
            .horizontal_margin(4)
            .split(area);

        let sidebar_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Percentage(100)])
            .margin(1)
            .split(chunks[1]);

        self.app_selector.render(chunks[0], buf);
        self.audio_module.render(chunks[1], buf);
    }
}

impl ViewController for DashboardView {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        self.app_selector.handle_event(event)
    }
}
