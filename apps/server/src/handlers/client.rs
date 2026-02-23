use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    Json,
};
use chrono::{Datelike, FixedOffset, Utc};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{auth, models::*, AppState};

// ── Constants ──

/// Moscow timezone offset (UTC+3).
const MSK_OFFSET_SECS: i32 = 3 * 3600;

/// Prepayment amount in RUB.
const PREPAID_AMOUNT: i64 = 500;

/// Days threshold for "tight" booking mode (adjacent slots only).
const TIGHT_MODE_DAYS: i64 = 3;

/// Moscow timezone (UTC+3).
fn moscow_now() -> chrono::DateTime<FixedOffset> {
    let msk = FixedOffset::east_opt(MSK_OFFSET_SECS).unwrap();
    Utc::now().with_timezone(&msk)
}

fn moscow_today() -> String {
    moscow_now().format("%Y-%m-%d").to_string()
}

/// Helper: extract TelegramUser from Authorization header.
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

/// Calculate how many 1-hour slots a service needs.
fn slots_needed_for_duration(duration_min: i64) -> usize {
    (duration_min as f64 / 60.0).ceil() as usize
}

// ── Shared booking query (eliminates duplication across client/admin) ──

/// The shared SELECT columns for booking detail queries.
const BOOKING_DETAIL_SELECT: &str =
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
            END as total_price,
            b.payment_status,
            b.prepaid_amount
     FROM bookings b
     JOIN services s ON s.id = b.service_id
     LEFT JOIN available_slots sl ON sl.id = b.slot_id";

// ── Endpoints ──

