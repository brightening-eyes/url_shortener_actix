use crate::entities::{prelude::*, url};
use crate::entities::click_event;
use chrono::Utc;
use sea_orm::*;
use std::time::Duration;

pub async fn establish_connection(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true);

    Database::connect(opt).await
}

#[derive(Clone)]
pub struct DbService {
    db_conn: DatabaseConnection,
}

impl DbService {
    pub fn new(db_conn: DatabaseConnection) -> Self {
        Self { db_conn }
    }

    pub async fn find_url_by_short_code(
        &self,
        short_code: &str,
    ) -> Result<Option<url::Model>, sea_orm::DbErr> {
        Url::find()
            .filter(url::Column::ShortCode.eq(short_code))
            .one(&self.db_conn)
            .await
    }

    pub async fn short_code_exists(&self, short_code: &str) -> Result<bool, sea_orm::DbErr> {
        Ok(Url::find()
            .filter(url::Column::ShortCode.eq(short_code))
            .count(&self.db_conn)
            .await?
            > 0)
    }

    pub async fn get_all_urls(&self) -> Result<Vec<url::Model>, sea_orm::DbErr> {
        Url::find().all(&self.db_conn).await
    }

    pub fn is_url_expired(model: &url::Model) -> bool {
        match model.expires_at {
            Some(exp) => exp < Utc::now(),
            None => false,
        }
    }

    pub async fn save_short_url(
        &self,
        long_url: &str,
        short_code: &str,
        expires_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> Result<url::Model, sea_orm::DbErr> {
        let mut new_url = url::ActiveModel {
            long_url: Set(long_url.to_owned()),
            short_code: Set(short_code.to_owned()),
            ..Default::default()
        };
        if let Some(exp) = expires_at {
            new_url.expires_at = Set(Some(exp));
        }
        new_url.insert(&self.db_conn).await
    }

    pub async fn record_click(
        &self,
        short_code: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        referer: Option<&str>,
    ) -> Result<click_event::Model, sea_orm::DbErr> {
        let new_click = click_event::ActiveModel {
            short_code: Set(short_code.to_owned()),
            ip_address: Set(ip_address.map(|s| s.to_owned())),
            user_agent: Set(user_agent.map(|s| s.to_owned())),
            referer: Set(referer.map(|s| s.to_owned())),
            ..Default::default()
        };
        new_click.insert(&self.db_conn).await
    }

    pub async fn get_click_stats(
        &self,
        short_code: &str,
    ) -> Result<Vec<click_event::Model>, sea_orm::DbErr> {
        ClickEvent::find()
            .filter(click_event::Column::ShortCode.eq(short_code))
            .all(&self.db_conn)
            .await
    }
}

