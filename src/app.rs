use crate::{
    audio::{pulse::PulseClientError, AudioSystem},
    dickcord::DiscordSystem,
    interface::{Interface, Splash},
    state::Config,
};
use std::sync::Arc;
use thiserror::Error;
use tokio::runtime::{Builder, Runtime};

pub struct App {
    rt: Arc<Runtime>,

    interface: Interface,
    audio: Arc<AudioSystem>,
    discord: Arc<DiscordSystem>,
}

#[derive(Clone)]
pub struct AppContext {
    audio: Arc<AudioSystem>,
    discord: Arc<DiscordSystem>,
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    PulseClient(#[from] PulseClientError),
    #[error("Unknown error")]
    Unknown,
}

impl App {
    pub fn new() -> Result<Self, AppError> {
        let rt: Arc<_> = Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .thread_name("pulseshitter-async")
            .build()
            .unwrap()
            .into();

        let audio = AudioSystem::new().map_err(AppError::PulseClient)?;
        let discord = DiscordSystem::new(rt.clone(), audio.stream());
        let interface = Interface::new(Splash);

        let app = Self {
            rt,
            audio,
            discord,
            interface,
        };
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
        }
    }

    fn context(&self) -> AppContext {
        AppContext {
            audio: self.audio.clone(),
            discord: self.discord.clone(),
        }
    }
}

impl AppContext {}
