use std::{fs::File, io::Read, sync::Mutex};

use crossbeam::channel::{unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::interface::View;

pub struct State {
    config: Mutex<Option<Config>>,
    pub current_view: Mutex<View>,

    pub action_sender: Sender<Action>,
    pub action_receiver: Receiver<Action>,
}

impl State {
    pub fn new() -> Self {
        let config = Config::restore();
        let (action_sender, action_receiver) = unbounded();

        // Existing setup
        if let Some(config) = config {
            return Self {
                config: Some(config).into(),
                current_view: View::Dashboard.into(),

                action_sender,
                action_receiver,
            };
        }

        // New setup
        Self {
            config: Config::restore().into(),
            current_view: Default::default(),

            action_sender,
            action_receiver,
        }
    }

    pub fn handle_action(&self, action: Action) {
        match action {
            Action::SetConfig(new_config) => {
                let mut config = self.config.lock().unwrap();
                *config = Some(new_config)
            }
        };
    }
}

pub enum Action {
    SetConfig(Config),
}

impl Default for View {
    fn default() -> Self {
        Self::Setup(Default::default())
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub bot_token: String,
    pub user_id: u64,
}

impl Config {
    const FILE_NAME: &str = "config.ron";

    pub fn new(bot_token: String, user_id: u64) -> Self {
        Self { bot_token, user_id }
    }

    pub fn restore() -> Option<Self> {
        File::open(Self::FILE_NAME)
            .ok()
            .and_then(|mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents).map(|_| contents).ok()
            })
            .and_then(|content| ron::from_str(&content).ok())
    }

    pub fn save(&self) {
        if let Err(err) = ron::to_string(self) {
            eprintln!("Config save failed: {:?}", err)
        }
    }
}
