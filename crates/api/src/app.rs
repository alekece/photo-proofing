use actix_web::web;

use crate::{auth, health};

/// Mounts every HTTP route under `/api/v1`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .route("/health", web::get().to(health::report_health))
            .configure(auth::configure),
    );
}
