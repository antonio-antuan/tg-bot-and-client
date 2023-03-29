use crate::models;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;

#[derive(Clone)]
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

    pub async fn save_user(&self, user: models::NewUser) -> anyhow::Result<()> {
        sqlx::query_as!(
            models::Channel,
            r#"INSERT INTO users (id, enabled, chat_id)
            VALUES ($1, $2, $3)
            ON CONFLICT(id) DO UPDATE SET enabled = excluded.enabled"#,
            user.user_id,
            user.enabled,
            user.chat_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_channel(&self, channel: models::NewChannel) -> anyhow::Result<()> {
        sqlx::query_as!(
            Channel,
            r#"INSERT INTO channels (id, title, username)
            VALUES ($1, $2, $3)
            -- TODO: do update id??? 0_o
            ON CONFLICT(username) DO UPDATE SET title = excluded.title, id=excluded.id"#,
            channel.telegram_id,
            channel.title,
            channel.username,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_user_channel(&self, channel: models::NewUserChannel) -> anyhow::Result<()> {
        sqlx::query_as!(
            Channel,
            r#"INSERT INTO user_channel (user_id, channel_id)
            VALUES ($1, $2)
            ON CONFLICT(user_id, channel_id) DO NOTHING"#,
            channel.user_id,
            channel.channel_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_user_channel(
        &self,
        channel: models::RemoveUserChannel,
    ) -> anyhow::Result<()> {
        sqlx::query_as!(
            Channel,
            r#"DELETE FROM user_channel uc
                USING channels c
            WHERE c.id = uc.channel_id
                AND uc.user_id = $1 AND c.username = $2"#,
            channel.user_id,
            channel.channel_name,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_channel_posts(&self, posts: &Vec<models::Post>) -> anyhow::Result<()> {
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

    pub async fn get_channel(&self, channel_name: &str) -> anyhow::Result<Option<models::Channel>> {
        Ok(sqlx::query_as!(
            models::Channel,
            "SELECT id, title, username from channels where username = $1",
            channel_name
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn get_user_channels(
        &self,
        user_id: i64,
    ) -> anyhow::Result<(i64, Vec<models::Channel>)> {
        let rec = sqlx::query!(r#"select chat_id from users where id = $1"#, user_id,)
            .fetch_one(&self.pool)
            .await?;
        let chat_id = rec.chat_id;

        let channels = sqlx::query_as!(
            models::Channel,
            r#"SELECT c.id, c.title, c.username
             FROM channels c
             INNER JOIN user_channel uc
                ON uc.channel_id = c.id
             WHERE uc.user_id = $1
             "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await?;
        Ok((chat_id, channels))
    }

    pub async fn get_channel_post_ids(
        &self,
        chat_id: models::TelegramChatId,
        limit: i64,
    ) -> anyhow::Result<Vec<(i32, models::TelegramPostId)>> {
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
    ) -> anyhow::Result<Option<(models::Channel, Vec<models::Post>)>> {
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
        .bind(ch.id)
        .fetch_all(&self.pool)
        .await?;
        let mut posts = Vec::with_capacity(rows.len());
        rows.into_iter().for_each(|r| {
            let post = models::Post {
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
