use tui::widgets::Widget;

use super::ViewController;

pub struct DashboardView;

impl Widget for &DashboardView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {}
}

impl ViewController for DashboardView {
    fn handle_event(&mut self, event: crossterm::event::Event) {}
}
