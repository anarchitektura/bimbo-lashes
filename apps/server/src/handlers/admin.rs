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

/// Helper: extract admin user (validates both auth and admin status)
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

/// GET /api/admin/services — list ALL services (including inactive)
pub async fn list_all_services(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse<Vec<Service>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let services = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(services)))
}

/// POST /api/admin/services — create a new service
pub async fn create_service(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateServiceRequest>,
) -> Result<Json<ApiResponse<Service>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let id = sqlx::query(
        "INSERT INTO services (name, description, price, duration_min, sort_order)
         VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&body.name)
    .bind(body.description.as_deref().unwrap_or(""))
    .bind(body.price)
    .bind(body.duration_min)
    .bind(body.sort_order.unwrap_or(0))
    .execute(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .last_insert_rowid();

    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(service)))
}

/// PUT /api/admin/services/:id — update a service
pub async fn update_service(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<UpdateServiceRequest>,
) -> Result<Json<ApiResponse<Service>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    if let Some(name) = &body.name {
        sqlx::query("UPDATE services SET name = ? WHERE id = ?")
            .bind(name).bind(id).execute(&state.db).await.ok();
    }
    if let Some(desc) = &body.description {
        sqlx::query("UPDATE services SET description = ? WHERE id = ?")
            .bind(desc).bind(id).execute(&state.db).await.ok();
    }
    if let Some(price) = body.price {
        sqlx::query("UPDATE services SET price = ? WHERE id = ?")
            .bind(price).bind(id).execute(&state.db).await.ok();
    }
    if let Some(dur) = body.duration_min {
        sqlx::query("UPDATE services SET duration_min = ? WHERE id = ?")
            .bind(dur).bind(id).execute(&state.db).await.ok();
    }
    if let Some(active) = body.is_active {
        sqlx::query("UPDATE services SET is_active = ? WHERE id = ?")
            .bind(active).bind(id).execute(&state.db).await.ok();
    }
    if let Some(order) = body.sort_order {
        sqlx::query("UPDATE services SET sort_order = ? WHERE id = ?")
            .bind(order).bind(id).execute(&state.db).await.ok();
    }

    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(service)))
}

/// GET /api/admin/slots?date=YYYY-MM-DD — list slots (all, including booked)
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
         ORDER BY start_time ASC"
    )
    .bind(&query.date)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(slots)))
}

/// POST /api/admin/slots — create available slots for a date
pub async fn create_slots(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateSlotsRequest>,
) -> Result<Json<ApiResponse<Vec<AvailableSlot>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    for slot in &body.slots {
        sqlx::query(
            "INSERT INTO available_slots (date, start_time, end_time) VALUES (?, ?, ?)"
        )
        .bind(&body.date)
        .bind(&slot.start_time)
        .bind(&slot.end_time)
        .execute(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;
    }

    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ?
         ORDER BY start_time ASC"
    )
    .bind(&body.date)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(slots)))
}

/// POST /api/admin/openday — create 1-hour slots for a full working day (12:00–20:00)
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

    // Create 8 one-hour slots: 12:00-13:00, ..., 19:00-20:00
    for hour in 12..20 {
        let start = format!("{:02}:00", hour);
        let end = format!("{:02}:00", hour + 1);

        // Idempotent: skip if already exists
        let exists: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM available_slots WHERE date = ? AND start_time = ?"
        )
        .bind(&body.date)
        .bind(&start)
        .fetch_one(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

        if !exists {
            sqlx::query(
                "INSERT INTO available_slots (date, start_time, end_time) VALUES (?, ?, ?)"
            )
            .bind(&body.date)
            .bind(&start)
            .bind(&end)
            .execute(&state.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;
        }
    }

    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ?
         ORDER BY start_time ASC"
    )
    .bind(&body.date)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(slots)))
}

/// DELETE /api/admin/slots/:id — delete a slot (only if not booked)
pub async fn delete_slot(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<&'static str>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let slot = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id FROM available_slots WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
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
        .ok();

    Ok(Json(ApiResponse::success("Слот удалён")))
}

