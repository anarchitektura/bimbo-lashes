/** Format price: 2500 → "2 500 ₽" */
export function formatPrice(price: number): string {
  return price.toLocaleString("ru-RU") + " ₽";
}

/** Format date full: "2026-02-26" → "26.02 (26 февраля)" */
export function formatDate(dateStr: string): string {
  const months = [
    "января", "февраля", "марта", "апреля", "мая", "июня",
    "июля", "августа", "сентября", "октября", "ноября", "декабря",
  ];
  const [, m, d] = dateStr.split("-");
  const month = months[parseInt(m, 10) - 1];
  const day = parseInt(d, 10);
  return `${d}.${m} (${day} ${month})`;
}

/** Format date short for chips: "2026-02-26" → "26.02, чт" */
export function formatDateShort(dateStr: string): string {
  const days = ["вс", "пн", "вт", "ср", "чт", "пт", "сб"];
  const [, m, d] = dateStr.split("-");
  const date = new Date(dateStr + "T00:00:00");
  const wd = days[date.getDay()];
  return `${d}.${m}, ${wd}`;
}

/** Format time range: "14:00" → "14:00" */
export function formatTime(time: string): string {
  return time.slice(0, 5);
}

/** Duration label: 120 → "2 ч", 90 → "1.5 ч", 30 → "30 мин" */
export function formatDuration(min: number): string {
  if (min < 60) return `${min} мин`;
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (m === 0) return `${h} ч`;
  return `${h} ч ${m} мин`;
}

/** Check if a date is today */
export function isToday(dateStr: string): boolean {
  const today = new Date().toISOString().split("T")[0];
  return dateStr === today;
}

/** Check if a date is tomorrow */
export function isTomorrow(dateStr: string): boolean {
  const tomorrow = new Date(Date.now() + 86400000).toISOString().split("T")[0];
  return dateStr === tomorrow;
}

/** Friendly date: today/tomorrow/formatted */
export function friendlyDate(dateStr: string): string {
  if (isToday(dateStr)) return "Сегодня";
  if (isTomorrow(dateStr)) return "Завтра";
  return formatDate(dateStr);
}
