use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    Json,
};
use std::sync::Arc;

use crate::{
    auth,
    models::*,
    AppState,
};

/// Helper: extract TelegramUser from Authorization header
fn extract_user(
    auth_header: Option<&str>,
    bot_token: &str,
) -> Result<TelegramUser, (StatusCode, Json<ApiResponse<()>>)> {
    let header = auth_header.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Missing Authorization header")),
        )
    })?;
    auth::extract_user_from_header(header, bot_token).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Invalid Telegram auth")),
        )
    })
}

/// GET /api/services — list all active services
pub async fn list_services(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<Service>>>, StatusCode> {
    let services = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order
         FROM services WHERE is_active = 1 ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(services)))
}

/// GET /api/slots/dates — list dates that have available (unbooked) slots
pub async fn available_dates(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let dates: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT date FROM available_slots
         WHERE is_booked = 0 AND date >= date('now')
         ORDER BY date ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(dates)))
}

/// GET /api/slots?date=YYYY-MM-DD — list available slots for a specific date
pub async fn slots_by_date(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SlotsQuery>,
) -> Result<Json<ApiResponse<Vec<AvailableSlot>>>, StatusCode> {
    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked
         FROM available_slots
         WHERE date = ? AND is_booked = 0
         ORDER BY start_time ASC"
    )
    .bind(&query.date)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(slots)))
}

/// POST /api/bookings — create a new booking
pub async fn create_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateBookingRequest>,
) -> Result<Json<ApiResponse<BookingDetail>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    // Check slot exists and is available
    let slot = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked FROM available_slots WHERE id = ?"
    )
    .bind(body.slot_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Слот не найден"))))?;

    if slot.is_booked {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::error("Этот слот уже занят")),
        ));
    }

    // Check service exists
    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order
         FROM services WHERE id = ? AND is_active = 1"
    )
    .bind(body.service_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Услуга не найдена"))))?;

    // Create booking
    let booking_id = sqlx::query(
        "INSERT INTO bookings (service_id, slot_id, client_tg_id, client_username, client_first_name, status)
         VALUES (?, ?, ?, ?, ?, 'confirmed')"
    )
    .bind(body.service_id)
    .bind(body.slot_id)
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.first_name)
    .execute(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .last_insert_rowid();

    // Mark slot as booked
    sqlx::query("UPDATE available_slots SET is_booked = 1 WHERE id = ?")
        .bind(body.slot_id)
        .execute(&state.db)
        .await
        .ok();

    // Notify admin via Telegram bot
    let mention = user
        .username
        .as_ref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| user.first_name.clone());

    let message = format!(
        "📋 Новая запись!\n\n\
         👤 {} \n\
         💅 {}\n\
         📅 {} в {}\n\
         💰 {} ₽",
        mention, service.name, slot.date, slot.start_time, service.price
    );

    notify_admin(&state.bot_token, state.admin_tg_id, &message).await;

    let detail = BookingDetail {
        id: booking_id,
        service_name: service.name,
        service_price: service.price,
        date: slot.date,
        start_time: slot.start_time,
        end_time: slot.end_time,
        client_tg_id: user.id,
        client_username: user.username,
        client_first_name: user.first_name,
        status: "confirmed".into(),
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    Ok(Json(ApiResponse::success(detail)))
}

/// GET /api/bookings/my — list current user's bookings
pub async fn my_bookings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse<Vec<BookingDetail>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    let bookings = sqlx::query_as::<_, BookingDetail>(
        "SELECT b.id, s.name as service_name, s.price as service_price,
                sl.date, sl.start_time, sl.end_time,
                b.client_tg_id, b.client_username, b.client_first_name,
                b.status, b.created_at
         FROM bookings b
         JOIN services s ON s.id = b.service_id
         JOIN available_slots sl ON sl.id = b.slot_id
         WHERE b.client_tg_id = ? AND b.status = 'confirmed'
         AND sl.date >= date('now')
         ORDER BY sl.date ASC, sl.start_time ASC"
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(bookings)))
}

/// DELETE /api/bookings/:id — cancel a booking
pub async fn cancel_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<&'static str>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    // Verify booking belongs to this user
    let booking = sqlx::query_as::<_, Booking>(
        "SELECT * FROM bookings WHERE id = ? AND client_tg_id = ? AND status = 'confirmed'"
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Запись не найдена"))))?;

    // Cancel booking
    sqlx::query("UPDATE bookings SET status = 'cancelled', cancelled_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .ok();

    // Free the slot
    sqlx::query("UPDATE available_slots SET is_booked = 0 WHERE id = ?")
        .bind(booking.slot_id)
        .execute(&state.db)
        .await
        .ok();

    // Get slot info for notification
    let slot = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked FROM available_slots WHERE id = ?"
    )
    .bind(booking.slot_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order FROM services WHERE id = ?"
    )
    .bind(booking.service_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Notify admin about cancellation
    let mention = user
        .username
        .as_ref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| user.first_name.clone());

    if let (Some(sl), Some(svc)) = (slot, service) {
        let message = format!(
            "❌ Отмена записи\n\n\
             👤 {}\n\
             💅 {}\n\
             📅 {} в {}",
            mention, svc.name, sl.date, sl.start_time
        );
        notify_admin(&state.bot_token, state.admin_tg_id, &message).await;
    }

    Ok(Json(ApiResponse::success("Запись отменена")))
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
