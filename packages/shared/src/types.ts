// ── Domain models ──
// These mirror the Rust structs in apps/server/src/models.rs

export interface Service {
  id: number;
  name: string;
  description: string;
  price: number;
  duration_min: number;
  is_active: boolean;
  sort_order: number;
}

export interface AvailableSlot {
  id: number;
  date: string; // YYYY-MM-DD
  start_time: string; // HH:MM
  end_time: string; // HH:MM
  is_booked: boolean;
}

export interface Booking {
  id: number;
  service_id: number;
  slot_id: number;
  client_tg_id: number;
  client_username: string | null;
  client_first_name: string;
  status: BookingStatus;
  reminder_sent: boolean;
  created_at: string;
  cancelled_at: string | null;
}

export type BookingStatus = "confirmed" | "cancelled";

export interface BookingDetail {
  id: number;
  service_name: string;
  service_price: number;
  date: string;
  start_time: string;
  end_time: string;
  client_tg_id: number;
  client_username: string | null;
  client_first_name: string;
  status: BookingStatus;
  created_at: string;
}

// ── API request types ──

export interface CreateBookingRequest {
  service_id: number;
  slot_id: number;
}

export interface CreateServiceRequest {
  name: string;
  description?: string;
  price: number;
  duration_min: number;
  sort_order?: number;
}

export interface UpdateServiceRequest {
  name?: string;
  description?: string;
  price?: number;
  duration_min?: number;
  is_active?: boolean;
  sort_order?: number;
}

export interface CreateSlotsRequest {
  date: string;
  slots: SlotTime[];
}

export interface SlotTime {
  start_time: string;
  end_time: string;
}

// ── API response wrapper ──

export interface ApiResponse<T> {
  ok: boolean;
  data: T | null;
  error: string | null;
}

// ── Query params ──

export interface SlotsQuery {
  date: string;
}

export interface BookingsQuery {
  date?: string;
  from?: string;
  to?: string;
}

// ── Telegram user (from initData) ──

export interface TelegramUser {
  id: number;
  first_name: string;
  last_name?: string;
  username?: string;
  language_code?: string;
  is_premium?: boolean;
}
