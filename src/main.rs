use std::{
    io::Stdin,
    sync::{Arc, Mutex},
    thread,
};

use crossbeam::channel::{unbounded, Receiver, Sender};
use dickcord::Discord;
use interface::{run_ui, setup::SetupView, View};
use state::Config;

use crate::audio::AudioSystem;

mod audio;
mod dickcord;
mod interface;
mod pulse;
mod state;

pub struct App {
    config: Mutex<Option<Config>>,
    discord: Discord,

    pub current_view: Mutex<View>,
    pub action_sender: Sender<Action>,
    pub action_receiver: Receiver<Action>,
}

impl App {
    fn new() -> Self {
        let discord = Discord::default();
        let config = Config::restore();
        let (action_sender, action_receiver) = unbounded();

        // Existing setup
        if let Some(config) = config {
            return Self {
                discord,
                action_sender,
                action_receiver,
                config: Some(config).into(),
                current_view: View::Dashboard.into(),
            };
        }

        let setup_view = SetupView::new(action_sender.clone(), discord.status.clone());

        // New setup
        Self {
            current_view: View::Setup(setup_view).into(),
            config: Config::restore().into(),
            action_receiver,
            action_sender,
            discord,
        }
    }

    pub fn handle_action(&self, action: Action) {
        match action {
            Action::SetConfig(new_config) => {
                let mut config = self.config.lock().unwrap();
                self.discord.connect(new_config.clone());
                *config = Some(new_config);
            }
        };
    }
}

pub enum Action {
    SetConfig(Config),
}

fn main() {
    let app = Arc::new(App::new());

    thread::spawn({
        let state = Arc::clone(&app);
        let receiver = state.action_receiver.clone();

        move || loop {
            if let Ok(action) = receiver.recv() {
                state.handle_action(action)
            }
        }
    });

    run_ui(app).unwrap();
}

trait Prompt {
    fn prompt(&self, message: &str) -> String;
}

impl Prompt for Stdin {
    fn prompt(&self, message: &str) -> String {
        let mut result = String::new();
        println!("{}: ", message);

        self.read_line(&mut result).expect("Read line correctly");
        result
    }
}
