use std::{
    io, thread,
    time::{Duration, Instant},
};

use crate::app::AppContext;

mod view;
pub use view::*;

mod splash;
pub use splash::*;

use crossbeam::channel::{unbounded, Receiver};
use crossterm::{
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parking_lot::Mutex;
use tui::{backend::CrosstermBackend, Terminal};

pub const TARGET_FPS: u32 = 144;
pub const MS_PER_FRAME: f32 = 1000. / TARGET_FPS as f32;

/// Handles rendering logic
pub struct Interface {
    context: AppContext,
    view: Mutex<BoxedView>,
}

impl Interface {
    pub fn new<V>(context: AppContext, view: V) -> Self
    where
        V: View + 'static,
    {
        Self {
            context,
            view: Mutex::new(BoxedView::new(view)),
        }
    }

    pub fn set_view<V>(&self, view: V)
    where
        V: View + 'static,
    {
        *self.view.lock() = BoxedView::new(view)
    }

    /// Renders the TUI until exit
    pub fn run(&self) -> Result<(), io::Error> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let events = spawn_event_loop();

        let mut terminal = Terminal::new(backend)?;

        'ui: loop {
            let frame_now = Instant::now();
            let mut view = self.view.lock();

            let result = terminal.draw(|f| f.render_widget(&*view, f.size()));

            if let Err(err) = result {
                eprintln!("Failed to draw: {:?}", err);
                break;
            };

            while let Ok(event) = events.try_recv() {
                if let Event::Key(key) = &event {
                    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
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
}

fn spawn_event_loop() -> Receiver<Event> {
    let (sender, receiver) = unbounded();

    let run = move || loop {
        match read() {
            Ok(event) => {
                if matches!(event, Event::Key(_)) {
                    sender.send(event).expect("Send");
                }
            }
            Err(err) => eprintln!("{:?}", err),
        };
    };

    thread::Builder::new()
        .name("tui-events".to_string())
        .spawn(run)
        .unwrap();

    receiver
}
