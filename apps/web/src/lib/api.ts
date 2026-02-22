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
}

export interface Slot {
  id: number;
  date: string;
  start_time: string;
  end_time: string;
  is_booked: boolean;
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
}

// ── Client API ──

export const api = {
  getServices: () => request<Service[]>("/api/services"),

  getAvailableDates: () => request<string[]>("/api/slots/dates"),

  getSlotsByDate: (date: string) =>
    request<Slot[]>(`/api/slots?date=${date}`),

  createBooking: (serviceId: number, slotId: number) =>
    request<BookingDetail>("/api/bookings", {
      method: "POST",
      body: JSON.stringify({ service_id: serviceId, slot_id: slotId }),
    }),

  getMyBookings: () => request<BookingDetail[]>("/api/bookings/my"),

  cancelBooking: (id: number) =>
    request<string>(`/api/bookings/${id}`, { method: "DELETE" }),
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
