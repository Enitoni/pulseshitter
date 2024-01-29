use crossterm::event::Event;
use tui::{buffer::Buffer, layout::Rect, widgets::Widget};

pub trait View {
    fn handle_event(&mut self, _event: Event) {}

    fn render(&self, area: Rect, buf: &mut Buffer);
}

pub struct BoxedView {
    view: Box<dyn View>,
}

impl BoxedView {
    pub fn new<V>(view: V) -> Self
    where
        V: View + 'static,
    {
        Self {
            view: Box::new(view),
        }
    }
}

impl Widget for &BoxedView {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        self.view.render(area, buf)
    }
}

impl View for BoxedView {
    fn render(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        self.view.render(area, buf)
    }

    fn handle_event(&mut self, event: Event) {
        self.view.handle_event(event)
    }
}
