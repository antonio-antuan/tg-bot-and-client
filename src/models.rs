pub type TelegramPostId = i64;
pub type TelegramChatId = i64;

#[derive(Debug, sqlx::FromRow)]
pub struct Post {
    pub title: Option<String>,
    pub link: String,
    pub telegram_id: TelegramPostId,
    pub pub_date: i32,
    pub content: String,
    pub chat_id: TelegramChatId,
}

impl Post {
    pub fn title(&self) -> &Option<String> {
        &self.title
    }
    pub fn link(&self) -> &str {
        &self.link
    }
    pub fn telegram_id(&self) -> TelegramPostId {
        self.telegram_id
    }
    pub fn pub_date(&self) -> i32 {
        self.pub_date
    }
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn chat_id(&self) -> TelegramChatId {
        self.chat_id
    }
}

#[derive(Debug)]
pub struct NewUser {
    pub user_id: i64,
    pub chat_id: i64,
    pub enabled: bool,
}

#[derive(Debug)]
pub struct NewChannel {
    pub title: String,
    pub telegram_id: TelegramChatId,
    pub username: String,
}

#[derive(Debug)]
pub struct NewUserChannel {
    pub user_id: i64,
    pub channel_id: i64,
}

#[derive(Debug)]
pub struct RemoveUserChannel {
    pub user_id: i64,
    pub channel_name: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Channel {
    pub id: i64,
    pub title: String,
    pub username: String,
    // pub telegram_id: TelegramChatId,
}
