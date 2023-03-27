use super::bot::{init_bot_updates_reader, BotClient};
use super::user::UserClient;
use super::{TgClient, TgWorker};
use crate::telegram::user::init_client_updates_reader;
use anyhow::{bail, Result};
use rust_tdlib::client::tdlib_client::TdLibClient;
use rust_tdlib::client::{
    AuthStateHandlerProxy, Client, ClientIdentifier, ConsoleClientStateHandlerIdentified, Worker,
};
use rust_tdlib::tdjson::set_log_verbosity_level;
use rust_tdlib::types::Update::User;
use rust_tdlib::types::{TdlibParameters, Update};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

#[derive(Clone)]
pub struct TelegramService {
    api_hash: String,
    app_id: i32,
    user_phone: String,
    bot_token: String,
    inner: Arc<RwLock<Option<Inner>>>,
}

struct Inner {
    pub bot_client: BotClient,
    pub user_client: UserClient,
    pub worker: TgWorker,
}

impl TelegramService {
    pub fn new(api_hash: String, app_id: i32, user_phone: String, bot_token: String) -> Self {
        Self {
            api_hash,
            user_phone,
            bot_token,
            app_id,
            inner: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<JoinHandle<()>> {
        if self.inner.read().await.is_some() {
            bail!("service already started");
        }

        set_log_verbosity_level(
            std::env::var("TDLIB_LOG_VERBOSITY")
                .unwrap_or("1".to_string())
                .parse()?,
        );
        let auth_handler = AuthStateHandlerProxy::new_with_encryption_key("".to_string());

        let mut worker = Worker::builder()
            .with_auth_state_handler(auth_handler)
            .build()?;

        let worker_waiter = worker.start();
        let (bcl, brecv) = self
            .build_client(
                &mut worker,
                ClientIdentifier::BotToken(self.bot_token.clone()),
                init_bot_updates_reader,
            )
            .await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
        let (ucl, urecv) = self
            .build_client(
                &mut worker,
                ClientIdentifier::PhoneNumber(self.user_phone.clone()),
                init_client_updates_reader,
            )
            .await?;

        let mut bot_client = BotClient::new(bcl);
        let (bss, bsr) = mpsc::channel(10);
        let (sbs, sbr) = mpsc::channel(10);
        let bot_handle = bot_client.start(brecv, bsr, sbs).await?;

        let mut user_client = UserClient::new(ucl);
        let (uss, usr) = mpsc::channel(10);
        let (sus, sur) = mpsc::channel(10);
        let user_handle = user_client.start(urecv, usr, sus).await?;

        let join = tokio::spawn(async move {
            tokio::select! {
                _ = worker_waiter => {
                    log::info!("worker exited");
                }
                _ = bot_handle => {
                    log::info!("bot exited");
                }
                _ = user_handle => {
                    log::info!("user exited");
                }
            };
        });

        self.inner.write().await.insert(Inner {
            bot_client,
            user_client,
            worker,
        });
        Ok(join)
    }

    async fn build_client<F, A>(
        &self,
        worker: &mut TgWorker,
        ident: ClientIdentifier,
        updates_handler: F,
    ) -> Result<(TgClient, A)>
    where
        F: FnOnce(Receiver<Box<Update>>) -> A,
    {
        let (sender, receiver) = mpsc::channel::<Box<Update>>(1000);
        let db_dir = match ident {
            ClientIdentifier::PhoneNumber(_) => "user",
            ClientIdentifier::BotToken(_) => "bot",
        };
        let client = Client::builder()
            .with_tdlib_parameters(
                TdlibParameters::builder()
                    .database_directory(db_dir)
                    .use_test_dc(false)
                    .api_id(self.app_id)
                    .api_hash(self.api_hash.clone())
                    .system_language_code("en")
                    .device_model("Unknown")
                    .system_version("Unknown")
                    .application_version("0.0.1")
                    .enable_storage_optimizer(true)
                    .build(),
            )
            .with_client_auth_state_handler(ConsoleClientStateHandlerIdentified::new(ident))
            .with_updates_sender(sender)
            .build()?;

        let recv = updates_handler(receiver);

        let client = worker.bind_client(client).await?;
        Ok((client, recv))
    }

    pub async fn stop(&self) {
        let mut guard = self.inner.write().await;
        if let Some(inner) = guard.take() {
            inner.worker.stop();
        }
    }
}
