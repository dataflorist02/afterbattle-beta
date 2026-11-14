/**
 * The window calls of the custom title bar (E5-maps-hd §6). Each one needs exactly one window
 * permission in `src-tauri/capabilities/default.json`; nothing else of the window API is used.
 * Closing asks the window to close, which the backend turns into hiding it to the tray, the same
 * way as Alt+F4.
 */
import { getCurrentWindow } from "@tauri-apps/api/window";

export function minimizeWindow(): void {
  void getCurrentWindow()
    .minimize()
    .catch(() => undefined);
}

export function toggleMaximizeWindow(): void {
  void getCurrentWindow()
    .toggleMaximize()
    .catch(() => undefined);
}

export function closeWindow(): void {
  void getCurrentWindow()
    .close()
    .catch(() => undefined);
}

/**
 * True when the window fills the screen's work area, i.e. it is maximized. Read from the page
 * itself, so no further window permission (`is-maximized`, events) is needed.
 */
export function isMaximized(): boolean {
  return (
    window.outerWidth >= window.screen.availWidth && window.outerHeight >= window.screen.availHeight
  );
}
