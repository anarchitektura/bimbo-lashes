/** Минимальное время до визита для отмены (часы) */
export const MIN_CANCEL_HOURS = 3;

/** За сколько часов до визита отправлять напоминание */
export const REMINDER_HOURS_BEFORE = 24;

/** Максимум дней вперёд для записи */
export const MAX_BOOKING_DAYS_AHEAD = 30;

/** Максимум слотов на один день */
export const MAX_SLOTS_PER_DAY = 10;

/** Статусы записей */
export const BOOKING_STATUS = {
  CONFIRMED: "confirmed",
  CANCELLED: "cancelled",
} as const;

/** Шаблоны слотов для быстрого добавления */
export const SLOT_TEMPLATES = {
  MORNING: {
    label: "Утро 9–13",
    icon: "🌅",
    slots: [
      { start_time: "09:00", end_time: "11:00" },
      { start_time: "11:00", end_time: "13:00" },
    ],
  },
  AFTERNOON: {
    label: "День 13–18",
    icon: "☀️",
    slots: [
      { start_time: "13:00", end_time: "15:00" },
      { start_time: "15:00", end_time: "17:00" },
      { start_time: "17:00", end_time: "18:00" },
    ],
  },
  FULL_DAY: {
    label: "Полный день",
    icon: "📅",
    slots: [
      { start_time: "09:00", end_time: "11:00" },
      { start_time: "11:00", end_time: "13:00" },
      { start_time: "13:00", end_time: "15:00" },
      { start_time: "15:00", end_time: "17:00" },
      { start_time: "17:00", end_time: "19:00" },
    ],
  },
} as const;

/** Дефолтные услуги (для seed) */
export const DEFAULT_SERVICES = [
  { name: "Классика", description: "Классическое наращивание 1:1", price: 2500, duration_min: 120 },
  { name: "2D", description: "Объёмное наращивание 2D", price: 3000, duration_min: 150 },
  { name: "3D", description: "Объёмное наращивание 3D", price: 3500, duration_min: 150 },
  { name: "Мега-объём", description: "Голливудское наращивание 4D-6D", price: 4500, duration_min: 180 },
  { name: "Коррекция", description: "Коррекция наращивания", price: 2000, duration_min: 90 },
  { name: "Снятие", description: "Снятие наращенных ресниц", price: 500, duration_min: 30 },
  { name: "Ламинирование", description: "Ламинирование и ботокс ресниц", price: 2500, duration_min: 60 },
] as const;
