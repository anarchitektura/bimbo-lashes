use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Database models ──

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Service {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub price: i64,
    pub duration_min: i64,
    pub is_active: bool,
    pub sort_order: i64,
    pub service_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AvailableSlot {
    pub id: i64,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub is_booked: bool,
    pub booking_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Booking {
    pub id: i64,
    pub service_id: i64,
    pub slot_id: i64,
    pub client_tg_id: i64,
    pub client_username: Option<String>,
    pub client_first_name: String,
    pub status: String,
    pub reminder_sent: bool,
    pub created_at: String,
    pub cancelled_at: Option<String>,
    pub date: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub with_lower_lashes: bool,
    pub payment_status: String,
    pub yookassa_payment_id: Option<String>,
    pub prepaid_amount: i64,
}

// ── API request/response types ──

#[derive(Debug, Deserialize)]
pub struct CreateBookingRequest {
    pub service_id: i64,
    pub date: String,
    pub start_time: String,
    #[serde(default)]
    pub with_lower_lashes: bool,
}

#[derive(Debug, Deserialize)]
pub struct AvailableTimesQuery {
    pub date: String,
    pub service_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct AvailableDatesQuery {
    pub service_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TimeBlock {
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Serialize)]
pub struct AvailableTimesResponse {
    pub mode: String,
    pub times: Vec<TimeBlock>,
}

#[derive(Debug, Serialize)]
pub struct AddonInfo {
    pub name: String,
    pub price: i64,
    pub service_id: i64,
}

#[derive(Debug, Deserialize)]
pub struct OpenDayRequest {
    pub date: String,
    pub start_hour: Option<u32>,
    pub end_hour: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CalendarQuery {
    pub year: i32,
    pub month: u32,
    pub service_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CalendarDay {
    pub date: String,
    pub total: i64,
    pub free: i64,
    pub bookable: bool,
}

#[derive(Debug, Deserialize)]
pub struct SlotsQuery {
    pub date: String,
}

#[derive(Debug, Deserialize)]
pub struct BookingsQuery {
    pub date: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateServiceRequest {
    pub name: String,
    pub description: Option<String>,
    pub price: i64,
    pub duration_min: i64,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateServiceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<i64>,
    pub duration_min: Option<i64>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSlotsRequest {
    pub date: String,
    pub slots: Vec<SlotTime>,
}

#[derive(Debug, Deserialize)]
pub struct SlotTime {
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BookingDetail {
    pub id: i64,
    pub service_name: String,
    pub service_price: i64,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub client_tg_id: i64,
    pub client_username: Option<String>,
    pub client_first_name: String,
    pub status: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with_lower_lashes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_price: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepaid_amount: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// ── Payment types ──

#[derive(Debug, Serialize)]
pub struct CreateBookingResponse {
    pub booking: BookingDetail,
    pub payment_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BookingStatusResponse {
    pub status: String,
    pub payment_status: String,
}

#[derive(Debug, Deserialize)]
pub struct YooKassaWebhookEvent {
    pub event: String,
    pub object: YooKassaPaymentObject,
}

#[derive(Debug, Deserialize)]
pub struct YooKassaPaymentObject {
    pub id: String,
    pub status: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct CancelBookingResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refund_info: Option<String>,
}

// ── Telegram auth ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}
