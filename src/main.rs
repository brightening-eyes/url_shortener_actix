mod entities;
mod routes;
mod services;

use mimalloc::MiMalloc;
use actix_web::{web, App, HttpServer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use utoipauto::utoipauto;
use tracing::info;
use tracing_subscriber::EnvFilter;
use routes::{url::shorten_url, url::redirect_to_long_url};
use services::{db::establish_connection, db::DbService, cache::CacheService};

// global allocator based on mimalloc
#[global_allocator]
static GLOBAL_ALLOCATOR: MiMalloc = MiMalloc;

#[utoipauto]
#[derive(OpenApi)]
#[openapi(
    // General API information
    info(
        title = "url shortener service",
        description = "url shortener service with actix web.",
        version = "1.0.0"
    ),
)]
struct ApiDoc;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
    let database_url = std::env::var("DATABASE_URL").expect("failed to retreive database url");
    let server_addr = std::env::var("SERVER_ADDR").expect("failed to obtain the server address to bind to.");
    let db_conn = establish_connection(&database_url).await.expect("failed to connect to database");
    info!("connected to database successfully");
    let db_service = web::Data::new(DbService::new(db_conn));
    let cache_service = web::Data::new(CacheService::new());
    info!("Starting server at http://{}", &server_addr);
    let openapi = ApiDoc::openapi();
    HttpServer::new(move || {
        App::new()
.service(SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi.clone()))
            .app_data(db_service.clone())
            .app_data(cache_service.clone())
            .service(shorten_url)
            .service(redirect_to_long_url)

    }).bind(server_addr)?.run().await
}