/// GET /api/services — list active main services (hides addons).
pub async fn list_services(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<Service>>>, StatusCode> {
    let services = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE is_active = 1 AND service_type = 'main' ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("list_services: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ApiResponse::success(services)))
}

/// GET /api/addon-info — returns addon (lower lashes) info for frontend.
pub async fn addon_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Option<AddonInfo>>>, StatusCode> {
    let addon = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT id, name, price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("addon_info: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let info = addon.map(|(id, name, price)| AddonInfo {
        service_id: id,
        name,
        price,
    });

    Ok(Json(ApiResponse::success(info)))
}

/// GET /api/available-dates?service_id=N — dates with enough consecutive free slots.
pub async fn available_dates_for_service(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AvailableDatesQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    let slots_needed = if let Some(service_id) = query.service_id {
        let service = sqlx::query_as::<_, Service>(
            "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
             FROM services WHERE id = ? AND is_active = 1",
        )
        .bind(service_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match service {
            Some(s) => slots_needed_for_duration(s.duration_min) as i64,
            None => return Ok(Json(ApiResponse::success(vec![]))),
        }
    } else {
        1
    };

    // Get all dates with free slots in the future
    let dates: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT date FROM available_slots
         WHERE is_booked = 0 AND date >= date('now', '+3 hours')
         ORDER BY date ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Filter: only dates with enough consecutive free slots
    let mut valid_dates = Vec::new();
    for date in &dates {
        let slots = sqlx::query_as::<_, AvailableSlot>(
            "SELECT id, date, start_time, end_time, is_booked, booking_id
             FROM available_slots WHERE date = ? ORDER BY start_time ASC",
        )
        .bind(date)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if has_consecutive_free_slots(&slots, slots_needed) {
            valid_dates.push(date.clone());
        }
    }

    Ok(Json(ApiResponse::success(valid_dates)))
}

/// GET /api/available-times?date=YYYY-MM-DD&service_id=N — smart slot availability.
pub async fn available_times(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AvailableTimesQuery>,
) -> Result<Json<ApiResponse<AvailableTimesResponse>>, StatusCode> {
    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ? AND is_active = 1",
    )
    .bind(query.service_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let service = match service {
        Some(s) => s,
        None => {
            return Ok(Json(ApiResponse::success(AvailableTimesResponse {
                mode: "free".into(),
                times: vec![],
            })))
        }
    };

    let slots_needed = slots_needed_for_duration(service.duration_min);

    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ? ORDER BY start_time ASC",
    )
    .bind(&query.date)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let today = moscow_today();
    let days_until = days_between(&today, &query.date);
    let is_tight = days_until <= TIGHT_MODE_DAYS;

    let time_blocks = find_bookable_blocks(&slots, slots_needed, is_tight);

    Ok(Json(ApiResponse::success(AvailableTimesResponse {
        mode: if is_tight { "tight".into() } else { "free".into() },
        times: time_blocks,
    })))
}

/// POST /api/bookings — create a new booking with YooKassa prepayment.
pub async fn create_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateBookingRequest>,
) -> Result<Json<ApiResponse<CreateBookingResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    // Validate date format
    if chrono::NaiveDate::parse_from_str(&body.date, "%Y-%m-%d").is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Неверный формат даты")),
        ));
    }

    // Validate time format
    if body.start_time.len() != 5 || !body.start_time.contains(':') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("Неверный формат времени")),
        ));
    }

    // Get service
    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ? AND is_active = 1",
    )
    .bind(body.service_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Услуга не найдена"))))?;

    // Calculate end_time
    let end_time = add_minutes_to_time(&body.start_time, service.duration_min as u32);

    // Find all slots between start_time and end_time on this date
    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots
         WHERE date = ? AND start_time >= ? AND end_time <= ?
         ORDER BY start_time ASC",
    )
    .bind(&body.date)
    .bind(&body.start_time)
    .bind(&end_time)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    let slots_needed = slots_needed_for_duration(service.duration_min);
    if slots.len() < slots_needed {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::error("Недостаточно слотов для записи")),
        ));
    }

    // Verify all are free
    for slot in &slots {
        if slot.is_booked {
            return Err((
                StatusCode::CONFLICT,
                Json(ApiResponse::error("Одно из выбранных времён уже занято")),
            ));
        }
    }

    // Calculate price
    let addon_price = if body.with_lower_lashes {
        sqlx::query_scalar::<_, i64>(
            "SELECT price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1",
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
        .unwrap_or(500)
    } else {
        0
    };
    let total_price = service.price + addon_price;

    // Create booking as pending_payment
    let first_slot_id = slots[0].id;
    let created_at = moscow_now().format("%Y-%m-%d %H:%M:%S").to_string();
    let booking_id = sqlx::query(
        "INSERT INTO bookings (service_id, slot_id, client_tg_id, client_username, client_first_name,
         status, date, start_time, end_time, with_lower_lashes,
         payment_status, prepaid_amount, created_at)
         VALUES (?, ?, ?, ?, ?, 'pending_payment', ?, ?, ?, ?, 'pending', ?, ?)",
    )
    .bind(body.service_id)
    .bind(first_slot_id)
    .bind(user.id)
    .bind(&user.username)
    .bind(&user.first_name)
    .bind(&body.date)
    .bind(&body.start_time)
    .bind(&end_time)
    .bind(body.with_lower_lashes)
    .bind(PREPAID_AMOUNT)
    .bind(&created_at)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("create_booking INSERT failed: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
    })?
    .last_insert_rowid();

    // Lock slots immediately (prevent double booking)
    for slot in &slots {
        if let Err(e) = sqlx::query(
            "UPDATE available_slots SET is_booked = 1, booking_id = ? WHERE id = ? AND is_booked = 0",
        )
        .bind(booking_id)
        .bind(slot.id)
        .execute(&state.db)
        .await
        {
            tracing::error!("Failed to lock slot {}: {}", slot.id, e);
            // Rollback booking
            rollback_booking(&state.db, booking_id, &slots).await;
            return Err((
                StatusCode::CONFLICT,
                Json(ApiResponse::error("Не удалось забронировать слоты. Попробуйте снова.")),
            ));
        }
    }

    // Create YooKassa payment
    let addon_text = if body.with_lower_lashes {
        format!("{} + нижние", service.name)
    } else {
        service.name.clone()
    };
    let description = format!("Предоплата: {} на {}", addon_text, body.date);

    let payment_result = super::payment::create_yookassa_payment(
        &state.yookassa_shop_id,
        &state.yookassa_secret_key,
        booking_id,
        PREPAID_AMOUNT,
        &description,
        &state.webapp_url,
    )
    .await;

    let payment_url = match payment_result {
        Ok((payment_id, confirmation_url)) => {
            // Save payment_id
            if let Err(e) = sqlx::query("UPDATE bookings SET yookassa_payment_id = ? WHERE id = ?")
                .bind(&payment_id)
                .bind(booking_id)
                .execute(&state.db)
                .await
            {
                tracing::error!("Failed to save payment_id for booking {}: {}", booking_id, e);
            }
            Some(confirmation_url)
        }
        Err(e) => {
            tracing::error!("YooKassa payment creation failed for booking {}: {}", booking_id, e);
            rollback_booking(&state.db, booking_id, &slots).await;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::error("Ошибка создания платежа. Попробуйте позже.")),
            ));
        }
    };

    let detail = BookingDetail {
        id: booking_id,
        service_name: service.name,
        service_price: service.price,
        date: body.date,
        start_time: body.start_time,
        end_time,
        client_tg_id: user.id,
        client_username: user.username,
        client_first_name: user.first_name,
        status: "pending_payment".into(),
        created_at,
        with_lower_lashes: Some(body.with_lower_lashes),
        total_price: Some(total_price),
        payment_status: Some("pending".into()),
        prepaid_amount: Some(PREPAID_AMOUNT),
    };

    Ok(Json(ApiResponse::success(CreateBookingResponse {
        booking: detail,
        payment_url,
    })))
}

