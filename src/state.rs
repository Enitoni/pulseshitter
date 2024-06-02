use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::{fs::File, io::Read};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub bot_token: String,
    pub user_id: u64,

    pub show_meter: bool,
    pub screen_share_only: bool,
}

#[derive(Debug, Clone)]
pub struct ReadOnlyConfig {
    pub bot_token: String,
    pub user_id: u64,
    pub show_meter: bool,
    pub screen_share_only: bool,
}

impl Config {
    fn path() -> String {
        let config_dir = env::var("XDG_CONFIG_HOME")
            .or_else(|_| env::var("HOME").map(|path| path + "/.config"))
            .unwrap_or_else(|_| ".".to_string());

        format!("{}/pulseshitter-config.ron", config_dir)
    }

    pub fn new(bot_token: String, user_id: u64) -> Self {
        Self {
            bot_token,
            user_id,
            show_meter: true,
            screen_share_only: false,
        }
    }

    pub fn restore() -> Option<Self> {
        File::open(Self::path())
            .ok()
            .and_then(|mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents).map(|_| contents).ok()
            })
            .and_then(|content| ron::from_str(&content).ok())
    }

    pub fn save(&self) {
        match ron::to_string(self) {
            Ok(result) => {
                File::create(Self::path())
                    .ok()
                    .and_then(|mut f| write!(f, "{}", result).ok());
            }
            Err(err) => eprintln!("Config save failed: {:?}", err),
        }
    }

    pub fn read_only(&self) -> ReadOnlyConfig {
        ReadOnlyConfig {
            bot_token: self.bot_token.clone(),
            user_id: self.user_id,
            show_meter: self.show_meter,
            screen_share_only: self.screen_share_only,
        }
    }
}
