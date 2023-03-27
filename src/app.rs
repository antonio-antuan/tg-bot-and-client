use crate::db::DbService;
use crate::telegram::{NewUpdate, TelegramService};
use std::sync::Arc;
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
        let h = self.inner.tg.start().await?;
        log::info!("telegram service started");
        Ok(h)
    }
    //
    // pub async fn synchronize_channels(&self) -> anyhow::Result<()> {
    //     let channels = self.inner.tg.get_all_channels().await?;
    //     for channel in channels.into_iter() {
    //         self.inner.db.save_channel(channel).await?;
    //     }
    //     Ok(())
    // }
}

fn rss_err<E: std::fmt::Debug>(err: E) -> anyhow::Error {
    anyhow::anyhow!("error building rss feed: {:?}", err)
}
