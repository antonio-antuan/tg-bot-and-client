use crate::telegram::{TgClient, TgWorker, SEND_UPDATE_TIMEOUT};
use anyhow::{anyhow, Result};
use rust_tdlib::client::tdlib_client::TdJson;
use rust_tdlib::client::{Client, ClientIdentifier};
use rust_tdlib::types::{BotCommand, FormattedText, GetMe, InputMessageContent, InputMessageText, MessageContent, MessageSender, SendMessage, SetCommands, TdlibParameters, TextEntityType, Update, UpdateNewMessage};
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

#[derive(Debug, Clone)]
pub struct BotUpdate {
    chat_id: i64,
    user_id: i64,
    message: String,
}

type TgUpdate = Receiver<BotUpdate>;
type FromService = Receiver<String>;
type ToService = Sender<String>;

pub struct BotClient {
    client: Option<TgClient>,
}

impl BotClient {
    pub fn new(client: TgClient) -> Self {
        Self {
            client: Some(client),
        }
    }

    pub async fn start(
        &mut self,
        mut tg_update: TgUpdate,
        mut from_service: FromService,
        to_service: ToService,
    ) -> Result<JoinHandle<()>> {
        let client = self.client.take().unwrap();
        client
            .set_commands(SetCommands::builder().commands(vec![
                BotCommand::builder().command("/start").description("start").build(),
                BotCommand::builder().command("/add").description("adds new channel").build(),
            ]))
            .await?;
        let me = client.get_me(GetMe::builder().build()).await?;

        Ok(tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(tg_upd) = tg_update.recv() => {
                        log::debug!("new command: {}", tg_upd.message);
                        if tg_upd.user_id == me.id() {
                            continue
                        }
                        client.send_message(
                            SendMessage::builder()
                            .chat_id(tg_upd.chat_id)
                            .input_message_content(
                                InputMessageContent::InputMessageText(
                                    InputMessageText::builder().text(
                                        FormattedText::builder().text(
                                            format!("got message: {}", tg_upd.message)
                                        ).build()
                                    ).build()
                                )
                            )
                            .build(),
                        ).await;
                    },

                    Some(from_srv) = from_service.recv() => {
                        log::debug!("new command: {from_srv}");
                    }
                }
            }
        }))
    }
}

pub fn init_bot_updates_reader(mut receiver: Receiver<Box<Update>>) -> TgUpdate {
    let (sx, rx) = mpsc::channel(2000);

    tokio::spawn(async move {
        while let Some(update) = receiver.recv().await {
            let new_update = match update.as_ref() {
                Update::MessageContent(content) => None,
                Update::NewMessage(new_message) => handle_message_to_bot(new_message),
                _ => None,
            };
            if let Some(new_update) = new_update {
                if let Err(err) = sx
                    .send_timeout(new_update, SEND_UPDATE_TIMEOUT)
                    .await
                {
                    log::error!("cannot send new update");
                }
            }
        }
    });

    rx
}

fn handle_message_to_bot(new_message: &UpdateNewMessage) -> Option<BotUpdate> {
    log::debug!("{new_message:?}");
    match new_message.message().content() {
        MessageContent::MessageText(message_text) => {
            let text = message_text.text();
            let is_bot_command = text.entities().iter().any(|te| match te.type_() {
                TextEntityType::BotCommand(_) => true,
                _ => false,
            });
            if is_bot_command {
                match new_message.message().sender_id() {
                    MessageSender::_Default => {
                        todo!()
                    }
                    MessageSender::Chat(_) => {
                        todo!()
                    }
                    MessageSender::User(user) => {
                        return Some(BotUpdate{
                            chat_id: new_message.message().chat_id(),
                            user_id: user.user_id(),
                            message: text.text().clone(),
                        })
                    }
                }
            }
            None
        }
        _ => None,
    }
}
