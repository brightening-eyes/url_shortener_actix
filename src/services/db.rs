use sea_orm::*;
use std::time::Duration;
use crate::entities::{prelude::*, url};

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

    pub async fn find_url_by_short_code(&self, short_code: &str) -> Result<Option<url::Model>, sea_orm::DbErr> {
        Url::find()
            .filter(url::Column::ShortCode.eq(short_code))
            .one(&self.db_conn)
            .await
    }

    pub async fn save_short_url(&self, long_url: &str, short_code: &str) -> Result<url::Model, sea_orm::DbErr> {
        let new_url = url::ActiveModel {
            long_url: Set(long_url.to_owned()),
            short_code: Set(short_code.to_owned()),
            ..Default::default()
        };

        new_url.insert(&self.db_conn).await
    }
}
