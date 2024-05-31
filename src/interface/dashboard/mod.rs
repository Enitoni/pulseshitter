use tui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget, Wrap},
};

mod source_selector;
use source_selector::*;

mod meter;
use meter::*;

mod discord_module;
use discord_module::*;

use crate::app::AppContext;

use super::{View, LOGO};

pub struct Dashboard {
    content: Content,
}

pub struct Content {
    selector: SourceSelector,
    discord_module: DiscordModule,
    meter: Meter,
}

impl Dashboard {
    pub fn new(context: AppContext) -> Self {
        Self {
            content: Content {
                selector: SourceSelector::new(context.clone()),
                discord_module: DiscordModule::new(context.clone()),
                meter: Meter::new(context),
            },
        }
    }
}

impl View for Dashboard {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let is_big_enough = area.width >= 70 && area.height >= 29;

        if !is_big_enough {
            let text = "Please resize your terminal window.";

            let centered_y = area.height / 2;
            let centered_area = Rect::new(area.x, centered_y, area.width, area.height - centered_y);

            let notice = Paragraph::new(text)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: false });

            notice.render(centered_area, buf);
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Length(area.height.saturating_sub(6)),
                Constraint::Length(2),
            ])
            .horizontal_margin(1)
            .split(area);

        let logo = Paragraph::new(LOGO).alignment(Alignment::Center);

        let footer_style = Style::default().fg(Color::DarkGray);
        let copyright = Paragraph::new("© 2023 Enitoni, Some rights reserved.")
            .alignment(Alignment::Left)
            .style(footer_style);

        let version = Paragraph::new(format!("v{}", env!("CARGO_PKG_VERSION")))
            .alignment(Alignment::Right)
            .style(footer_style);

        let footer_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .horizontal_margin(1)
            .split(chunks[2]);

        logo.render(chunks[0], buf);

        self.content.render(chunks[1], buf);

        copyright.render(footer_chunks[0], buf);
        version.render(footer_chunks[1], buf);
    }

    fn handle_event(&mut self, event: crossterm::event::Event) {
        self.content.handle_event(event)
    }
}

impl View for Content {
    fn render(&self, area: Rect, buf: &mut Buffer) {
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

        self.selector.render(main_chunks[0], buf);
        //self.audio_module.render(sidebar_chunks[1], buf);
        self.discord_module.render(sidebar_chunks[0], buf);

        let mut meter_area = chunks[1];
        meter_area.x += 1;
        meter_area.y += 1;
        meter_area.width -= 1;

        self.meter.render(meter_area, buf);
    }

    fn handle_event(&mut self, event: crossterm::event::Event) {
        self.selector.handle_event(event)
    }
}
