import { createResource, createSignal, For, Show } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { adminApi, type Service } from "../lib/api";
import { formatPrice, formatDuration } from "../lib/utils";
import Loader from "../components/Loader";

export default function AdminServicesPage() {
  const [services, { refetch }] = createResource(() => adminApi.getServices());
  const [editing, setEditing] = createSignal<number | null>(null);
  const [showAdd, setShowAdd] = createSignal(false);

  // Edit form state
  const [editName, setEditName] = createSignal("");
  const [editPrice, setEditPrice] = createSignal("");
  const [editDuration, setEditDuration] = createSignal("");

  const startEdit = (service: Service) => {
    setEditing(service.id);
    setEditName(service.name);
    setEditPrice(String(service.price));
    setEditDuration(String(service.duration_min));
  };

  const saveEdit = async (id: number) => {
    try {
      await adminApi.updateService(id, {
        name: editName(),
        price: parseInt(editPrice()),
        duration_min: parseInt(editDuration()),
      });
      WebApp.HapticFeedback.notificationOccurred("success");
      setEditing(null);
      refetch();
    } catch (e: any) {
      WebApp.showAlert(e.message || "Ошибка");
    }
  };

  const toggleActive = async (service: Service) => {
    try {
      await adminApi.updateService(service.id, { is_active: !service.is_active });
      WebApp.HapticFeedback.impactOccurred("light");
      refetch();
    } catch (e: any) {
      WebApp.showAlert(e.message || "Ошибка");
    }
  };

  // Add new service
  const [newName, setNewName] = createSignal("");
  const [newPrice, setNewPrice] = createSignal("");
  const [newDuration, setNewDuration] = createSignal("");

  const addService = async () => {
    if (!newName() || !newPrice() || !newDuration()) {
      WebApp.showAlert("Заполни все поля");
      return;
    }
    try {
      await adminApi.createService({
        name: newName(),
        price: parseInt(newPrice()),
        duration_min: parseInt(newDuration()),
      });
      WebApp.HapticFeedback.notificationOccurred("success");
      setShowAdd(false);
      setNewName("");
      setNewPrice("");
      setNewDuration("");
      refetch();
    } catch (e: any) {
      WebApp.showAlert(e.message || "Ошибка");
    }
  };

  return (
    <div class="animate-fade-in">
      <div class="px-4 pt-6 pb-2 flex justify-between items-center">
        <h2 class="text-xl font-bold" style={{ color: "var(--text)" }}>
          💅 Услуги
        </h2>
        <button
          class="chip chip-active"
          onClick={() => setShowAdd(!showAdd())}
        >
          {showAdd() ? "✕" : "+ Новая"}
        </button>
      </div>

      {/* Add new service form */}
      <Show when={showAdd()}>
        <div class="px-4 mb-4 animate-slide-up">
          <div class="card">
            <input
              class="w-full p-3 rounded-xl mb-2 text-sm"
              style={{ background: "var(--secondary-bg)", color: "var(--text)" }}
              placeholder="Название"
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
            />
            <div class="flex gap-2 mb-2">
              <input
                class="flex-1 p-3 rounded-xl text-sm"
                style={{ background: "var(--secondary-bg)", color: "var(--text)" }}
                placeholder="Цена ₽"
                type="number"
                value={newPrice()}
                onInput={(e) => setNewPrice(e.currentTarget.value)}
              />
              <input
                class="flex-1 p-3 rounded-xl text-sm"
                style={{ background: "var(--secondary-bg)", color: "var(--text)" }}
                placeholder="Мин."
                type="number"
                value={newDuration()}
                onInput={(e) => setNewDuration(e.currentTarget.value)}
              />
            </div>
            <button class="btn-primary text-sm" onClick={addService}>
              Добавить
            </button>
          </div>
        </div>
      </Show>

      {/* Services list */}
      <div class="px-4">
        <Show when={!services.loading} fallback={<Loader />}>
          <For each={services()}>
            {(service) => (
              <div
                class="card animate-slide-up"
                style={{ opacity: service.is_active ? 1 : 0.5 }}
              >
                <Show
                  when={editing() === service.id}
                  fallback={
                    <div class="flex justify-between items-start">
                      <div class="flex-1" onClick={() => startEdit(service)}>
                        <div class="font-semibold">{service.name}</div>
                        <div class="text-sm mt-0.5" style={{ color: "var(--hint)" }}>
                          {formatPrice(service.price)} · {formatDuration(service.duration_min)}
                        </div>
                      </div>
                      <button
                        class="text-xs px-3 py-1.5 rounded-xl"
                        style={{
                          background: service.is_active ? "#e8f5e9" : "#fce4e4",
                          color: service.is_active ? "#2e7d32" : "#d32f2f",
                        }}
                        onClick={() => toggleActive(service)}
                      >
                        {service.is_active ? "Вкл" : "Выкл"}
                      </button>
                    </div>
                  }
                >
                  {/* Edit mode */}
                  <input
                    class="w-full p-2 rounded-lg mb-2 text-sm"
                    style={{ background: "var(--secondary-bg)", color: "var(--text)" }}
                    value={editName()}
                    onInput={(e) => setEditName(e.currentTarget.value)}
                  />
                  <div class="flex gap-2 mb-2">
                    <input
                      class="flex-1 p-2 rounded-lg text-sm"
                      style={{ background: "var(--secondary-bg)", color: "var(--text)" }}
                      type="number"
                      value={editPrice()}
                      onInput={(e) => setEditPrice(e.currentTarget.value)}
                    />
                    <input
                      class="flex-1 p-2 rounded-lg text-sm"
                      style={{ background: "var(--secondary-bg)", color: "var(--text)" }}
                      type="number"
                      value={editDuration()}
                      onInput={(e) => setEditDuration(e.currentTarget.value)}
                    />
                  </div>
                  <div class="flex gap-2">
                    <button
                      class="flex-1 btn-primary text-sm py-2"
                      onClick={() => saveEdit(service.id)}
                    >
                      Сохранить
                    </button>
                    <button
                      class="btn-secondary text-sm py-2"
                      onClick={() => setEditing(null)}
                    >
                      ✕
                    </button>
                  </div>
                </Show>
              </div>
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}