/// GET /api/bookings/my — list current user's bookings (confirmed + pending_payment).
pub async fn my_bookings(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse<Vec<BookingDetail>>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    let query = format!(
        "{} WHERE b.client_tg_id = ? AND b.status IN ('confirmed', 'pending_payment')
         AND COALESCE(b.date, sl.date) >= date('now', '+3 hours')
         ORDER BY COALESCE(b.date, sl.date) ASC, COALESCE(b.start_time, sl.start_time) ASC",
        BOOKING_DETAIL_SELECT
    );

    let bookings = sqlx::query_as::<_, BookingDetail>(&query)
        .bind(user.id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("my_bookings: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error")))
        })?;

    Ok(Json(ApiResponse::success(bookings)))
}

/// DELETE /api/bookings/:id — cancel a booking (with refund logic).
pub async fn cancel_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<CancelBookingResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    // Verify booking belongs to this user
    let booking = sqlx::query_as::<_, Booking>(
        "SELECT * FROM bookings WHERE id = ? AND client_tg_id = ? AND status IN ('confirmed', 'pending_payment')",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Запись не найдена"))))?;

    let refund_info = process_refund_if_needed(&state, &booking, false).await;

    // Cancel booking
    if let Err(e) = sqlx::query(
        "UPDATE bookings SET status = 'cancelled', cancelled_at = datetime('now', '+3 hours') WHERE id = ?",
    )
    .bind(id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to cancel booking {}: {}", id, e);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))));
    }

    // Free all slots belonging to this booking
    free_booking_slots(&state.db, id, booking.slot_id).await;

    // Notify admin
    let service_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM services WHERE id = ?",
    )
    .bind(booking.service_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "?".into());

    let mention = user
        .username
        .as_ref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| user.first_name.clone());

    let b_date = booking.date.as_deref().unwrap_or("?");
    let b_start = booking.start_time.as_deref().unwrap_or("?");
    let refund_text = refund_info.as_deref().unwrap_or("");

    let message = format!(
        "❌ Отмена записи\n\n\
         👤 {}\n\
         💅 {}\n\
         📅 {} в {}{}",
        mention,
        service_name,
        b_date,
        b_start,
        if refund_text.is_empty() {
            String::new()
        } else {
            format!("\n💰 {}", refund_text)
        }
    );
    notify_admin(&state.bot_token, state.admin_tg_id, &message).await;

    Ok(Json(ApiResponse::success(CancelBookingResponse {
        message: "Запись отменена".into(),
        refund_info,
    })))
}

