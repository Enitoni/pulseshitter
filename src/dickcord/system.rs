use crate::{
    app::{AppContext, AppEvent},
    audio::{AudioStream, Source},
    state::Config,
};

use super::{Bot, BotEvent};
use crossbeam::channel::Sender;
use parking_lot::Mutex;
use serenity::model::{channel::GuildChannel, user::CurrentUser};
use std::{default, sync::Arc, thread, time::Duration};
use tokio::runtime::Runtime;

/// Manages all discord related things
pub struct DiscordSystem {
    rt: Arc<Runtime>,
    app_events: Sender<AppEvent>,

    bot: Mutex<Option<Arc<Bot>>>,
    state: Mutex<State>,

    stream: AudioStream,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    Idle,
    Connecting,
    Connected(CurrentUser, VoiceState),
    Error(String),
}

#[derive(Debug, Default, Clone)]
pub enum VoiceState {
    #[default]
    Idle,
    Joining(GuildChannel),
    Active(GuildChannel),
    Error(String),
}

impl DiscordSystem {
    pub fn new(rt: Arc<Runtime>, app_events: Sender<AppEvent>, stream: AudioStream) -> Arc<Self> {
        let system = Arc::new(Self {
            rt,
            stream,
            bot: Default::default(),
            state: Default::default(),
            app_events,
        });

        spawn_discord_event_thread(system.clone());
        system
    }

    pub fn connect(&self, config: &Config) {
        let bot = Bot::new(self.rt.clone(), config).into();

        *self.bot.lock() = Some(bot);
        self.set_state(State::Connecting);
    }

    pub fn disconnect(&self) {
        let bot = self.bot.lock().take();

        if let Some(bot) = bot {
            self.rt.block_on(async move { bot.stop().await });
        }

        self.set_state(State::Idle);
    }

    pub fn announce_source_streaming(&self, source: Option<Source>) {
        let bot = self.bot_unwrapped();
        let name = source.map(|s| s.name());

        self.rt
            .spawn(async move { bot.set_streaming_status(name).await });
    }

    pub fn state(&self) -> State {
        self.state.lock().clone()
    }

    fn stream_on_demand(&self) {
        let bot = self.bot_unwrapped();
        let audio = self.stream.clone();

        self.rt
            .spawn(async move { bot.attempt_join_and_stream(audio).await });
    }

    fn set_state(&self, new_state: State) {
        *self.state.lock() = new_state.clone();

        self.app_events
            .send(AppEvent::DiscordStateUpdate(new_state))
            .unwrap();
    }

    fn set_voice_state(&self, new_voice_state: VoiceState) {
        self.state.lock().set_voice_state(new_voice_state)
    }

    fn handle_event(&self, event: BotEvent) {
        match event {
            BotEvent::Connected(user) => self.handle_connected(user),
            BotEvent::Joined(channel) => self.handle_joined(channel),
            BotEvent::Joining(channel) => self.handle_joining(channel),
            BotEvent::Left => self.handle_left(),
            BotEvent::ClientError(error) => self.handle_client_error(error),
            BotEvent::VoiceError(error) => self.handle_voice_error(error),
            BotEvent::TargetUserMoved(new_channel) => self.handle_target_user_moved(new_channel),
            BotEvent::Reconnected => {}
        }
    }

    fn handle_connected(&self, user: CurrentUser) {
        self.set_state(State::Connected(user, VoiceState::Idle));
        self.stream_on_demand();
    }

    fn handle_client_error(&self, error: String) {
        self.set_state(State::Error(error))
    }

    fn handle_joining(&self, channel: GuildChannel) {
        self.set_voice_state(VoiceState::Joining(channel))
    }

    fn handle_joined(&self, channel: GuildChannel) {
        self.set_voice_state(VoiceState::Active(channel))
    }

    fn handle_left(&self) {
        self.set_voice_state(VoiceState::Idle)
    }

    fn handle_voice_error(&self, error: String) {
        self.set_voice_state(VoiceState::Error(error))
    }

    fn handle_target_user_moved(&self, new_channel: Option<GuildChannel>) {
        let new_voice_state = new_channel.map(VoiceState::Active).unwrap_or_default();

        self.set_voice_state(new_voice_state);
        self.stream_on_demand();
    }

    fn bot_unwrapped(&self) -> Arc<Bot> {
        self.bot
            .lock()
            .clone()
            .expect("bot_unwrapped() is not called when there is not a bot")
    }
}

impl State {
    fn set_voice_state(&mut self, new_state: VoiceState) {
        match self {
            Self::Connected(user, _) => *self = Self::Connected(user.clone(), new_state),
            _ => {
                eprintln!("set_voice_state() was called when not connected.")
            }
        }
    }

    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected(_, _))
    }
}

fn spawn_discord_event_thread(discord: Arc<DiscordSystem>) {
    let run = move || loop {
        let bot = discord.bot.lock().clone();
        let event = bot.map(|b| b.poll());

        if let Some(event) = event {
            discord.handle_event(event)
        } else {
            thread::sleep(Duration::from_millis(1))
        }
    };

    thread::Builder::new()
        .name("discord-events".to_string())
        .spawn(run)
        .unwrap();
}
