use crate::db::DbService;
use crate::models;
use crate::telegram::{
    BotRequests, BotResponseListChannels, BotResponses, NewUpdate, ServiceRequests,
    ServiceResponses, TelegramService,
};
use anyhow::anyhow;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const HISTORY_LIMIT: i32 = 100;

struct Inner {
    tg: TelegramService,
    db: DbService,
}

#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

impl App {
    pub fn new(tg: TelegramService, db: DbService) -> Self {
        Self {
            inner: Arc::new(Inner { tg, db }),
        }
    }

    pub async fn start(&self) -> anyhow::Result<JoinHandle<()>> {
        log::info!("starting telegram service");
        let (fas, far) = mpsc::channel(10);
        let (tas, mut tar) = mpsc::channel(10);
        let h = self.inner.tg.start(far, tas).await?;
        log::info!("telegram service started");

        let db = self.inner.db.clone();
        tokio::spawn(async move {
            while let Some(r) = tar.recv().await {
                log::info!("new app request: {:?}", r);
                let result = match &r {
                    ServiceRequests::Bot(bot_request) => match bot_request {
                        BotRequests::AddUser(add_user) => {
                            db.save_user(models::NewUser {
                                user_id: add_user.user_id,
                                chat_id: add_user.chat_id,
                                enabled: true,
                            })
                            .await
                        }
                        BotRequests::RemoveUser(remove_user) => {
                            db.save_user(models::NewUser {
                                user_id: remove_user.user_id,
                                chat_id: remove_user.chat_id,
                                enabled: false,
                            })
                            .await
                        }
                        BotRequests::AddUserChannel(add_channel) => {
                            if let Err(err) = db
                                .save_channel(models::NewChannel {
                                    title: add_channel.title.clone(),
                                    telegram_id: add_channel.channel_id,
                                    username: add_channel.channel_name.clone(),
                                })
                                .await
                            {
                                Err(err)
                            } else {
                                db.save_user_channel(models::NewUserChannel {
                                    user_id: add_channel.user_id,
                                    channel_id: add_channel.channel_id,
                                })
                                .await
                            }
                        }
                        BotRequests::ListChannels(user_id) => {
                            match db.get_user_channels(*user_id).await {
                                Err(e) => Err(e),
                                Ok((chat_id, channels)) => fas
                                    .send(ServiceResponses::Bot(BotResponses::ListChannels(
                                        BotResponseListChannels { chat_id, channels },
                                    )))
                                    .await
                                    .map_err(anyhow::Error::msg),
                            }
                        }
                        BotRequests::RemoveUserChannel(remove_channel) => {
                            db.remove_user_channel(models::RemoveUserChannel {
                                user_id: remove_channel.user_id,
                                channel_name: remove_channel.channel_name.clone(),
                            })
                            .await
                        }
                    },
                };
                if let Err(err) = result {
                    log::error!("error {err} for request {r:?}");
                }
            }
        });
        Ok(h)
    }
}