/// GET /api/admin/bookings — list bookings
pub async fn list_bookings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(query): Query<BookingsQuery>,
) -> Result<Json<ApiResponse<Vec<BookingDetail>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let bookings = if let Some(date) = &query.date {
        sqlx::query_as::<_, BookingDetail>(
            "SELECT b.id, s.name as service_name, s.price as service_price,
                    COALESCE(b.date, sl.date) as date,
                    COALESCE(b.start_time, sl.start_time) as start_time,
                    COALESCE(b.end_time, sl.end_time) as end_time,
                    b.client_tg_id, b.client_username, b.client_first_name,
                    b.status, b.created_at,
                    CASE WHEN b.with_lower_lashes = 1 THEN 1 ELSE 0 END as with_lower_lashes,
                    CASE WHEN b.with_lower_lashes = 1
                         THEN s.price + COALESCE((SELECT price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1), 500)
                         ELSE s.price
                    END as total_price
             FROM bookings b
             JOIN services s ON s.id = b.service_id
             LEFT JOIN available_slots sl ON sl.id = b.slot_id
             WHERE COALESCE(b.date, sl.date) = ? AND b.status = 'confirmed'
             ORDER BY COALESCE(b.start_time, sl.start_time) ASC"
        )
        .bind(date)
        .fetch_all(&state.db)
        .await
    } else if let (Some(from), Some(to)) = (&query.from, &query.to) {
        sqlx::query_as::<_, BookingDetail>(
            "SELECT b.id, s.name as service_name, s.price as service_price,
                    COALESCE(b.date, sl.date) as date,
                    COALESCE(b.start_time, sl.start_time) as start_time,
                    COALESCE(b.end_time, sl.end_time) as end_time,
                    b.client_tg_id, b.client_username, b.client_first_name,
                    b.status, b.created_at,
                    CASE WHEN b.with_lower_lashes = 1 THEN 1 ELSE 0 END as with_lower_lashes,
                    CASE WHEN b.with_lower_lashes = 1
                         THEN s.price + COALESCE((SELECT price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1), 500)
                         ELSE s.price
                    END as total_price
             FROM bookings b
             JOIN services s ON s.id = b.service_id
             LEFT JOIN available_slots sl ON sl.id = b.slot_id
             WHERE COALESCE(b.date, sl.date) BETWEEN ? AND ? AND b.status = 'confirmed'
             ORDER BY COALESCE(b.date, sl.date) ASC, COALESCE(b.start_time, sl.start_time) ASC"
        )
        .bind(from)
        .bind(to)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, BookingDetail>(
            "SELECT b.id, s.name as service_name, s.price as service_price,
                    COALESCE(b.date, sl.date) as date,
                    COALESCE(b.start_time, sl.start_time) as start_time,
                    COALESCE(b.end_time, sl.end_time) as end_time,
                    b.client_tg_id, b.client_username, b.client_first_name,
                    b.status, b.created_at,
                    CASE WHEN b.with_lower_lashes = 1 THEN 1 ELSE 0 END as with_lower_lashes,
                    CASE WHEN b.with_lower_lashes = 1
                         THEN s.price + COALESCE((SELECT price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1), 500)
                         ELSE s.price
                    END as total_price
             FROM bookings b
             JOIN services s ON s.id = b.service_id
             LEFT JOIN available_slots sl ON sl.id = b.slot_id
             WHERE COALESCE(b.date, sl.date) >= date('now') AND b.status = 'confirmed'
             ORDER BY COALESCE(b.date, sl.date) ASC, COALESCE(b.start_time, sl.start_time) ASC"
        )
        .fetch_all(&state.db)
        .await
    }
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    Ok(Json(ApiResponse::success(bookings)))
}

/// POST /api/admin/bookings/:id/cancel — admin cancels a booking
pub async fn cancel_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<&'static str>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    extract_admin(auth_header, &state)?;

    let booking = sqlx::query_as::<_, Booking>(
        "SELECT * FROM bookings WHERE id = ? AND status = 'confirmed'"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Запись не найдена"))))?;

    sqlx::query("UPDATE bookings SET status = 'cancelled', cancelled_at = datetime('now') WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .ok();

    // Free all slots belonging to this booking
    sqlx::query("UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE booking_id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .ok();

    sqlx::query("UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE id = ?")
        .bind(booking.slot_id)
        .execute(&state.db)
        .await
        .ok();

    // Notify client
    let b_date = booking.date.as_deref().unwrap_or("?");
    let b_start = booking.start_time.as_deref().unwrap_or("?");

    let message = format!(
        "😔 Твоя запись на {} в {} была отменена мастером.\n\nВыбери другое время 💕",
        b_date, b_start
    );

    let url = format!("https://api.telegram.org/bot{}/sendMessage", state.bot_token);
    let client = reqwest::Client::new();
    let _ = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": booking.client_tg_id,
            "text": message
        }))
        .send()
        .await;

    Ok(Json(ApiResponse::success("Запись отменена")))
}
