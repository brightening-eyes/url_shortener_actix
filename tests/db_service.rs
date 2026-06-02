use chrono::Utc;
use sea_orm::{ConnectionTrait, Database, Statement};
use url_shortener::services::db::DbService;

async fn setup_test_db() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.expect("Failed to connect to in-memory SQLite");

    db.execute(Statement::from_string(
        db.get_database_backend(),
        "CREATE TABLE IF NOT EXISTS url (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            short_code TEXT NOT NULL UNIQUE,
            long_url TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            expires_at TIMESTAMPTZ
        )"
    )).await.expect("Failed to create url table");

    db.execute(Statement::from_string(
        db.get_database_backend(),
        "CREATE TABLE IF NOT EXISTS click_event (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            short_code TEXT NOT NULL,
            clicked_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ip_address TEXT,
            user_agent TEXT,
            referer TEXT
        )"
    )).await.expect("Failed to create click_event table");

    db
}

#[tokio::test]
async fn test_save_and_find() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    let model = service
        .save_short_url("https://example.com", "abc123", None)
        .await
        .expect("Failed to save URL");

    assert_eq!(model.short_code, "abc123");
    assert_eq!(model.long_url, "https://example.com");
    assert_eq!(model.expires_at, None);

    let found = service
        .find_url_by_short_code("abc123")
        .await
        .expect("Failed to find URL")
        .expect("URL not found");

    assert_eq!(found.short_code, "abc123");
    assert_eq!(found.long_url, "https://example.com");
}

#[tokio::test]
async fn test_find_nonexistent() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    let result = service.find_url_by_short_code("nonexistent").await.expect("Query failed");
    assert!(result.is_none());
}

#[tokio::test]
async fn test_short_code_exists() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    service.save_short_url("https://example.com", "exists", None).await.expect("Failed to save");

    assert!(service.short_code_exists("exists").await.expect("Query failed"));
    assert!(!service.short_code_exists("missing").await.expect("Query failed"));
}

#[tokio::test]
async fn test_url_uniqueness() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    service.save_short_url("https://example.com", "unique", None).await.expect("First insert failed");

    let result = service.save_short_url("https://other.com", "unique", None).await;
    assert!(result.is_err(), "Duplicate short_code should fail");
}

#[tokio::test]
async fn test_save_with_expiry() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    let future = Utc::now() + chrono::Duration::hours(24);
    let expires_at = future.fixed_offset();

    let model = service
        .save_short_url("https://example.com", "expiring", Some(expires_at))
        .await
        .expect("Failed to save URL with expiry");

    assert!(model.expires_at.is_some());
    assert_eq!(model.expires_at.unwrap().timestamp(), expires_at.timestamp());
}

#[tokio::test]
async fn test_is_url_expired() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    let past = (Utc::now() - chrono::Duration::hours(1)).fixed_offset();
    let expired_model = service
        .save_short_url("https://expired.com", "expired", Some(past))
        .await
        .expect("Failed to save expired URL");
    assert!(DbService::is_url_expired(&expired_model));

    let no_expiry_model = service
        .save_short_url("https://permanent.com", "perm", None)
        .await
        .expect("Failed to save permanent URL");
    assert!(!DbService::is_url_expired(&no_expiry_model));

    let future = (Utc::now() + chrono::Duration::hours(24)).fixed_offset();
    let future_model = service
        .save_short_url("https://future.com", "future", Some(future))
        .await
        .expect("Failed to save future URL");
    assert!(!DbService::is_url_expired(&future_model));
}

#[tokio::test]
async fn test_record_click() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    service.save_short_url("https://example.com", "clicktest", None).await.expect("Failed to save");

    let click = service
        .record_click("clicktest", Some("127.0.0.1"), Some("test-agent"), Some("https://referer.com"))
        .await
        .expect("Failed to record click");

    assert_eq!(click.short_code, "clicktest");
    assert_eq!(click.ip_address, Some("127.0.0.1".to_string()));
    assert_eq!(click.user_agent, Some("test-agent".to_string()));
    assert_eq!(click.referer, Some("https://referer.com".to_string()));
}

#[tokio::test]
async fn test_get_click_stats() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    service.save_short_url("https://example.com", "stats", None).await.expect("Failed to save");

    for _ in 0..3 {
        service.record_click("stats", None, None, None).await.expect("Failed to record click");
    }

    let clicks = service.get_click_stats("stats").await.expect("Failed to get stats");
    assert_eq!(clicks.len(), 3);
}

#[tokio::test]
async fn test_get_all_urls() {
    let db = setup_test_db().await;
    let service = DbService::new(db);

    service.save_short_url("https://a.com", "aaa", None).await.expect("Failed to save");
    service.save_short_url("https://b.com", "bbb", None).await.expect("Failed to save");

    let urls = service.get_all_urls().await.expect("Failed to get all URLs");
    assert_eq!(urls.len(), 2);
}
