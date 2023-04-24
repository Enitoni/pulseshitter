use self::{dashboard::DashboardView, setup::SetupView};
use crate::{Action, App};
use crossterm::{
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    io,
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Paragraph, Widget, Wrap},
    Terminal,
};

mod animation;
mod app_selector;
mod audio_module;
mod discord_module;
mod field;
mod meter;

pub mod dashboard;
pub mod setup;

pub fn run_ui(app: Arc<App>) -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;
    let events = run_event_loop();

    'ui: loop {
        let frame_now = Instant::now();

        let mut view = app.current_view.lock().unwrap();

        let draw_result = terminal.draw(|f| {
            let area = f.size();

            let is_big_enough = area.width >= 70 && area.height >= 29;
            if !is_big_enough {
                let text = "Please resize your terminal window.";

                let centered_y = area.height / 2;
                let centered_area =
                    Rect::new(area.x, centered_y, area.width, area.height - centered_y);

                let notice = Paragraph::new(text)
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: false });

                f.render_widget(notice, centered_area);
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

            f.render_widget(logo, chunks[0]);
            f.render_widget(&*view, chunks[1]);
            f.render_widget(copyright, footer_chunks[0]);
            f.render_widget(version, footer_chunks[1]);
        });

        if let Err(err) = draw_result {
            eprintln!("Failed to draw: {:?}", err);
            break;
        };

        while let Ok(event) = events.try_recv() {
            if let Event::Key(key) = &event {
                if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
                    app.action_sender.send(Action::Exit).unwrap();
                    break 'ui;
                }
            }

            view.handle_event(event);
        }

        let elapsed = frame_now.elapsed().as_secs_f32();
        let remainder = (MS_PER_FRAME / 1000.) - elapsed;

        let sleep_duration = Duration::from_secs_f32(remainder.max(0.));

        drop(view);
        thread::sleep(sleep_duration);
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn run_event_loop() -> Receiver<Event> {
    let (sender, receiver) = mpsc::channel();

    thread::Builder::new()
        .name("ui-events".to_string())
        .spawn(move || loop {
            match read() {
                Ok(event) => {
                    if matches!(event, Event::Key(_)) {
                        sender.send(event).expect("Send");
                    }
                }
                Err(err) => eprintln!("{:?}", err),
            };
        })
        .unwrap();

    receiver
}

pub enum View {
    Setup(SetupView),
    Dashboard(DashboardView),
}

pub trait ViewController {
    fn handle_event(&mut self, event: Event);
}

impl ViewController for View {
    fn handle_event(&mut self, event: Event) {
        match self {
            Self::Setup(setup_view) => setup_view.handle_event(event),
            Self::Dashboard(dashboard_view) => dashboard_view.handle_event(event),
        }
    }
}

impl Widget for &View {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        match self {
            View::Setup(setup_view) => setup_view.render(area, buf),
            View::Dashboard(dashboard_view) => dashboard_view.render(area, buf),
        }
    }
}

const LOGO: &str = "
█▀█ █░█ █░░ █▀ █▀▀ █▀ █░█ █ ▀█▀ ▀█▀ █▀▀ █▀█
█▀▀ █▄█ █▄▄ ▄█ ██▄ ▄█ █▀█ █ ░█░ ░█░ ██▄ █▀▄
";

pub const TARGET_FPS: u32 = 90;
pub const MS_PER_FRAME: f32 = 1000. / TARGET_FPS as f32;
