import type { ReactNode } from "react";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";

export type Page = "session" | "tanks" | "goals" | "history" | "maps" | "settings";

const svg = (children: ReactNode) => (
  <svg
    aria-hidden="true"
    width="22"
    height="22"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
    strokeLinecap="round"
    strokeLinejoin="round"
  >
    {children}
  </svg>
);

const icons: Record<Page, ReactNode> = {
  session: svg(<polyline points="3 12 7 12 10 5 14 19 17 12 21 12" />),
  tanks: svg(
    <>
      <rect x="3" y="14" width="18" height="5" rx="2.5" />
      <rect x="6" y="9" width="9" height="5" rx="1.5" />
      <line x1="15" y1="11.5" x2="21" y2="11.5" />
    </>,
  ),
  goals: svg(
    <>
      <circle cx="12" cy="12" r="8" />
      <circle cx="12" cy="12" r="4" />
      <circle cx="12" cy="12" r="0.8" />
    </>,
  ),
  history: svg(
    <>
      <circle cx="12" cy="12" r="8.5" />
      <polyline points="12 7.5 12 12 15 14" />
    </>,
  ),
  maps: svg(
    <>
      <polygon points="3 6 9 4 15 6 21 4 21 18 15 20 9 18 3 20" />
      <line x1="9" y1="4" x2="9" y2="18" />
      <line x1="15" y1="6" x2="15" y2="20" />
    </>,
  ),
  settings: svg(
    <>
      <line x1="4" y1="7" x2="20" y2="7" />
      <line x1="4" y1="17" x2="20" y2="17" />
      <circle cx="9" cy="7" r="2.2" />
      <circle cx="15" cy="17" r="2.2" />
    </>,
  ),
};

const labels: Record<Page, UiKey> = {
  session: "navSession",
  tanks: "navTanks",
  goals: "navGoals",
  history: "navHistory",
  maps: "navMaps",
  settings: "navSettings",
};

export function Navigation({
  page,
  onNavigate,
  publicBuild,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  publicBuild: boolean;
}) {
  const { tr } = useLanguage();
  const item = (target: Page) => (
    <button
      key={target}
      type="button"
      className="rail-link"
      aria-current={page === target ? "page" : undefined}
      onClick={() => onNavigate(target)}
    >
      <span className="rail-icon">{icons[target]}</span>
      {tr(labels[target])}
    </button>
  );
  return (
    <nav className="rail" aria-label={tr("mainNavigation")}>
      <div className="rail-logo" aria-hidden="true">
        A
      </div>
      {(["session", "tanks", "goals", "history"] as const).map(item)}
      {!publicBuild && (
        <button
          type="button"
          className="rail-link"
          aria-disabled="true"
          title={tr("navMapsLater")}
          aria-describedby="maps-later"
        >
          <span className="rail-icon">{icons.maps}</span>
          {tr("navMaps")}
          <span id="maps-later" className="visually-hidden">
            {tr("navMapsLater")}
          </span>
        </button>
      )}
      <div className="rail-spacer" />
      {item("settings")}
    </nav>
  );
}
