use crossbeam::atomic::AtomicCell;
use tui::{
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Paragraph, Widget},
};

use super::TARGET_FPS;

pub struct Animation {
    speed: usize,
    frame: AtomicCell<f32>,
    spans: Vec<AnimatedSpan<'static>>,
}

pub type AnimatedSpan<'a> = (Vec<&'a str>, Vec<Style>);

impl Animation {
    // Keyframes per second
    const TIME_FRAME: usize = 8;

    pub fn new<T: Into<AnimatedSpan<'static>>>(speed: usize, spans: Vec<T>) -> Self {
        let spans = spans.into_iter().map(|f| f.into()).collect();
        let speed = speed.max(1);

        dbg!(&spans);

        Self {
            speed,
            spans,
            frame: Default::default(),
        }
    }

    pub fn tick(&self) {
        let time_frame = Self::TIME_FRAME as f32;
        let increment = time_frame / TARGET_FPS as f32;

        let current_frame = self.frame.load();
        let new_frame = (current_frame + increment) % time_frame;

        self.frame.store(new_frame);
    }
}

impl Widget for &Animation {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let current_frame = self.frame.load();
        let frame_progress = current_frame / Animation::TIME_FRAME as f32;
        let speed = self.speed;

        let spans: Vec<_> = self
            .spans
            .iter()
            .map(|(symbols, styles)| {
                let symbols = symbols.repeat(speed);
                let styles = styles.repeat(speed);

                let current_symbol = (symbols.len() - 1) as f32 * frame_progress;
                let current_style = (styles.len() - 1) as f32 * frame_progress;

                Span::styled(
                    symbols[current_symbol as usize],
                    styles[current_style as usize],
                )
            })
            .collect();

        Paragraph::new(Spans::from(spans)).render(area, buf);

        self.tick()
    }
}

pub struct Loading;
impl<'a> From<Loading> for AnimatedSpan<'a> {
    fn from(_: Loading) -> AnimatedSpan<'a> {
        (
            vec!["🞔", "🞕", "🞓", "🞒", "🞑", "🞐", "🞏", "🞎"],
            vec![Style::default().fg(Color::Yellow)],
        )
    }
}

pub struct Error;
impl<'a> From<Error> for AnimatedSpan<'a> {
    fn from(_: Error) -> AnimatedSpan<'a> {
        (vec!["ⓧ", " "], vec![Style::default().fg(Color::Red)])
    }
}
