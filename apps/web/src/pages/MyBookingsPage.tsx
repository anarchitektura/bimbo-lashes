import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type BookingDetail } from "../lib/api";
import { goHome } from "../lib/router";
import { friendlyDate, formatTime, formatPrice } from "../lib/utils";
import Loader from "../components/Loader";

export default function MyBookingsPage() {
  const [bookings, { refetch }] = createResource(() => api.getMyBookings());
  const [cancelling, setCancelling] = createSignal<number | null>(null);

  /** Calculate hours until appointment */
  const hoursUntil = (booking: BookingDetail): number => {
    try {
      const appointmentStr = `${booking.date}T${booking.start_time}`;
      const appointment = new Date(appointmentStr);
      const now = new Date();
      return (appointment.getTime() - now.getTime()) / (1000 * 60 * 60);
    } catch {
      return 999;
    }
  };

  const handleCancel = async (booking: BookingDetail) => {
    const hours = hoursUntil(booking);
    const isPaid = booking.payment_status === "paid";

    let confirmText = `Отменить запись на ${friendlyDate(booking.date)} в ${formatTime(booking.start_time)}?`;

    if (isPaid && hours <= 24) {
      confirmText += `\n\n⚠️ Предоплата ${formatPrice(booking.prepaid_amount ?? 500)} НЕ возвращается (менее 24ч до записи).`;
    } else if (isPaid && hours > 24) {
      confirmText += `\n\n💰 Предоплата ${formatPrice(booking.prepaid_amount ?? 500)} будет возвращена.`;
    }

    WebApp.showConfirm(
      confirmText,
      async (confirmed) => {
        if (!confirmed) return;

        setCancelling(booking.id);
        try {
          const result = await api.cancelBooking(booking.id);
          WebApp.HapticFeedback.notificationOccurred("success");

          // Show refund info if available
          if (result.refund_info) {
            WebApp.showAlert(result.refund_info);
          }

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

  const paymentBadge = (booking: BookingDetail) => {
    if (booking.payment_status === "paid") {
      return (
        <div
          class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium mt-1"
          style={{ background: "#e8f5e9", color: "#2e7d32" }}
        >
          ✓ {formatPrice(booking.prepaid_amount ?? 500)}
        </div>
      );
    }
    if (booking.status === "pending_payment" || booking.payment_status === "pending") {
      return (
        <div
          class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium mt-1"
          style={{ background: "#fff3e0", color: "#e65100" }}
        >
          ⏳ Ожидание оплаты
        </div>
      );
    }
    return null;
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
                      {paymentBadge(booking)}
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
