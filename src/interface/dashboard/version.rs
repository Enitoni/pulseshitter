use tui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget},
};

use crate::{
    app::{AppContext, VERSION},
    interface::View,
};

pub struct Version {
    context: AppContext,
}

impl Version {
    pub fn new(context: AppContext) -> Self {
        Self { context }
    }
}

impl View for Version {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let new_update = self.context.update_available();

        if let Some(new_update) = new_update {
            Paragraph::new(format!("Update available! v{} > {}", VERSION, new_update))
                .alignment(Alignment::Right)
                .style(Style::default().fg(Color::Green))
                .render(area, buf);

            return;
        }

        Paragraph::new(format!("v{}", VERSION))
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::DarkGray))
            .render(area, buf);
    }
}
