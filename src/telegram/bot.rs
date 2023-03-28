use crate::telegram::{TgClient, SEND_UPDATE_TIMEOUT};
use anyhow::{anyhow, Result};
use rust_tdlib::types::{BotCommand as TdLibBotCommand, FormattedText, GetMe, InputMessageContent, InputMessageText, MessageContent, MessageSender, SearchPublicChat, SendMessage, SetCommands, TextEntityType, Update, UpdateNewMessage};
use strum::{EnumIter, IntoEnumIterator, Display, EnumMessage};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct BotUpdate {
    chat_id: i64,
    user_id: i64,
    message: BotCommand,
}

#[derive(Debug)]
pub struct UserChannel {
    user_id: i64,
    channel_name: String,
}


#[derive(Debug)]
pub enum BotRequests {
    AddUser(i64),
    RemoveUser(i64),
    AddUserChannel(UserChannel),
    RemoveUserChannel(UserChannel)
}

type TgUpdate = Receiver<BotUpdate>;
type FromTgService = Receiver<String>;
type ToTgService = Sender<BotRequests>;

pub struct BotClient {
    client: Option<TgClient>,
}

#[derive(Debug, Display, EnumMessage, EnumIter)]
enum BotCommand {
    #[strum(message = "/start", detailed_message = "starts bot interaction")]
    Start,
    #[strum(message = "/add", detailed_message = "adds a channel")]
    Add(String),
    Invalid,
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
        mut from_service: FromTgService,
        to_service: ToTgService,
    ) -> Result<JoinHandle<()>> {
        let client = self.client.take().unwrap();

        client
            .set_commands(SetCommands::builder().commands(
                BotCommand::iter().filter_map(|cmd| {
                    let message = cmd.get_message();
                    let det_message = cmd.get_detailed_message();

                    match (message, det_message) {
                        (Some(message), Some(det_message)) => {
                            Some(TdLibBotCommand::builder().command(message).description(det_message).build())
                        }
                        _ => None
                    }

                }).collect()
            ))
            .await?;

        let me = client.get_me(GetMe::builder().build()).await?;

        Ok(tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(tg_upd) = tg_update.recv() => {
                        if tg_upd.user_id == me.id() {
                            continue
                        }
                        let msg = match &tg_upd.message {
                            BotCommand::Invalid => {
                                make_invalid_request_resp(tg_upd.chat_id)
                            },
                            BotCommand::Add(channel_name) => {
                                match client.search_public_chat(SearchPublicChat::builder().username(channel_name).build()).await {
                                    Err(_) => make_channel_not_found_resp(tg_upd.chat_id, channel_name),
                                    Ok(_) => {
                                        let resp = make_channel_added_resp(tg_upd.chat_id, channel_name);
                                        to_service.send(BotRequests::AddUserChannel(UserChannel{
                                            user_id: tg_upd.user_id,
                                            channel_name: channel_name.clone(),
                                        })).await;
                                        resp
                                    }
                                }
                            },
                            BotCommand::Start => {
                                to_service.send(BotRequests::AddUser(tg_upd.user_id)).await;
                                make_start_resp(tg_upd.chat_id)
                            }
                        };
                        client.send_message(msg).await;
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
                Update::NewMessage(new_message) => handle_message_to_bot(new_message),
                _ => None,
            };
            if let Some(new_update) = new_update {
                if let Err(err) = sx
                    .send_timeout(new_update, SEND_UPDATE_TIMEOUT)
                    .await
                {
                    log::error!("cannot send new update: {}", err);
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

            match new_message.message().sender_id() {
                MessageSender::_Default => {
                    todo!()
                }
                MessageSender::Chat(_) => {
                    todo!()
                }
                MessageSender::User(user) => {
                    let message: BotCommand;
                    if !is_bot_command {
                        message = BotCommand::Invalid
                    } else {
                        message = match text.text() {
                            x if x.starts_with("/add") => {
                                BotCommand::Add(text.text().clone().chars().skip(4).collect())
                            },
                            x if x.starts_with("/start") => {
                                BotCommand::Start
                            },
                            _ => BotCommand::Invalid
                        };
                    }
                    return Some(BotUpdate{
                        chat_id: new_message.message().chat_id(),
                        user_id: user.user_id(),
                        message,
                    })
                }
            }
        }
        _ => None,
    }
}

fn make_channel_not_found_resp(chat_id: i64, channel_name: &str) -> SendMessage {
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(
            InputMessageContent::InputMessageText(
                InputMessageText::builder().text(
                    FormattedText::builder().text(
                        format!("channel {} not found", channel_name)
                    ).build()
                ).build()
            )
        )
        .build()
}

fn make_channel_added_resp(chat_id: i64, channel_name: &str) -> SendMessage {
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(
            InputMessageContent::InputMessageText(
                InputMessageText::builder().text(
                    FormattedText::builder().text(
                        format!("channel {} added", channel_name)
                    ).build()
                ).build()
            )
        )
        .build()
}

fn make_invalid_request_resp(chat_id: i64) -> SendMessage {
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(
            InputMessageContent::InputMessageText(
                InputMessageText::builder().text(
                    FormattedText::builder().text(
                        "invalid request"
                    ).build()
                ).build()
            )
        )
        .build()
}

fn make_start_resp(chat_id: i64) -> SendMessage {
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(
            InputMessageContent::InputMessageText(
                InputMessageText::builder().text(
                    FormattedText::builder().text(
                        "started"
                    ).build()
                ).build()
            )
        )
        .build()
}