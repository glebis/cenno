import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/tokens.css";
import "./styles/theme.css";
import App from "./App";
import { loadUserConfig } from "./userConfig";

function mount() {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

// Apply external config (~/.cenno tokens + widgets) before first paint so the
// theme override and custom widgets are in place; never block startup on it.
loadUserConfig().finally(mount);
