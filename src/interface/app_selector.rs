use std::sync::{Arc, Mutex};

use crossbeam::channel::Sender;
use tui::widgets::{Paragraph, Widget};

use crate::{
    audio::SelectedApp,
    pulse::{Application, PulseAudio},
    Action,
};

use super::ViewController;

pub struct AppSelector {
    pulse: Arc<PulseAudio>,
    actions: Sender<Action>,

    selected_app: SelectedApp,
    selected_index: Mutex<u32>,
}

impl AppSelector {
    pub fn new(pulse: Arc<PulseAudio>, selected_app: SelectedApp, actions: Sender<Action>) -> Self {
        Self {
            pulse,
            actions,
            selected_app,
            selected_index: Default::default(),
        }
    }
}

impl Widget for &AppSelector {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        self.pulse.update_applications();
        let apps = self.pulse.applications();

        let top = area.top();

        for (index, app) in apps.iter().enumerate() {
            let paragraph_area =
                tui::layout::Rect::new(area.left(), top + index as u16, area.width, 1);

            let paragraph = Paragraph::new(app.name.clone());
            paragraph.render(paragraph_area, buf);
        }
    }
}

impl ViewController for AppSelector {
    fn handle_event(&mut self, event: crossterm::event::Event) {}
}
