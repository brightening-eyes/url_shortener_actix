pub mod entities;
pub mod routes;
pub mod services;

use utoipa::OpenApi;
use utoipauto::utoipauto;

#[utoipauto]
#[derive(OpenApi)]
#[openapi(
    info(
        title = "url shortener service",
        description = "url shortener service with actix web.",
        version = "1.0.0"
    ),
)]
pub struct ApiDoc;
