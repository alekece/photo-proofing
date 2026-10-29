//! Gallery HTTP API: route wiring, authentication state and the [`server::Api`] runner.

use actix_web::web;

pub mod credentials;
pub mod routes;
pub mod server;
pub mod state;

/// Mounts every HTTP route under `/api/v1`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(routes::health::report_health)
            .configure(routes::auth::configure),
    );
}
