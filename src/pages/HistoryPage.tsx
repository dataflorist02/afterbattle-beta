import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../api";
import { HistoryRows } from "../components/HistoryRows";
import { MapBackdrop } from "../components/MapBackdrop";
import { ImportBar } from "../components/ImportBar";
import { useTankLabel } from "../components/TankName";
import { Async, Empty, ErrorState, SegmentedControl } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useLanguage } from "../i18n/language";
import { formatDate, formatNumber } from "../lib/format";
import { entranceIds, measuredWave, prefersReducedMotion } from "../lib/motion";
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
  snapshot,
  progress,
  onOpenBattle,
  initialMapKey,
  initialMapName,
  entrance,
}: {
  snapshot: AppSnapshot;
  progress: ImportProgressEvent | null;
  onOpenBattle: (battleId: string) => void;
  initialMapKey: string | null;
  initialMapName: string | null;
  /** Called once with the first page and the wave size; returns the ids that enter with motion. */
  entrance: (ids: string[], wave: number) => Set<string>;
}) {
  const { tr } = useLanguage();
  const label = useTankLabel();
  const [vehicleId, setVehicleId] = useState<number | null>(null);
  const [mapKey, setMapKey] = useState<string | null>(initialMapKey);
  const [more, setMore] = useState<{
    items: BattleHistoryItem[];
    next: BattleHistoryCursor | null;
  } | null>(null);
  const [moreError, setMoreError] = useState<string | undefined | null>(null);
  const tanks = useAsync(() => api.vehicleHistoryOverview(), [snapshot]);
  const first = useAsync(() => {
    setMore(null);
    return api.battleHistory(filterFor(vehicleId, mapKey, null));
  }, [vehicleId, mapKey, snapshot]);
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
  const rowsRef = useRef<HTMLDivElement>(null);
  // Measured before the first paint: the rows visible now form the wave (E8-ui-fix), and they
  // never show still for a frame before the cascade starts.
  useLayoutEffect(() => {
    if (first.state.status !== "ready") return;
    const rows = Array.from(rowsRef.current?.querySelectorAll(".hrow-list > li") ?? []);
    const ids = entrance(
      first.state.data.items.map((item) => item.battleId),
      measuredWave(rows),
    );
    if (ids.size === 0) return;
    setEntering(ids);
    // The cascade lasts well under a second; afterwards no row carries the entrance class.
    const timer = setTimeout(() => setEntering(new Set()), 1_000);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [first.state]);
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
          const items = more?.items ?? page.items;
          const next = more ? more.next : page.nextCursor;
          if (items.length === 0) {
            if (filtered) return <Empty text={tr("filterEmpty")} />;
            return progress ? (
              <>
                <ImportBar progress={progress} />
                <Empty text={tr("historyFilling")} />
              </>
            ) : (
              <Empty text={tr("historyEmpty")} />
            );
          }
          return (
            <>
              <div ref={rowsRef} className="contents">
                <HistoryRows items={items} onOpenBattle={onOpenBattle} entering={entering} />
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
  const wn8 = useAsync(() => api.recentSessionsWn8(), [snapshot]);
  const ids = snapshot.recentSessions.map((session) => session.id);
  const maps = useAsync(() => api.sessionMaps(ids), [snapshot]);
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
  const entrance = useCallback((ids: string[], wave: number) => {
    const result = entranceIds(ids, cascadeLeft.current && !prefersReducedMotion() ? wave : 0);
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
          snapshot={snapshot}
          progress={progress}
          onOpenBattle={onOpenBattle}
          initialMapKey={initialMapKey}
          initialMapName={initialMapName}
          entrance={entrance}
        />
      ) : (
        <Sessions snapshot={snapshot} onOpenSession={onOpenSession} />
      )}
    </>
  );
}
