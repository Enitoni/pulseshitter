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
    time::Duration,
};
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::{Paragraph, Widget},
    Terminal,
};

mod app_selector;
mod audio_module;
mod discord_module;
mod field;

pub mod dashboard;
pub mod setup;

pub fn run_ui(app: Arc<App>) -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;
    let events = run_event_loop();

    loop {
        {
            let mut view = app.current_view.lock().unwrap();

            let draw_result = terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(4), Constraint::Percentage(100)])
                    .horizontal_margin(1)
                    .split(f.size());

                let logo = Paragraph::new(LOGO).alignment(Alignment::Center);

                f.render_widget(logo, chunks[0]);
                f.render_widget(&*view, chunks[1]);
            });

            if let Err(err) = draw_result {
                eprintln!("Failed to draw: {:?}", err);
                break;
            };

            if let Ok(event) = events.try_recv() {
                if let Event::Key(key) = &event {
                    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
                        app.action_sender.send(Action::Exit).unwrap();
                        break;
                    }
                }

                view.handle_event(event);
            };
        }

        thread::sleep(Duration::from_secs_f32(FPS / 1000.));
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

    thread::spawn(move || loop {
        match read() {
            Ok(event) => sender.send(event).expect("Send"),
            Err(err) => eprintln!("{:?}", err),
        };
    });

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

const FPS: f32 = 1000. / 144.;
