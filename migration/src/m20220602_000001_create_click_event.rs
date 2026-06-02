use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ClickEvent::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ClickEvent::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ClickEvent::ShortCode).string().not_null())
                    .col(
                        ColumnDef::new(ClickEvent::ClickedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(ClickEvent::IpAddress).string())
                    .col(ColumnDef::new(ClickEvent::UserAgent).string())
                    .col(ColumnDef::new(ClickEvent::Referer).string())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ClickEvent::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ClickEvent {
    Table,
    Id,
    ShortCode,
    ClickedAt,
    IpAddress,
    UserAgent,
    Referer,
}
