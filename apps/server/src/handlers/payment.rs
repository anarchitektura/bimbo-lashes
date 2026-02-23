use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{
    models::*,
    AppState,
};

/// YooKassa allowed IP ranges (for webhook validation)
#[allow(dead_code)]
const YOOKASSA_IPS: &[&str] = &[
    // 185.71.76.0/27
    "185.71.76.", // 0-31
    // 185.71.77.0/27
    "185.71.77.", // 0-31
    // 77.75.153.0/25
    "77.75.153.", // 0-127
    // 77.75.154.128/25
    "77.75.154.", // 128-255
    // 77.75.156.35
    "77.75.156.35",
];

/// Validate that request comes from YooKassa IP
#[allow(dead_code)]
fn is_yookassa_ip(ip: &str) -> bool {
    // In production, validate against exact CIDR ranges
    // For now, prefix-based check is sufficient
    for prefix in YOOKASSA_IPS {
        if ip.starts_with(prefix) {
            return true;
        }
    }
    // Allow localhost for testing
    ip == "127.0.0.1" || ip == "::1"
}

/// Create a payment in YooKassa
pub async fn create_yookassa_payment(
    shop_id: &str,
    secret_key: &str,
    booking_id: i64,
    amount: i64,
    description: &str,
    return_url: &str,
) -> anyhow::Result<(String, String)> {
    let client = reqwest::Client::new();

    let idempotence_key = format!(
        "booking-{}-{}",
        booking_id,
        chrono::Utc::now().timestamp_millis()
    );

    let body = serde_json::json!({
        "amount": {
            "value": format!("{}.00", amount),
            "currency": "RUB"
        },
        "capture": true,
        "confirmation": {
            "type": "redirect",
            "return_url": return_url
        },
        "description": description,
        "metadata": {
            "booking_id": booking_id.to_string()
        }
    });

    let resp = client
        .post("https://api.yookassa.ru/v3/payments")
        .basic_auth(shop_id, Some(secret_key))
        .header("Idempotence-Key", &idempotence_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::error!("YooKassa payment creation failed: {} - {}", status, text);
        anyhow::bail!("YooKassa API error: {}", status);
    }

    let json: serde_json::Value = resp.json().await?;

    let payment_id = json["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing payment id"))?
        .to_string();

    let confirmation_url = json["confirmation"]["confirmation_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing confirmation URL"))?
        .to_string();

    tracing::info!(
        "YooKassa payment created: {} for booking {}",
        payment_id,
        booking_id
    );

    Ok((payment_id, confirmation_url))
}

/// Create a refund in YooKassa
pub async fn create_yookassa_refund(
    shop_id: &str,
    secret_key: &str,
    payment_id: &str,
    amount: i64,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let idempotence_key = format!(
        "refund-{}-{}",
        payment_id,
        chrono::Utc::now().timestamp_millis()
    );

    let body = serde_json::json!({
        "payment_id": payment_id,
        "amount": {
            "value": format!("{}.00", amount),
            "currency": "RUB"
        }
    });

    let resp = client
        .post("https://api.yookassa.ru/v3/refunds")
        .basic_auth(shop_id, Some(secret_key))
        .header("Idempotence-Key", &idempotence_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        tracing::error!("YooKassa refund failed: {} - {}", status, text);
        anyhow::bail!("YooKassa refund error: {}", status);
    }

    tracing::info!("YooKassa refund created for payment {}", payment_id);
    Ok(())
}

