use crate::services::{cache::CacheService, db::DbService};
use actix_web::{HttpRequest, HttpResponse, Responder, get, post, web};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::ToSchema;
use validator::Validate;

const MAX_URL_LENGTH: usize = 2048;
const MAX_SHORT_CODE_RETRIES: u32 = 5;
const CUSTOM_SHORT_CODE_MIN: usize = 3;
const CUSTOM_SHORT_CODE_MAX: usize = 32;

fn is_unique_violation(err: &sea_orm::DbErr) -> bool {
    err.to_string().contains("UNIQUE constraint")
}

fn expires_at_from_seconds(seconds: Option<u64>) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    seconds.map(|secs| {
        let utc_now = chrono::Utc::now();
        let expiry = utc_now + chrono::Duration::seconds(secs as i64);
        expiry.fixed_offset()
    })
}

fn unix_timestamp_from_datetime(dt: chrono::DateTime<chrono::FixedOffset>) -> i64 {
    dt.timestamp()
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct ShortenRequest {
    #[validate(url)]
    pub url: String,
    pub expires_in_seconds: Option<u64>,
}

#[derive(Serialize, ToSchema)]
pub struct ShortenResponse {
    pub short_url: String,
}

#[derive(Deserialize, Validate, ToSchema)]
pub struct CustomShortenRequest {
    #[validate(url)]
    pub url: String,
    pub short_code: String,
    pub expires_in_seconds: Option<u64>,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
}

fn is_valid_custom_code(code: &str) -> bool {
    let len = code.len();
    (CUSTOM_SHORT_CODE_MIN..=CUSTOM_SHORT_CODE_MAX).contains(&len)
        && code.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

#[utoipa::path(
    post,
    path = "/custom",
    responses(
        (status = 200, description = "url shortened with custom code", body = ShortenResponse),
        (status = 400, description = "invalid request", body = ErrorResponse),
        (status = 409, description = "short code already taken", body = ErrorResponse),
    )
)]
#[instrument(skip(db_service, cache_service, req, http_req), fields(url = %req.url, custom_code = %req.short_code))]
#[post("/custom")]
pub async fn shorten_url_custom(
    db_service: web::Data<DbService>,
    cache_service: web::Data<CacheService>,
    req: web::Json<CustomShortenRequest>,
    http_req: HttpRequest,
) -> impl Responder {
    if let Err(e) = req.validate() {
        error!("Validation failed: {}", e);
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: e.to_string(),
        });
    }

    if req.url.len() > MAX_URL_LENGTH {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!("URL exceeds maximum length of {} characters", MAX_URL_LENGTH),
        });
    }

    if !is_valid_custom_code(&req.short_code) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!(
                "Short code must be {}-{} alphanumeric characters (underscores and hyphens allowed)",
                CUSTOM_SHORT_CODE_MIN, CUSTOM_SHORT_CODE_MAX
            ),
        });
    }

    match db_service.short_code_exists(&req.short_code).await {
        Ok(true) => {
            return HttpResponse::Conflict().json(ErrorResponse {
                error: "Short code already taken".to_string(),
            });
        }
        Err(e) => {
            error!("Failed to check short code existence: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to create short URL".to_string(),
            });
        }
        _ => {}
    }

    let expires_at = expires_at_from_seconds(req.expires_in_seconds);
    let expires_at_ts = expires_at.map(unix_timestamp_from_datetime);

    match db_service.save_short_url(&req.url, &req.short_code, expires_at).await {
        Ok(model) => {
            info!("Successfully created custom short code: {}", model.short_code);
            cache_service.set_with_expiry(model.short_code.clone(), model.long_url.clone(), expires_at_ts);

            let scheme = http_req.connection_info().scheme().to_string();
            let host = http_req.connection_info().host().to_string();
            let short_url = format!("{}://{}/{}", scheme, host, model.short_code);

            HttpResponse::Ok().json(ShortenResponse { short_url })
        }
        Err(e) => {
            error!("Failed to save custom short URL: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Failed to create short URL".to_string(),
            })
        }
    }
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
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: e.to_string(),
        });
    }

    if req.url.len() > MAX_URL_LENGTH {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: format!("URL exceeds maximum length of {} characters", MAX_URL_LENGTH),
        });
    }

    let expires_at = expires_at_from_seconds(req.expires_in_seconds);
    let expires_at_ts = expires_at.map(unix_timestamp_from_datetime);

    for attempt in 1..=MAX_SHORT_CODE_RETRIES {
        let short_code = nanoid!(8);

        match db_service.save_short_url(&req.url, &short_code, expires_at.clone()).await {
            Ok(model) => {
                info!("Successfully created short code: {}", short_code);
                cache_service.set_with_expiry(model.short_code.clone(), model.long_url.clone(), expires_at_ts);

                let scheme = http_req.connection_info().scheme().to_string();
                let host = http_req.connection_info().host().to_string();
                let short_url = format!("{}://{}/{}", scheme, host, model.short_code);

                return HttpResponse::Ok().json(ShortenResponse { short_url });
            }
            Err(e) if is_unique_violation(&e) && attempt < MAX_SHORT_CODE_RETRIES => {
                info!("Short code collision, retrying (attempt {}/{})", attempt, MAX_SHORT_CODE_RETRIES);
            }
            Err(e) => {
                error!("Failed to save URL to database: {}", e);
                return HttpResponse::InternalServerError().json("Failed to create short URL");
            }
        }
    }

    error!("Exhausted {} retries for short code generation", MAX_SHORT_CODE_RETRIES);
    HttpResponse::InternalServerError().json("Failed to create short URL")
}

#[utoipa::path(
    get,
    path = "/{short_code}",
    responses(
        (status = 301, description = "redirection to the main url")
    )
)]
#[instrument(skip(db_service, cache_service, http_req), fields(short_code = %short_code))]
#[get("/{short_code}")]
pub async fn redirect_to_long_url(
    db_service: web::Data<DbService>,
    cache_service: web::Data<CacheService>,
    http_req: HttpRequest,
    short_code: web::Path<String>,
) -> impl Responder {
    let code = short_code.into_inner();

    let ip = http_req.peer_addr().map(|addr| addr.ip().to_string());
    let user_agent = http_req
        .headers()
        .get("User-Agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());
    let referer = http_req
        .headers()
        .get("Referer")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned());

    // 1. Check cache first
    if let Some(long_url) = cache_service.get(&code) {
        info!("Cache HIT");
        let _ = db_service
            .record_click(&code, ip.as_deref(), user_agent.as_deref(), referer.as_deref())
            .await
            .map_err(|e| error!("Failed to record click: {}", e));
        return HttpResponse::MovedPermanently()
            .append_header(("Location", long_url))
            .finish();
    }
    info!("Cache MISS");

    // 2. If not in cache, check database
    match db_service.find_url_by_short_code(&code).await {
        Ok(Some(model)) => {
            if DbService::is_url_expired(&model) {
                info!("Short code {} has expired", code);
                return HttpResponse::Gone().body("URL has expired");
            }
            // 3. Cache the result for future requests (with expiry if present)
            let expires_at_ts = model.expires_at.map(unix_timestamp_from_datetime);
            cache_service.set_with_expiry(model.short_code.clone(), model.long_url.clone(), expires_at_ts);
            info!("DB HIT. Caching result.");
            let _ = db_service
                .record_click(&code, ip.as_deref(), user_agent.as_deref(), referer.as_deref())
                .await
                .map_err(|e| error!("Failed to record click: {}", e));
            HttpResponse::MovedPermanently()
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

