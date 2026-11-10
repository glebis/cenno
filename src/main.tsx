import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles/tokens.css";
import "./styles/theme.css";
import App from "./App";
import SettingsWindow from "./SettingsWindow";
import HelpWindow from "./HelpWindow";
import { loadUserConfig } from "./userConfig";

// One frontend bundle serves three windows. The `main` window is the floating
// prompt panel (App); `settings` and `help` (both opened from the tray) render
// their own UI. Branch on the Tauri window label.
function rootFor(label: string) {
  if (label === "settings") return <SettingsWindow />;
  if (label === "help") return <HelpWindow />;
  return <App />;
}

function mount() {
  let label = "main";
  try {
    label = getCurrentWindow().label;
  } catch {
    // Non-Tauri context (e.g. plain Vite preview) → default to the panel.
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>{rootFor(label)}</React.StrictMode>,
  );
}

// Apply external config (~/.cenno tokens + widgets) before first paint so the
// theme override and custom widgets are in place; never block startup on it.
loadUserConfig().finally(mount);
