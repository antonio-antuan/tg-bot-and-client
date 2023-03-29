mod app;
mod db;
pub mod models;
mod settings;
mod telegram;

extern crate time;

use crate::app::App;
use db::DbService;
use settings::Settings;
use std::time::Duration;
use telegram::TelegramService;

#[tokio::main]
async fn main() {
    env_logger::init();
    let settings = Settings::new().expect("can't get config");
    log::info!("initializing database");
    let db = DbService::new(settings.db.path.as_str())
        .await
        .expect("can't connect to db");

    let telegram = TelegramService::new(
        settings.telegram.api_hash,
        settings.telegram.api_id,
        settings.telegram.phone,
        settings.telegram.bot_token,
    );

    let app = App::new(telegram, db);
    let waiter = app.start().await.expect("cannot start application");
    waiter.await;
    log::info!("finished");
}
