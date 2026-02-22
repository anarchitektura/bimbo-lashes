/* @refresh reload */
import { render } from "solid-js/web";
import WebApp from "@twa-dev/sdk";
import App from "./App";
import "./app.css";

// Initialize Telegram Mini App
WebApp.ready();
WebApp.expand();

const root = document.getElementById("root");

if (root) {
  render(() => <App />, root);
}
