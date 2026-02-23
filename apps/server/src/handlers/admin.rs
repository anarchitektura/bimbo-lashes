use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    Json,
};
use std::sync::Arc;

use crate::{auth, models::*, AppState};

/// Helper: extract admin user (validates both auth and admin status).
fn extract_admin(
    auth_header: Option<&str>,
    state: &AppState,
) -> Result<TelegramUser, (StatusCode, Json<ApiResponse<()>>)> {
    let header = auth_header.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Missing Authorization header")),
        )
    })?;
    let user = auth::extract_user_from_header(header, &state.bot_token).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::error("Invalid Telegram auth")),
        )
    })?;

    if !auth::is_admin(&user, state.admin_tg_id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse::error("Доступ запрещён")),
        ));
    }

    Ok(user)
}

/// GET /api/admin/services — list ALL services (including inactive).
pub async fn list_all_services(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse<Vec<Service>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let services = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("list_all_services: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(services)))
}

/// POST /api/admin/services — create a new service.
pub async fn create_service(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateServiceRequest>,
) -> Result<Json<ApiResponse<Service>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let id = sqlx::query(
        "INSERT INTO services (name, description, price, duration_min, sort_order)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&body.name)
    .bind(body.description.as_deref().unwrap_or(""))
    .bind(body.price)
    .bind(body.duration_min)
    .bind(body.sort_order.unwrap_or(0))
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("create_service: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?
    .last_insert_rowid();

    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("create_service fetch: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(service)))
}

/// PUT /api/admin/services/:id — update a service.
///
/// Uses COALESCE to only overwrite columns that were provided (NULL = keep existing).
pub async fn update_service(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<UpdateServiceRequest>,
) -> Result<Json<ApiResponse<Service>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    sqlx::query(
        "UPDATE services SET
         name = COALESCE(?, name),
         description = COALESCE(?, description),
         price = COALESCE(?, price),
         duration_min = COALESCE(?, duration_min),
         is_active = COALESCE(?, is_active),
         sort_order = COALESCE(?, sort_order)
         WHERE id = ?",
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.price)
    .bind(body.duration_min)
    .bind(body.is_active)
    .bind(body.sort_order)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("update_service: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("update_service fetch: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(service)))
}

/// GET /api/admin/slots?date=YYYY-MM-DD — list slots (all, including booked).
pub async fn list_slots(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<SlotsQuery>,
) -> Result<Json<ApiResponse<Vec<AvailableSlot>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ?
         ORDER BY start_time ASC",
    )
    .bind(&query.date)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("list_slots: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(slots)))
}

/// POST /api/admin/slots — create available slots for a date.
pub async fn create_slots(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateSlotsRequest>,
) -> Result<Json<ApiResponse<Vec<AvailableSlot>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    for slot in &body.slots {
        sqlx::query("INSERT INTO available_slots (date, start_time, end_time) VALUES (?, ?, ?)")
            .bind(&body.date)
            .bind(&slot.start_time)
            .bind(&slot.end_time)
            .execute(&state.db)
            .await
            .map_err(|e| {
                tracing::error!("create_slots: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
            })?;
    }

    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ?
         ORDER BY start_time ASC",
    )
    .bind(&body.date)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("create_slots fetch: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(slots)))
}

/// POST /api/admin/openday — create 1-hour slots for a working day.
pub async fn open_day(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<OpenDayRequest>,
) -> Result<Json<ApiResponse<Vec<AvailableSlot>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    if chrono::NaiveDate::parse_from_str(&body.date, "%Y-%m-%d").is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Неверный формат даты")),
        ));
    }

    let start_h = body.start_hour.unwrap_or(12).min(23);
    let end_h = body.end_hour.unwrap_or(20).clamp(1, 24);

    if start_h >= end_h {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("start_hour должен быть меньше end_hour")),
        ));
    }

    for hour in start_h..end_h {
        let start = format!("{:02}:00", hour);
        let end = format!("{:02}:00", hour + 1);

        // Idempotent: skip if already exists
        let exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM available_slots WHERE date = ? AND start_time = ?",
        )
        .bind(&body.date)
        .bind(&start)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("open_day exists check: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
        })?;

        if !exists {
            sqlx::query("INSERT INTO available_slots (date, start_time, end_time) VALUES (?, ?, ?)")
                .bind(&body.date)
                .bind(&start)
                .bind(&end)
                .execute(&state.db)
                .await
                .map_err(|e| {
                    tracing::error!("open_day insert: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
                })?;
        }
    }

    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ?
         ORDER BY start_time ASC",
    )
    .bind(&body.date)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("open_day fetch: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(slots)))
}

