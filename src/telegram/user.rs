use crate::models::{NewChannel, Post};
use crate::telegram::{parsers, TgClient, TgWorker, SEND_UPDATE_TIMEOUT};
use anyhow::{anyhow, Result};
use rust_tdlib::client::tdlib_client::TdJson;
use rust_tdlib::client::{Client, ClientIdentifier};
use rust_tdlib::types::{
    Chat, ChatType, GetChat, GetChatHistory, GetChats, GetSupergroup, MessageContent,
    SearchPublicChat, TdlibParameters, TextEntityType, Update, UpdateNewMessage,
};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::task::JoinHandle;

type TgUpdate = Receiver<String>;
type FromService = Receiver<String>;
type ToService = Sender<String>;

#[derive(Clone)]
pub struct UserClient {
    client: Client<TdJson>,
}

impl UserClient {
    pub fn new(client: TgClient) -> Self {
        Self {
            client
        }
    }

    pub async fn start(
        &self,
        mut tg_update: TgUpdate,
        mut from_service: FromService,
        to_service: ToService,
    ) -> Result<JoinHandle<()>> {
        Ok(tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(tg_update) = tg_update.recv() => {
                        log::debug!("new update: {tg_update}");
                    },

                    Some(from_service) = from_service.recv() => {
                        log::debug!("new service request: {from_service}");
                    }
                }
            }
        }))
    }

    pub async fn get_channel_history(&self, chat_id: i64, limit: i32) -> anyhow::Result<Vec<Post>> {
        let history = self
            .client
            .get_chat_history(
                GetChatHistory::builder()
                    .chat_id(chat_id)
                    .limit(limit)
                    .offset(-50)
                    .from_message_id(0)
                    .build(),
            )
            .await?;

        let mut result = Vec::with_capacity(history.messages().len());
        for msg in history.messages().into_iter() {
            if let Some(msg) = msg {
                let content = match parsers::parse_message_content(msg.content()) {
                    None => continue,
                    Some(content) => content,
                };

                result.push(Post {
                    title: None,
                    link: "".to_string(),
                    telegram_id: msg.id(),
                    pub_date: msg.date(),
                    content: content,
                    chat_id,
                })
            }
        }
        Ok(result)
    }

    pub async fn get_all_channels(&self) -> anyhow::Result<Vec<NewChannel>> {
        let chats = self
            .client
            .get_chats(GetChats::builder().limit(9999).build())
            .await?;
        let mut result = Vec::with_capacity(chats.chat_ids().len());
        for chat_id in chats.chat_ids().into_iter() {
            let chat = self
                .client
                .get_chat(GetChat::builder().chat_id(*chat_id).build())
                .await?;

            if let ChatType::Supergroup(type_sg) = chat.type_() {
                if type_sg.is_channel() {
                    let sg = self
                        .client
                        .get_supergroup(
                            GetSupergroup::builder()
                                .supergroup_id(type_sg.supergroup_id())
                                .build(),
                        )
                        .await?;

                    result.push(new_channel(chat, sg.username()))
                }
            }
        }
        Ok(result)
    }

    pub async fn search_channel(&self, channel_name: &str) -> anyhow::Result<Option<NewChannel>> {
        let chat = self
            .client
            .search_public_chat(SearchPublicChat::builder().username(channel_name).build())
            .await?;
        if !is_channel(&chat) {
            return Ok(None);
        }

        Ok(Some(new_channel(chat, channel_name)))
    }
}

pub fn init_client_updates_reader(mut receiver: Receiver<Box<Update>>) -> TgUpdate {
    let (sx, rx) = mpsc::channel(2000);

    tokio::spawn(async move {
        while let Some(update) = receiver.recv().await {
            let new_update = match update.as_ref() {
                Update::MessageContent(content) => None,
                Update::NewMessage(new_message) => {
                    parsers::parse_message_content(new_message.message().content())
                }
                _ => None,
            };
            if let Some(new_update) = new_update {
                if let Err(err) = sx
                    .send_timeout(new_update.to_string(), SEND_UPDATE_TIMEOUT)
                    .await
                {
                    log::error!("cannot send new update");
                }
            }
        }
    });

    rx
}

fn new_channel(chat: Chat, channel_name: &str) -> NewChannel {
    NewChannel {
        title: chat.title().clone(),
        telegram_id: chat.id(),
        username: channel_name.to_string(),
    }
}

fn is_channel(chat: &Chat) -> bool {
    match chat.type_() {
        ChatType::_Default => false,
        ChatType::BasicGroup(g) => false,
        ChatType::Private(_) => false,
        ChatType::Secret(_) => false,
        ChatType::Supergroup(sg) => sg.is_channel(),
    }
}
