use actix_web::web;

use crate::health;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api/v1").route("/health", web::get().to(health::health)));
}
