import { Component, type ReactNode } from "react";
import { errorMessage } from "../i18n/errors";
import { storedLanguage } from "../i18n/language";

/** Last-resort screen when rendering fails; no raw error is logged or shown. */
export class AppErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  render() {
    if (!this.state.failed) return this.props.children;
    const language = storedLanguage();
    return (
      <main className="page" role="alert">
        <h1 className="page-title">Afterbattle</h1>
        <p className="muted">{errorMessage("unknown_error", language)}</p>
        <button type="button" className="button" onClick={() => window.location.reload()}>
          {language === "pl" ? "Uruchom widok ponownie" : "Reload the view"}
        </button>
      </main>
    );
  }
}
