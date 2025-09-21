use crate::services::{cache::CacheService, db::DbService};
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::ToSchema;
use validator::Validate;

#[derive(Deserialize, Validate, ToSchema)]
pub struct ShortenRequest {
    #[validate(url)]
    pub url: String,
}

#[derive(Serialize, ToSchema)]
pub struct ShortenResponse {
    pub short_url: String,
}

#[utoipa::path(
    post,
    path = "/",
    responses(
        (status = 200, description = "url shortened", body = ShortenResponse)
    )
)]
#[instrument(skip(db_service, cache_service, req, http_req), fields(url = %req.url))]
#[post("/")]
pub async fn shorten_url(
    db_service: web::Data<DbService>,
    cache_service: web::Data<CacheService>,
    req: web::Json<ShortenRequest>,
    http_req: HttpRequest,
) -> impl Responder {
    // Validate the request payload
    if let Err(e) = req.validate() {
        error!("Validation failed: {}", e);
        return HttpResponse::BadRequest().json(e.to_string());
    }

    // Generate a unique short code.
    // A production app would handle collisions, but it's very unlikely with nanoid.
    let short_code = nanoid!(8);

    match db_service.save_short_url(&req.url, &short_code).await {
        Ok(model) => {
            info!("Successfully created short code: {}", short_code);
            cache_service.set(model.short_code.clone(), model.long_url.clone());

            let scheme = http_req.connection_info().scheme().to_string();
            let host = http_req.connection_info().host().to_string();
            let short_url = format!("{}://{}/{}", scheme, host, model.short_code);

            HttpResponse::Ok().json(ShortenResponse { short_url })
        }
        Err(e) => {
            error!("Failed to save URL to database: {}", e);
            HttpResponse::InternalServerError().json("Failed to create short URL")
        }
    }
}

#[utoipa::path(
    get,
    path = "/{short_code}",
    responses(
        (status = 301, description = "redirection to the main url")
    )
)]
#[instrument(skip(db_service, cache_service), fields(short_code = %short_code))]
#[get("/{short_code}")]
pub async fn redirect_to_long_url(
    db_service: web::Data<DbService>,
    cache_service: web::Data<CacheService>,
    short_code: web::Path<String>,
) -> impl Responder {
    let code = short_code.into_inner();

    // 1. Check cache first
    if let Some(long_url) = cache_service.get(&code) {
        info!("Cache HIT");
        return HttpResponse::Found()
            .append_header(("Location", long_url))
            .finish();
    }
    info!("Cache MISS");

    // 2. If not in cache, check database
    match db_service.find_url_by_short_code(&code).await {
        Ok(Some(model)) => {
            // 3. Cache the result for future requests
            cache_service.set(model.short_code.clone(), model.long_url.clone());
            info!("DB HIT. Caching result.");
            HttpResponse::Found()
                .append_header(("Location", model.long_url))
                .finish()
        }
        Ok(None) => {
            info!("Short code NOT FOUND in DB");
            HttpResponse::NotFound().body("URL not found")
        }
        Err(e) => {
            error!("Database error on redirect: {}", e);
            HttpResponse::InternalServerError().body("An error occurred")
        }
    }
}