/// GET /api/bookings/:id/status — poll booking payment status.
pub async fn booking_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<ApiResponse<BookingStatusResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    let result = sqlx::query_as::<_, (String, String)>(
        "SELECT status, payment_status FROM bookings WHERE id = ? AND client_tg_id = ?",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ApiResponse::error("Запись не найдена"))))?;

    Ok(Json(ApiResponse::success(BookingStatusResponse {
        status: result.0,
        payment_status: result.1,
    })))
}

/// GET /api/calendar?year=2026&month=2&service_id=1 — calendar data with slot stats.
///
/// Fetches ALL slots for the month in a single query (no N+1).
pub async fn calendar(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
) -> Result<Json<ApiResponse<Vec<CalendarDay>>>, StatusCode> {
    let slots_needed = if let Some(service_id) = query.service_id {
        let service = sqlx::query_as::<_, Service>(
            "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
             FROM services WHERE id = ? AND is_active = 1",
        )
        .bind(service_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match service {
            Some(s) => slots_needed_for_duration(s.duration_min) as i64,
            None => 1,
        }
    } else {
        1
    };

    let year = query.year;
    let month = query.month;
    let days_in_month = chrono::NaiveDate::from_ymd_opt(
        if month == 12 { year + 1 } else { year },
        if month == 12 { 1 } else { month + 1 },
        1,
    )
    .unwrap_or(chrono::NaiveDate::from_ymd_opt(year, month, 28).unwrap())
    .pred_opt()
    .map(|d| d.day())
    .unwrap_or(28);

    let today = moscow_today();

    // Single query: fetch ALL slots for the month at once (fixes N+1)
    let month_start = format!("{:04}-{:02}-01", year, month);
    let month_end = format!("{:04}-{:02}-{:02}", year, month, days_in_month);

    let all_slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots
         WHERE date >= ? AND date <= ?
         ORDER BY date ASC, start_time ASC",
    )
    .bind(&month_start)
    .bind(&month_end)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Group slots by date
    let mut slots_by_date: HashMap<String, Vec<AvailableSlot>> = HashMap::new();
    for slot in all_slots {
        slots_by_date
            .entry(slot.date.clone())
            .or_default()
            .push(slot);
    }

    let mut calendar_days = Vec::new();

    for day in 1..=days_in_month {
        let date = format!("{:04}-{:02}-{:02}", year, month, day);

        if date < today {
            continue;
        }

        let slots = slots_by_date.get(&date);
        let total = slots.map_or(0, |s| s.len() as i64);
        let free = slots.map_or(0, |s| s.iter().filter(|sl| !sl.is_booked).count() as i64);

        let bookable = if total == 0 {
            false
        } else if query.service_id.is_some() {
            slots
                .is_some_and(|s| has_consecutive_free_slots(s, slots_needed))
        } else {
            free > 0
        };

        calendar_days.push(CalendarDay {
            date,
            total,
            free,
            bookable,
        });
    }

    Ok(Json(ApiResponse::success(calendar_days)))
}

// ── Shared helpers (pub for admin.rs) ──

/// The shared booking detail SELECT string (used by admin.rs too).
pub fn booking_detail_select() -> &'static str {
    BOOKING_DETAIL_SELECT
}

/// Send a message to admin via Telegram Bot API.
pub async fn notify_admin(bot_token: &str, chat_id: i64, text: &str) {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
    let client = reqwest::Client::new();
    if let Err(e) = client
        .post(&url)
        .json(&serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML"
        }))
        .send()
        .await
    {
        tracing::error!("Failed to notify admin: {}", e);
    }
}

/// Free all slots belonging to a booking.
pub async fn free_booking_slots(db: &sqlx::SqlitePool, booking_id: i64, slot_id: i64) {
    if let Err(e) = sqlx::query(
        "UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE booking_id = ?",
    )
    .bind(booking_id)
    .execute(db)
    .await
    {
        tracing::error!("Failed to free slots for booking {}: {}", booking_id, e);
    }

    // Also free by slot_id for backward compat
    if let Err(e) = sqlx::query(
        "UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE id = ?",
    )
    .bind(slot_id)
    .execute(db)
    .await
    {
        tracing::error!("Failed to free slot_id {} for booking: {}", slot_id, e);
    }
}

