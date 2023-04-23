use std::fmt::Write as _;
use std::sync::Arc;

use tui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::{
    audio::pulse::PulseAudio,
    audio::{AudioError, AudioLatency, AudioStatus, AudioTime, CurrentAudioStatus},
};

use super::animation::{self, AnimatedSpan, Animation};

pub struct AudioModule {
    status: CurrentAudioStatus,
    latency: AudioLatency,
    time: AudioTime,
    pulse: Arc<PulseAudio>,
    animation: Animation,
}

impl AudioModule {
    pub fn new(
        status: CurrentAudioStatus,
        pulse: Arc<PulseAudio>,
        time: AudioTime,
        latency: AudioLatency,
    ) -> Self {
        Self {
            status,
            pulse,
            time,
            latency,
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

    fn render_error(
        &self,
        error: AudioError,
        area: tui::layout::Rect,
        buf: &mut tui::buffer::Buffer,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Percentage(100)])
            .split(area);

        let error_status: AnimatedSpan = animation::Error.into();

        let error_help = match error {
            AudioError::TimedOut => {
                "pulseshitter was unable to connect to your application.
            
            Make sure it is open and playing audio, then try again."
            }
            AudioError::ParecMissing => {
                "pulseshitter was unable to spawn the parec process for recording.
            
            Is pulseaudio or pipewire installed? Is parec in path?"
            }
        };

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
            (vec![format!("  {}", error)], error_status.1),
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

        let info_lines = vec![
            Spans::from(Span::styled("Device:", Style::default().fg(Color::Gray))),
            Spans::from(Span::raw(self.pulse.device_name())),
            Spans::default(),
            Spans::from(Span::styled(
                "Latency:         Time elapsed:",
                Style::default().fg(Color::Gray),
            )),
            Spans::from(Span::raw(format!(
                "{:.3}ms          {}",
                self.latency.load() as f32 / 1000.,
                format_seconds(self.time.load()),
            ))),
        ];

        let info_paragraph = Paragraph::new(info_lines).wrap(Wrap { trim: false });

        status_text.render(chunks[0], buf);
        info_paragraph.render(chunks[1], buf);
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

        let status = self.status.lock().unwrap();

        match &*status {
            AudioStatus::Idle => self.render_idle(block_inner, buf),
            AudioStatus::Connecting(_) => self.render_connecting(block_inner, buf),
            AudioStatus::Failed(err) => self.render_error(*err, block_inner, buf),
            AudioStatus::Connected(_) => self.render_connected(block_inner, buf),
            _ => {}
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
