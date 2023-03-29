use crate::models;
use crate::telegram::{TgClient, SEND_UPDATE_TIMEOUT};
use anyhow::{anyhow, Result};
use rust_tdlib::types::{
    BotCommand as TdLibBotCommand, FormattedText, GetMe, InputMessageContent, InputMessageText,
    MessageContent, MessageSender, SearchPublicChat, SendMessage, SetCommands, TextEntityType,
    Update, UpdateNewMessage,
};
use strum::{Display, EnumIter, EnumMessage, IntoEnumIterator};
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
pub struct AddUserChannel {
    pub user_id: i64,
    pub channel_id: i64,
    pub channel_name: String,
    pub title: String,
}

#[derive(Debug)]
pub struct RemoveUserChannel {
    pub user_id: i64,
    pub channel_name: String,
}

#[derive(Debug)]
pub struct UserChat {
    pub user_id: i64,
    pub chat_id: i64,
}

#[derive(Debug)]
pub enum BotRequests {
    AddUser(UserChat),
    RemoveUser(UserChat),
    AddUserChannel(AddUserChannel),
    RemoveUserChannel(RemoveUserChannel),
    ListChannels(i64),
}

#[derive(Debug)]
pub struct BotResponseListChannels {
    pub chat_id: i64,
    pub channels: Vec<models::Channel>,
}

#[derive(Debug)]
pub enum BotResponses {
    ListChannels(BotResponseListChannels),
}

type TgUpdate = Receiver<BotUpdate>;
type FromTgService = Receiver<BotResponses>;
type ToTgService = Sender<BotRequests>;

pub struct BotClient {
    client: Option<TgClient>,
}

#[derive(Debug, Display, EnumMessage, EnumIter)]
enum BotCommand {
    #[strum(message = "/start", detailed_message = "starts bot interaction")]
    Start,
    #[strum(message = "/stop", detailed_message = "stops bot interaction")]
    Stop,
    #[strum(message = "/add", detailed_message = "adds a channel")]
    Add(String),
    #[strum(message = "/list", detailed_message = "list of channels")]
    List,
    #[strum(message = "/remove", detailed_message = "removes a channel")]
    Remove(String),
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
            .set_commands(
                SetCommands::builder().commands(
                    BotCommand::iter()
                        .filter_map(|cmd| {
                            let message = cmd.get_message();
                            let det_message = cmd.get_detailed_message();

                            match (message, det_message) {
                                (Some(message), Some(det_message)) => Some(
                                    TdLibBotCommand::builder()
                                        .command(message)
                                        .description(det_message)
                                        .build(),
                                ),
                                _ => None,
                            }
                        })
                        .collect(),
                ),
            )
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
                                Some(make_invalid_request_resp(tg_upd.chat_id))
                            },
                            BotCommand::List => {
                                to_service.send(BotRequests::ListChannels(tg_upd.user_id)).await;
                                None
                            }
                            BotCommand::Remove(channel_name) => {
                                let resp = make_channel_removed_resp(tg_upd.chat_id, channel_name);
                                to_service.send(BotRequests::RemoveUserChannel(RemoveUserChannel{
                                    user_id: tg_upd.user_id,
                                    channel_name: channel_name.trim().to_string(),
                                })).await;
                                Some(resp)
                            }
                            BotCommand::Add(channel_name) => {
                                match client.search_public_chat(SearchPublicChat::builder().username(channel_name).build()).await {
                                    Err(_) => Some(make_channel_not_found_resp(tg_upd.chat_id, channel_name)),
                                    Ok(ch) => {
                                        let resp = make_channel_added_resp(tg_upd.chat_id, channel_name);
                                        to_service.send(BotRequests::AddUserChannel(AddUserChannel{
                                            user_id: tg_upd.user_id,
                                            channel_name: channel_name.trim().to_string(),
                                            title: ch.title().trim().to_string(),
                                            channel_id: ch.id(),
                                        })).await;
                                        Some(resp)
                                    }
                                }
                            },
                            BotCommand::Stop => {
                                to_service.send(BotRequests::RemoveUser(
                                    UserChat{
                                        user_id: tg_upd.user_id,
                                        chat_id: tg_upd.chat_id,
                                }
                                )).await;
                                Some(make_stop_resp(tg_upd.chat_id))
                            }
                            BotCommand::Start => {
                                to_service.send(BotRequests::AddUser(
                                    UserChat{
                                        user_id: tg_upd.user_id,
                                        chat_id: tg_upd.chat_id,
                                })).await;
                                Some(make_start_resp(tg_upd.chat_id))
                            }
                        };
                        if let Some(msg) = msg {
                            client.send_message(msg).await;
                        }
                    },

                    Some(from_srv) = from_service.recv() => {
                        match from_srv {
                            BotResponses::ListChannels(channels) => {
                                client.send_message(make_list_channels(channels.chat_id, channels.channels)).await;
                            }
                        }
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
                if let Err(err) = sx.send_timeout(new_update, SEND_UPDATE_TIMEOUT).await {
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
                            x if x.starts_with("/add") => BotCommand::Add(
                                text.text().clone().chars().skip("/add".len()).collect(),
                            ),
                            x if x.starts_with("/remove") => BotCommand::Remove(
                                text.text().clone().chars().skip("/remove".len()).collect(),
                            ),
                            x if x.starts_with("/list") => BotCommand::List,
                            x if x.starts_with("/start") => BotCommand::Start,
                            x if x.starts_with("/stop") => BotCommand::Stop,
                            _ => BotCommand::Invalid,
                        };
                    }
                    return Some(BotUpdate {
                        chat_id: new_message.message().chat_id(),
                        user_id: user.user_id(),
                        message,
                    });
                }
            }
        }
        _ => None,
    }
}

fn make_channel_not_found_resp(chat_id: i64, channel_name: &str) -> SendMessage {
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(InputMessageContent::InputMessageText(
            InputMessageText::builder()
                .text(
                    FormattedText::builder()
                        .text(format!("channel {} not found", channel_name))
                        .build(),
                )
                .build(),
        ))
        .build()
}

fn make_channel_added_resp(chat_id: i64, channel_name: &str) -> SendMessage {
    make_text_resp(chat_id, format!("channel {} added", channel_name))
}

fn make_channel_removed_resp(chat_id: i64, channel_name: &str) -> SendMessage {
    make_text_resp(chat_id, format!("channel {} removed", channel_name))
}

fn make_invalid_request_resp(chat_id: i64) -> SendMessage {
    make_text_resp(chat_id, "invalid request")
}

fn make_start_resp(chat_id: i64) -> SendMessage {
    make_text_resp(chat_id, "started")
}

fn make_stop_resp(chat_id: i64) -> SendMessage {
    make_text_resp(chat_id, "stopped")
}

fn make_text_resp<T: AsRef<str>>(chat_id: i64, text: T) -> SendMessage {
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(InputMessageContent::InputMessageText(
            InputMessageText::builder()
                .text(FormattedText::builder().text(text).build())
                .build(),
        ))
        .build()
}

fn make_list_channels(chat_id: i64, channels: Vec<models::Channel>) -> SendMessage {
    let mut s = "".to_string();
    for ch in channels {
        s += format!("{}: {}\n", ch.username, ch.title).as_str()
    }
    SendMessage::builder()
        .chat_id(chat_id)
        .input_message_content(InputMessageContent::InputMessageText(
            InputMessageText::builder()
                .text(FormattedText::builder().text(s).build())
                .build(),
        ))
        .build()
}
