import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { VehicleNamesProvider } from "../components/TankName";
import { Async } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useLanguage } from "../i18n/language";
import { BattlePage } from "../pages/BattlePage";
import { GoalsPage } from "../pages/GoalsPage";
import { HistoryPage } from "../pages/HistoryPage";
import { SessionPage } from "../pages/SessionPage";
import { SettingsPage } from "../pages/SettingsPage";
import { TanksPage } from "../pages/TanksPage";
import { APP_VERSION } from "../release";
import type { AppSnapshot, ImportProgressEvent } from "../types";
import { isPublicBuild } from "./buildFlags";
import { Navigation, type Page } from "./Navigation";
import { TitleBar } from "./TitleBar";

export interface Route {
  page: Page;
  battleId?: string;
  sessionId?: string;
  vehicleId?: number;
  /** History opened with a map filter (from a Battle or Tank screen). */
  mapKey?: string;
  mapName?: string;
  /** "back": returning to a screen (from Battle); it reappears without entry motion. */
  entry?: "back";
}

function Footer({ snapshot }: { snapshot: AppSnapshot | null }) {
  const { tr } = useLanguage();
  const version = snapshot?.wn8Presentation.expectedValues.sourceVersion;
  return (
    <footer className="app-footer">
      {tr("footerVersion", { version: APP_VERSION })} · {tr("footerWn8")}:{" "}
      {version ? `XVM · ${version}` : tr("footerWn8Off")}
    </footer>
  );
}

export function App() {
  const [route, setRoute] = useState<Route>({ page: "session" });
  const [importProgress, setImportProgress] = useState<ImportProgressEvent | null>(null);
  const [endedSessionId, setEndedSessionId] = useState<string | null>(null);
  const { state, reload } = useAsync(() => api.snapshot(), []);

  useEffect(
    () =>
      api.onEvents((event) => {
        if (event.name === "import-progress") {
          setImportProgress(event.payload.finished ? null : event.payload);
          if (event.payload.finished) reload();
        } else if (event.name === "session-ended") {
          // A6: the backend says which session ended; no local timers or refs.
          setEndedSessionId(event.payload.sessionId);
          reload();
        } else {
          reload();
        }
      }),
    [reload],
  );

  const navigate = useCallback((next: Route) => {
    setRoute(next);
    window.scrollTo?.(0, 0);
  }, []);
  const openMap = (mapKey: string, mapName: string | null) =>
    navigate({ page: "history", mapKey, mapName: mapName ?? undefined });
  const snapshot = state.status === "ready" ? state.data : null;
  const progress =
    importProgress ??
    (snapshot?.sourceStatus.importProgress
      ? { ...snapshot.sourceStatus.importProgress, finished: false, cancelled: false }
      : null);

  const renderPage = (data: AppSnapshot) => {
    switch (route.page) {
      case "tanks":
        return route.battleId ? (
          <BattlePage
            battleId={route.battleId}
            onBack={() => navigate({ page: "tanks", vehicleId: route.vehicleId, entry: "back" })}
            onOpenMap={openMap}
          />
        ) : (
          <TanksPage
            snapshot={data}
            initialVehicleId={route.vehicleId}
            onOpenBattle={(battleId, vehicleId) => navigate({ page: "tanks", battleId, vehicleId })}
            onOpenMap={(mapKey) => openMap(mapKey, null)}
            entranceMotion={route.entry !== "back"}
          />
        );
      case "goals":
        return <GoalsPage snapshot={data} onChanged={reload} />;
      case "history":
        return route.battleId ? (
          <BattlePage
            battleId={route.battleId}
            onBack={() => navigate({ page: "history", entry: "back" })}
            onOpenMap={openMap}
          />
        ) : (
          <HistoryPage
            key={route.mapKey ?? "all"}
            entranceMotion={route.entry !== "back"}
            initialMapKey={route.mapKey ?? null}
            initialMapName={route.mapName ?? null}
            snapshot={data}
            progress={progress}
            onOpenBattle={(battleId) => navigate({ page: "history", battleId })}
            onOpenSession={(sessionId) => navigate({ page: "session", sessionId })}
          />
        );
      case "settings":
        return <SettingsPage snapshot={data} onChanged={reload} />;
      default:
        return route.battleId ? (
          <BattlePage
            battleId={route.battleId}
            onBack={() => navigate({ page: "session", sessionId: route.sessionId, entry: "back" })}
            onOpenMap={openMap}
          />
        ) : (
          <SessionPage
            snapshot={data}
            sessionId={route.sessionId ?? null}
            endedSessionId={endedSessionId}
            progress={progress}
            onChanged={reload}
            onOpenBattle={(battleId, sessionId) =>
              navigate({ page: "session", battleId, sessionId })
            }
            onOpenHistory={() => navigate({ page: "history" })}
            onOpenSettings={() => navigate({ page: "settings" })}
          />
        );
    }
  };

  return (
    <VehicleNamesProvider refreshKey={snapshot}>
      <TitleBar />
      <div className="shell">
        <Navigation
          page={route.page}
          onNavigate={(page) => navigate({ page })}
          publicBuild={isPublicBuild()}
        />
        <div className="workspace">
          <main className="page">
            <Async state={state} onRetry={reload}>
              {(data) => (
                <div
                  className={
                    route.entry === "back" ||
                    ((route.page === "history" || route.page === "tanks") && !route.battleId)
                      ? "screen screen-still"
                      : "screen"
                  }
                  key={[route.page, route.battleId, route.sessionId, route.mapKey].join("|")}
                >
                  {renderPage(data)}
                </div>
              )}
            </Async>
          </main>
          <Footer snapshot={snapshot} />
        </div>
      </div>
    </VehicleNamesProvider>
  );
}
