use actix_web::{HttpResponse, Responder};

pub async fn report_health() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}
