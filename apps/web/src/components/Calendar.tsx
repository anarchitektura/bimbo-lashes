import { createSignal, createResource, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type CalendarDay } from "../lib/api";
import Loader from "./Loader";

const MONTH_NAMES = [
  "Январь", "Февраль", "Март", "Апрель", "Май", "Июнь",
  "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь",
];
const WEEKDAYS = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];

interface CalendarProps {
  serviceId?: number;
  onSelect: (date: string) => void;
  selectedDate?: string;
  adminMode?: boolean; // admin can click any future date
}

export default function Calendar(props: CalendarProps) {
  const today = new Date();
  const todayStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, "0")}-${String(today.getDate()).padStart(2, "0")}`;

  const [year, setYear] = createSignal(today.getFullYear());
  const [month, setMonth] = createSignal(today.getMonth() + 1); // 1-based

  // Fetch calendar data from API
  const [calendarData, { refetch }] = createResource(
    () => ({ y: year(), m: month(), sid: props.serviceId }),
    (params) => api.getCalendar(params.y, params.m, params.sid)
  );

  const prevMonth = () => {
    WebApp.HapticFeedback.selectionChanged();
    if (month() === 1) {
      setMonth(12);
      setYear((y) => y - 1);
    } else {
      setMonth((m) => m - 1);
    }
  };

  const nextMonth = () => {
    WebApp.HapticFeedback.selectionChanged();
    if (month() === 12) {
      setMonth(1);
      setYear((y) => y + 1);
    } else {
      setMonth((m) => m + 1);
    }
  };

  // Can go back only if current month is not this month
  const canGoPrev = () => {
    const now = new Date();
    return year() > now.getFullYear() || (year() === now.getFullYear() && month() > now.getMonth() + 1);
  };

  // Build grid cells for the month
  const gridCells = () => {
    const firstDay = new Date(year(), month() - 1, 1);
    // Monday = 0, Sunday = 6
    let startDay = firstDay.getDay() - 1;
    if (startDay < 0) startDay = 6;

    const daysInMonth = new Date(year(), month(), 0).getDate();
    const data = calendarData() || [];
    const dayMap = new Map<string, CalendarDay>();
    for (const d of data) {
      dayMap.set(d.date, d);
    }

    const cells: Array<{
      day: number;
      date: string;
      info: CalendarDay | null;
      isPast: boolean;
      isToday: boolean;
    } | null> = [];

    // Empty cells for offset
    for (let i = 0; i < startDay; i++) {
      cells.push(null);
    }

    for (let d = 1; d <= daysInMonth; d++) {
      const dateStr = `${year()}-${String(month()).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
      const info = dayMap.get(dateStr) || null;
      const isPast = dateStr < todayStr;
      const isToday = dateStr === todayStr;

      cells.push({ day: d, date: dateStr, info, isPast, isToday });
    }

    return cells;
  };

  const getDayStyle = (cell: NonNullable<ReturnType<typeof gridCells>[0]>) => {
    // Selected
    if (cell.date === props.selectedDate) {
      return { background: "var(--btn)", color: "var(--btn-text)" };
    }
    // Past
    if (cell.isPast) {
      return { background: "transparent", color: "var(--hint)", opacity: "0.4" };
    }
    const info = cell.info;
    if (!info || info.total === 0) {
      // No slots opened
      return { background: "transparent", color: "var(--hint)", opacity: "0.6" };
    }
    if (info.free === 0) {
      // Fully booked
      return { background: "#fce4e4", color: "#d32f2f" };
    }
    if (info.free >= info.total * 0.5) {
      // Many free (>= 50%)
      return { background: "#e8f5e9", color: "#2e7d32" };
    }
    // Partially booked
    return { background: "#fff3e0", color: "#e65100" };
  };

  const isClickable = (cell: NonNullable<ReturnType<typeof gridCells>[0]>) => {
    if (cell.isPast) return false;
    if (props.adminMode) return true; // admin can click any future date
    return cell.info?.bookable === true;
  };

  const handleClick = (cell: NonNullable<ReturnType<typeof gridCells>[0]>) => {
    if (!isClickable(cell)) return;
    WebApp.HapticFeedback.selectionChanged();
    props.onSelect(cell.date);
  };

  return (
    <div class="calendar">
      {/* Header with month/year and nav */}
      <div class="cal-header">
        <button
          class="cal-nav"
          onClick={prevMonth}
          disabled={!canGoPrev()}
          style={{ opacity: canGoPrev() ? "1" : "0.3" }}
        >
          ←
        </button>
        <span class="cal-title">
          {MONTH_NAMES[month() - 1]} {year()}
        </span>
        <button class="cal-nav" onClick={nextMonth}>
          →
        </button>
      </div>

      {/* Weekday headers */}
      <div class="cal-grid">
        <For each={WEEKDAYS}>
          {(wd) => (
            <div class="cal-weekday" style={{ color: "var(--hint)" }}>
              {wd}
            </div>
          )}
        </For>
      </div>

      {/* Day cells */}
      <Show when={!calendarData.loading} fallback={<div class="py-6"><Loader /></div>}>
        <div class="cal-grid">
          <For each={gridCells()}>
            {(cell) => {
              if (!cell) {
                return <div class="cal-cell" />;
              }
              const style = getDayStyle(cell);
              const clickable = isClickable(cell);

              return (
                <div
                  class={`cal-cell ${clickable ? "cal-clickable" : ""} ${cell.isToday ? "cal-today" : ""}`}
                  style={style}
                  onClick={() => handleClick(cell)}
                >
                  {cell.day}
                </div>
              );
            }}
          </For>
        </div>
      </Show>

      {/* Legend */}
      <div class="cal-legend">
        <span class="cal-legend-item">
          <span class="cal-dot" style={{ background: "#4caf50" }} /> свободно
        </span>
        <span class="cal-legend-item">
          <span class="cal-dot" style={{ background: "#ff9800" }} /> почти
        </span>
        <span class="cal-legend-item">
          <span class="cal-dot" style={{ background: "#d32f2f" }} /> занято
        </span>
      </div>
    </div>
  );
}
