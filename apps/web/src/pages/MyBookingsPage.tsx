import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type BookingDetail } from "../lib/api";
import { goHome } from "../lib/router";
import { friendlyDate, formatTime, formatPrice } from "../lib/utils";
import Loader from "../components/Loader";

export default function MyBookingsPage() {
  const [bookings, { refetch }] = createResource(() => api.getMyBookings());
  const [cancelling, setCancelling] = createSignal<number | null>(null);

  const handleCancel = async (booking: BookingDetail) => {
    WebApp.showConfirm(
      `Отменить запись на ${friendlyDate(booking.date)} в ${formatTime(booking.start_time)}?`,
      async (confirmed) => {
        if (!confirmed) return;

        setCancelling(booking.id);
        try {
          await api.cancelBooking(booking.id);
          WebApp.HapticFeedback.notificationOccurred("success");
          refetch();
        } catch {
          WebApp.HapticFeedback.notificationOccurred("error");
          WebApp.showAlert("Не удалось отменить запись");
        } finally {
          setCancelling(null);
        }
      }
    );
  };

  return (
    <div class="animate-fade-in">
      <div class="px-4 pt-6 pb-4">
        <h2 class="text-xl font-bold" style={{ color: "var(--text)" }}>
          📋 Мои записи
        </h2>
      </div>

      <div class="px-4">
        <Show when={!bookings.loading} fallback={<Loader />}>
          <Show
            when={bookings()?.length}
            fallback={
              <div class="text-center py-12" style={{ color: "var(--hint)" }}>
                <p class="text-4xl mb-2">🤷‍♀️</p>
                <p>Пока нет записей</p>
                <button
                  class="btn-primary mt-6"
                  onClick={() => goHome()}
                >
                  💅 Записаться
                </button>
              </div>
            }
          >
            <For each={bookings()}>
              {(booking) => (
                <div class="card animate-slide-up">
                  <div class="flex justify-between items-start">
                    <div>
                      <div class="font-semibold">{booking.service_name}</div>
                      <div class="text-sm mt-1" style={{ color: "var(--hint)" }}>
                        📅 {friendlyDate(booking.date)}
                      </div>
                      <div class="text-sm" style={{ color: "var(--hint)" }}>
                        🕐 {formatTime(booking.start_time)} — {formatTime(booking.end_time)}
                      </div>
                    </div>
                    <div class="text-right">
                      <div class="font-bold" style={{ color: "var(--btn)" }}>
                        {formatPrice(booking.total_price ?? booking.service_price)}
                      </div>
                    </div>
                  </div>

                  <button
                    class="mt-3 text-sm font-medium w-full text-center py-2 rounded-xl"
                    style={{
                      color: "#d32f2f",
                      background: "#fce4e4",
                    }}
                    disabled={cancelling() === booking.id}
                    onClick={() => handleCancel(booking)}
                  >
                    {cancelling() === booking.id ? "Отменяю..." : "Отменить"}
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
