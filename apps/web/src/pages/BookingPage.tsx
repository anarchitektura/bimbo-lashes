import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type TimeBlock } from "../lib/api";
import { goHome, goMyBookings } from "../lib/router";
import { formatPrice, formatDateShort, friendlyDate, formatTime } from "../lib/utils";
import Loader from "../components/Loader";

interface Props {
  serviceId: number;
  serviceName: string;
  servicePrice: number;
  withLowerLashes: boolean;
}

type Step = "date" | "time" | "confirm" | "done";

export default function BookingPage(props: Props) {
  const [step, setStep] = createSignal<Step>("date");
  const [selectedDate, setSelectedDate] = createSignal<string>("");
  const [selectedTime, setSelectedTime] = createSignal<TimeBlock | null>(null);
  const [bookingMode, setBookingMode] = createSignal<"free" | "tight">("free");
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal("");

  // Fetch available dates for this service
  const [dates] = createResource(() => api.getAvailableDates(props.serviceId));

  // Fetch available times when date is selected
  const [timesData] = createResource(
    () => selectedDate(),
    (date) =>
      date
        ? api.getAvailableTimes(date, props.serviceId)
        : Promise.resolve({ mode: "free" as const, times: [] })
  );

  const selectDate = (date: string) => {
    WebApp.HapticFeedback.selectionChanged();
    setSelectedDate(date);
    setStep("time");
  };

  const selectTime = (time: TimeBlock) => {
    WebApp.HapticFeedback.selectionChanged();
    setSelectedTime(time);
    if (timesData()) {
      setBookingMode(timesData()!.mode);
    }
    setStep("confirm");
  };

  const confirmBooking = async () => {
    const time = selectedTime();
    if (!time) return;

    setLoading(true);
    setError("");

    try {
      await api.createBooking(
        props.serviceId,
        selectedDate(),
        time.start_time,
        props.withLowerLashes
      );
      WebApp.HapticFeedback.notificationOccurred("success");
      setStep("done");
    } catch (e: any) {
      WebApp.HapticFeedback.notificationOccurred("error");
      setError(e.message || "Не удалось записаться");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div class="animate-fade-in">
      {/* Header */}
      <div class="px-4 pt-6 pb-2">
        <h2 class="text-xl font-bold" style={{ color: "var(--text)" }}>
          {props.serviceName}
        </h2>
        <p class="text-sm mt-0.5" style={{ color: "var(--btn)" }}>
          {formatPrice(props.servicePrice)}
        </p>
      </div>

      {/* Progress indicator */}
      <div class="px-4 py-3 flex gap-1.5">
        <div
          class="h-1 rounded-full flex-1 transition-all duration-300"
          style={{
            background: step() !== "done" || step() === "date" || step() === "time" || step() === "confirm"
              ? "var(--btn)"
              : "var(--secondary-bg)",
          }}
        />
        <div
          class="h-1 rounded-full flex-1 transition-all duration-300"
          style={{
            background: step() === "time" || step() === "confirm" || step() === "done"
              ? "var(--btn)"
              : "var(--secondary-bg)",
          }}
        />
        <div
          class="h-1 rounded-full flex-1 transition-all duration-300"
          style={{
            background: step() === "confirm" || step() === "done"
              ? "var(--btn)"
              : "var(--secondary-bg)",
          }}
        />
      </div>

      {/* Step: Select date */}
      <Show when={step() === "date"}>
        <div class="px-4 animate-slide-up">
          <p class="text-sm font-medium mb-3" style={{ color: "var(--hint)" }}>
            📅 Выбери дату
          </p>
          <Show when={!dates.loading} fallback={<Loader />}>
            <Show when={dates()?.length} fallback={
              <div class="text-center py-12" style={{ color: "var(--hint)" }}>
                <p class="text-4xl mb-2">😿</p>
                <p>Нет доступных дат</p>
                <p class="text-sm mt-1">Мастер скоро откроет запись</p>
              </div>
            }>
              <div class="flex flex-wrap gap-2">
                <For each={dates()}>
                  {(date) => (
                    <button
                      class="chip chip-inactive"
                      onClick={() => selectDate(date)}
                    >
                      {formatDateShort(date)}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </Show>
        </div>
      </Show>

      {/* Step: Select time */}
      <Show when={step() === "time"}>
        <div class="px-4 animate-slide-up">
          <p class="text-sm font-medium mb-1" style={{ color: "var(--hint)" }}>
            🕐 Выбери время
          </p>
          <div class="flex items-center gap-2 mb-3">
            <p class="text-xs" style={{ color: "var(--hint)" }}>
              {friendlyDate(selectedDate())}
            </p>
            <Show when={timesData()?.mode === "tight"}>
              <span
                class="text-xs px-2 py-0.5 rounded-full"
                style={{ background: "#fff3e0", color: "#e65100" }}
              >
                оптимальное
              </span>
            </Show>
          </div>
          <Show when={!timesData.loading} fallback={<Loader />}>
            <Show when={timesData()?.times?.length} fallback={
              <div class="text-center py-8" style={{ color: "var(--hint)" }}>
                <p>Нет свободного времени</p>
              </div>
            }>
              <div class="grid grid-cols-3 gap-2">
                <For each={timesData()?.times}>
                  {(time) => (
                    <button
                      class="chip chip-inactive text-center justify-center"
                      onClick={() => selectTime(time)}
                    >
                      {formatTime(time.start_time)}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </Show>

          <button
            class="mt-4 text-sm font-medium"
            style={{ color: "var(--link)" }}
            onClick={() => {
              setSelectedDate("");
              setStep("date");
            }}
          >
            ← Другую дату
          </button>
        </div>
      </Show>

      {/* Step: Confirm */}
      <Show when={step() === "confirm"}>
        <div class="px-4 animate-slide-up">
          <div class="card">
            <p class="text-sm font-medium mb-3" style={{ color: "var(--hint)" }}>
              Подтверди запись
            </p>

            <div class="space-y-3">
              <div class="flex justify-between">
                <span style={{ color: "var(--hint)" }}>Услуга</span>
                <span class="font-medium">{props.serviceName}</span>
              </div>
              <div class="flex justify-between">
                <span style={{ color: "var(--hint)" }}>Дата</span>
                <span class="font-medium">{friendlyDate(selectedDate())}</span>
              </div>
              <div class="flex justify-between">
                <span style={{ color: "var(--hint)" }}>Время</span>
                <span class="font-medium">
                  {formatTime(selectedTime()!.start_time)} — {formatTime(selectedTime()!.end_time)}
                </span>
              </div>
              <div
                class="flex justify-between pt-2 border-t"
                style={{ "border-color": "var(--secondary-bg)" }}
              >
                <span class="font-semibold">Стоимость</span>
                <span class="font-bold" style={{ color: "var(--btn)" }}>
                  {formatPrice(props.servicePrice)}
                </span>
              </div>
            </div>
          </div>

          <Show when={error()}>
            <div
              class="mt-3 p-3 rounded-xl text-sm text-center"
              style={{ background: "#fce4e4", color: "#d32f2f" }}
            >
              {error()}
            </div>
          </Show>

          <button
            class="btn-primary mt-4"
            disabled={loading()}
            onClick={confirmBooking}
          >
            {loading() ? "Записываю..." : "💅 Записаться"}
          </button>

          <button
            class="mt-3 text-sm font-medium w-full text-center"
            style={{ color: "var(--link)" }}
            onClick={() => setStep("time")}
          >
            ← Другое время
          </button>
        </div>
      </Show>

      {/* Step: Done */}
      <Show when={step() === "done"}>
        <div class="px-4 text-center animate-slide-up py-8">
          <p class="text-5xl mb-4">🎉</p>
          <h3 class="text-xl font-bold mb-2">Ты записана!</h3>
          <p class="text-sm mb-1" style={{ color: "var(--hint)" }}>
            {props.serviceName}
          </p>
          <p class="text-sm" style={{ color: "var(--hint)" }}>
            {friendlyDate(selectedDate())} в {formatTime(selectedTime()!.start_time)}
          </p>
          <p class="text-xs mt-4" style={{ color: "var(--hint)" }}>
            Мы напомним тебе за день до визита 💕
          </p>

          <div class="mt-6 flex flex-col gap-2">
            <button class="btn-primary" onClick={() => goMyBookings()}>
              📋 Мои записи
            </button>
            <button class="btn-secondary w-full" onClick={() => goHome()}>
              На главную
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
}
