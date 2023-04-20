use std::sync::{Arc, Mutex};

use crossbeam::channel::Sender;
use crossterm::event::{Event, KeyCode};
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
    selected_index: Mutex<usize>,
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

    pub fn navigate(&mut self, amount: isize) {
        let mut selected_index = self.selected_index.lock().unwrap();
        let app_length = self.pulse.applications().len() as isize;

        let new_index = ((*selected_index) as isize + amount).rem_euclid(app_length);
        *selected_index = new_index as usize;
    }
}

impl Widget for &AppSelector {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        self.pulse.update_applications();
        let apps = self.pulse.applications();

        let selected_index = self.selected_index.lock().unwrap();
        let top = area.top();

        for (index, app) in apps.iter().enumerate() {
            let is_over = *selected_index == index;

            let paragraph_area =
                tui::layout::Rect::new(area.left(), top + index as u16, area.width, 1);

            let symbol = if is_over { HOVER } else { IDLE };
            let paragraph = Paragraph::new(format!("{} {}", symbol, app.name.clone()));

            paragraph.render(paragraph_area, buf);
        }
    }
}

impl ViewController for AppSelector {
    fn handle_event(&mut self, event: crossterm::event::Event) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Up => self.navigate(-1),
                KeyCode::Down => self.navigate(1),
                _ => {}
            }
        }
    }
}

const IDLE: &str = "○";
const HOVER: &str = "●";
