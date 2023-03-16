use crossterm::{
    event::{self, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
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
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Widget},
    Terminal,
};

mod setup;

use crate::state::State;

use self::setup::SetupView;

pub fn run_ui(state: Arc<State>) -> Result<(), io::Error> {
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);

    let mut terminal = Terminal::new(backend)?;
    let events = run_event_loop();

    loop {
        let mut view = state.current_view.lock().unwrap();
        let draw_result = terminal.draw(|rect| {});

        if let Err(err) = draw_result {
            eprintln!("Failed to draw: {:?}", err);
            break;
        };

        if let Ok(event) = events.try_recv() {
            view.handle_event(event);
        };
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
    Dashboard,
}

pub trait ViewController {
    fn handle_event(&mut self, event: Event);
}

impl ViewController for View {
    fn handle_event(&mut self, event: Event) {
        match self {
            Self::Setup(setup_view) => setup_view.handle_event(event),
            Self::Dashboard => todo!(),
        }
    }
}
