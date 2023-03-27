pub use crate::models::{Channel, NewChannel, Post};
use crate::models::{TelegramChatId, TelegramPostId};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

pub struct DbService {
    pool: PgPool,
}

impl DbService {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(db_path)
            .await?;
        Ok(Self { pool })
    }

    pub async fn save_channel(&self, channel: NewChannel) -> anyhow::Result<()> {
        sqlx::query_as!(
            Channel,
            r#"INSERT INTO channels (title, username, telegram_id)
            VALUES ($1, $2, $3)
            ON CONFLICT(username) DO UPDATE SET title = excluded.title, telegram_id=excluded.telegram_id"#,
            channel.title,
            channel.username,
            channel.telegram_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_channel_posts(&self, posts: &Vec<Post>) -> anyhow::Result<()> {
        for p in posts.iter() {
            sqlx::query!(
                r#"INSERT INTO posts (title, link, telegram_id, pub_date, content, chat_id)
                VALUES ($1, $2, $3, $4, $5, $6)"#,
                p.title,
                p.link,
                p.telegram_id,
                p.pub_date,
                p.content,
                p.chat_id
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_channel(&self, channel_name: &str) -> anyhow::Result<Option<Channel>> {
        Ok(sqlx::query_as!(
            Channel,
            "SELECT id, title, username, telegram_id from channels where username = $1",
            channel_name
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn get_channel_post_ids(
        &self,
        chat_id: TelegramChatId,
        limit: i64,
    ) -> anyhow::Result<Vec<(i32, TelegramPostId)>> {
        let rows = sqlx::query!(
            r#"SELECT id, telegram_id
            FROM posts
            WHERE chat_id = $1
            LIMIT $2"#,
            chat_id,
            limit,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|v| (v.id, v.telegram_id)).collect())
    }

    pub async fn get_channel_posts(
        &self,
        channel_name: &str,
    ) -> anyhow::Result<Option<(Channel, Vec<Post>)>> {
        let ch = match self.get_channel(channel_name).await? {
            None => return Ok(None),
            Some(ch) => ch,
        };
        let rows = sqlx::query(
            r#"SELECT title, link, telegram_id, pub_date as "pub_date: i32", content, chat_id
            FROM posts
            WHERE chat_id = $1
            LIMIT 25"#,
            // ch.telegram_id
        )
        .bind(ch.telegram_id)
        .fetch_all(&self.pool)
        .await?;
        let mut posts = Vec::with_capacity(rows.len());
        rows.into_iter().for_each(|r| {
            let post = Post {
                title: r.get("title"),
                link: r.get("link"),
                telegram_id: r.get("telegram_id"),
                pub_date: r.get("pub_date"),
                content: r.get("content"),
                chat_id: r.get("chat_id"),
            };
            posts.push(post);
        });
        Ok(Some((ch, posts)))
    }
}
