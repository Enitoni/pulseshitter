use crate::{
    audio::{pulse::PulseClientError, AudioSystem, Source},
    dickcord::{self, DiscordSystem},
    interface::{Dashboard, Interface, Setup, Splash},
    state::{Config, ReadOnlyConfig},
};
use crossbeam::channel::{unbounded, Receiver, Sender};
use parking_lot::Mutex;
use reqwest::ClientBuilder;
use serde::Deserialize;
use std::{sync::Arc, thread};
use thiserror::Error;
use tokio::runtime::{Builder, Runtime};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/enitoni/pulseshitter/releases/latest";

pub struct App {
    rt: Arc<Runtime>,
    interface: Interface,
    audio: Arc<AudioSystem>,
    discord: Arc<DiscordSystem>,

    events: Sender<AppEvent>,
    state: Arc<Mutex<AppState>>,

    config: Arc<Mutex<Option<Config>>>,
    update_available: Arc<Mutex<Option<String>>>,
}

#[derive(Clone)]
pub struct AppContext {
    events: Sender<AppEvent>,
    audio: Arc<AudioSystem>,
    discord: Arc<DiscordSystem>,
    config: Arc<Mutex<Option<Config>>>,
    update_available: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone)]
enum AppState {
    Exiting,
    Idle,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    PulseClient(#[from] PulseClientError),
}

#[derive(Debug, Clone)]
pub enum AppAction {
    SetConfig(Config),
    SetAudioSource(Source),
    ToggleScreenshareOnly,
    ToggleMeter,
    StopStream,
    RedoSetup,
    Exit,
}

#[allow(clippy::large_enum_variant)]
pub enum AppEvent {
    DiscordStateUpdate(dickcord::State),
    Action(AppAction),
}

impl App {
    pub fn new() -> Result<Arc<Self>, AppError> {
        let rt: Arc<_> = Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .thread_name("pulseshitter-async")
            .build()
            .unwrap()
            .into();

        let (sender, receiver) = unbounded();

        let config = Config::restore();
        let audio = AudioSystem::new().map_err(AppError::PulseClient)?;
        let discord = DiscordSystem::new(rt.clone(), sender.clone(), audio.stream());
        let interface = Interface::new(Splash, sender.clone());

        let app = Arc::new(Self {
            rt,
            audio,
            discord,
            interface,
            events: sender,
            state: Arc::new(AppState::Idle.into()),
            config: Arc::new(Mutex::new(config)),
            update_available: Default::default(),
        });

        spawn_poll_thread(app.clone(), receiver);

        app.check_for_update();
        app.restore();
        Ok(app)
    }

    pub fn run_tui(&self) {
        let result = self.interface.run();

        if let Err(err) = result {
            eprintln!("Render error: {}", err)
        }
    }

    fn restore(&self) {
        let config = Config::restore();

        if let Some(config) = config {
            self.discord.connect(&config);
            self.interface.set_view(Dashboard::new(self.context()))
        } else {
            self.interface.set_view(Setup::new(self.context()))
        }
    }

    fn context(&self) -> AppContext {
        AppContext {
            events: self.events.clone(),
            audio: self.audio.clone(),
            discord: self.discord.clone(),
            config: self.config.clone(),
            update_available: self.update_available.clone(),
        }
    }

    fn exit(&self) {
        self.set_state(AppState::Exiting);

        if let dickcord::State::Connected(_, _) = self.discord.state() {
            self.discord.disconnect();
        } else {
            self.interface.stop();
        }
    }

    fn handle_event(&self, event: AppEvent) {
        match event {
            AppEvent::Action(action) => self.handle_action(action),
            AppEvent::DiscordStateUpdate(new_state) => self.handle_discord_state_update(new_state),
        }
    }

