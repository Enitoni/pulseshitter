use std::{
    sync::{Arc, Mutex},
    thread,
};

use crossbeam::channel::{unbounded, Receiver, Sender};
use dickcord::Discord;
use interface::{
    dashboard::{DashboardView, DashboardViewContext},
    run_ui,
    setup::SetupView,
    View,
};
use pulse::{Application, PulseAudio};
use state::Config;

use crate::audio::AudioSystem;

mod audio;
mod dickcord;
mod interface;
mod pulse;
mod state;

pub struct App {
    config: Arc<Mutex<Option<Config>>>,

    pulse: Arc<PulseAudio>,
    audio: Arc<AudioSystem>,
    discord: Discord,

    pub current_view: Mutex<View>,
    pub action_sender: Sender<Action>,
    pub action_receiver: Receiver<Action>,
}

impl App {
    fn new() -> Self {
        let discord = Discord::default();
        let pulse = Arc::new(PulseAudio::new());
        let audio = Arc::new(AudioSystem::new(pulse.clone()));

        let config = Config::restore();
        let (action_sender, action_receiver) = unbounded();

        AudioSystem::run(audio.clone());

        // Existing setup
        if let Some(config) = config {
            discord.connect(audio.stream(), config.clone(), action_sender.clone());

            let dashboard_context = DashboardViewContext {
                pulse: pulse.clone(),
                actions: action_sender.clone(),
                audio_status: audio.status.clone(),
                discord_status: discord.status.clone(),
                discord_user: discord.current_user.clone(),
                selected_app: audio.selected_app.clone(),
                latency: audio.latency.clone(),
                time: audio.time.clone(),
            };

            let dashboard_view = DashboardView::new(dashboard_context);

            return Self {
                audio,
                pulse,
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
            pulse,
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

                let dashboard_context = DashboardViewContext {
                    pulse: self.pulse.clone(),
                    actions: self.action_sender.clone(),
                    audio_status: self.audio.status.clone(),
                    discord_status: self.discord.status.clone(),
                    selected_app: self.audio.selected_app.clone(),
                    discord_user: self.discord.current_user.clone(),
                    latency: self.audio.latency.clone(),
                    time: self.audio.time.clone(),
                };

                let dashboard_view = DashboardView::new(dashboard_context);
                *view = View::Dashboard(dashboard_view);
            }
            Action::StopStream => self.audio.clear(),
            Action::SetApplication(app) => self.audio.set_application(app),
            Action::Exit => self.discord.disconnect(),
        };
    }
}

pub enum Action {
    SetConfig(Config),
    SetApplication(Application),
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
