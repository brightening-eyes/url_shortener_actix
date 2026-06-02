use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Url::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Url::ExpiresAt).timestamp_with_time_zone().null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Url::Table)
                    .drop_column(Url::ExpiresAt)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Url {
    Table,
    #[allow(dead_code)]
    ExpiresAt,
}
