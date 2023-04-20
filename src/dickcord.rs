use std::fmt::Display;
use std::sync::{Arc, Mutex};

use crate::audio::AudioStream;
use crate::state::Config;
use crate::Action;
use crossbeam::channel::Sender;
use serenity::async_trait;
use serenity::client::bridge::gateway::ShardManager;
use serenity::model::gateway::Ready;
use serenity::model::prelude::{ChannelType, GuildChannel, GuildId};
use serenity::model::voice::VoiceState;
use serenity::prelude::*;
use songbird::error::JoinError;
use songbird::SerenityInit;
use tokio::runtime::Runtime;

/// Main Discord connection state managing
#[derive(Default)]
pub struct Discord {
    client: Arc<Mutex<Option<DroppableClient>>>,
    pub status: Arc<Mutex<DiscordStatus>>,
}

impl Discord {
    pub fn connect(&self, audio_stream: AudioStream, config: Config, actions: Sender<Action>) {
        let mut client = self.client.lock().unwrap();

        // Kill the current connection
        if client.is_some() {
            *client = None;
        }

        *(self.status.lock().unwrap()) = DiscordStatus::Connecting;
        *client = DroppableClient::new(audio_stream, self.status.clone(), actions, config).into();
    }
}

#[derive(Default)]
pub enum DiscordStatus {
    #[default]
    Idle,
    Connecting,
    Connected,
    Joining(GuildChannel),
    Active(GuildChannel),
    Failed(DiscordError),
}

pub enum DiscordError {
    Serenity(SerenityError),
    Songbird(JoinError),
}

impl Display for DiscordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscordError::Serenity(e) => e.fmt(f),
            DiscordError::Songbird(e) => e.fmt(f),
        }
    }
}

struct Bot {
    user_id: u64,
    status: Arc<Mutex<DiscordStatus>>,
    actions: Sender<Action>,
    audio_stream: AudioStream,
}

impl Bot {
    async fn connect_and_stream(&self, context: Context, channel: GuildChannel) {
        {
            *(self.status.lock().unwrap()) = DiscordStatus::Joining(channel.clone());
        }

        let manager = songbird::get(&context).await.unwrap();
        let (handler, result) = manager.join(channel.guild_id, channel.id).await;

        if let Err(why) = result {
            *(self.status.lock().unwrap()) = DiscordStatus::Failed(DiscordError::Songbird(why));
        }

        let mut call = handler.lock().await;
        call.play_source(self.audio_stream.clone().into_input());

        *(self.status.lock().unwrap()) = DiscordStatus::Active(channel.clone());
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, context: Context, _ready: Ready) {
        {
            *(self.status.lock().unwrap()) = DiscordStatus::Connected;
        }

        self.actions.send(Action::Activate).unwrap();

        let guilds = context.cache.guilds();

        if let Some(channel) = find_voice_channel(&context, self.user_id, guilds.clone()).await {
            self.connect_and_stream(context, channel).await;
        }
    }

    async fn voice_state_update(&self, context: Context, old: Option<VoiceState>, new: VoiceState) {
        if let Some((old_member, guild_id)) = old.and_then(|a| a.member.zip(a.guild_id)) {
            if old_member.user.id == self.user_id {
                let manager = songbird::get(&context).await.unwrap();
                let _ = manager.leave(guild_id).await;
            }
        }

        if let Some((member, channel_id)) = new.member.zip(new.channel_id) {
            if member.user.id != self.user_id {
                return;
            }

            if let Some(channel) = context.cache.guild_channel(channel_id) {
                self.connect_and_stream(context, channel).await;
            }
        }
    }
}

async fn find_voice_channel(
    context: &Context,
    user_id: u64,
    guilds: Vec<GuildId>,
) -> Option<GuildChannel> {
    for guild in guilds {
        let channels = guild.channels(context).await.unwrap();

        for channel in channels {
            let channel = channel.1;

            if matches!(channel.kind, ChannelType::Voice) {
                for member in channel.members(context).await.unwrap() {
                    if member.user.id == user_id {
                        return Some(channel.clone());
                    }
                }
            }
        }
    }

    None
}

/// A Discord client that can be stopped by dropping it
struct DroppableClient {
    manager: Arc<tokio::sync::Mutex<ShardManager>>,
    rt: Arc<Runtime>,
}

impl DroppableClient {
    pub fn new(
        audio_stream: AudioStream,
        status: Arc<Mutex<DiscordStatus>>,
        actions: Sender<Action>,
        config: Config,
    ) -> Self {
        let rt = Runtime::new().unwrap();

        let handler = Bot {
            audio_stream,
            actions,
            status: status.clone(),
            user_id: config.user_id,
        };

        let mut new_client = rt.block_on(async move {
            Client::builder(&config.bot_token, intents())
                .register_songbird()
                .event_handler(handler)
                .await
                .expect("Err creating client")
        });

        let manager = new_client.shard_manager.clone();

        rt.spawn(async move {
            if let Err(why) = new_client.start().await {
                *(status.lock().unwrap()) = DiscordStatus::Failed(DiscordError::Serenity(why));
            }
        });

        Self {
            manager,
            rt: Arc::new(rt),
        }
    }
}

impl Drop for DroppableClient {
    fn drop(&mut self) {
        let manager = self.manager.lock();

        self.rt.block_on(async move {
            manager.await.shutdown_all().await;
        })
    }
}

fn intents() -> GatewayIntents {
    GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
}
