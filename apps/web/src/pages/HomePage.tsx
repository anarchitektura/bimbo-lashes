import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type Service, type AddonInfo } from "../lib/api";
import { goBooking, goMyBookings, goAdmin } from "../lib/router";
import { formatPrice, formatDuration } from "../lib/utils";
import Loader from "../components/Loader";

export default function HomePage() {
  const [services] = createResource(() => api.getServices());
  const [addonInfo] = createResource(() => api.getAddonInfo());
  const [selectedService, setSelectedService] = createSignal<Service | null>(null);
  const [withLowerLashes, setWithLowerLashes] = createSignal(false);

  const isAdmin = () => {
    try {
      const user = WebApp.initDataUnsafe?.user;
      const adminId = parseInt(import.meta.env.VITE_ADMIN_TG_ID || "0");
      return user?.id === adminId;
    } catch {
      return false;
    }
  };

  const handleSelect = (service: Service) => {
    WebApp.HapticFeedback.impactOccurred("light");

    // If it's наращивание — show addon selector
    if (service.duration_min >= 120 && addonInfo()) {
      setSelectedService(service);
      setWithLowerLashes(false);
    } else {
      // Коррекция — go straight to booking
      goBooking(service.id, service.name, service.price, false);
    }
  };

  const confirmService = () => {
    const svc = selectedService();
    if (!svc) return;

    WebApp.HapticFeedback.impactOccurred("light");
    const addon = addonInfo();
    const totalPrice = svc.price + (withLowerLashes() && addon ? addon.price : 0);
    const name = withLowerLashes() ? `${svc.name} + нижние` : svc.name;
    goBooking(svc.id, name, totalPrice, withLowerLashes());
  };

  const cancelSelection = () => {
    setSelectedService(null);
    setWithLowerLashes(false);
  };

  return (
    <div class="animate-fade-in">
      {/* Header */}
      <div class="px-4 pt-6 pb-4 text-center">
        <h1 class="text-2xl font-bold" style={{ color: "var(--text)" }}>
          ✨ Bimbo Lashes ✨
        </h1>
        <p class="mt-1 text-sm" style={{ color: "var(--hint)" }}>
          Выбери услугу и запишись
        </p>
      </div>

      {/* Navigation */}
      <div class="px-4 mb-4 flex gap-2">
        <button
          class="btn-secondary flex-1 text-center"
          onClick={() => goMyBookings()}
        >
          📋 Мои записи
        </button>
        <Show when={isAdmin()}>
          <button
            class="btn-secondary flex-1 text-center"
            onClick={() => goAdmin()}
          >
            ⚙️ Админ
          </button>
        </Show>
      </div>

      {/* Addon selector (shown when наращивание selected) */}
      <Show when={selectedService()}>
        {(svc) => (
          <div class="px-4 mb-4 animate-slide-up">
            <div class="card">
              <div class="font-semibold text-base mb-3" style={{ color: "var(--text)" }}>
                {svc().name}
              </div>

              <Show when={addonInfo()}>
                {(addon) => (
                  <label
                    class="flex items-center gap-3 p-3 rounded-xl cursor-pointer"
                    style={{ background: "var(--secondary-bg)" }}
                    onClick={() => {
                      WebApp.HapticFeedback.selectionChanged();
                      setWithLowerLashes(!withLowerLashes());
                    }}
                  >
                    <div
                      class="w-6 h-6 rounded-lg flex items-center justify-center text-sm font-bold shrink-0"
                      style={{
                        background: withLowerLashes() ? "var(--btn)" : "transparent",
                        color: withLowerLashes() ? "var(--btn-text)" : "var(--hint)",
                        border: withLowerLashes() ? "none" : "2px solid var(--hint)",
                      }}
                    >
                      {withLowerLashes() ? "✓" : ""}
                    </div>
                    <div class="flex-1">
                      <div class="text-sm font-medium" style={{ color: "var(--text)" }}>
                        + {addon().name}
                      </div>
                    </div>
                    <div class="text-sm font-bold" style={{ color: "var(--btn)" }}>
                      +{formatPrice(addon().price)}
                    </div>
                  </label>
                )}
              </Show>

              <div class="mt-3 flex justify-between items-center">
                <div class="text-sm" style={{ color: "var(--hint)" }}>
                  Итого: <span class="font-bold" style={{ color: "var(--btn)" }}>
                    {formatPrice(svc().price + (withLowerLashes() && addonInfo() ? addonInfo()!.price : 0))}
                  </span>
                </div>
              </div>

              <button class="btn-primary mt-3" onClick={confirmService}>
                Выбрать время →
              </button>

              <button
                class="mt-2 text-sm font-medium w-full text-center"
                style={{ color: "var(--link)" }}
                onClick={cancelSelection}
              >
                ← Назад
              </button>
            </div>
          </div>
        )}
      </Show>

      {/* Services list (hidden when addon selector is shown) */}
      <Show when={!selectedService()}>
        <div class="px-4">
          <Show when={!services.loading} fallback={<Loader />}>
            <Show when={services()?.length} fallback={
              <div class="text-center py-12" style={{ color: "var(--hint)" }}>
                <p class="text-4xl mb-2">💤</p>
                <p>Услуги скоро появятся</p>
              </div>
            }>
              <For each={services()}>
                {(service) => (
                  <button
                    class="card w-full text-left flex items-center gap-4 animate-slide-up"
                    onClick={() => handleSelect(service)}
                  >
                    <div class="flex-1">
                      <div class="font-semibold text-base" style={{ color: "var(--text)" }}>
                        {service.name}
                      </div>
                      <Show when={service.description}>
                        <div class="text-sm mt-0.5" style={{ color: "var(--hint)" }}>
                          {service.description}
                        </div>
                      </Show>
                      <div class="text-xs mt-1" style={{ color: "var(--hint)" }}>
                        🕐 {formatDuration(service.duration_min)}
                      </div>
                    </div>
                    <div
                      class="text-base font-bold whitespace-nowrap"
                      style={{ color: "var(--btn)" }}
                    >
                      {formatPrice(service.price)}
                    </div>
                    <div
                      class="text-xl"
                      style={{ color: "var(--hint)", opacity: 0.5 }}
                    >
                      ›
                    </div>
                  </button>
                )}
              </For>
            </Show>
          </Show>
        </div>
      </Show>
    </div>
  );
}
