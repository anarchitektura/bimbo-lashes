import WebApp from "@twa-dev/sdk";

const BASE_URL = import.meta.env.VITE_API_URL || "";

function getAuthHeader(): string {
  return `tma ${WebApp.initData}`;
}

async function request<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const res = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      Authorization: getAuthHeader(),
      ...options.headers,
    },
  });

  const json = await res.json();

  if (!json.ok) {
    throw new Error(json.error || "Ошибка сервера");
  }

  return json.data;
}

// ── Types ──

export interface Service {
  id: number;
  name: string;
  description: string;
  price: number;
  duration_min: number;
  is_active: boolean;
  sort_order: number;
  service_type: string;
}

export interface Slot {
  id: number;
  date: string;
  start_time: string;
  end_time: string;
  is_booked: boolean;
  booking_id: number | null;
}

export interface TimeBlock {
  start_time: string;
  end_time: string;
}

export interface AvailableTimes {
  mode: "free" | "tight";
  times: TimeBlock[];
}

export interface CalendarDay {
  date: string;
  total: number;
  free: number;
  bookable: boolean;
}

export interface AddonInfo {
  name: string;
  price: number;
  service_id: number;
}

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
  status: string;
  created_at: string;
  with_lower_lashes?: boolean;
  total_price?: number;
  payment_status?: string;
  prepaid_amount?: number;
}

export interface CreateBookingResponse {
  booking: BookingDetail;
  payment_url?: string;
}

export interface BookingStatusResponse {
  status: string;
  payment_status: string;
}

export interface CancelBookingResponse {
  message: string;
  refund_info?: string;
}

// ── Client API ──

export const api = {
  getServices: () => request<Service[]>("/api/services"),

  getAddonInfo: () => request<AddonInfo | null>("/api/addon-info"),

  getAvailableDates: (serviceId: number) =>
    request<string[]>(`/api/available-dates?service_id=${serviceId}`),

  getCalendar: (year: number, month: number, serviceId?: number) => {
    const params = new URLSearchParams({ year: String(year), month: String(month) });
    if (serviceId) params.set("service_id", String(serviceId));
    return request<CalendarDay[]>(`/api/calendar?${params}`);
  },

  getAvailableTimes: (date: string, serviceId: number) =>
    request<AvailableTimes>(`/api/available-times?date=${date}&service_id=${serviceId}`),

  createBooking: (serviceId: number, date: string, startTime: string, withLowerLashes: boolean = false) =>
    request<CreateBookingResponse>("/api/bookings", {
      method: "POST",
      body: JSON.stringify({
        service_id: serviceId,
        date,
        start_time: startTime,
        with_lower_lashes: withLowerLashes,
      }),
    }),

  getMyBookings: () => request<BookingDetail[]>("/api/bookings/my"),

  cancelBooking: (id: number) =>
    request<CancelBookingResponse>(`/api/bookings/${id}`, { method: "DELETE" }),

  getBookingStatus: (id: number) =>
    request<BookingStatusResponse>(`/api/bookings/${id}/status`),
};

// ── Admin API ──

export const adminApi = {
  getServices: () => request<Service[]>("/api/admin/services"),

  createService: (data: {
    name: string;
    description?: string;
    price: number;
    duration_min: number;
  }) =>
    request<Service>("/api/admin/services", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  updateService: (id: number, data: Partial<Service>) =>
    request<Service>(`/api/admin/services/${id}`, {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  getSlots: (date: string) =>
    request<Slot[]>(`/api/admin/slots?date=${date}`),

  createSlots: (date: string, slots: { start_time: string; end_time: string }[]) =>
    request<Slot[]>("/api/admin/slots", {
      method: "POST",
      body: JSON.stringify({ date, slots }),
    }),

  openDay: (date: string, startHour?: number, endHour?: number) =>
    request<Slot[]>("/api/admin/openday", {
      method: "POST",
      body: JSON.stringify({ date, start_hour: startHour, end_hour: endHour }),
    }),

  deleteSlot: (id: number) =>
    request<string>(`/api/admin/slots/${id}`, { method: "DELETE" }),

  getBookings: (params?: { date?: string; from?: string; to?: string }) => {
    const query = new URLSearchParams();
    if (params?.date) query.set("date", params.date);
    if (params?.from) query.set("from", params.from);
    if (params?.to) query.set("to", params.to);
    const qs = query.toString();
    return request<BookingDetail[]>(`/api/admin/bookings${qs ? `?${qs}` : ""}`);
  },

  cancelBooking: (id: number) =>
    request<string>(`/api/admin/bookings/${id}/cancel`, { method: "POST" }),
};
