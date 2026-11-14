/**
 * The window's own title bar (E5-maps-hd §6) instead of the Windows frame: the whole bar except
 * the buttons drags the window, a double click maximizes or restores it (both through
 * `data-tauri-drag-region`); on the right minimize, maximize/restore and close (to the tray).
 */
import { useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useLanguage } from "../i18n/language";
import { APP_DISPLAY_NAME } from "../release";
import { closeWindow, isMaximized, minimizeWindow, toggleMaximizeWindow } from "./windowControls";

const svg = (children: ReactNode) => (
  <svg
    aria-hidden="true"
    width="16"
    height="16"
    viewBox="0 0 16 16"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.3"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    {children}
  </svg>
);

const minimizeIcon = svg(<line x1="3" y1="8" x2="13" y2="8" />);
const maximizeIcon = svg(<rect x="3" y="3" width="10" height="10" rx="1.5" />);
const restoreIcon = svg(
  <>
    <rect x="3" y="5" width="8" height="8" rx="1.5" />
    <path d="M5.5 5V4a1.5 1.5 0 0 1 1.5-1.5h5A1.5 1.5 0 0 1 13.5 4v5A1.5 1.5 0 0 1 12 10.5h-1" />
  </>,
);
const closeIcon = svg(
  <>
    <line x1="3.5" y1="3.5" x2="12.5" y2="12.5" />
    <line x1="12.5" y1="3.5" x2="3.5" y2="12.5" />
  </>,
);

export function TitleBar() {
  const { tr } = useLanguage();
  const [maximized, setMaximized] = useState(isMaximized);
  useEffect(() => {
    const update = () => setMaximized(isMaximized());
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);
  return (
    <header className="titlebar" data-tauri-drag-region>
      <span className="titlebar-title" data-tauri-drag-region>
        {APP_DISPLAY_NAME}
      </span>
      <div className="titlebar-controls">
        <button
          type="button"
          className="titlebar-button"
          aria-label={tr("windowMinimize")}
          onClick={minimizeWindow}
        >
          {minimizeIcon}
        </button>
        <button
          type="button"
          className="titlebar-button"
          aria-label={tr(maximized ? "windowRestore" : "windowMaximize")}
          data-state={maximized ? "maximized" : "normal"}
          onClick={toggleMaximizeWindow}
        >
          {maximized ? restoreIcon : maximizeIcon}
        </button>
        <button
          type="button"
          className="titlebar-button titlebar-close"
          aria-label={tr("windowClose")}
          onClick={closeWindow}
        >
          {closeIcon}
        </button>
      </div>
    </header>
  );
}
