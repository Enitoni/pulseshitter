use std::fmt::Display;
use std::sync::{Arc, Mutex};

use crate::audio::AudioStream;
use crate::state::Config;
use crate::Action;
use crossbeam::channel::Sender;
use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::futures::future::{join_all, try_join_all};
use serenity::model::gateway::Ready;
use serenity::model::prelude::{ChannelType, GuildChannel, GuildId};
use serenity::model::user::CurrentUser;
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use songbird::error::JoinError;
use songbird::{Call, SerenityInit};
use tokio::runtime::{Builder, Runtime};

/// Main Discord connection state managing
#[derive(Default)]
pub struct Discord {
    client: Arc<Mutex<Option<DroppableClient>>>,
    context: DynamicContext,

    pub current_user: CurrentDiscordUser,
    pub status: CurrentDiscordStatus,
}

#[derive(Clone)]
pub struct DiscordContext {
    current_user: CurrentDiscordUser,
    current_status: CurrentDiscordStatus,
}

impl DiscordContext {
    pub fn current_user(&self) -> Option<CurrentUser> {
        self.current_user.lock().unwrap().clone()
    }

    pub fn current_status(&self) -> DiscordStatus {
        self.current_status.lock().unwrap().clone()
    }
}

pub type CurrentDiscordUser = Arc<Mutex<Option<CurrentUser>>>;
pub type CurrentDiscordStatus = Arc<Mutex<DiscordStatus>>;

impl Discord {
    pub fn connect(&self, audio_stream: AudioStream, config: Config, actions: Sender<Action>) {
        let mut client = self.client.lock().unwrap();

        // Kill the current connection
        if client.is_some() {
            *client = None;
        }

        *(self.status.lock().unwrap()) = DiscordStatus::Connecting;
        *client = DroppableClient::new(
            audio_stream,
            self.status.clone(),
            self.current_user.clone(),
            actions,
            self.context.clone(),
            config,
        )
        .into();
    }

    pub fn disconnect(&self) {
        let mut client = self.client.lock().unwrap();

        if let Some(client) = &*client {
            client.rt.block_on(async { client.stop().await });
        }

        *client = None;
        *self.status.lock().unwrap() = DiscordStatus::Idle;
    }

    pub fn context(&self) -> DiscordContext {
        DiscordContext {
            current_user: self.current_user.clone(),
            current_status: self.status.clone(),
        }
    }
}

#[derive(Default, Clone)]
pub enum DiscordStatus {
    #[default]
    Idle,
    Connecting,
    Connected,
    Joining(GuildChannel),
    Active(GuildChannel),
    Failed(DiscordError),
}

#[derive(Debug, Clone)]
pub enum DiscordError {
    Serenity(Arc<SerenityError>),
    Songbird(Arc<JoinError>),
}

impl Display for DiscordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscordError::Serenity(e) => e.fmt(f),
            DiscordError::Songbird(e) => e.fmt(f),
        }
    }
}

type DynamicContext = Arc<Mutex<Option<Context>>>;

struct Bot {
    user_id: u64,
    status: CurrentDiscordStatus,
    user: CurrentDiscordUser,
    actions: Sender<Action>,
    audio_stream: AudioStream,
    context: DynamicContext,
}

impl Bot {
    const MAX_RETRIES: usize = 5;

    async fn connect_and_stream(&self, context: Context, channel: GuildChannel) {
        let mut retries = 0;

        loop {
            let result = self.connect(&context, &channel).await;

            match result {
                Ok(call) => {
                    call.lock()
                        .await
                        .play_only_source(self.audio_stream.clone().into_input());

                    *self.status.lock().unwrap() = DiscordStatus::Active(channel.clone());
                    break;
                }
                Err(why) => {
                    *self.status.lock().unwrap() =
                        DiscordStatus::Failed(DiscordError::Songbird(why.into()));

                    if retries >= Self::MAX_RETRIES {
                        break;
                    }

                    retries += 1;
                }
            }
        }
    }

    async fn connect(
        &self,
        context: &Context,
        channel: &GuildChannel,
    ) -> Result<Arc<tokio::sync::Mutex<Call>>, JoinError> {
        *self.status.lock().unwrap() = DiscordStatus::Joining(channel.clone());

        let manager = songbird::get(context).await.unwrap();
        let (handler, result) = manager.join(channel.guild_id, channel.id).await;

        result.map(|_| handler)
    }

