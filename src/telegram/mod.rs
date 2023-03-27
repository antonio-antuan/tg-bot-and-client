use crate::models::{NewChannel, Post};
use rust_tdlib::client::tdlib_client::TdJson;
use rust_tdlib::client::{AuthStateHandlerProxy, Client, Worker};
use std::time::Duration;
use tokio::sync::mpsc;

mod bot;
mod parsers;
mod service;
mod user;

pub use service::TelegramService;

const SEND_UPDATE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub enum NewUpdate {
    Post(Post),
    Channel(NewChannel),
}

type TgWorker = Worker<AuthStateHandlerProxy, TdJson>;
type TgClient = Client<TdJson>;
