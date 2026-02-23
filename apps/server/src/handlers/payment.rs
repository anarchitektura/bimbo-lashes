use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use std::sync::Arc;

use crate::{models::*, AppState};

/// Payment expiry timeout (minutes).
const PAYMENT_EXPIRY_MINUTES: i32 = 15;

/// YooKassa allowed IP prefixes (for future webhook validation).
///
/// Ranges: 185.71.76.0/27, 185.71.77.0/27, 77.75.153.0/25, 77.75.154.128/25, 77.75.156.35
#[allow(dead_code)]
const YOOKASSA_IP_PREFIXES: &[&str] = &[
    "185.71.76.",
    "185.71.77.",
    "77.75.153.",
    "77.75.154.",
    "77.75.156.35",
];

/// Validate that a request comes from YooKassa IP range.
#[allow(dead_code)]
fn is_yookassa_ip(ip: &str) -> bool {
    for prefix in YOOKASSA_IP_PREFIXES {
        if ip.starts_with(prefix) {
            return true;
        }
    }
    ip == "127.0.0.1" || ip == "::1"
}

/// Create a payment in YooKassa.
///
/// Returns `(payment_id, confirmation_url)` on success.
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
        .ok_or_else(|| anyhow::anyhow!("Missing payment id in YooKassa response"))?
        .to_string();

    let confirmation_url = json["confirmation"]["confirmation_url"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing confirmation URL in YooKassa response"))?
        .to_string();

    tracing::info!(
        booking_id,
        payment_id = %payment_id,
        "YooKassa payment created"
    );

    Ok((payment_id, confirmation_url))
}

/// Create a refund in YooKassa.
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

    tracing::info!(payment_id, "YooKassa refund created");
    Ok(())
}

/// POST /api/payments/webhook — handle YooKassa webhook notifications.
pub async fn payment_webhook(
    State(state): State<Arc<AppState>>,
    _headers: axum::http::HeaderMap,
    Json(event): Json<YooKassaWebhookEvent>,
) -> StatusCode {
    tracing::info!(
        event = %event.event,
        payment_id = %event.object.id,
        status = %event.object.status,
        "YooKassa webhook received"
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
            return StatusCode::OK;
        }
    };

    match event.event.as_str() {
        "payment.succeeded" => {
            tracing::info!(booking_id, "Payment succeeded");

            let result = sqlx::query(
                "UPDATE bookings SET status = 'confirmed', payment_status = 'paid'
                 WHERE id = ? AND status = 'pending_payment'",
            )
            .bind(booking_id)
            .execute(&state.db)
            .await;

            if let Err(e) = result {
                tracing::error!(booking_id, error = %e, "Failed to update booking");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }

            // Notify admin about successful payment
            if let Some(booking) = fetch_booking(&state.db, booking_id).await {
                let mention = booking
                    .client_username
                    .as_ref()
                    .map(|u| format!("@{}", u))
                    .unwrap_or_else(|| booking.client_first_name.clone());

                let service_name: String =
                    sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
                        .bind(booking.service_id)
                        .fetch_optional(&state.db)
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "?".into());

                let b_date = booking.date.as_deref().unwrap_or("?");
                let b_start = booking.start_time.as_deref().unwrap_or("?");
                let b_end = booking.end_time.as_deref().unwrap_or("?");
                let addon_text = if booking.with_lower_lashes {
                    "\n   + нижние ресницы"
                } else {
                    ""
                };

                let message = format!(
                    "📋 Новая запись! 💳 Оплачено\n\n\
                     👤 {}\n\
                     💅 {}{}\n\
                     📅 {} в {} — {}\n\
                     💰 Предоплата {} ₽",
                    mention, service_name, addon_text, b_date, b_start, b_end,
                    booking.prepaid_amount
                );

                super::client::notify_admin(&state.bot_token, state.admin_tg_id, &message).await;
            }
        }

        "payment.canceled" => {
            tracing::info!(booking_id, "Payment canceled");

            if let Err(e) = sqlx::query(
                "UPDATE bookings SET status = 'expired', payment_status = 'none'
                 WHERE id = ? AND status = 'pending_payment'",
            )
            .bind(booking_id)
            .execute(&state.db)
            .await
            {
                tracing::error!(booking_id, error = %e, "Failed to expire booking");
            }

            if let Err(e) = sqlx::query(
                "UPDATE available_slots SET is_booked = 0, booking_id = NULL
                 WHERE booking_id = ?",
            )
            .bind(booking_id)
            .execute(&state.db)
            .await
            {
                tracing::error!(booking_id, error = %e, "Failed to free slots");
            }
        }

        other => {
            tracing::debug!(event = other, "Ignoring webhook event");
        }
    }

    StatusCode::OK
}

/// Expire pending_payment bookings older than the timeout.
pub async fn expire_pending_payments(db: &sqlx::SqlitePool) {
    let expired_ids: Vec<i64> = match sqlx::query_scalar(&format!(
        "SELECT id FROM bookings
         WHERE status = 'pending_payment'
         AND datetime(created_at, '+{} minutes') < datetime('now', '+3 hours')",
        PAYMENT_EXPIRY_MINUTES
    ))
    .fetch_all(db)
    .await
    {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!("expire_pending_payments query failed: {}", e);
            return;
        }
    };

    if expired_ids.is_empty() {
        return;
    }

    tracing::info!(count = expired_ids.len(), "Expiring unpaid bookings");

    for booking_id in expired_ids {
        tracing::info!(booking_id, "Expiring unpaid booking");

        if let Err(e) = sqlx::query(
            "UPDATE bookings SET status = 'expired', payment_status = 'none'
             WHERE id = ? AND status = 'pending_payment'",
        )
        .bind(booking_id)
        .execute(db)
        .await
        {
            tracing::error!(booking_id, error = %e, "Failed to expire booking");
        }

        if let Err(e) = sqlx::query(
            "UPDATE available_slots SET is_booked = 0, booking_id = NULL
             WHERE booking_id = ?",
        )
        .bind(booking_id)
        .execute(db)
        .await
        {
            tracing::error!(booking_id, error = %e, "Failed to free slots");
        }
    }
}

/// Fetch a booking by ID.
async fn fetch_booking(db: &sqlx::SqlitePool, booking_id: i64) -> Option<Booking> {
    sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE id = ?")
        .bind(booking_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
}
