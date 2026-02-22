mod auth;
mod db;
mod handlers;
mod models;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub bot_token: String,
    pub admin_tg_id: i64,
    pub started_at: Instant,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:bimbo.db?mode=rwc".into());
    let bot_token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN must be set");
    let admin_tg_id: i64 = std::env::var("ADMIN_TG_ID")
        .expect("ADMIN_TG_ID must be set")
        .parse()
        .expect("ADMIN_TG_ID must be a number");
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    db::run_migrations(&pool).await?;

    let state = Arc::new(AppState {
        db: pool,
        bot_token,
        admin_tg_id,
        started_at: Instant::now(),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        // Health check (no auth required)
        .route("/api/health", get(handlers::health::health))
        // Client endpoints
        .route("/api/services", get(handlers::client::list_services))
        .route("/api/addon-info", get(handlers::client::addon_info))
        .route("/api/available-dates", get(handlers::client::available_dates_for_service))
        .route("/api/available-times", get(handlers::client::available_times))
        .route("/api/calendar", get(handlers::client::calendar))
        .route("/api/slots/dates", get(handlers::client::available_dates_for_service)) // backward compat
        .route("/api/bookings", post(handlers::client::create_booking))
        .route("/api/bookings/my", get(handlers::client::my_bookings))
        .route("/api/bookings/{id}", delete(handlers::client::cancel_booking))
        // Admin endpoints
        .route("/api/admin/services", get(handlers::admin::list_all_services))
        .route("/api/admin/services", post(handlers::admin::create_service))
        .route("/api/admin/services/{id}", put(handlers::admin::update_service))
        .route("/api/admin/slots", get(handlers::admin::list_slots))
        .route("/api/admin/slots", post(handlers::admin::create_slots))
        .route("/api/admin/slots/{id}", delete(handlers::admin::delete_slot))
        .route("/api/admin/openday", post(handlers::admin::open_day))
        .route("/api/admin/bookings", get(handlers::admin::list_bookings))
        .route("/api/admin/bookings/{id}/cancel", post(handlers::admin::cancel_booking))
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    tracing::info!("🔮 Bimbo Lashes server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
