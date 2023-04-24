use crossbeam::atomic::AtomicCell;
use tui::{
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Paragraph, Widget},
};

use super::TARGET_FPS;

#[derive(Default)]
pub struct Animation {
    frame: AtomicCell<f32>,
}

pub type AnimatedSpan<T = String> = (Vec<T>, Vec<Style>);

// TODO: Clean this garbage up
impl Animation {
    // Keyframes per second
    const TIME_FRAME: usize = 8;

    pub fn tick(&self) {
        let time_frame = Self::TIME_FRAME as f32;
        let increment = time_frame / TARGET_FPS as f32;

        let current_frame = self.frame.load();
        let new_frame = (current_frame + increment) % time_frame;

        self.frame.store(new_frame);
    }

    pub fn render<S, T>(
        &self,
        speed: usize,
        spans: Vec<T>,
        area: tui::layout::Rect,
        buf: &mut tui::buffer::Buffer,
    ) where
        S: Into<String>,
        T: Into<AnimatedSpan<S>>,
    {
        let speed = speed.max(1);

        let current_frame = self.frame.load();
        let frame_progress = current_frame / Animation::TIME_FRAME as f32;
        let speed = speed;

        let spans: Vec<_> = spans
            .into_iter()
            .map(|x| x.into())
            .map(|(symbols, styles)| {
                (
                    symbols.into_iter().map(S::into).collect::<Vec<String>>(),
                    styles,
                )
            })
            .map(|(symbols, styles)| {
                let symbols = symbols.iter().collect::<Vec<_>>().repeat(speed);
                let styles = styles.repeat(speed);

                let current_symbol = symbols.len() as f32 * frame_progress;
                let current_style = styles.len() as f32 * frame_progress;

                Span::styled(
                    symbols[current_symbol as usize].to_owned(),
                    styles[current_style as usize],
                )
            })
            .collect();

        Paragraph::new(Spans::from(spans)).render(area, buf);
        self.tick();
    }
}

pub struct Loading;
impl From<Loading> for AnimatedSpan {
    fn from(_: Loading) -> AnimatedSpan {
        (
            vec!["🞔", "🞕", "🞓", "🞒", "🞑", "🞐", "🞏", "🞎"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            vec![Style::default().fg(Color::Yellow)],
        )
    }
}

pub struct Error;
impl From<Error> for AnimatedSpan {
    fn from(_: Error) -> AnimatedSpan {
        (
            vec!["⚠"].into_iter().map(str::to_string).collect(),
            vec![
                Style::default().fg(Color::Red),
                Style::default().fg(Color::DarkGray),
            ],
        )
    }
}
