import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../api";
import { HistoryRows } from "../components/HistoryRows";
import { MapBackdrop } from "../components/MapBackdrop";
import { ImportBar } from "../components/ImportBar";
import { useTankLabel } from "../components/TankName";
import { Async, Empty, ErrorState, SegmentedControl } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import { formatDate, formatNumber } from "../lib/format";
import { mergeRefreshedPage } from "../lib/pages";
import {
  afterFontsAndLayout,
  entranceIds,
  measuredWave,
  prefersReducedMotion,
  useArrivals,
  useFlip,
} from "../lib/motion";
import type {
  AppSnapshot,
  BattleHistoryCursor,
  BattleHistoryFilter,
  BattleHistoryItem,
  ImportProgressEvent,
} from "../types";

const PAGE = 50;

function filterFor(
  vehicleId: number | null,
  mapKey: string | null,
  cursor: BattleHistoryCursor | null,
): BattleHistoryFilter {
  return {
    vehicleId,
    mapKey,
    dataQuality: "all",
    wn8Availability: "all",
    dateFrom: null,
    dateTo: null,
    cursor,
    pageSize: PAGE,
  };
}

function Battles({
  progress,
  onOpenBattle,
  initialMapKey,
  initialMapName,
  takeEntrance,
  canImport,
}: {
  progress: ImportProgressEvent | null;
  /** The game folder is set: the empty history offers reading the battle files (spec 31 §5). */
  canImport: boolean;
  onOpenBattle: (battleId: string) => void;
  initialMapKey: string | null;
  initialMapName: string | null;
  /** True once per visit: the first page shown enters with the wave. */
  takeEntrance: () => boolean;
}) {
  const { tr } = useLanguage();
  const label = useTankLabel();
  const [importError, setImportError] = useState<string | undefined | null>(null);
  const [vehicleId, setVehicleId] = useState<number | null>(null);
  const [mapKey, setMapKey] = useState<string | null>(initialMapKey);
  // Rows loaded with "Show more", for the filter they were loaded with only; a refresh keeps
  // them below the refreshed first page (spec 29 §2).
  const filterKey = `${vehicleId ?? ""}|${mapKey ?? ""}`;
  const [loadedMore, setMore] = useState<{
    filter: string;
    items: BattleHistoryItem[];
    next: BattleHistoryCursor | null;
  } | null>(null);
  const more = loadedMore?.filter === filterKey ? loadedMore : null;
  const version = useDataVersion();
  const [moreError, setMoreError] = useState<string | undefined | null>(null);
  const tanks = useAsync(() => api.vehicleHistoryOverview(), [], { refreshKey: version });
  const first = useAsync(
    () => api.battleHistory(filterFor(vehicleId, mapKey, null)),
    [vehicleId, mapKey],
    { refreshKey: version },
  );
  // Map choices come from the battles already seen; the filter itself runs in the database.
  const [maps, setMaps] = useState<Map<string, string>>(
    () => new Map(initialMapKey ? [[initialMapKey, initialMapName ?? initialMapKey]] : []),
  );
  const remember = (items: BattleHistoryItem[]) => {
    const next = new Map(maps);
    let changed = false;
    for (const item of items)
      if (item.mapKey && !next.has(item.mapKey)) {
        next.set(item.mapKey, item.mapName ?? item.mapKey);
        changed = true;
      }
    if (changed) setMaps(next);
  };
  const [entering, setEntering] = useState<Set<string>>(() => new Set());
  // Rows of the first page wait hidden (opacity only) until the wave is measured.
  const [wavePending, setWavePending] = useState(false);
  const rowsRef = useRef<HTMLDivElement>(null);
  // Spec 29 §1: the wave is measured in the real layout, after the fonts and the first frame,
  // from the rows' own positions (day headers between rows do not shorten it). Until then the
  // rows are hidden, so they never show still for a frame before the cascade starts.
  useLayoutEffect(() => {
    if (first.state.status !== "ready" || !takeEntrance()) return;
    const ids = first.state.data.items.map((item) => item.battleId);
    setWavePending(true);
    const cancel = afterFontsAndLayout(() => {
      const rows = Array.from(rowsRef.current?.querySelectorAll(".hrow-list > li") ?? []);
      setEntering(entranceIds(ids, measuredWave(rows)));
      setWavePending(false);
    });
    return () => {
      cancel();
      setWavePending(false);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [first.state]);
  // Spec 29 §6: a battle that arrives while History is open enters at the top and the rows that
  // moved glide down; never on a filter change, "Show more" or entering the screen.
  const firstIds =
    first.state.status === "ready" ? first.state.data.items.map((item) => item.battleId) : null;
  const arrivals = useArrivals(filterKey, firstIds, true);
  const moving = arrivals.length > 0 && !prefersReducedMotion();
  useFlip(rowsRef, firstIds?.join(","), moving);
  const shownEntering = moving ? new Set([...entering, ...arrivals]) : entering;
  // The cascade lasts about a second; afterwards no row carries the entrance class.
  useEffect(() => {
    if (entering.size === 0) return;
    const timer = setTimeout(() => setEntering(new Set()), 1_100);
    return () => clearTimeout(timer);
  }, [entering]);
  useEffect(() => {
    if (first.state.status === "ready") remember(first.state.data.items);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [first.state]);
  const filtered = vehicleId !== null || mapKey !== null;
  return (
    <section className="section">
      <div className="filters">
        <label className="field">
          {tr("filterTank")}
          <select
            value={vehicleId ?? ""}
            onChange={(event) =>
              setVehicleId(event.target.value ? Number(event.target.value) : null)
            }
          >
            <option value="">{tr("filterAll")}</option>
            {tanks.state.status === "ready" &&
              tanks.state.data.vehicles.map((tank) => (
                <option
                  key={tank.vehicleId}
                  value={tank.vehicleId}
                  title={label(tank.vehicleKey, tank.tankName).full}
                >
                  {label(tank.vehicleKey, tank.tankName).short}
                </option>
              ))}
          </select>
        </label>
        <label className="field">
          {tr("filterMap")}
          <select value={mapKey ?? ""} onChange={(event) => setMapKey(event.target.value || null)}>
            <option value="">{tr("filterAll")}</option>
            {[...maps.entries()].map(([key, name]) => (
              <option key={key} value={key}>
                {name}
              </option>
            ))}
          </select>
        </label>
        {tanks.state.status === "error" && (
          <p className="state-error" role="alert">
            {tr("tanksListError")}{" "}
            <button type="button" className="link" onClick={tanks.reload}>
              {tr("retry")}
            </button>
          </p>
        )}
        {filtered && (
          <button
            type="button"
            className="button"
            onClick={() => {
              setVehicleId(null);
              setMapKey(null);
            }}
          >
            {tr("filterClear")}
          </button>
        )}
      </div>
      <Async state={first.state} onRetry={first.reload}>
        {(page) => {
          const items = more ? mergeRefreshedPage(page.items, more.items) : page.items;
          const next = more ? more.next : page.nextCursor;
          if (items.length === 0) {
            if (filtered) return <Empty text={tr("filterEmpty")} />;
            return progress ? (
              <>
                <ImportBar progress={progress} />
                <Empty text={tr("historyFilling")} />
              </>
            ) : (
              <>
                {importError !== null && <ErrorState code={importError} />}
                <Empty
                  text={tr("historyEmpty")}
                  action={
                    <button
                      type="button"
                      className="button"
                      disabled={!canImport}
                      onClick={() => {
                        setImportError(null);
                        api
                          .startImport()
                          .catch((failure: { code?: string }) => setImportError(failure?.code));
                      }}
                    >
                      {tr("settingsImportStart")}
                    </button>
                  }
                />
              </>
            );
          }
          return (
            <>
              <div ref={rowsRef} className={wavePending ? "contents wave-pending" : "contents"}>
                <HistoryRows items={items} onOpenBattle={onOpenBattle} entering={shownEntering} />
              </div>
              {moreError !== null && <ErrorState code={moreError} />}
              {next && (
                <button
                  type="button"
                  className="button"
                  onClick={() => {
                    setMoreError(null);
                    api.battleHistory(filterFor(vehicleId, mapKey, next)).then(
                      (nextPage) => {
                        remember(nextPage.items);
                        setMore({
                          filter: filterKey,
                          items: [...items, ...nextPage.items],
                          next: nextPage.nextCursor,
                        });
                      },
                      (error: { code?: string }) => setMoreError(error?.code),
                    );
                  }}
                >
                  {tr("loadMore")}
                </button>
              )}
            </>
          );
        }}
      </Async>
    </section>
  );
}

function Sessions({
  snapshot,
  onOpenSession,
}: {
  snapshot: AppSnapshot;
  onOpenSession: (sessionId: string) => void;
}) {
  const { language, tr } = useLanguage();
  const version = useDataVersion();
  const wn8 = useAsync(() => api.recentSessionsWn8(), [], { refreshKey: version });
  const ids = snapshot.recentSessions.map((session) => session.id);
  const maps = useAsync(() => api.sessionMaps(ids), [ids.join("|")], { refreshKey: version });
  if (snapshot.recentSessions.length === 0) return <Empty text={tr("sessionsEmpty")} />;
  const xvmOn = snapshot.advancedStatus.xvmEnabled;
  return (
    <ul className="session-cards">
      {snapshot.recentSessions.map((session) => {
        const value =
          wn8.state.status === "ready"
            ? (wn8.state.data.find((item) => item.sessionId === session.id)?.wn8.value ?? null)
            : null;
        const keys =
          maps.state.status === "ready"
            ? (maps.state.data.find((item) => item.sessionId === session.id)?.mapKeys ?? [])
            : [];
        return (
          <li key={session.id}>
            <button
              type="button"
              className="card session-card"
              onClick={() => onOpenSession(session.id)}
            >
              <span className="session-collage" aria-hidden="true">
                {(keys.length > 0 ? keys : [null]).map((key, index) => (
                  <MapBackdrop
                    key={key ?? index}
                    mapKey={key}
                    width={120}
                    className="collage-tile"
                  />
                ))}
              </span>
              <span className="session-card-text">
                <span>
                  {tr("sessionRow", {
                    date: formatDate(session.startedAt, language),
                    battles: session.analyzedBattleCount,
                  })}
                </span>
                {xvmOn && value !== null && (
                  <span className="num">WN8 {formatNumber(value, language)}</span>
                )}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

export function HistoryPage({
  snapshot,
  progress,
  onOpenBattle,
  onOpenSession,
  initialMapKey = null,
  initialMapName = null,
  entranceMotion = true,
}: {
  snapshot: AppSnapshot;
  progress: ImportProgressEvent | null;
  onOpenBattle: (battleId: string) => void;
  onOpenSession: (sessionId: string) => void;
  initialMapKey?: string | null;
  initialMapName?: string | null;
  /** False when returning from a Battle: the list reappears without motion. */
  entranceMotion?: boolean;
}) {
  const { tr } = useLanguage();
  // The cascade happens once per visit: not after switching Battles/Sessions, not on a filter
  // change, not for rows loaded later.
  const cascadeLeft = useRef(entranceMotion);
  const takeEntrance = useCallback(() => {
    const result = cascadeLeft.current && !prefersReducedMotion();
    cascadeLeft.current = false;
    return result;
  }, []);
  const [tab, setTab] = useState<"battles" | "sessions">("battles");
  return (
    <>
      <header className="page-header">
        <h1 className="page-title">{tr("historyTitle")}</h1>
        <SegmentedControl
          label={tr("historyView")}
          value={tab}
          options={[
            { value: "battles", label: tr("historyBattles") },
            { value: "sessions", label: tr("historySessions") },
          ]}
          onChange={setTab}
        />
      </header>
      {tab === "battles" ? (
        <Battles
          progress={progress}
          onOpenBattle={onOpenBattle}
          initialMapKey={initialMapKey}
          initialMapName={initialMapName}
          takeEntrance={takeEntrance}
          canImport={snapshot.sourceStatus.configured}
        />
      ) : (
        <Sessions snapshot={snapshot} onOpenSession={onOpenSession} />
      )}
    </>
  );
}
