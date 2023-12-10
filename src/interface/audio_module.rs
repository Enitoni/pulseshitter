use std::fmt::Write as _;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::audio::{AudioContext, AudioStatus};

use super::animation::{self, AnimatedSpan, Animation};

pub struct AudioModule {
    audio: AudioContext,
    animation: Animation,
}

impl AudioModule {
    pub fn new(audio: AudioContext) -> Self {
        Self {
            audio,
            animation: Default::default(),
        }
    }
}

impl AudioModule {
    fn render_idle(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let status_text = Paragraph::new("⏻ Offline").style(Style::default().fg(Color::DarkGray));

        let help_text = Paragraph::new(
            "Use the arrow keys to select an application to stream.

            Once you press enter, you should be able to hear the audio coming from your bot.
        ",
        )
        .wrap(Wrap { trim: true });

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(area);

        status_text.render(chunks[0], buf);
        help_text.render(chunks[1], buf);
    }

    fn render_connecting(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(area);

        let loading: AnimatedSpan = animation::Loading.into();

        let help_text = Paragraph::new(
            "Make sure the application is streaming audio if it doesn't connect immediately.
            ",
        )
        .wrap(Wrap { trim: true });

        help_text.render(chunks[1], buf);

        self.animation.render(
            1,
            vec![
                loading.clone(),
                (vec![" Connecting".to_string()], loading.1.clone()),
                (
                    vec![
                        "".to_string(),
                        ".".to_string(),
                        "..".to_string(),
                        "...".to_string(),
                    ],
                    loading.1,
                ),
            ],
            chunks[0],
            buf,
        );
    }

    fn render_searching(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(area);

        let loading: AnimatedSpan = animation::Loading.into();

        let help_text = Paragraph::new(
            "Streaming will resume once the source is available again.
            ",
        )
        .wrap(Wrap { trim: true });

        help_text.render(chunks[1], buf);

        self.animation.render(
            1,
            vec![
                loading.clone(),
                (vec![" Searching".to_string()], loading.1.clone()),
                (
                    vec![
                        "".to_string(),
                        ".".to_string(),
                        "..".to_string(),
                        "...".to_string(),
                    ],
                    loading.1,
                ),
            ],
            chunks[0],
            buf,
        );
    }

    fn render_error(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(area);

        let error_status: AnimatedSpan = animation::Error.into();

        let error_help = "pulseshitter was unable to connect to your application.
            
        Make sure it is open and playing audio, then try again.";

        let help_text = Paragraph::new(format!(
            "{}
            
            {}
            ",
            error_help,
            "If the problem persists, please create a GitHub issue with a reproducible example."
        ))
        .wrap(Wrap { trim: true });

        let animations = vec![
            error_status.clone(),
            (vec![format!("  {}", "Timed out")], error_status.1),
        ];

        self.animation.render(1, animations, chunks[0], buf);
        help_text.render(chunks[1], buf);
    }

    fn render_connected(&self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(area);

        let status_text = Paragraph::new("⏵ Streaming").style(Style::default().fg(Color::Green));
        status_text.render(chunks[0], buf);
    }
}

impl Widget for &AudioModule {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let block = Block::default()
            .border_style(Style::default().fg(Color::DarkGray))
            .title("─ Audio ")
            .borders(Borders::all());

        let block_inner = {
            let area = block.inner(area);
            tui::layout::Rect::new(
                area.left() + 2,
                area.top() + 1,
                area.width.saturating_sub(3),
                area.height.saturating_sub(1),
            )
        };

        block.render(area, buf);
        let status = self.audio.status();

        match status {
            x => self.render_idle(block_inner, buf),
            AudioStatus::Connecting => self.render_connecting(block_inner, buf),
            AudioStatus::Failed(err) => self.render_error(block_inner, buf),
            AudioStatus::Connected => self.render_connected(block_inner, buf),
        }
    }
}

fn format_seconds(seconds: f32) -> String {
    let mut result = String::new();

    let seconds = seconds.floor() as u32;
    let minutes = seconds / 60;
    let hours = minutes / 60;

    let mut minute_padding = 0;

    if hours >= 1 {
        let _ = write!(&mut result, "{:0}:", hours);
        minute_padding = 2;
    }

    let _ = write!(
        &mut result,
        "{:0mw$}:{:02}",
        minutes % 60,
        seconds % 60,
        mw = minute_padding
    );

    result
}