/// Process refund logic for a booking cancellation.
///
/// - `admin_override`: if true, always refund (admin cancel). Otherwise, check 24h rule.
pub async fn process_refund_if_needed(
    state: &AppState,
    booking: &Booking,
    admin_override: bool,
) -> Option<String> {
    if booking.payment_status != "paid" {
        return None;
    }

    let b_date = booking.date.as_deref().unwrap_or("2099-01-01");
    let b_time = booking.start_time.as_deref().unwrap_or("00:00");
    let appointment_str = format!("{} {}", b_date, b_time);

    let hours_until = chrono::NaiveDateTime::parse_from_str(&appointment_str, "%Y-%m-%d %H:%M")
        .map(|appointment| {
            let now = moscow_now().naive_local();
            (appointment - now).num_hours()
        })
        .unwrap_or(999); // Default to refundable on parse error

    let should_refund = admin_override || hours_until > 24;

    if should_refund {
        if let Some(payment_id) = &booking.yookassa_payment_id {
            let refund_result = super::payment::create_yookassa_refund(
                &state.yookassa_shop_id,
                &state.yookassa_secret_key,
                payment_id,
                booking.prepaid_amount,
            )
            .await;

            if refund_result.is_ok() {
                if let Err(e) = sqlx::query(
                    "UPDATE bookings SET payment_status = 'refunded' WHERE id = ?",
                )
                .bind(booking.id)
                .execute(&state.db)
                .await
                {
                    tracing::error!("Failed to update payment_status for booking {}: {}", booking.id, e);
                }
                Some(format!("Предоплата {} ₽ будет возвращена", booking.prepaid_amount))
            } else {
                tracing::error!("Refund failed for booking {}", booking.id);
                Some("Возврат будет обработан вручную".into())
            }
        } else {
            None
        }
    } else {
        // ≤24h → no refund
        Some(format!(
            "Предоплата {} ₽ не возвращается (отмена менее чем за 24ч)",
            booking.prepaid_amount
        ))
    }
}

// ── Private helpers ──

/// Rollback a failed booking: set to expired and free slots.
async fn rollback_booking(db: &sqlx::SqlitePool, booking_id: i64, slots: &[AvailableSlot]) {
    sqlx::query("UPDATE bookings SET status = 'expired', payment_status = 'none' WHERE id = ?")
        .bind(booking_id)
        .execute(db)
        .await
        .ok();
    for slot in slots {
        sqlx::query("UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE id = ?")
            .bind(slot.id)
            .execute(db)
            .await
            .ok();
    }
}

/// Check if there are N consecutive free slots in the list.
fn has_consecutive_free_slots(slots: &[AvailableSlot], needed: i64) -> bool {
    let needed = needed as usize;
    for i in 0..slots.len() {
        if slots[i].is_booked {
            continue;
        }
        if i + needed > slots.len() {
            break;
        }
        let mut ok = true;
        for j in 0..needed {
            let idx = i + j;
            if slots[idx].is_booked {
                ok = false;
                break;
            }
            if j > 0 && slots[i + j - 1].end_time != slots[idx].start_time {
                ok = false;
                break;
            }
        }
        if ok {
            return true;
        }
    }
    false
}