/// POST /api/payments/webhook — handle YooKassa webhook notifications
pub async fn payment_webhook(
    State(state): State<Arc<AppState>>,
    _headers: axum::http::HeaderMap,
    Json(event): Json<YooKassaWebhookEvent>,
) -> StatusCode {
    // Log webhook event
    tracing::info!(
        "YooKassa webhook: event={}, payment_id={}, status={}",
        event.event,
        event.object.id,
        event.object.status
    );

    // Extract booking_id from metadata
    let booking_id: i64 = match event
        .object
        .metadata
        .as_ref()
        .and_then(|m| m.get("booking_id"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
    {
        Some(id) => id,
        None => {
            tracing::warn!("Webhook missing booking_id in metadata");
            return StatusCode::OK; // Return 200 to prevent retries
        }
    };

    match event.event.as_str() {
        "payment.succeeded" => {
            tracing::info!("Payment succeeded for booking {}", booking_id);

            // Update booking status
            let result = sqlx::query(
                "UPDATE bookings SET status = 'confirmed', payment_status = 'paid'
                 WHERE id = ? AND status = 'pending_payment'"
            )
            .bind(booking_id)
            .execute(&state.db)
            .await;

            if let Err(e) = result {
                tracing::error!("Failed to update booking {}: {}", booking_id, e);
                return StatusCode::INTERNAL_SERVER_ERROR;
            }

            // Get booking details for notification
            let booking = sqlx::query_as::<_, Booking>(
                "SELECT * FROM bookings WHERE id = ?"
            )
            .bind(booking_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some(booking) = booking {
                // Get service name
                let service_name: Option<String> = sqlx::query_scalar(
                    "SELECT name FROM services WHERE id = ?"
                )
                .bind(booking.service_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();

                let mention = booking
                    .client_username
                    .as_ref()
                    .map(|u| format!("@{}", u))
                    .unwrap_or_else(|| booking.client_first_name.clone());

                let b_date = booking.date.as_deref().unwrap_or("?");
                let b_start = booking.start_time.as_deref().unwrap_or("?");
                let b_end = booking.end_time.as_deref().unwrap_or("?");
                let svc = service_name.as_deref().unwrap_or("?");

                let addon_text = if booking.with_lower_lashes { "\n   + нижние ресницы" } else { "" };
                let message = format!(
                    "📋 Новая запись! 💳 Оплачено\n\n\
                     👤 {}\n\
                     💅 {}{}\n\
                     📅 {} в {} — {}\n\
                     💰 Предоплата {} ₽",
                    mention, svc, addon_text, b_date, b_start, b_end, booking.prepaid_amount
                );

                notify_admin(&state.bot_token, state.admin_tg_id, &message).await;
            }
        }

        "payment.canceled" => {
            tracing::info!("Payment canceled for booking {}", booking_id);

            // Expire the booking
            sqlx::query(
                "UPDATE bookings SET status = 'expired', payment_status = 'none'
                 WHERE id = ? AND status = 'pending_payment'"
            )
            .bind(booking_id)
            .execute(&state.db)
            .await
            .ok();

            // Free slots
            sqlx::query(
                "UPDATE available_slots SET is_booked = 0, booking_id = NULL
                 WHERE booking_id = ?"
            )
            .bind(booking_id)
            .execute(&state.db)
            .await
            .ok();
        }

        _ => {
            tracing::info!("Ignoring webhook event: {}", event.event);
        }
    }

    StatusCode::OK
}

/// Expire pending_payment bookings older than 15 minutes
pub async fn expire_pending_payments(db: &sqlx::SqlitePool) {
    // Find expired bookings
    let expired_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM bookings
         WHERE status = 'pending_payment'
         AND datetime(created_at, '+15 minutes') < datetime('now', '+3 hours')"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for booking_id in expired_ids {
        tracing::info!("Expiring unpaid booking {}", booking_id);

        sqlx::query(
            "UPDATE bookings SET status = 'expired', payment_status = 'none'
             WHERE id = ? AND status = 'pending_payment'"
        )
        .bind(booking_id)
        .execute(db)
        .await
        .ok();

        // Free slots
        sqlx::query(
            "UPDATE available_slots SET is_booked = 0, booking_id = NULL
             WHERE booking_id = ?"
        )
        .bind(booking_id)
        .execute(db)
        .await
        .ok();
    }
}

/// Send a message to admin via Telegram Bot API
async fn notify_admin(bot_token: &str, chat_id: i64, text: &str) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    let _ = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML"
        }))
        .send()
        .await;
}