/// DELETE /api/admin/slots/:id — delete a slot (only if not booked).
pub async fn delete_slot(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<&'static str>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let slot = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id FROM available_slots WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("delete_slot: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Слот не найден"))))?;

    if slot.is_booked {
        return Err((
            StatusCode::CONFLICT,
            Json(ApiResponse::error("Нельзя удалить занятый слот. Сначала отмените запись.")),
        ));
    }

    sqlx::query("DELETE FROM available_slots WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("delete_slot delete: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
        })?;

    Ok(Json(ApiResponse::success("Слот удалён")))
}

/// GET /api/admin/bookings — list bookings (uses shared query).
pub async fn list_bookings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<BookingsQuery>,
) -> Result<Json<ApiResponse<Vec<BookingDetail>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let base = super::client::booking_detail_select();

    let bookings = if let Some(date) = &query.date {
        let sql = format!(
            "{} WHERE COALESCE(b.date, sl.date) = ? AND b.status IN ('confirmed', 'pending_payment')
             ORDER BY COALESCE(b.start_time, sl.start_time) ASC",
            base
        );
        sqlx::query_as::<_, BookingDetail>(&sql)
            .bind(date)
            .fetch_all(&state.db)
            .await
    } else if let (Some(from), Some(to)) = (&query.from, &query.to) {
        let sql = format!(
            "{} WHERE COALESCE(b.date, sl.date) BETWEEN ? AND ? AND b.status IN ('confirmed', 'pending_payment')
             ORDER BY COALESCE(b.date, sl.date) ASC, COALESCE(b.start_time, sl.start_time) ASC",
            base
        );
        sqlx::query_as::<_, BookingDetail>(&sql)
            .bind(from)
            .bind(to)
            .fetch_all(&state.db)
            .await
    } else {
        let sql = format!(
            "{} WHERE COALESCE(b.date, sl.date) >= date('now', '+3 hours') AND b.status IN ('confirmed', 'pending_payment')
             ORDER BY COALESCE(b.date, sl.date) ASC, COALESCE(b.start_time, sl.start_time) ASC",
            base
        );
        sqlx::query_as::<_, BookingDetail>(&sql)
            .fetch_all(&state.db)
            .await
    }
    .map_err(|e| {
        tracing::error!("list_bookings: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?;

    Ok(Json(ApiResponse::success(bookings)))
}

/// POST /api/admin/bookings/:id/cancel — admin cancels a booking (always refund if paid).
pub async fn cancel_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<&'static str>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let booking = sqlx::query_as::<_, Booking>(
        "SELECT * FROM bookings WHERE id = ? AND status IN ('confirmed', 'pending_payment')",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("admin cancel_booking fetch: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Запись не найдена"))))?;

    // Admin cancellation → always refund if paid
    let refund_info = super::client::process_refund_if_needed(&state, &booking, true).await;

    if let Err(e) = sqlx::query(
        "UPDATE bookings SET status = 'cancelled', cancelled_at = datetime('now', '+3 hours') WHERE id = ?",
    )
    .bind(id)
    .execute(&state.db)
    .await
    {
        tracing::error!("admin cancel_booking update: {}", e);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))));
    }

    super::client::free_booking_slots(&state.db, id, booking.slot_id).await;

    // Notify client
    let b_date = booking.date.as_deref().unwrap_or("?");
    let b_start = booking.start_time.as_deref().unwrap_or("?");

    let refund_text = refund_info
        .as_ref()
        .map(|r| format!("\n\n💰 {}", r))
        .unwrap_or_default();

    let message = format!(
        "😔 Твоя запись на {} в {} была отменена мастером.{}",
        b_date, b_start, refund_text
    );

    let url = format!("https://api.telegram.org/bot{}/sendMessage", state.bot_token);
    let client = reqwest::Client::new();
    if let Err(e) = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": booking.client_tg_id,
            "text": message
        }))
        .send()
        .await
    {
        tracing::error!("Failed to notify client {}: {}", booking.client_tg_id, e);
    }

    Ok(Json(ApiResponse::success("Запись отменена")))
}
