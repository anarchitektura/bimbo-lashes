import { createResource, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { api, type Service } from "../lib/api";
import { goBooking, goMyBookings, goAdmin } from "../lib/router";
import { formatPrice, formatDuration } from "../lib/utils";
import Loader from "../components/Loader";

export default function HomePage() {
  const [services] = createResource(() => api.getServices());

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
    goBooking(service.id, service.name, service.price);
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

      {/* Services list */}
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
    </div>
  );
}