    fn handle_discord_state_update(&self, new_state: dickcord::State) {
        if let dickcord::State::Connected(_, _) = new_state {
            self.save_config();
            self.interface.set_view(Dashboard::new(self.context()))
        }

        if let dickcord::State::Idle = new_state {
            if let AppState::Exiting = self.state() {
                self.interface.stop();
            }
        }
    }

    fn handle_action(&self, action: AppAction) {
        match action {
            AppAction::SetConfig(config) => {
                self.discord.connect(&config);
                self.set_config(config);
            }
            AppAction::RedoSetup => {
                self.discord.disconnect();
                self.interface.set_view(Setup::new(self.context()));
            }
            AppAction::ToggleScreenshareOnly => {
                self.edit_config(|config| {
                    config.screen_share_only = !config.screen_share_only;
                });

                self.discord.set_config(self.read_only_config());
            }
            AppAction::ToggleMeter => {
                self.edit_config(|config| {
                    config.show_meter = !config.show_meter;
                });
            }
            AppAction::SetAudioSource(source) => {
                self.audio.select(Some(source.clone()));
                self.discord.announce_source_streaming(Some(source));
            }
            AppAction::StopStream => {
                self.audio.select(None);
                self.discord.announce_source_streaming(None);
            }
            AppAction::Exit => self.exit(),
        }
    }

    fn set_state(&self, state: AppState) {
        *self.state.lock() = state;
    }

    fn state(&self) -> AppState {
        self.state.lock().clone()
    }

    fn set_config(&self, config: Config) {
        let mut previous_config = self.config.lock();
        *previous_config = Some(config);
    }

    fn save_config(&self) {
        let config = self.config.lock();

        if let Some(config) = config.as_ref() {
            config.save();
        }
    }

    fn edit_config(&self, cb: impl FnOnce(&mut Config)) {
        let mut config = self.config.lock();

        if let Some(config) = config.as_mut() {
            cb(config);
            config.save();
        }
    }

    fn read_only_config(&self) -> ReadOnlyConfig {
        let config = self.config.lock();

        config
            .as_ref()
            .expect("Config is set when config() is called on AppContext")
            .read_only()
    }

    fn check_for_update(&self) {
        let update_available = self.update_available.clone();

        self.rt.spawn(async move {
            let new_version = fetch_update().await;

            if let Some(new_version) = new_version {
                *update_available.lock() = Some(new_version);
            }
        });
    }
}

fn spawn_poll_thread(app: Arc<App>, receiver: Receiver<AppEvent>) {
    let run = move || loop {
        if let Ok(event) = receiver.recv() {
            app.handle_event(event)
        }
    };

    thread::Builder::new()
        .name("pulseshitter-polling".to_string())
        .spawn(run)
        .unwrap();
}

impl AppContext {
    pub fn dispatch_action(&self, action: AppAction) {
        self.events.send(AppEvent::Action(action)).unwrap()
    }

    pub fn sources(&self) -> Vec<Source> {
        self.audio.sources()
    }

    pub fn current_source(&self) -> Option<Source> {
        self.audio.current_source()
    }

    pub fn selected_source(&self) -> Option<Source> {
        self.audio.selected_source()
    }

    pub fn meter_value_ranged(&self) -> (f32, f32) {
        self.audio.meter_value_ranged()
    }

    pub fn discord_state(&self) -> dickcord::State {
        self.discord.state()
    }

    pub fn config(&self) -> ReadOnlyConfig {
        let config = self.config.lock();

        config
            .as_ref()
            .expect("Config is set when config() is called on AppContext")
            .read_only()
    }

    pub fn update_available(&self) -> Option<String> {
        self.update_available.lock().clone()
    }
}

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
}

async fn fetch_update() -> Option<String> {
    let client = ClientBuilder::new()
        .user_agent("enitoni/pulseshitter")
        .build()
        .unwrap();

    let release: LatestRelease = client
        .get(LATEST_RELEASE_URL)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if release.tag_name != format!("v{}", VERSION) {
        return Some(release.tag_name);
    }

    None
}
