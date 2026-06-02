use actix_web::{App, test, web};
use sea_orm::{ConnectionTrait, Database, Statement};
use url_shortener::routes::url::{redirect_to_long_url, shorten_url, shorten_url_custom};
use url_shortener::services::cache::CacheService;
use url_shortener::services::db::DbService;

async fn setup_db() -> sea_orm::DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.expect("Failed to connect");

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

macro_rules! init_test_app {
    ($db:expr) => {
        test::init_service(
            App::new()
                .app_data(web::Data::new(DbService::new($db)))
                .app_data(web::Data::new(CacheService::new(std::time::Duration::from_secs(60))))
                .service(shorten_url)
                .service(shorten_url_custom)
                .service(redirect_to_long_url)
        )
    };
}

#[actix_web::test]
async fn test_shorten_url_success() {
    let db = setup_db().await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(DbService::new(db)))
            .app_data(web::Data::new(CacheService::new(std::time::Duration::from_secs(60))))
            .service(shorten_url)
            .service(shorten_url_custom)
            .service(redirect_to_long_url)
    ).await;

    let req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"url": "https://example.com"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("short_url").and_then(|v| v.as_str()).is_some());
}

#[actix_web::test]
async fn test_shorten_url_validation_fails() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"url": "not-a-url"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_shorten_url_too_long() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let long_url = "https://example.com/".to_string() + &"a".repeat(2048);
    let req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"url": long_url}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_shorten_url_with_expiry() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"url": "https://example.com", "expires_in_seconds": 3600}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_redirect_success() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let create_req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"url": "https://example.com"}))
        .to_request();
    let create_resp: serde_json::Value = test::read_body_json(test::call_service(&app, create_req).await).await;
    let short_url = create_resp.get("short_url").and_then(|v| v.as_str()).unwrap().to_string();
    let short_code = short_url.rsplit('/').next().unwrap().to_string();

    let redirect_req = test::TestRequest::get()
        .uri(&format!("/{}", short_code))
        .to_request();
    let redirect_resp = test::call_service(&app, redirect_req).await;
    assert_eq!(redirect_resp.status(), 301);
    assert_eq!(
        redirect_resp.headers().get("Location").and_then(|v| v.to_str().ok()),
        Some("https://example.com")
    );
}

#[actix_web::test]
async fn test_redirect_not_found() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req = test::TestRequest::get()
        .uri("/nonexistent")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 404);
}

#[actix_web::test]
async fn test_custom_short_code_success() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req = test::TestRequest::post()
        .uri("/custom")
        .set_json(serde_json::json!({"url": "https://example.com", "short_code": "my-custom-link"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    let short_url = body.get("short_url").and_then(|v| v.as_str()).unwrap();
    assert!(short_url.contains("my-custom-link"));
}

#[actix_web::test]
async fn test_custom_short_code_taken() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req1 = test::TestRequest::post()
        .uri("/custom")
        .set_json(serde_json::json!({"url": "https://example.com", "short_code": "taken-code"}))
        .to_request();
    let resp1 = test::call_service(&app, req1).await;
    assert!(resp1.status().is_success());

    let req2 = test::TestRequest::post()
        .uri("/custom")
        .set_json(serde_json::json!({"url": "https://other.com", "short_code": "taken-code"}))
        .to_request();
    let resp2 = test::call_service(&app, req2).await;
    assert_eq!(resp2.status(), 409);
}

#[actix_web::test]
async fn test_custom_short_code_invalid_format() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req = test::TestRequest::post()
        .uri("/custom")
        .set_json(serde_json::json!({"url": "https://example.com", "short_code": "ab"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);
}

#[actix_web::test]
async fn test_custom_short_code_with_expiry() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let req = test::TestRequest::post()
        .uri("/custom")
        .set_json(serde_json::json!({"url": "https://example.com", "short_code": "custom-expiry", "expires_in_seconds": 3600}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_redirect_expired_url() {
    let db = setup_db().await;
    let app = init_test_app!(db).await;

    let create_req = test::TestRequest::post()
        .uri("/")
        .set_json(serde_json::json!({"url": "https://example.com", "expires_in_seconds": 0}))
        .to_request();
    let create_resp: serde_json::Value = test::read_body_json(test::call_service(&app, create_req).await).await;
    let short_code = create_resp.get("short_url")
        .and_then(|v| v.as_str())
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_string();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let redirect_req = test::TestRequest::get()
        .uri(&format!("/{}", short_code))
        .to_request();
    let redirect_resp = test::call_service(&app, redirect_req).await;
    assert_eq!(redirect_resp.status(), 410);
}
