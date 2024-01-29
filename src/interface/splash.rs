use tui::{
    layout::Alignment,
    widgets::{Paragraph, Widget},
};

use super::{View, LOGO};
pub struct Splash;

impl View for Splash {
    fn render(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let logo = Paragraph::new(LOGO).alignment(Alignment::Center);
        logo.render(area, buf);
    }
}
