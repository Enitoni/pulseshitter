use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read};

#[derive(Deserialize, Serialize, Clone)]
pub struct Config {
    pub bot_token: String,
    pub user_id: u64,
}

impl Config {
    const FILE_NAME: &str = "config.ron";

    pub fn new(bot_token: String, user_id: u64) -> Self {
        Self { bot_token, user_id }
    }

    pub fn restore() -> Option<Self> {
        File::open(Self::FILE_NAME)
            .ok()
            .and_then(|mut file| {
                let mut contents = String::new();
                file.read_to_string(&mut contents).map(|_| contents).ok()
            })
            .and_then(|content| ron::from_str(&content).ok())
    }

    pub fn save(&self) {
        if let Err(err) = ron::to_string(self) {
            eprintln!("Config save failed: {:?}", err)
        }
    }
}