/// Find all bookable time blocks given a list of slots.
///
/// In tight mode (within 3 days), only shows blocks adjacent to existing bookings
/// to minimize schedule fragmentation.
fn find_bookable_blocks(
    slots: &[AvailableSlot],
    slots_needed: usize,
    is_tight: bool,
) -> Vec<TimeBlock> {
    let mut blocks = Vec::new();
    let has_bookings = slots.iter().any(|s| s.is_booked);

    for i in 0..slots.len() {
        if slots[i].is_booked || i + slots_needed > slots.len() {
            continue;
        }

        // Check N consecutive free slots
        let mut valid = true;
        for j in 0..slots_needed {
            let idx = i + j;
            if slots[idx].is_booked {
                valid = false;
                break;
            }
            if j > 0 && slots[i + j - 1].end_time != slots[idx].start_time {
                valid = false;
                break;
            }
        }

        if !valid {
            continue;
        }

        let block_start = &slots[i].start_time;
        let block_end = &slots[i + slots_needed - 1].end_time;

        if is_tight && has_bookings {
            // Tight mode: only adjacent to booked slots
            if is_adjacent_to_booked(block_start, block_end, slots) {
                blocks.push(TimeBlock {
                    start_time: block_start.clone(),
                    end_time: block_end.clone(),
                });
            }
        } else {
            blocks.push(TimeBlock {
                start_time: block_start.clone(),
                end_time: block_end.clone(),
            });
        }
    }

    blocks
}

/// Check if a time block is adjacent to a booked slot.
fn is_adjacent_to_booked(block_start: &str, block_end: &str, all_slots: &[AvailableSlot]) -> bool {
    all_slots.iter().any(|slot| {
        slot.is_booked && (block_start == slot.end_time || block_end == slot.start_time)
    })
}

/// Calculate days between two date strings (YYYY-MM-DD).
fn days_between(from: &str, to: &str) -> i64 {
    let from_date = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d");
    let to_date = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d");

    match (from_date, to_date) {
        (Ok(f), Ok(t)) => (t - f).num_days(),
        _ => 999, // default to free mode if parsing fails
    }
}

