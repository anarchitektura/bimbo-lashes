use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    Json,
};
use chrono::Datelike;
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

/// GET /api/services — list active main services (hides addons)
pub async fn list_services(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<Service>>>, StatusCode> {
    let services = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE is_active = 1 AND service_type = 'main' ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ApiResponse::success(services)))
}

/// GET /api/addon-info — returns addon (lower lashes) info for frontend
pub async fn addon_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Option<AddonInfo>>>, StatusCode> {
    let addon = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT id, name, price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1"
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let info = addon.map(|(id, name, price)| AddonInfo {
        service_id: id,
        name,
        price,
    });

    Ok(Json(ApiResponse::success(info)))
}

/// GET /api/available-dates?service_id=N — dates with enough consecutive free slots
pub async fn available_dates_for_service(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AvailableDatesQuery>,
) -> Result<Json<ApiResponse<Vec<String>>>, StatusCode> {
    // Get the service to know how many slots needed (default: 1 if no service_id)
    let slots_needed = if let Some(service_id) = query.service_id {
        let service = sqlx::query_as::<_, Service>(
            "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
             FROM services WHERE id = ? AND is_active = 1"
        )
        .bind(service_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match service {
            Some(s) => (s.duration_min as f64 / 60.0).ceil() as i64,
            None => return Ok(Json(ApiResponse::success(vec![]))),
        }
    } else {
        1 // Default: any date with at least 1 free slot
    };

    // Get all dates with free slots in the future
    let dates: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT date FROM available_slots
         WHERE is_booked = 0 AND date >= date('now')
         ORDER BY date ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Filter: only dates with enough consecutive free slots
    let mut valid_dates = Vec::new();
    for date in &dates {
        let slots = sqlx::query_as::<_, AvailableSlot>(
            "SELECT id, date, start_time, end_time, is_booked, booking_id
             FROM available_slots WHERE date = ? ORDER BY start_time ASC"
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

/// GET /api/available-times?date=YYYY-MM-DD&service_id=N — smart slot availability
pub async fn available_times(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AvailableTimesQuery>,
) -> Result<Json<ApiResponse<AvailableTimesResponse>>, StatusCode> {
    // Get service
    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ? AND is_active = 1"
    )
    .bind(query.service_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let service = match service {
        Some(s) => s,
        None => return Ok(Json(ApiResponse::success(AvailableTimesResponse {
            mode: "free".into(),
            times: vec![],
        }))),
    };

    let slots_needed = (service.duration_min as f64 / 60.0).ceil() as usize;

    // Get all slots for this date
    let slots = sqlx::query_as::<_, AvailableSlot>(
        "SELECT id, date, start_time, end_time, is_booked, booking_id
         FROM available_slots WHERE date = ? ORDER BY start_time ASC"
    )
    .bind(&query.date)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Calculate days until
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let days_until = days_between(&today, &query.date);
    let is_tight = days_until <= 3;

    // Find all valid time blocks (groups of N consecutive free slots)
    let mut time_blocks = Vec::new();
    let has_bookings = slots.iter().any(|s| s.is_booked);

    for i in 0..slots.len() {
        if slots[i].is_booked {
            continue;
        }

        // Check if we have slots_needed consecutive free slots starting at i
        if i + slots_needed > slots.len() {
            break;
        }

        let mut all_free = true;
        let mut consecutive = true;
        for j in 0..slots_needed {
            let idx = i + j;
            if slots[idx].is_booked {
                all_free = false;
                break;
            }
            // Check consecutive: this slot's start == previous slot's end
            if j > 0 {
                let prev = &slots[i + j - 1];
                let curr = &slots[idx];
                if prev.end_time != curr.start_time {
                    consecutive = false;
                    break;
                }
            }
        }

        if all_free && consecutive {
            let block_start = &slots[i].start_time;
            let block_end = &slots[i + slots_needed - 1].end_time;

            if is_tight {
                if !has_bookings {
                    // Tight mode but no bookings yet — show all (no reference point to optimize)
                    time_blocks.push(TimeBlock {
                        start_time: block_start.clone(),
                        end_time: block_end.clone(),
                    });
                } else {
                    // Tight mode with bookings — only adjacent to booked slots (no edges)
                    if is_strictly_adjacent_to_booked(block_start, block_end, &slots) {
                        time_blocks.push(TimeBlock {
                            start_time: block_start.clone(),
                            end_time: block_end.clone(),
                        });
                    }
                }
            } else {
                // Free mode: all valid blocks
                time_blocks.push(TimeBlock {
                    start_time: block_start.clone(),
                    end_time: block_end.clone(),
                });
            }
        }
    }

    Ok(Json(ApiResponse::success(AvailableTimesResponse {
        mode: if is_tight { "tight".into() } else { "free".into() },
        times: time_blocks,
    })))
}

/// POST /api/bookings — create a new booking (smart multi-slot)
pub async fn create_booking(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateBookingRequest>,
) -> Result<Json<ApiResponse<BookingDetail>>, (StatusCode, Json<ApiResponse<()>>)> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    let user = extract_user(auth_header, &state.bot_token)?;

    // Get service
    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
         FROM services WHERE id = ? AND is_active = 1"
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
         ORDER BY start_time ASC"
    )
    .bind(&body.date)
    .bind(&body.start_time)
    .bind(&end_time)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?;

    let slots_needed = (service.duration_min as f64 / 60.0).ceil() as usize;
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
            "SELECT price FROM services WHERE service_type = 'addon' AND is_active = 1 LIMIT 1"
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
        .unwrap_or(500)
    } else {
        0
    };
    let total_price = service.price + addon_price;

    // Create booking (slot_id = first slot for backward compat)
    let first_slot_id = slots[0].id;
    let booking_id = sqlx::query(
        "INSERT INTO bookings (service_id, slot_id, client_tg_id, client_username, client_first_name, status, date, start_time, end_time, with_lower_lashes)
         VALUES (?, ?, ?, ?, ?, 'confirmed', ?, ?, ?, ?)"
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
    .execute(&state.db)
    .await
    .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiResponse::error("DB error"))))?
    .last_insert_rowid();

    // Mark all slots as booked with this booking_id
    for slot in &slots {
        sqlx::query("UPDATE available_slots SET is_booked = 1, booking_id = ? WHERE id = ?")
            .bind(booking_id)
            .bind(slot.id)
            .execute(&state.db)
            .await
            .ok();
    }

    // Notify admin via Telegram bot
    let mention = user
        .username
        .as_ref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| user.first_name.clone());

    let addon_text = if body.with_lower_lashes { "\n   + нижние ресницы" } else { "" };
    let message = format!(
        "📋 Новая запись!\n\n\
         👤 {} \n\
         💅 {}{}\n\
         📅 {} в {} — {}\n\
         💰 {} ₽",
        mention, service.name, addon_text, body.date, body.start_time, end_time, total_price
    );

    notify_admin(&state.bot_token, state.admin_tg_id, &message).await;

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
        status: "confirmed".into(),
        created_at: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        with_lower_lashes: Some(body.with_lower_lashes),
        total_price: Some(total_price),
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
         WHERE b.client_tg_id = ? AND b.status = 'confirmed'
         AND COALESCE(b.date, sl.date) >= date('now')
         ORDER BY COALESCE(b.date, sl.date) ASC, COALESCE(b.start_time, sl.start_time) ASC"
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

    // Free all slots belonging to this booking
    sqlx::query("UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE booking_id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .ok();

    // Also free by slot_id for backward compat
    sqlx::query("UPDATE available_slots SET is_booked = 0, booking_id = NULL WHERE id = ?")
        .bind(booking.slot_id)
        .execute(&state.db)
        .await
        .ok();

    // Get info for notification
    let service = sqlx::query_as::<_, Service>(
        "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type FROM services WHERE id = ?"
    )
    .bind(booking.service_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let mention = user
        .username
        .as_ref()
        .map(|u| format!("@{}", u))
        .unwrap_or_else(|| user.first_name.clone());

    let b_date = booking.date.as_deref().unwrap_or("?");
    let b_start = booking.start_time.as_deref().unwrap_or("?");

    if let Some(svc) = service {
        let message = format!(
            "❌ Отмена записи\n\n\
             👤 {}\n\
             💅 {}\n\
             📅 {} в {}",
            mention, svc.name, b_date, b_start
        );
        notify_admin(&state.bot_token, state.admin_tg_id, &message).await;
    }

    Ok(Json(ApiResponse::success("Запись отменена")))
}

/// GET /api/calendar?year=2026&month=2&service_id=1 — calendar data with slot stats
pub async fn calendar(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
) -> Result<Json<ApiResponse<Vec<CalendarDay>>>, StatusCode> {
    // Calculate slots_needed from service if provided
    let slots_needed = if let Some(service_id) = query.service_id {
        let service = sqlx::query_as::<_, Service>(
            "SELECT id, name, description, price, duration_min, is_active, sort_order, service_type
             FROM services WHERE id = ? AND is_active = 1"
        )
        .bind(service_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        match service {
            Some(s) => (s.duration_min as f64 / 60.0).ceil() as i64,
            None => 1,
        }
    } else {
        1
    };

    // Build date range for the month
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

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let mut calendar_days = Vec::new();

    for day in 1..=days_in_month {
        let date = format!("{:04}-{:02}-{:02}", year, month, day);

        // Skip past dates
        if date < today {
            continue;
        }

        // Get all slots for this date
        let slots = sqlx::query_as::<_, AvailableSlot>(
            "SELECT id, date, start_time, end_time, is_booked, booking_id
             FROM available_slots WHERE date = ? ORDER BY start_time ASC"
        )
        .bind(&date)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let total = slots.len() as i64;
        let free = slots.iter().filter(|s| !s.is_booked).count() as i64;

        let bookable = if total == 0 {
            false
        } else if query.service_id.is_some() {
            has_consecutive_free_slots(&slots, slots_needed)
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

// ── Helper functions ──

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

/// Check if there are N consecutive free slots in the list
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

/// Check if a time block is strictly adjacent to a booked slot (no edge check)
fn is_strictly_adjacent_to_booked(
    block_start: &str,
    block_end: &str,
    all_slots: &[AvailableSlot],
) -> bool {
    for slot in all_slots {
        if !slot.is_booked {
            continue;
        }
        // Block starts right after a booked slot ends
        if block_start == slot.end_time {
            return true;
        }
        // Block ends right before a booked slot starts
        if block_end == slot.start_time {
            return true;
        }
    }

    false
}

/// Calculate days between two date strings (YYYY-MM-DD)
fn days_between(from: &str, to: &str) -> i64 {
    let from_date = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d");
    let to_date = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d");

    match (from_date, to_date) {
        (Ok(f), Ok(t)) => (t - f).num_days(),
        _ => 999, // default to free mode if parsing fails
    }
}

/// Add minutes to a time string "HH:MM" → "HH:MM"
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
