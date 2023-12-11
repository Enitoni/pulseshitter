use std::{
    sync::{Arc, Mutex},
    thread,
};

use audio::Source;
use crossbeam::channel::{unbounded, Receiver, Sender};
use dickcord_old::{Discord, DiscordContext};
use interface::{dashboard::DashboardView, run_ui, setup::SetupView, View};

use crate::audio::{AudioContext, AudioSystem};
use state::Config;

mod audio;
mod dickcord_old;
mod interface;
mod state;

mod app;
mod new_main;

pub struct App {
    config: Arc<Mutex<Option<Config>>>,

    audio: Arc<AudioSystem>,
    discord: Discord,

    pub current_view: Mutex<View>,
    pub action_sender: Sender<Action>,
    pub action_receiver: Receiver<Action>,
}

#[derive(Clone)]
pub struct AppContext {
    actions: Sender<Action>,

    pub discord: DiscordContext,
    pub audio: AudioContext,
}

impl AppContext {
    pub fn dispatch_action(&self, action: Action) {
        self.actions.send(action).expect("send action")
    }
}

impl App {
    fn new() -> Self {
        let discord = Discord::default();
        let audio = AudioSystem::new().unwrap_or_else(|_| todo!());

        let config = Config::restore();
        let (action_sender, action_receiver) = unbounded();

        let context = AppContext {
            actions: action_sender.clone(),
            discord: discord.context(),
            audio: audio.context(),
        };

        // Existing setup
        if let Some(config) = config {
            discord.connect(audio.stream(), config.clone(), action_sender.clone());

            let dashboard_view = DashboardView::new(context);

            return Self {
                audio,
                discord,
                action_sender,
                action_receiver,
                config: Mutex::new(Some(config)).into(),
                current_view: View::Dashboard(dashboard_view).into(),
            };
        }

        let setup_view = SetupView::new(action_sender.clone(), discord.status.clone());

        // New setup
        Self {
            current_view: View::Setup(setup_view).into(),
            config: Mutex::new(Config::restore()).into(),
            action_receiver,
            action_sender,
            discord,
            audio,
        }
    }

    pub fn handle_action(&self, action: Action) {
        match action {
            Action::SetConfig(new_config) => {
                let mut config = self.config.lock().unwrap();
                self.discord.connect(
                    self.audio.stream(),
                    new_config.clone(),
                    self.action_sender.clone(),
                );

                *config = Some(new_config);
            }
            Action::Activate => {
                let config = self.config.lock().unwrap();

                // We save because the config allowed a connection
                config
                    .as_ref()
                    .expect("Cannot activate without config")
                    .save();

                let mut view = self.current_view.lock().unwrap();

                let dashboard_view = DashboardView::new(self.context());
                *view = View::Dashboard(dashboard_view);
            }
            Action::StopStream => self.audio.select(None),
            Action::SetAudioSource(app) => self.audio.select(Some(app)),
            Action::Exit => self.discord.disconnect(),
        };
    }

    pub fn context(&self) -> AppContext {
        AppContext {
            actions: self.action_sender.clone(),
            discord: self.discord.context(),
            audio: self.audio.context(),
        }
    }
}

pub enum Action {
    SetConfig(Config),
    SetAudioSource(Source),
    StopStream,
    Activate,
    Exit,
}

fn main() {
    let app = Arc::new(App::new());

    thread::Builder::new()
        .name("action-polling".to_string())
        .spawn({
            let state = Arc::clone(&app);
            let receiver = state.action_receiver.clone();

            move || loop {
                if let Ok(action) = receiver.recv() {
                    state.handle_action(action)
                }
            }
        })
        .unwrap();

    run_ui(app).unwrap();
}
