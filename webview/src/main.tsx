import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { connectHost } from "./host";
import { resolveLocale } from "./i18n";
import { readyMessage } from "./protocol";
import "./tokens.css";
import "./app.css";

const container = document.getElementById("root");
if (!container) {
  throw new Error("Timonier webview: the #root element is missing.");
}

// Front-ends set <html lang> to the IDE's display language.
const locale = resolveLocale(document.documentElement.lang || navigator.language);
const host = connectHost();
let announced = false;
const announceReady = () => {
  // StrictMode runs effects twice in development: announce only once.
  if (announced) return;
  announced = true;
  host?.postMessage(readyMessage());
};

createRoot(container).render(
  <StrictMode>
    <App locale={locale} onReady={announceReady} />
  </StrictMode>,
);
