use crate::{
    audio::{AudioStream, AudioSystem},
    dickcord::DiscordSystem,
    state::Config,
};
use std::sync::Arc;
use tokio::runtime::{Builder, Runtime};

pub struct App {
    rt: Arc<Runtime>,

    audio: Arc<AudioSystem>,
    discord: Arc<DiscordSystem>,
}

pub struct AppContext {
    audio: Arc<AudioSystem>,
    discord: Arc<DiscordSystem>,
}

impl App {
    // TODO: Fix this error being unit
    pub fn new() -> Result<Self, ()> {
        let rt: Arc<_> = Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .thread_name("pulseshitter-async")
            .build()
            .unwrap()
            .into();

        let audio = AudioSystem::new().map_err(|_| ())?;
        let discord = DiscordSystem::new(rt.clone(), audio.stream());

        let app = Self { rt, audio, discord };
        app.restore();

        Ok(app)
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
