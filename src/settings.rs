use config::{Config, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DbSettings {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct TelegramSettings {
    pub api_hash: String,
    pub api_id: i32,
    pub phone: String,
    pub bot_token: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub telegram: TelegramSettings,
    pub db: DbSettings,
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::builder()
            .add_source(File::with_name("config").required(true))
            .build()?;
        Ok(config.try_deserialize()?)
    }
}
