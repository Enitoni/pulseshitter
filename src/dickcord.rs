use std::env;

use pulsectl::controllers::types::{ApplicationInfo, DeviceInfo};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::{ChannelType, GuildChannel, GuildId, Member};
use serenity::model::user::User;
use serenity::prelude::*;
use songbird::input::Input;
use songbird::SerenityInit;

use crate::audio::ParecStream;

struct Handler {
    user_id: u64,
    device: DeviceInfo,
    app: ApplicationInfo,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, context: Context, ready: Ready) {
        println!("Finding user {}", self.user_id);

        let guilds = context.cache.guilds();

        println!("Searching in {} guilds", guilds.len());

        let member = find_member_in_guilds(&context, self.user_id, guilds.clone()).await;

        member
            .expect("User not found")
            .user
            .direct_message(&context, |f| f.content("your mother"))
            .await
            .unwrap();

        let channel = find_voice_channel(&context, self.user_id, guilds.clone())
            .await
            .expect("Could not find voice channel");

        let manager = songbird::get(&context).await.unwrap();
        let (handler, _) = manager.join(channel.guild_id, channel.id).await;
        let mut call = handler.lock().await;

        let input = ParecStream::new(self.device.clone(), self.app.clone()).into_input();
        call.play_source(input);

        println!("{}", channel.id)
    }
}

async fn find_member_in_guilds(
    context: &Context,
    user_id: u64,
    guilds: Vec<GuildId>,
) -> Option<Member> {
    for guild in guilds {
        let members = guild.members(context, None, None).await.unwrap();

        for member in members {
            if member.user.id == user_id {
                return Some(member);
            }
        }
    }

    None
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

pub async fn dickcord(device: DeviceInfo, app: ApplicationInfo) {
    let token =
        env::var("DISCORD_TOKEN").expect("Expected a DISCORD_TOKEN in the environment youi fhfjck");

    let user_id: u64 = env::var("DISCORD_USER")
        .expect("Expected a DISCORD_USER id in the environment. Who am i supposed to follow?")
        .parse()
        .unwrap();

    let handler = Handler {
        user_id,
        device,
        app,
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
