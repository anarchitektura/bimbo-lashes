import { createResource, createSignal, For, Show, onCleanup } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type TimeBlock } from "../lib/api";
import { goHome, goMyBookings } from "../lib/router";
import { formatPrice, friendlyDate, formatTime } from "../lib/utils";
import Loader from "../components/Loader";
import Calendar from "../components/Calendar";

interface Props {
  serviceId: number;
  serviceName: string;
  servicePrice: number;
  withLowerLashes: boolean;
}

type Step = "date" | "time" | "confirm" | "paying" | "done";

const PREPAID_AMOUNT = 500;
/** Max polling time before giving up (5 minutes). */
const POLL_TIMEOUT_MS = 5 * 60 * 1000;

export default function BookingPage(props: Props) {
  const [step, setStep] = createSignal<Step>("date");
  const [selectedDate, setSelectedDate] = createSignal<string>("");
  const [selectedTime, setSelectedTime] = createSignal<TimeBlock | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal("");
  const [paymentUrl, setPaymentUrl] = createSignal<string | null>(null);
  let pollTimer: ReturnType<typeof setInterval> | undefined;
  let pollTimeout: ReturnType<typeof setTimeout> | undefined;

  onCleanup(() => stopPolling());

  // Fetch available times when date is selected
  const [timesData] = createResource(
    () => selectedDate(),
    (date) =>
      date
        ? api.getAvailableTimes(date, props.serviceId)
        : Promise.resolve({ mode: "free" as const, times: [] })
  );

  const selectDate = (date: string) => {
    setSelectedDate(date);
    setStep("time");
  };

  const selectTime = (time: TimeBlock) => {
    WebApp.HapticFeedback.selectionChanged();
    setSelectedTime(time);
    setStep("confirm");
  };

  const stopPolling = () => {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = undefined; }
    if (pollTimeout) { clearTimeout(pollTimeout); pollTimeout = undefined; }
  };

  const startPolling = (id: number) => {
    stopPolling();

    pollTimer = setInterval(async () => {
      try {
        const status = await api.getBookingStatus(id);
        if (status.payment_status === "paid" && status.status === "confirmed") {
          stopPolling();
          WebApp.HapticFeedback.notificationOccurred("success");
          setStep("done");
        } else if (status.status === "expired" || status.status === "cancelled") {
          stopPolling();
          WebApp.HapticFeedback.notificationOccurred("error");
          setError("Время оплаты истекло. Попробуйте снова.");
          setStep("confirm");
        }
      } catch {
        // Ignore polling errors — will retry on next interval
      }
    }, 3000);

    // Safety timeout: stop polling after POLL_TIMEOUT_MS
    pollTimeout = setTimeout(() => {
      stopPolling();
      setError("Не удалось подтвердить оплату. Проверьте раздел «Мои записи».");
      setStep("confirm");
    }, POLL_TIMEOUT_MS);
  };

  const confirmBooking = async () => {
    const time = selectedTime();
    if (!time) return;

    setLoading(true);
    setError("");

    try {
      const result = await api.createBooking(
        props.serviceId,
        selectedDate(),
        time.start_time,
        props.withLowerLashes
      );

      if (result.payment_url) {
        setPaymentUrl(result.payment_url);
        // Open payment page
        WebApp.openLink(result.payment_url);
        setStep("paying");
        startPolling(result.booking.id);
      } else {
        // No payment required (shouldn't happen, but fallback)
        WebApp.HapticFeedback.notificationOccurred("success");
        setStep("done");
      }
    } catch (e: any) {
      WebApp.HapticFeedback.notificationOccurred("error");
      setError(e.message || "Не удалось записаться");
    } finally {
      setLoading(false);
    }
  };

  const reopenPayment = () => {
    const url = paymentUrl();
    if (url) {
      WebApp.openLink(url);
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
        {[0, 1, 2, 3].map((i) => (
          <div
            class="h-1 rounded-full flex-1 transition-all duration-300"
            style={{
              background:
                (i === 0) ||
                (i === 1 && ["time", "confirm", "paying", "done"].includes(step())) ||
                (i === 2 && ["confirm", "paying", "done"].includes(step())) ||
                (i === 3 && ["paying", "done"].includes(step()))
                  ? "var(--btn)"
                  : "var(--secondary-bg)",
            }}
          />
        ))}
      </div>

      {/* Step: Select date */}
      <Show when={step() === "date"}>
        <div class="px-4 animate-slide-up">
          <p class="text-sm font-medium mb-3" style={{ color: "var(--hint)" }}>
            📅 Выбери дату
          </p>
          <Calendar
            serviceId={props.serviceId}
            onSelect={selectDate}
          />
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

      {/* Step: Confirm + Prepayment info */}
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

          {/* Prepayment info block */}
          <div
            class="mt-3 p-4 rounded-xl"
            style={{
              background: "var(--secondary-bg)",
              border: "1px solid rgba(232, 160, 191, 0.2)",
            }}
          >
            <div class="flex items-center gap-2 mb-2">
              <span class="text-lg">💳</span>
              <span class="font-semibold text-sm" style={{ color: "var(--text)" }}>
                Предоплата {formatPrice(PREPAID_AMOUNT)}
              </span>
            </div>
            <p class="text-xs" style={{ color: "var(--hint)" }}>
              Для подтверждения записи необходима предоплата. Остаток оплачивается на месте.
            </p>
            <p class="text-xs mt-1" style={{ color: "var(--hint)" }}>
              При отмене менее чем за 24 часа предоплата не возвращается.
            </p>
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
            {loading() ? "Создаю запись..." : `💳 Оплатить ${formatPrice(PREPAID_AMOUNT)}`}
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

      {/* Step: Paying — waiting for payment */}
      <Show when={step() === "paying"}>
        <div class="px-4 text-center animate-slide-up py-8">
          <Loader />
          <h3 class="text-lg font-bold mt-4 mb-2" style={{ color: "var(--text)" }}>
            Ожидание оплаты...
          </h3>
          <p class="text-sm" style={{ color: "var(--hint)" }}>
            Завершите оплату в открывшемся окне.
          </p>
          <p class="text-sm mt-1" style={{ color: "var(--hint)" }}>
            Страница обновится автоматически после оплаты.
          </p>

          <div class="mt-6 flex flex-col gap-2">
            <button class="btn-primary" onClick={reopenPayment}>
              🔄 Открыть оплату заново
            </button>
            <button
              class="btn-secondary w-full"
              onClick={() => {
                stopPolling();
                setStep("confirm");
                setError("");
              }}
            >
              ← Назад
            </button>
          </div>
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
          <div
            class="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full text-xs font-medium"
            style={{ background: "#e8f5e9", color: "#2e7d32" }}
          >
            ✓ Предоплата {formatPrice(PREPAID_AMOUNT)} оплачена
          </div>
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
