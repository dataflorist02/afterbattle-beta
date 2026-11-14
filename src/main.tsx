import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { LanguageProvider } from "./i18n/language";
import { AppErrorBoundary } from "./components/ErrorBoundary";
import "./styles/tokens.css";
import "./styles/base.css";

const root = document.getElementById("root");
if (!root) throw new Error("Application root is missing");

createRoot(root).render(
  <StrictMode>
    <AppErrorBoundary>
      <LanguageProvider>
        <App />
      </LanguageProvider>
    </AppErrorBoundary>
  </StrictMode>,
);
