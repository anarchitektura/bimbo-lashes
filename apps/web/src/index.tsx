/* @refresh reload */
import { render } from "solid-js/web";
import WebApp from "@twa-dev/sdk";
import App from "./App";
import "./app.css";

// Initialize Telegram Mini App
WebApp.ready();
WebApp.expand();

// ── Theme detection: set body class for CSS theming ──
function applyThemeClass() {
  const scheme = WebApp.colorScheme; // "light" | "dark"
  document.body.classList.remove("theme-light", "theme-dark");
  document.body.classList.add(scheme === "dark" ? "theme-dark" : "theme-light");
}
applyThemeClass();
WebApp.onEvent("themeChanged", applyThemeClass);

const root = document.getElementById("root");

if (root) {
  render(() => <App />, root);
}
