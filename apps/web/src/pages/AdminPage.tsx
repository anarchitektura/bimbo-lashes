import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { adminApi, type BookingDetail } from "../lib/api";
import { goAdminSchedule, goAdminServices } from "../lib/router";
import { friendlyDate, formatTime, formatPrice, formatDateShort } from "../lib/utils";
import Loader from "../components/Loader";

export default function AdminPage() {
  const _now = new Date();
  const _fmt = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const today = _fmt(_now);
  const _tmrw = new Date(_now); _tmrw.setDate(_tmrw.getDate() + 1);
  const tomorrow = _fmt(_tmrw);

  const [activeTab, setActiveTab] = createSignal<"today" | "tomorrow" | "week">("today");

  // Compute date range for the week (next 7 days)
  const weekFrom = today;
  const _wEnd = new Date(_now); _wEnd.setDate(_wEnd.getDate() + 6);
  const weekTo = _fmt(_wEnd);

  const [todayBookings, { refetch: refetchToday }] = createResource(() =>
    adminApi.getBookings({ date: today })
  );
  const [tomorrowBookings, { refetch: refetchTomorrow }] = createResource(() =>
    adminApi.getBookings({ date: tomorrow })
  );
  const [weekBookings, { refetch: refetchWeek }] = createResource(() =>
    adminApi.getBookings({ from: weekFrom, to: weekTo })
  );

  const activeBookings = () => {
    switch (activeTab()) {
      case "today": return todayBookings();
      case "tomorrow": return tomorrowBookings();
      case "week": return weekBookings();
    }
  };

  const isLoading = () => {
    switch (activeTab()) {
      case "today": return todayBookings.loading;
      case "tomorrow": return tomorrowBookings.loading;
      case "week": return weekBookings.loading;
    }
  };

  const totalRevenue = () =>
    (activeBookings() || []).reduce((sum, b) => sum + b.service_price, 0);

  const handleCancel = (booking: BookingDetail) => {
    WebApp.showConfirm(
      `Отменить запись ${booking.client_first_name} (${booking.service_name})?`,
      async (ok) => {
        if (!ok) return;
        try {
          await adminApi.cancelBooking(booking.id);
          WebApp.HapticFeedback.notificationOccurred("success");
          refetchToday();
          refetchTomorrow();
          refetchWeek();
        } catch {
          WebApp.showAlert("Не удалось отменить");
        }
      }
    );
  };

  const tabLabel = () => {
    switch (activeTab()) {
      case "today": return "Сегодня";
      case "tomorrow": return "Завтра";
      case "week": return "Неделя";
    }
  };

  return (
    <div class="animate-fade-in">
      <div class="px-4 pt-6 pb-3">
        <h2 class="text-xl font-bold" style={{ color: "var(--text)" }}>
          ⚙️ Админ-панель
        </h2>
      </div>

      {/* Quick actions */}
      <div class="px-4 mb-4 flex gap-2">
        <button
          class="btn-secondary flex-1 text-center"
          onClick={() => goAdminSchedule()}
        >
          📅 Расписание
        </button>
        <button
          class="btn-secondary flex-1 text-center"
          onClick={() => goAdminServices()}
        >
          💅 Услуги
        </button>
      </div>

      {/* Tab selector */}
      <div class="px-4 mb-3 flex gap-1.5">
        {(["today", "tomorrow", "week"] as const).map((tab) => (
          <button
            class={`chip flex-1 justify-center ${
              activeTab() === tab ? "chip-active" : "chip-inactive"
            }`}
            onClick={() => {
              WebApp.HapticFeedback.selectionChanged();
              setActiveTab(tab);
            }}
          >
            {tab === "today" ? "Сегодня" : tab === "tomorrow" ? "Завтра" : "Неделя"}
          </button>
        ))}
      </div>

      {/* Stats */}
      <Show when={activeBookings()?.length}>
        <div class="px-4 mb-3 flex gap-3">
          <div
            class="flex-1 rounded-xl p-3 text-center"
            style={{ background: "var(--secondary-bg)" }}
          >
            <div class="text-lg font-bold" style={{ color: "var(--btn)" }}>
              {activeBookings()?.length || 0}
            </div>
            <div class="text-xs" style={{ color: "var(--hint)" }}>записей</div>
          </div>
          <div
            class="flex-1 rounded-xl p-3 text-center"
            style={{ background: "var(--secondary-bg)" }}
          >
            <div class="text-lg font-bold" style={{ color: "#4caf50" }}>
              {formatPrice(totalRevenue())}
            </div>
            <div class="text-xs" style={{ color: "var(--hint)" }}>выручка</div>
          </div>
        </div>
      </Show>

      {/* Bookings list */}
      <div class="px-4">
        <p class="text-sm font-medium mb-2" style={{ color: "var(--hint)" }}>
          Записи — {tabLabel()}
        </p>

        <Show when={!isLoading()} fallback={<Loader />}>
          <Show
            when={activeBookings()?.length}
            fallback={
              <div
                class="text-center py-8 rounded-xl"
                style={{ background: "var(--secondary-bg)", color: "var(--hint)" }}
              >
                <p class="text-3xl mb-2">☀️</p>
                <p>
                  {activeTab() === "today"
                    ? "Сегодня свободный день"
                    : activeTab() === "tomorrow"
                    ? "Завтра пока пусто"
                    : "Нет записей на неделю"}
                </p>
              </div>
            }
          >
            <For each={activeBookings()}>
              {(b) => (
                <div class="card animate-slide-up">
                  <div class="flex justify-between items-start">
                    <div class="flex-1">
                      <div class="flex items-center gap-2">
                        <span class="font-semibold">
                          {formatTime(b.start_time)} — {formatTime(b.end_time)}
                        </span>
                        <Show when={activeTab() === "week"}>
                          <span
                            class="text-xs px-2 py-0.5 rounded-full"
                            style={{ background: "var(--secondary-bg)", color: "var(--hint)" }}
                          >
                            {formatDateShort(b.date)}
                          </span>
                        </Show>
                      </div>
                      <div class="text-sm mt-1" style={{ color: "var(--hint)" }}>
                        💅 {b.service_name} · {formatPrice(b.service_price)}
                      </div>
                    </div>
                    <div class="text-right ml-3">
                      <div class="font-medium">{b.client_first_name}</div>
                      <Show when={b.client_username}>
                        <div class="text-xs" style={{ color: "var(--link)" }}>
                          @{b.client_username}
                        </div>
                      </Show>
                    </div>
                  </div>
                  <button
                    class="mt-2 text-xs px-3 py-1.5 rounded-xl w-full text-center"
                    style={{ color: "#d32f2f", background: "#fce4e4" }}
                    onClick={() => handleCancel(b)}
                  >
                    Отменить
                  </button>
                </div>
              )}
            </For>
          </Show>
        </Show>
      </div>
    </div>
  );
}
