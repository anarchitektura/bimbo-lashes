import { createSignal } from "solid-js";

export type Route =
  | { page: "home" }
  | { page: "booking"; serviceId: number; serviceName: string; servicePrice: number; withLowerLashes: boolean }
  | { page: "my-bookings" }
  | { page: "admin" }
  | { page: "admin-schedule" }
  | { page: "admin-services" };

const [route, setRoute] = createSignal<Route>({ page: "home" });

export { route, setRoute };

export function goHome() {
  setRoute({ page: "home" });
}

export function goBooking(serviceId: number, serviceName: string, servicePrice: number, withLowerLashes: boolean = false) {
  setRoute({ page: "booking", serviceId, serviceName, servicePrice, withLowerLashes });
}

export function goMyBookings() {
  setRoute({ page: "my-bookings" });
}

export function goAdmin() {
  setRoute({ page: "admin" });
}

export function goAdminSchedule() {
  setRoute({ page: "admin-schedule" });
}

export function goAdminServices() {
  setRoute({ page: "admin-services" });
}
