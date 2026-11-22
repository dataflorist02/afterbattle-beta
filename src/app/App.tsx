import { useCallback, useEffect, useState } from "react";
import { api } from "../api";
import { LoadingScreen } from "../components/LoadingScreen";
import { VehicleNamesProvider } from "../components/TankName";
import { Async } from "../components/ui";
import { UpdateNotice, UpdatesProvider, useUpdates } from "../components/Updates";
import { UpdateWelcome, Welcome } from "../components/Welcome";
import { useAsync } from "../hooks/useAsync";
import { useLanguage } from "../i18n/language";
import { LOADING_DELAY_MS, hasSessions, importFraction, loadingNeeded } from "../lib/loading";
import { startPage } from "../lib/start";
import { BattlePage } from "../pages/BattlePage";
import { GoalsPage } from "../pages/GoalsPage";
import { HistoryPage } from "../pages/HistoryPage";
import { OverviewPage } from "../pages/OverviewPage";
import { SessionPage } from "../pages/SessionPage";
import { SettingsPage } from "../pages/SettingsPage";
import { TanksPage } from "../pages/TanksPage";
import { APP_VERSION } from "../release";
import type { AppSnapshot, ImportProgressEvent } from "../types";
import { HowItWorksContext } from "../components/HelpLink";
import { useWindowBackground } from "../components/Ember";
import type { HowId } from "../lib/howItWorks";
import { isPublicBuild } from "./buildFlags";
import { DataVersionContext } from "./dataVersion";
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
  /** Settings opened from a "?" (spec 33 §4): the "Jak to działa" entry to open. */
  how?: HowId;
  /** History opened on its Sessions tab ("Do historii" under a Damast plate, spec 34 §3). */
  historyTab?: "battles" | "sessions";
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

/** The first-start card (RC-Start) or the update question for players from a beta. */
type Gate = "welcome" | "updates" | null;

export function App() {
  return (
    <UpdatesProvider>
      <AppContent />
    </UpdatesProvider>
  );
}

function AppContent() {
  // Spec 32 §4.1: the start screen is decided once, from the first snapshot; until then (and
  // after any choice in the navigation) the route is the one set here.
  const [chosen, setRoute] = useState<Route | null>(null);
  const [importProgress, setImportProgress] = useState<ImportProgressEvent | null>(null);
  const [endedSessionId, setEndedSessionId] = useState<string | null>(null);
  const { state, reload } = useAsync(() => api.snapshot(), []);
  // Spec 33 §1: with the window in the background (the game in front) the hot spot stands still.
  useWindowBackground();

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
  const openHow = useCallback((how: HowId) => navigate({ page: "settings", how }), [navigate]);
  const openMap = (mapKey: string, mapName: string | null) =>
    navigate({ page: "history", mapKey, mapName: mapName ?? undefined });
  const snapshot = state.status === "ready" ? state.data : null;
  // RC-Start (fixup after STOP 2): decided once per start. Without a game folder the welcome
  // card; a player coming from a beta (folder set, update question never answered) gets the
  // update question alone. Each stays until its button, then never again in this run.
  const { consent } = useUpdates();
  const [gate, setGate] = useState<Gate | undefined>(undefined);
  useEffect(() => {
    if (gate !== undefined || !snapshot) return;
    if (!snapshot.configured) setGate("welcome");
    else if (consent !== null) setGate(consent === "unasked" ? "updates" : null);
  }, [gate, snapshot, consent]);
  const route: Route = chosen ?? { page: snapshot ? startPage(snapshot, Date.now()) : "session" };
  useEffect(() => {
    if (snapshot && chosen === null) setRoute({ page: startPage(snapshot, Date.now()) });
  }, [snapshot, chosen]);
  // Spec 29 §2: each new snapshot (after a backend event or an action) is a new data version;
  // the pages reload with it in place. Derived while rendering, so pages see it with the snapshot.
  const [version, setVersion] = useState<{ of: AppSnapshot | null; value: number }>({
    of: null,
    value: 0,
  });
  if (snapshot && snapshot !== version.of) setVersion({ of: snapshot, value: version.value + 1 });
  const progress =
    importProgress ??
    (snapshot?.sourceStatus.importProgress
      ? { ...snapshot.sourceStatus.importProgress, finished: false, cancelled: false }
      : null);
  // Spec 29 §5: the loading screen only when the start really takes time.
  const [late, setLate] = useState(false);
  useEffect(() => {
    const timer = setTimeout(() => setLate(true), LOADING_DELAY_MS);
    return () => clearTimeout(timer);
  }, []);
  // Without sessions the history says whether any battle is stored; asked again with every new
  // snapshot, the last answer kept meanwhile (no flicker of the loading screen).
  const empty = useAsync(
    () =>
      snapshot === null || hasSessions(snapshot)
        ? Promise.resolve(false)
        : api
            .battleHistory({
              vehicleId: null,
              mapKey: null,
              dataQuality: "all",
              wn8Availability: "all",
              dateFrom: null,
              dateTo: null,
              cursor: null,
              pageSize: 1,
            })
            .then((page) => page.items.length === 0),
    [snapshot],
    { keepData: true },
  );
  const loading = loadingNeeded({
    starting: state.status === "loading",
    late,
    emptyDatabase: empty.state.status === "ready" && empty.state.data,
    importing: progress !== null,
  });

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
            initialTab={route.historyTab ?? "battles"}
            snapshot={data}
            progress={progress}
            onOpenBattle={(battleId) => navigate({ page: "history", battleId })}
            onOpenSession={(sessionId) => navigate({ page: "session", sessionId })}
          />
        );
      case "settings":
        return <SettingsPage snapshot={data} onChanged={reload} openHow={route.how ?? null} />;
      case "overview":
        return (
          <OverviewPage
            snapshot={data}
            onOpenSession={(sessionId) => navigate({ page: "session", sessionId })}
            onOpenTank={(vehicleId) => navigate({ page: "tanks", vehicleId })}
            onOpenGoals={() => navigate({ page: "goals" })}
            onOpenHistory={() => navigate({ page: "history" })}
            onOpenTanks={() => navigate({ page: "tanks" })}
          />
        );
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
            onOpenHistorySessions={() => navigate({ page: "history", historyTab: "sessions" })}
            onOpenSettings={() => navigate({ page: "settings" })}
          />
        );
    }
  };

  if (gate && snapshot)
    return (
      <VehicleNamesProvider refreshKey={snapshot}>
        <TitleBar />
        {gate === "welcome" ? (
          <Welcome snapshot={snapshot} onChanged={reload} onDone={() => setGate(null)} />
        ) : (
          <UpdateWelcome onDone={() => setGate(null)} />
        )}
      </VehicleNamesProvider>
    );

  return (
    <DataVersionContext.Provider value={version.value}>
      <HowItWorksContext.Provider value={openHow}>
        <VehicleNamesProvider refreshKey={snapshot}>
          <TitleBar />
          <LoadingScreen needed={loading} fraction={importFraction(progress)} />
          <div className="shell">
            <Navigation
              page={route.page}
              onNavigate={(page) => navigate({ page })}
              publicBuild={isPublicBuild()}
              sessionRunning={Boolean(snapshot?.activeSession)}
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
              {/* Settings show the version in "O aplikacji" and WN8 in "Dane": no repeat below. */}
              {route.page !== "settings" && <Footer snapshot={snapshot} />}
              <UpdateNotice />
            </div>
          </div>
        </VehicleNamesProvider>
      </HowItWorksContext.Provider>
    </DataVersionContext.Provider>
  );
}
