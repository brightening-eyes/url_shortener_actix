use actix_cors::Cors;
use actix_governor::{Governor, GovernorConfig};
use actix_web::{App, HttpServer, web};
use mimalloc::MiMalloc;
use url_shortener::routes::{health::health_check, url::redirect_to_long_url, url::shorten_url, url::shorten_url_custom};
use url_shortener::services::{cache::CacheService, db::DbService, db::establish_connection};
use url_shortener::ApiDoc;
use utoipa::OpenApi;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;
use utoipa_swagger_ui::SwaggerUi;

#[global_allocator]
static GLOBAL_ALLOCATOR: MiMalloc = MiMalloc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let database_url = std::env::var("DATABASE_URL").expect("failed to retreive database url");
    let server_addr =
        std::env::var("SERVER_ADDR").expect("failed to obtain the server address to bind to.");
    let db_conn = establish_connection(&database_url)
        .await
        .expect("failed to connect to database");
    info!("connected to database successfully");
    let db_service = web::Data::new(DbService::new(db_conn));
    let cache_service = web::Data::new(CacheService::new(std::time::Duration::from_secs(3600)));

    // Warm up cache with existing URLs from database
    match db_service.get_all_urls().await {
        Ok(urls) => {
            info!("Warming up cache with {} URLs", urls.len());
            for url_model in &urls {
                let expires_at_ts = url_model.expires_at.map(|dt| dt.timestamp());
                cache_service.set_with_expiry(url_model.short_code.clone(), url_model.long_url.clone(), expires_at_ts);
            }
        }
        Err(e) => error!("Failed to load URLs for cache warm-up: {}", e),
    }

    info!("Starting server at http://{}", &server_addr);
    let openapi = ApiDoc::openapi();
    let governor_conf = GovernorConfig::default();
    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .wrap(Governor::new(&governor_conf))
            .service(health_check)
            .service(
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi.clone()),
            )
            .app_data(db_service.clone())
            .app_data(cache_service.clone())
            .service(shorten_url)
            .service(shorten_url_custom)
            .service(redirect_to_long_url)
    })
    .bind(server_addr)?
    .run()
    .await
}
