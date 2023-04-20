use std::sync::Arc;

use crossbeam::channel::Sender;
use tui::widgets::Widget;

use crate::{audio::SelectedApp, pulse::PulseAudio, Action};

use super::{app_selector::AppSelector, ViewController};

pub struct DashboardView {
    app_selector: AppSelector,
}

impl DashboardView {
    pub fn new(pulse: Arc<PulseAudio>, selected_app: SelectedApp, actions: Sender<Action>) -> Self {
        Self {
            app_selector: AppSelector::new(pulse, selected_app, actions),
        }
    }
}

impl Widget for &DashboardView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        self.app_selector.render(area, buf)
    }
}

impl ViewController for DashboardView {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        self.app_selector.handle_event(event)
    }
}
