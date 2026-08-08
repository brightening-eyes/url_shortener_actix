use actix_web::{HttpResponse, Responder, get};
use serde::Serialize;
use tracing::instrument;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "service is healthy", body = HealthResponse)
    )
)]
#[instrument]
#[get("/health")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse {
        status: "ok".to_string(),
    })
}
