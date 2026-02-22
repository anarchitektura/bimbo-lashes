import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { adminApi, type Slot } from "../lib/api";
import { formatDateShort, formatTime, friendlyDate } from "../lib/utils";
import Loader from "../components/Loader";

export default function AdminSchedulePage() {
  // Generate next 14 days
  const dates = Array.from({ length: 14 }, (_, i) => {
    const d = new Date();
    d.setDate(d.getDate() + i);
    return d.toISOString().split("T")[0];
  });

  const [selectedDate, setSelectedDate] = createSignal(dates[0]);
  const [slots, { refetch }] = createResource(
    () => selectedDate(),
    (date) => adminApi.getSlots(date)
  );

  const [adding, setAdding] = createSignal(false);

  // Open slots with optional range
  const openSlots = async (startHour?: number, endHour?: number) => {
    setAdding(true);
    try {
      await adminApi.openDay(selectedDate(), startHour, endHour);
      WebApp.HapticFeedback.notificationOccurred("success");
      refetch();
    } catch (e: any) {
      WebApp.showAlert(e.message || "Ошибка");
    } finally {
      setAdding(false);
    }
  };

  const deleteSlot = async (slot: Slot) => {
    if (slot.is_booked) {
      WebApp.showAlert("Нельзя удалить занятый слот");
      return;
    }

    WebApp.showConfirm(
      `Удалить слот ${formatTime(slot.start_time)}–${formatTime(slot.end_time)}?`,
      async (ok) => {
        if (!ok) return;
        try {
          await adminApi.deleteSlot(slot.id);
          WebApp.HapticFeedback.notificationOccurred("success");
          refetch();
        } catch (e: any) {
          WebApp.showAlert(e.message || "Ошибка");
        }
      }
    );
  };

  const deleteAllFree = async () => {
    const freeSlots = slots()?.filter((s) => !s.is_booked) || [];
    if (freeSlots.length === 0) {
      WebApp.showAlert("Нет свободных слотов для удаления");
      return;
    }
    WebApp.showConfirm(
      `Удалить все свободные слоты (${freeSlots.length} шт.) на ${formatDateShort(selectedDate())}?`,
      async (ok) => {
        if (!ok) return;
        for (const slot of freeSlots) {
          await adminApi.deleteSlot(slot.id).catch(() => {});
        }
        WebApp.HapticFeedback.notificationOccurred("success");
        refetch();
      }
    );
  };

  const bookedCount = () => slots()?.filter((s) => s.is_booked).length || 0;
  const freeCount = () => slots()?.filter((s) => !s.is_booked).length || 0;
  const hasSlots = () => (slots()?.length || 0) > 0;

  return (
    <div class="animate-fade-in">
      <div class="px-4 pt-6 pb-2">
        <h2 class="text-xl font-bold" style={{ color: "var(--text)" }}>
          📅 Расписание
        </h2>
        <p class="text-sm mt-0.5" style={{ color: "var(--hint)" }}>
          Открывай дни и управляй слотами
        </p>
      </div>

      {/* Date selector */}
      <div class="px-4 py-3 flex gap-2 overflow-x-auto">
        <For each={dates}>
          {(date) => (
            <button
              class={`chip whitespace-nowrap ${
                selectedDate() === date ? "chip-active" : "chip-inactive"
              }`}
              onClick={() => {
                WebApp.HapticFeedback.selectionChanged();
                setSelectedDate(date);
              }}
            >
              {formatDateShort(date)}
            </button>
          )}
        </For>
      </div>

      {/* Stats */}
      <Show when={hasSlots()}>
        <div class="px-4 mb-3 flex gap-3">
          <div
            class="flex-1 rounded-xl p-3 text-center"
            style={{ background: "var(--secondary-bg)" }}
          >
            <div class="text-lg font-bold" style={{ color: "var(--btn)" }}>
              {freeCount()}
            </div>
            <div class="text-xs" style={{ color: "var(--hint)" }}>
              свободно
            </div>
          </div>
          <div
            class="flex-1 rounded-xl p-3 text-center"
            style={{ background: "var(--secondary-bg)" }}
          >
            <div class="text-lg font-bold" style={{ color: "#e65100" }}>
              {bookedCount()}
            </div>
            <div class="text-xs" style={{ color: "var(--hint)" }}>
              занято
            </div>
          </div>
        </div>
      </Show>

      {/* Actions — 3 buttons */}
      <div class="px-4 mb-2">
        <p class="text-sm font-medium mb-2" style={{ color: "var(--hint)" }}>
          Управление
        </p>
        <div class="flex gap-2 flex-wrap">
          <button
            class="chip chip-inactive"
            disabled={adding()}
            onClick={() => openSlots()}
          >
            📅 День (12–20)
          </button>
          <button
            class="chip chip-inactive"
            disabled={adding()}
            onClick={() => openSlots(12, 16)}
          >
            🌅 Утро (12–16)
          </button>
          <button
            class="chip chip-inactive"
            disabled={adding()}
            onClick={() => openSlots(16, 20)}
          >
            🌆 Вечер (16–20)
          </button>
        </div>
      </div>

      {/* Slots timeline */}
      <div class="px-4">
        <div class="flex justify-between items-center mb-2">
          <p class="text-sm font-medium" style={{ color: "var(--hint)" }}>
            Слоты на {formatDateShort(selectedDate())}
          </p>
          <Show when={freeCount() > 1}>
            <button
              class="text-xs px-2 py-1 rounded-lg"
              style={{ color: "#d32f2f", background: "#fce4e4" }}
              onClick={deleteAllFree}
            >
              Очистить свободные
            </button>
          </Show>
        </div>

        <Show when={!slots.loading} fallback={<Loader />}>
          <Show
            when={hasSlots()}
            fallback={
              <div
                class="text-center py-8 rounded-xl"
                style={{ background: "var(--secondary-bg)", color: "var(--hint)" }}
              >
                <p class="text-3xl mb-2">📭</p>
                <p>Нет слотов на эту дату</p>
                <p class="text-xs mt-1">
                  Нажми «День» чтобы создать 8 слотов
                </p>
              </div>
            }
          >
            <For each={slots()}>
              {(slot) => (
                <div class="card flex justify-between items-center">
                  <div class="flex items-center gap-2">
                    <div
                      class="w-2 h-2 rounded-full"
                      style={{
                        background: slot.is_booked ? "#e65100" : "#4caf50",
                      }}
                    />
                    <span class="font-medium">
                      {formatTime(slot.start_time)} — {formatTime(slot.end_time)}
                    </span>
                    <Show when={slot.is_booked}>
                      <span
                        class="text-xs px-2 py-0.5 rounded-full"
                        style={{ background: "#fff3e0", color: "#e65100" }}
                      >
                        занят
                      </span>
                    </Show>
                  </div>
                  <Show when={!slot.is_booked}>
                    <button
                      class="text-sm px-3 py-1.5 rounded-xl"
                      style={{ color: "#d32f2f", background: "#fce4e4" }}
                      onClick={() => deleteSlot(slot)}
                    >
                      ✕
                    </button>
                  </Show>
                </div>
              )}
            </For>
          </Show>
        </Show>
      </div>
    </div>
  );
}
