mod auth;
mod db;
mod handlers;
mod models;
mod rate_limit;
mod telegram_layer;

use axum::{
    middleware::from_fn_with_state,
    routing::{delete, get, post, put},
    Router,
};
use sqlx::sqlite::SqlitePoolOptions;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use rate_limit::{
    rate_limit_admin, rate_limit_auth, rate_limit_booking, rate_limit_public, RateLimitConfig,
    RateLimiter,
};

/// Shared application state accessible from all handlers.
pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub bot_token: String,
    pub admin_tg_id: i64,
    pub started_at: Instant,
    pub yookassa_shop_id: String,
    pub yookassa_secret_key: String,
    pub webapp_url: String,
}

/// Payment expiry check interval (seconds).
const PAYMENT_EXPIRY_INTERVAL_SECS: u64 = 300;
/// Rate limit cleanup interval (seconds).
const RATE_LIMIT_CLEANUP_SECS: u64 = 300;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // ── Required env vars (read before tracing so TelegramLayer can use them) ──
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:bimbo.db?mode=rwc".into());
    let bot_token = std::env::var("BOT_TOKEN").expect("BOT_TOKEN must be set");
    let admin_tg_id: i64 = std::env::var("ADMIN_TG_ID")
        .expect("ADMIN_TG_ID must be set")
        .parse()
        .expect("ADMIN_TG_ID must be a number");

    // ── Tracing: console + optional Telegram error notifications ──
    let env_filter = EnvFilter::from_default_env().add_directive("info".parse()?);
    let fmt_layer = tracing_subscriber::fmt::layer();
    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);

    if !bot_token.is_empty() {
        let tg_layer = telegram_layer::TelegramLayer::new(bot_token.clone(), admin_tg_id);
        registry.with(tg_layer).init();
    } else {
        registry.init();
    }
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());

    // ── Optional env vars ──
    let yookassa_shop_id = std::env::var("YOOKASSA_SHOP_ID").unwrap_or_default();
    let yookassa_secret_key = std::env::var("YOOKASSA_SECRET_KEY").unwrap_or_default();
    let webapp_url =
        std::env::var("WEBAPP_URL").unwrap_or_else(|_| "https://example.com".into());

    if yookassa_shop_id.is_empty() {
        tracing::warn!("YOOKASSA_SHOP_ID not set — payments will fail");
    }

    // ── Database ──
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    db::run_migrations(&pool).await?;

    let state = Arc::new(AppState {
        db: pool,
        bot_token,
        admin_tg_id,
        started_at: Instant::now(),
        yookassa_shop_id,
        yookassa_secret_key,
        webapp_url: webapp_url.clone(),
    });

    // ── Background task: expire unpaid bookings ──
    let expire_db = state.db.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(
            PAYMENT_EXPIRY_INTERVAL_SECS,
        ));
        loop {
            interval.tick().await;
            handlers::payment::expire_pending_payments(&expire_db).await;
        }
    });

    // ── Rate limiter ──
    let rate_limiter = RateLimiter::new();
    rate_limiter.add_tier(
        "public",
        RateLimitConfig {
            max_requests: 60,
            window: Duration::from_secs(60),
        },
    );
    rate_limiter.add_tier(
        "auth",
        RateLimitConfig {
            max_requests: 30,
            window: Duration::from_secs(60),
        },
    );
    rate_limiter.add_tier(
        "booking",
        RateLimitConfig {
            max_requests: 5,
            window: Duration::from_secs(300),
        },
    );
    rate_limiter.add_tier(
        "admin",
        RateLimitConfig {
            max_requests: 120,
            window: Duration::from_secs(60),
        },
    );

    // ── Background task: cleanup stale rate limit entries ──
    let cleanup_limiter = rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(RATE_LIMIT_CLEANUP_SECS));
        loop {
            interval.tick().await;
            cleanup_limiter.cleanup();
        }
    });

    // ── CORS: whitelist WEBAPP_URL when configured, otherwise allow any ──
    let cors = if webapp_url != "https://example.com" {
        let origins: Vec<axum::http::HeaderValue> = vec![
            webapp_url.parse().expect("WEBAPP_URL must be a valid URL"),
            "http://localhost:5173".parse().unwrap(), // Vite dev server
        ];
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    // ── Router (5 groups with per-group rate limits) ──

    // 1. No-limit: health checks + payment webhooks
    let no_limit_routes = Router::new()
        .route("/api/health", get(handlers::health::health))
        .route(
            "/api/payments/webhook",
            post(handlers::payment::payment_webhook),
        );

    // 2. Public: read-only endpoints (no auth, 60 req/min)
    let public_routes = Router::new()
        .route("/api/services", get(handlers::client::list_services))
        .route("/api/addon-info", get(handlers::client::addon_info))
        .route(
            "/api/available-dates",
            get(handlers::client::available_dates_for_service),
        )
        .route(
            "/api/available-times",
            get(handlers::client::available_times),
        )
        .route("/api/calendar", get(handlers::client::calendar))
        .route(
            "/api/slots/dates",
            get(handlers::client::available_dates_for_service),
        )
        .layer(from_fn_with_state(rate_limiter.clone(), rate_limit_public));

    // 3. Booking creation: strictest limit (5 req/5min)
    let booking_routes = Router::new()
        .route("/api/bookings", post(handlers::client::create_booking))
        .layer(from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_booking,
        ));

    // 4. Auth: authenticated client endpoints (30 req/min)
    let auth_routes = Router::new()
        .route("/api/bookings/my", get(handlers::client::my_bookings))
        .route(
            "/api/bookings/{id}",
            delete(handlers::client::cancel_booking),
        )
        .route(
            "/api/bookings/{id}/status",
            get(handlers::client::booking_status),
        )
        .layer(from_fn_with_state(rate_limiter.clone(), rate_limit_auth));

    // 5. Admin: all admin endpoints (120 req/min)
    let admin_routes = Router::new()
        .route(
            "/api/admin/services",
            get(handlers::admin::list_all_services),
        )
        .route(
            "/api/admin/services",
            post(handlers::admin::create_service),
        )
        .route(
            "/api/admin/services/{id}",
            put(handlers::admin::update_service),
        )
        .route("/api/admin/slots", get(handlers::admin::list_slots))
        .route("/api/admin/slots", post(handlers::admin::create_slots))
        .route(
            "/api/admin/slots/{id}",
            delete(handlers::admin::delete_slot),
        )
        .route("/api/admin/openday", post(handlers::admin::open_day))
        .route(
            "/api/admin/bookings",
            get(handlers::admin::list_bookings),
        )
        .route(
            "/api/admin/bookings/{id}/cancel",
            post(handlers::admin::cancel_booking),
        )
        .layer(from_fn_with_state(rate_limiter.clone(), rate_limit_admin));

    let app = Router::new()
        .merge(no_limit_routes)
        .merge(public_routes)
        .merge(booking_routes)
        .merge(auth_routes)
        .merge(admin_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    tracing::info!("Bimbo Lashes server starting on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}
