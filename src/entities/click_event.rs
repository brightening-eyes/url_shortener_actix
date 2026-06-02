use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize)]
#[sea_orm(table_name = "click_event")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub short_code: String,
    pub clicked_at: DateTimeWithTimeZone,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