    async fn disconnect(&self, context: &Context, guild: &GuildId) {
        *self.status.lock().unwrap() = DiscordStatus::Connected;

        let manager = songbird::get(context).await.unwrap();
        let _ = manager.remove(*guild).await;
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, context: Context, ready: Ready) {
        context.online().await;

        *(self.status.lock().unwrap()) = DiscordStatus::Connected;
        *(self.context.lock().unwrap()) = Some(context.clone());
        *(self.user.lock().unwrap()) = Some(ready.user.clone());

        self.actions.send(Action::Activate).unwrap();
        let guilds = context.cache.guilds();

        if let Some(channel) = find_voice_channel(&context, self.user_id, guilds.clone()).await {
            self.connect_and_stream(context, channel).await;
        }
    }

    async fn voice_state_update(&self, context: Context, _: Option<VoiceState>, new: VoiceState) {
        let in_old_channel = match &*self.status.lock().unwrap() {
            DiscordStatus::Active(channel) => Some(channel.to_owned()),
            _ => None,
        }
        .map(|c| (c.id, c.guild_id));

        let in_new_channel = new.channel_id.zip(new.guild_id);

        // This user is not relevant
        if let Some(member) = new.member {
            if member.user.id != self.user_id {
                return;
            }
        }

        // Target left a channel
        if let (Some((_, guild)), None) = &(in_old_channel, in_new_channel) {
            self.disconnect(&context, guild).await
        }

        let channel_to_join = in_new_channel
            .and_then(|(new, _)| match in_old_channel {
                Some((old, _)) if old != new => Some(new),
                None => Some(new),
                _ => None,
            })
            .and_then(|c| context.cache.guild_channel(c));

        if let Some(channel) = channel_to_join {
            self.connect_and_stream(context, channel).await;
        }
    }
}

async fn find_voice_channel(
    context: &Context,
    user_id: u64,
    guilds: Vec<GuildId>,
) -> Option<GuildChannel> {
    let channel_futures = guilds.iter().map(|g| g.channels(context));
    let channels: Vec<_> = try_join_all(channel_futures)
        .await
        .unwrap_or_default()
        .into_iter()
        .flat_map(|h| h.into_values())
        .filter(|c| matches!(c.kind, ChannelType::Voice))
        .collect();

    let member_futures = channels
        .into_iter()
        .map(|c| async { (c.members(context).await, c) });

    join_all(member_futures)
        .await
        .into_iter()
        .find_map(|(r, c)| {
            r.ok()
                .unwrap_or_default()
                .into_iter()
                .map(|m| (m, &c))
                .find_map(|(m, c)| {
                    if m.user.id == user_id {
                        Some(c.to_owned())
                    } else {
                        None
                    }
                })
        })
}

/// A Discord client that can be stopped
struct DroppableClient {
    manager: Arc<tokio::sync::Mutex<ShardManager>>,
    status: CurrentDiscordStatus,
    context: DynamicContext,
    rt: Arc<Runtime>,
}

impl DroppableClient {
    pub fn new(
        audio_stream: AudioStream,
        status: CurrentDiscordStatus,
        user: CurrentDiscordUser,
        actions: Sender<Action>,
        context: DynamicContext,
        config: Config,
    ) -> Self {
        let rt = Builder::new_multi_thread()
            .worker_threads(1)
            .max_blocking_threads(1)
            .enable_all()
            .thread_name("discord")
            .build()
            .unwrap();

        let handler = Bot {
            audio_stream,
            actions,
            status: status.clone(),
            context: context.clone(),
            user_id: config.user_id,
            user,
        };

        let mut new_client = rt.block_on(async move {
            Client::builder(&config.bot_token, intents())
                .register_songbird()
                .event_handler(handler)
                .await
                .expect("Err creating client")
        });

        let manager = new_client.shard_manager.clone();
        let moved_status = status.clone();

        rt.spawn(async move {
            if let Err(why) = new_client.start().await {
                *(moved_status.lock().unwrap()) =
                    DiscordStatus::Failed(DiscordError::Serenity(why.into()));
            }
        });

        Self {
            status,
            manager,
            context,
            rt: Arc::new(rt),
        }
    }

    async fn stop(&self) {
        let context = self
            .context
            .lock()
            .unwrap()
            .clone()
            .expect("context exists");

        let status = self.status.lock().unwrap().clone();

        if let DiscordStatus::Active(channel) = status {
            let songbird = songbird::get(&context)
                .await
                .expect("songbird is registered");

            let _ = songbird.remove(channel.guild_id).await;
        }

        context.invisible().await;
        self.manager.lock().await.shutdown_all().await;
    }
}

fn intents() -> GatewayIntents {
    GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
}
