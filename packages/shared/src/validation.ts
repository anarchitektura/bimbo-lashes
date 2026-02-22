import type { CreateServiceRequest, SlotTime } from "./types";

/** Проверка формата даты YYYY-MM-DD */
export function isValidDate(date: string): boolean {
  return /^\d{4}-\d{2}-\d{2}$/.test(date) && !isNaN(Date.parse(date));
}

/** Проверка формата времени HH:MM */
export function isValidTime(time: string): boolean {
  return /^\d{2}:\d{2}$/.test(time);
}

/** Проверка: начало раньше конца */
export function isValidTimeRange(start: string, end: string): boolean {
  return isValidTime(start) && isValidTime(end) && start < end;
}

/** Проверка: дата не в прошлом */
export function isNotPastDate(date: string): boolean {
  const today = new Date().toISOString().split("T")[0];
  return date >= today;
}

/** Валидация запроса создания услуги */
export function validateCreateService(data: CreateServiceRequest): string | null {
  if (!data.name || data.name.trim().length === 0) {
    return "Название не может быть пустым";
  }
  if (data.name.length > 100) {
    return "Название слишком длинное (макс. 100 символов)";
  }
  if (data.price <= 0) {
    return "Цена должна быть больше 0";
  }
  if (data.price > 100000) {
    return "Цена слишком большая";
  }
  if (data.duration_min <= 0) {
    return "Длительность должна быть больше 0";
  }
  if (data.duration_min > 480) {
    return "Длительность слишком большая (макс. 8 часов)";
  }
  return null;
}

/** Валидация слота */
export function validateSlot(slot: SlotTime): string | null {
  if (!isValidTime(slot.start_time)) {
    return "Неверный формат времени начала";
  }
  if (!isValidTime(slot.end_time)) {
    return "Неверный формат времени конца";
  }
  if (slot.start_time >= slot.end_time) {
    return "Время начала должно быть раньше конца";
  }
  return null;
}

/** Форматирование цены: 2500 → "2 500 ₽" */
export function formatPrice(price: number): string {
  return price.toLocaleString("ru-RU") + " ₽";
}

/** Форматирование длительности: 120 → "2 ч", 90 → "1 ч 30 мин" */
export function formatDuration(min: number): string {
  if (min < 60) return `${min} мин`;
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (m === 0) return `${h} ч`;
  return `${h} ч ${m} мин`;
}
