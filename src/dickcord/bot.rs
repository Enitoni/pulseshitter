use super::TargetUser;
use crate::{audio::AudioStream, state::Config};
use crossbeam::channel::{unbounded, Receiver, Sender};
use serenity::{
    async_trait,
    client::{bridge::gateway::ShardManager, Context as SerenityContext, EventHandler},
    futures::future::{join_all, try_join_all},
    model::{
        channel::{ChannelType, GuildChannel},
        event::ResumedEvent,
        gateway::{Activity, GatewayIntents, Ready},
        guild::Member,
        user::CurrentUser,
        voice::VoiceState,
    },
    Client,
};
use songbird::{error::JoinError, Call, SerenityInit};
use std::sync::Arc;
use tokio::{runtime::Runtime, sync::Mutex};

/// The bot handling logic
pub struct Bot {
    rt: Arc<Runtime>,

    event_sender: Sender<BotEvent>,
    event_receiver: Receiver<BotEvent>,

    /// The user the bot should follow
    target_user: TargetUser,

    shard_manager: Arc<Mutex<ShardManager>>,
    context: Arc<Mutex<Option<SerenityContext>>>,

    connected_to_channel: Arc<Mutex<Option<GuildChannel>>>,
}

/// The event handler for the Serenity client
pub struct BotHandler {
    event_sender: Sender<BotEvent>,
    context: Arc<Mutex<Option<SerenityContext>>>,
}

#[derive(Clone)]
pub enum BotEvent {
    /// Bot has connected to Discord
    Connected(CurrentUser),
    /// Disconnected from Discord, either by an error or intentionally
    Reconnected,
    /// Joining a voice channel
    Joining(GuildChannel),
    /// Successfully joined a voice channel
    Joined(GuildChannel),
    /// Disconnected from voice channel
    Left,
    /// The user the bot is following connected or disconnected from a channel
    TargetUserMoved(Option<GuildChannel>),
    /// Something bad happened, duh.
    ClientError(String),
    /// An error occurred with the voice connection
    VoiceError(String),
}

impl Bot {
    pub fn new(rt: Arc<Runtime>, config: Config) -> Self {
        let (event_sender, event_receiver) = unbounded();

        let target_user = config.user_id;
        let context = Arc::new(Mutex::new(None));
        let event_handler = BotHandler::new(context.clone(), event_sender.clone());

        let mut client = rt.block_on(async move {
            Client::builder(&config.bot_token, intents())
                .register_songbird()
                .event_handler(event_handler)
                .await
                .expect("client is created")
        });

        let shard_manager = client.shard_manager.clone();
        rt.spawn(async move { client.start().await });

        Self {
            rt,
            context,
            target_user,
            event_sender,
            shard_manager,
            event_receiver,
            connected_to_channel: Default::default(),
        }
    }

    pub async fn set_streaming_status(&self, name: Option<String>) {
        let context = self.context().await;

        if let Some(name) = name {
            context
                .set_activity(Activity::streaming(
                    name,
                    "https://github.com/Enitoni/pulseshitter",
                ))
                .await;
        } else {
            context.reset_presence().await
        }
    }

    pub async fn attempt_join_and_stream(&self, audio: AudioStream) {
        let channel = self.locate_target_user_channel().await;

        let call = match channel {
            Some(c) => self.connect_to_channel(&c).await,
            None => None,
        };

        if let Some(call) = call {
            self.stream_call_audio(call, audio).await;
        }
    }

    pub fn poll(&self) -> BotEvent {
        self.event_receiver.recv().unwrap()
    }

    pub async fn connect_to_channel(&self, channel: &GuildChannel) -> Option<Arc<Mutex<Call>>> {
        let context = self.context().await;
        let manager = songbird::get(&context)
            .await
            .expect("get songbird instance");

        let (handler, result) = manager.join(channel.guild_id, channel.id).await;

        match result {
            Err(x) => {
                self.event_sender
                    .send(BotEvent::VoiceError(x.to_string()))
                    .unwrap();

                None
            }
            Ok(_) => {
                let _ = self
                    .connected_to_channel
                    .lock()
                    .await
                    .insert(channel.clone());

                Some(handler)
            }
        }
    }

    pub async fn disconnect_from_channel(&self) -> Result<(), JoinError> {
        let context = self.context().await;
        let manager = songbird::get(&context).await.unwrap();

        if let Some(channel) = self.connected_to_channel.lock().await.take() {
            return manager.remove(channel.guild_id).await;
        }

        Ok(())
    }

    pub async fn stream_call_audio(&self, call: Arc<Mutex<Call>>, audio: AudioStream) {
        call.lock().await.play_only_source(audio.into_input());
    }

    /// Finds the channel the target user is in, if any
    pub async fn locate_target_user_channel(&self) -> Option<GuildChannel> {
        let context = self.context().await;
        let members_in_channels = self.all_members_in_channels(&context).await;

        members_in_channels
            .into_iter()
            .find_map(|(members, guild_channel)| {
                members
                    .into_iter()
                    .find(|m| m.user.id == self.target_user)
                    .map(|_| guild_channel)
            })
    }

    async fn all_members_in_channels(
        &self,
        context: &SerenityContext,
    ) -> Vec<(Vec<Member>, GuildChannel)> {
        let channels = self.all_channels(context).await;

        let member_futures = channels
            .into_iter()
            .map(|c| async { (c.members(context).await, c) });

        join_all(member_futures)
            .await
            .into_iter()
            .flat_map(|(member, guild)| member.map(|m| (m, guild)))
            .collect()
    }

    async fn all_channels(&self, context: &SerenityContext) -> Vec<GuildChannel> {
        let guilds = context.cache.guilds();
        let channel_futures = guilds.into_iter().map(|g| g.channels(context));

        try_join_all(channel_futures)
            .await
            .unwrap_or_default()
            .into_iter()
            .flat_map(|h| h.into_values())
            .filter(|c| matches!(c.kind, ChannelType::Voice))
            .collect()
    }

    async fn context(&self) -> SerenityContext {
        self.context
            .lock()
            .await
            .clone()
            .expect("context() is not called before initialization")
    }

    async fn stop(&self) {
        let mut manager = self.shard_manager.lock().await;
        let context = self.context.lock().await;

        if let Some(context) = &*context {
            let _ = self.disconnect_from_channel().await;
            context.invisible().await
        }

        manager.shutdown_all().await
    }
}

#[async_trait]
impl EventHandler for BotHandler {
    async fn ready(&self, context: SerenityContext, ready: Ready) {
        *self.context.lock().await = Some(context.clone());

        self.event_sender
            .send(BotEvent::Connected(ready.user.clone()))
            .unwrap();
    }

    async fn resume(&self, _context: SerenityContext, _resumed: ResumedEvent) {
        self.event_sender.send(BotEvent::Reconnected).unwrap()
    }

    async fn voice_state_update(
        &self,
        context: SerenityContext,
        _: Option<VoiceState>,
        new: VoiceState,
    ) {
        todo!()
    }
}

impl BotHandler {
    fn new(context: Arc<Mutex<Option<SerenityContext>>>, event_sender: Sender<BotEvent>) -> Self {
        Self {
            context,
            event_sender,
        }
    }
}

impl Drop for Bot {
    fn drop(&mut self) {
        self.rt.block_on(self.stop());
    }
}

fn intents() -> GatewayIntents {
    GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
}
