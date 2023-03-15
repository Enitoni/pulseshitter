use std::env;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::{ChannelType, GuildChannel, GuildId};
use serenity::prelude::*;
use songbird::SerenityInit;

use crate::audio::AudioSystem;

struct Handler {
    user_id: u64,
    audio: Arc<AudioSystem>,
    ready: SyncSender<()>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, context: Context, _ready: Ready) {
        println!("Finding user {}", self.user_id);

        let guilds = context.cache.guilds();
        println!("Searching in {} guilds", guilds.len());

        let channel = find_voice_channel(&context, self.user_id, guilds.clone())
            .await
            .expect("Could not find voice channel");

        let manager = songbird::get(&context).await.unwrap();
        let (handler, _) = manager.join(channel.guild_id, channel.id).await;
        let mut call = handler.lock().await;

        let input = self.audio.stream().into_input();
        call.play_source(input);

        println!("{}", channel.id);
        self.ready.send(()).unwrap();
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

pub async fn dickcord(ready: SyncSender<()>, audio: Arc<AudioSystem>) {
    let token =
        env::var("DISCORD_TOKEN").expect("Expected a DISCORD_TOKEN in the environment youi fhfjck");

    let user_id: u64 = env::var("DISCORD_USER")
        .expect("Expected a DISCORD_USER id in the environment. Who am i supposed to follow?")
        .parse()
        .unwrap();

    let handler = Handler {
        user_id,
        ready,
        audio,
    };

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(&token, intents)
        .register_songbird()
        .event_handler(handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
