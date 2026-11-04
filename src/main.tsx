import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { AppProvider } from "./state";
import "./styles.css";

// Отключаем стандартное контекстное меню браузера (кроме полей ввода).
document.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement;
  if (!["INPUT", "TEXTAREA"].includes(t.tagName)) e.preventDefault();
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <AppProvider>
      <App />
    </AppProvider>
  </React.StrictMode>,
);