/// Add minutes to a time string "HH:MM" → "HH:MM".
fn add_minutes_to_time(time: &str, minutes: u32) -> String {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return time.to_string();
    }
    let hour: u32 = parts[0].parse().unwrap_or(0);
    let min: u32 = parts[1].parse().unwrap_or(0);
    let total = hour * 60 + min + minutes;
    format!("{:02}:{:02}", (total / 60).min(23), total % 60)
}

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build an AvailableSlot without boilerplate.
    fn make_slot(id: i64, date: &str, start: &str, end: &str, booked: bool) -> AvailableSlot {
        AvailableSlot {
            id,
            date: date.to_string(),
            start_time: start.to_string(),
            end_time: end.to_string(),
            is_booked: booked,
            booking_id: if booked { Some(100 + id) } else { None },
        }
    }

    // ── slots_needed_for_duration ──

    #[test]
    fn test_slots_needed_exact_hour() {
        assert_eq!(slots_needed_for_duration(60), 1);
    }

    #[test]
    fn test_slots_needed_two_hours() {
        assert_eq!(slots_needed_for_duration(120), 2);
    }

    #[test]
    fn test_slots_needed_round_up() {
        assert_eq!(slots_needed_for_duration(90), 2);
    }

    #[test]
    fn test_slots_needed_small() {
        assert_eq!(slots_needed_for_duration(30), 1);
    }

    #[test]
    fn test_slots_needed_three_hours() {
        assert_eq!(slots_needed_for_duration(180), 3);
    }

    #[test]
    fn test_slots_needed_just_over() {
        assert_eq!(slots_needed_for_duration(61), 2);
    }

    // ── days_between ──

    #[test]
    fn test_days_between_same_day() {
        assert_eq!(days_between("2026-03-01", "2026-03-01"), 0);
    }

    #[test]
    fn test_days_between_one_day() {
        assert_eq!(days_between("2026-03-01", "2026-03-02"), 1);
    }

    #[test]
    fn test_days_between_negative() {
        assert_eq!(days_between("2026-03-05", "2026-03-01"), -4);
    }

    #[test]
    fn test_days_between_large_gap() {
        assert_eq!(days_between("2026-01-01", "2026-12-31"), 364);
    }

    #[test]
    fn test_days_between_invalid_from() {
        assert_eq!(days_between("not-a-date", "2026-03-01"), 999);
    }

    #[test]
    fn test_days_between_invalid_to() {
        assert_eq!(days_between("2026-03-01", "garbage"), 999);
    }

    #[test]
    fn test_days_between_both_invalid() {
        assert_eq!(days_between("x", "y"), 999);
    }

    #[test]
    fn test_days_between_tight_boundary() {
        // TIGHT_MODE_DAYS = 3: 2 days is within tight range
        assert!(days_between("2026-03-01", "2026-03-03") <= TIGHT_MODE_DAYS);
    }

    // ── add_minutes_to_time ──

    #[test]
    fn test_add_minutes_basic() {
        assert_eq!(add_minutes_to_time("10:00", 60), "11:00");
    }

    #[test]
    fn test_add_minutes_half() {
        assert_eq!(add_minutes_to_time("10:00", 30), "10:30");
    }

    #[test]
    fn test_add_minutes_zero() {
        assert_eq!(add_minutes_to_time("10:00", 0), "10:00");
    }

    #[test]
    fn test_add_minutes_cross_hour() {
        assert_eq!(add_minutes_to_time("10:45", 30), "11:15");
    }

    #[test]
    fn test_add_minutes_cap_at_23() {
        assert_eq!(add_minutes_to_time("22:00", 180), "23:00");
    }

    #[test]
    fn test_add_minutes_already_23() {
        assert_eq!(add_minutes_to_time("23:00", 60), "23:00");
    }

    #[test]
    fn test_add_minutes_invalid_format() {
        assert_eq!(add_minutes_to_time("garbage", 30), "garbage");
    }

    #[test]
    fn test_add_minutes_midnight() {
        assert_eq!(add_minutes_to_time("00:00", 60), "01:00");
    }

    #[test]
    fn test_add_minutes_large() {
        assert_eq!(add_minutes_to_time("12:00", 480), "20:00");
    }

    // ── has_consecutive_free_slots ──

    #[test]
    fn test_consecutive_single_free() {
        let slots = vec![make_slot(1, "2026-03-01", "10:00", "11:00", false)];
        assert!(has_consecutive_free_slots(&slots, 1));
    }

    #[test]
    fn test_consecutive_empty() {
        let slots: Vec<AvailableSlot> = vec![];
        assert!(!has_consecutive_free_slots(&slots, 1));
    }

    #[test]
    fn test_consecutive_all_booked() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", true),
            make_slot(2, "2026-03-01", "11:00", "12:00", true),
        ];
        assert!(!has_consecutive_free_slots(&slots, 1));
    }

    #[test]
    fn test_consecutive_two_adjacent_free() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
        ];
        assert!(has_consecutive_free_slots(&slots, 2));
    }

    #[test]
    fn test_consecutive_two_with_gap() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "12:00", "13:00", false), // gap
        ];
        assert!(!has_consecutive_free_slots(&slots, 2));
    }

    #[test]
    fn test_consecutive_booked_then_free() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", true),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
        ];
        assert!(has_consecutive_free_slots(&slots, 2));
    }

    #[test]
    fn test_consecutive_not_enough() {
        let slots = vec![make_slot(1, "2026-03-01", "10:00", "11:00", false)];
        assert!(!has_consecutive_free_slots(&slots, 2));
    }

    #[test]
    fn test_consecutive_three_middle_booked() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", true),
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
        ];
        assert!(!has_consecutive_free_slots(&slots, 2));
    }

    #[test]
    fn test_consecutive_need_three() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
        ];
        assert!(has_consecutive_free_slots(&slots, 3));
    }

    // ── is_adjacent_to_booked ──

    #[test]
    fn test_adjacent_block_starts_where_booked_ends() {
        let slots = vec![make_slot(1, "2026-03-01", "09:00", "10:00", true)];
        assert!(is_adjacent_to_booked("10:00", "11:00", &slots));
    }

    #[test]
    fn test_adjacent_block_ends_where_booked_starts() {
        let slots = vec![make_slot(1, "2026-03-01", "13:00", "14:00", true)];
        assert!(is_adjacent_to_booked("12:00", "13:00", &slots));
    }

    #[test]
    fn test_adjacent_no_match() {
        let slots = vec![make_slot(1, "2026-03-01", "15:00", "16:00", true)];
        assert!(!is_adjacent_to_booked("10:00", "11:00", &slots));
    }

    #[test]
    fn test_adjacent_free_slot_ignored() {
        let slots = vec![make_slot(1, "2026-03-01", "09:00", "10:00", false)];
        assert!(!is_adjacent_to_booked("10:00", "11:00", &slots));
    }

    #[test]
    fn test_adjacent_empty_slots() {
        let slots: Vec<AvailableSlot> = vec![];
        assert!(!is_adjacent_to_booked("10:00", "11:00", &slots));
    }

    // ── find_bookable_blocks ──

    #[test]
    fn test_bookable_free_mode_single_slot() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
        ];
        let blocks = find_bookable_blocks(&slots, 1, false);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].start_time, "10:00");
        assert_eq!(blocks[2].end_time, "13:00");
    }

    #[test]
    fn test_bookable_free_mode_two_slots() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
        ];
        let blocks = find_bookable_blocks(&slots, 2, false);
        assert_eq!(blocks.len(), 2); // 10-12, 11-13
        assert_eq!(blocks[0].start_time, "10:00");
        assert_eq!(blocks[0].end_time, "12:00");
        assert_eq!(blocks[1].start_time, "11:00");
        assert_eq!(blocks[1].end_time, "13:00");
    }

    #[test]
    fn test_bookable_free_mode_with_booked() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", true),
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
        ];
        let blocks = find_bookable_blocks(&slots, 1, false);
        assert_eq!(blocks.len(), 2); // 10-11, 12-13
    }

    #[test]
    fn test_bookable_tight_no_bookings() {
        // No booked slots → tight mode degrades to free mode
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
        ];
        let blocks = find_bookable_blocks(&slots, 1, true);
        assert_eq!(blocks.len(), 2); // All shown (has_bookings=false)
    }

    #[test]
    fn test_bookable_tight_adjacent_only() {
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", true),  // booked
            make_slot(3, "2026-03-01", "12:00", "13:00", false),
            make_slot(4, "2026-03-01", "13:00", "14:00", false),
            make_slot(5, "2026-03-01", "14:00", "15:00", false),
        ];
        // Tight mode: only blocks adjacent to booked slot (11-12)
        // 10-11: end_time 11:00 == booked.start_time 11:00 → adjacent ✓
        // 12-13: start_time 12:00 == booked.end_time 12:00 → adjacent ✓
        // 13-14: start 13:00 ≠ 12:00, end 14:00 ≠ 11:00 → NOT adjacent
        // 14-15: NOT adjacent
        let blocks = find_bookable_blocks(&slots, 1, true);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].start_time, "10:00");
        assert_eq!(blocks[1].start_time, "12:00");
    }

    #[test]
    fn test_bookable_empty() {
        let slots: Vec<AvailableSlot> = vec![];
        let blocks = find_bookable_blocks(&slots, 1, false);
        assert_eq!(blocks.len(), 0);
    }

    #[test]
    fn test_bookable_tight_two_slot_blocks() {
        // Need 2-slot blocks in tight mode
        let slots = vec![
            make_slot(1, "2026-03-01", "10:00", "11:00", false),
            make_slot(2, "2026-03-01", "11:00", "12:00", false),
            make_slot(3, "2026-03-01", "12:00", "13:00", true), // booked
            make_slot(4, "2026-03-01", "13:00", "14:00", false),
            make_slot(5, "2026-03-01", "14:00", "15:00", false),
        ];
        // 2-slot blocks: 10-12 (end 12:00 == booked.start 12:00 → adj ✓),
        //                11-13 → slot 12-13 is booked → invalid
        //                13-15 (start 13:00 == booked.end 13:00 → adj ✓)
        let blocks = find_bookable_blocks(&slots, 2, true);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].start_time, "10:00");
        assert_eq!(blocks[0].end_time, "12:00");
        assert_eq!(blocks[1].start_time, "13:00");
        assert_eq!(blocks[1].end_time, "15:00");
    }
}
