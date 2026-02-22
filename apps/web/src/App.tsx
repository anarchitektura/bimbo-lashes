import { Match, Switch, onMount } from "solid-js";
import WebApp from "@twa-dev/sdk";
import { route, goHome, goAdmin } from "./lib/router";
import HomePage from "./pages/HomePage";
import BookingPage from "./pages/BookingPage";
import MyBookingsPage from "./pages/MyBookingsPage";
import AdminPage from "./pages/AdminPage";
import AdminSchedulePage from "./pages/AdminSchedulePage";
import AdminServicesPage from "./pages/AdminServicesPage";

export default function App() {
  onMount(() => {
    // Handle Telegram back button
    WebApp.BackButton.onClick(() => {
      const r = route();
      if (r.page === "admin-schedule" || r.page === "admin-services") {
        // Go back to admin from sub-pages
        goAdmin();
      } else {
        goHome();
      }
    });
  });

  // Show/hide back button based on route
  const showBack = () => route().page !== "home";

  return (
    <div class="min-h-screen pb-4 safe-bottom">
      {showBack()
        ? WebApp.BackButton.show()
        : WebApp.BackButton.hide()}

      <Switch>
        <Match when={route().page === "home"}>
          <HomePage />
        </Match>
        <Match when={route().page === "booking"}>
          {(() => {
            const r = route();
            if (r.page === "booking") {
              return (
                <BookingPage
                  serviceId={r.serviceId}
                  serviceName={r.serviceName}
                  servicePrice={r.servicePrice}
                />
              );
            }
            return null;
          })()}
        </Match>
        <Match when={route().page === "my-bookings"}>
          <MyBookingsPage />
        </Match>
        <Match when={route().page === "admin"}>
          <AdminPage />
        </Match>
        <Match when={route().page === "admin-schedule"}>
          <AdminSchedulePage />
        </Match>
        <Match when={route().page === "admin-services"}>
          <AdminServicesPage />
        </Match>
      </Switch>
    </div>
  );
}
