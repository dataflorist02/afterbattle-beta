import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { api } from "../api";
import { HistoryRows } from "../components/HistoryRows";
import { DamastSessions } from "../features/damast/DamastSessions";
import { ImportBar } from "../components/ImportBar";
import { useTankLabel } from "../components/TankName";
import { Async, Empty, ErrorState, SegmentedControl } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import { formatDate } from "../lib/format";
import { mergeRefreshedPage } from "../lib/pages";
import {
  afterFontsAndLayout,
  entranceIds,
  measuredWave,
  prefersReducedMotion,
  useArrivals,
  useFlip,
} from "../lib/motion";
import { heatAgainst, heatProps, type Heat } from "../lib/ember";
import { damastHeat } from "../features/damast/stats";
import { TEMPERATURE_KEYS } from "../components/Ember";
import type { UiKey } from "../i18n/ui";
import type {
  Language,
  AppSnapshot,
  BattleHistoryCursor,
  BattleHistoryFilter,
  BattleHistoryItem,
  ImportProgressEvent,
  Session,
} from "../types";

/**
 * Spec 33 §3: the temperature of each recent session, the same as on the Session screen and on
 * its plate: the Damast temperature (one request per session, again only after the data changed).
 */
function useSessionHeats(sessions: readonly Session[]): Map<string, Heat | null> {
  const version = useDataVersion();
  const ids = sessions.map((session) => session.id);
  const heats = useAsync(
    async () => {
      const plates = await Promise.all(
        ids.map((id) =>
          Promise.resolve()
            .then(() => api.damastSession(id))
            .catch(() => null),
        ),
      );
      return new Map(ids.map((id, index) => [id, damastHeat(plates[index])]));
    },
    [ids.join("|")],
    { keepData: true, refreshKey: version },
  );
  return heats.state.status === "ready" ? heats.state.data : new Map();
}

/** "1 sesję temu", "2 sesje temu", "30 sesji temu" (Polish plural forms; English one/other). */
export function sessionsAgo(
  count: number,
  language: Language,
  tr: (key: UiKey, params?: Record<string, string | number>) => string,
): string {
  const form = new Intl.PluralRules(language === "pl" ? "pl-PL" : "en-GB").select(count);
  const key: UiKey =
    form === "one"
      ? "historyThermogramAgoOne"
      : form === "few"
        ? "historyThermogramAgoFew"
        : "historyThermogramAgoMany";
  return tr(key, { count });
}

/** The thermogram (spec 33 §3): one cell per recent session, oldest on the left. */
function Thermogram({
  sessions,
  heats,
}: {
  sessions: readonly Session[];
  heats: Map<string, Heat | null>;
}) {
  const { language, tr } = useLanguage();
  if (sessions.length === 0) return null;
  const oldestFirst = [...sessions].sort((a, b) => a.startedAt - b.startedAt);
  return (
    <div className="thermogram-wrap">
      <ol
        className="thermogram"
        aria-label={tr("historyThermogram", { count: oldestFirst.length })}
      >
        {oldestFirst.map((session) => {
          const heat = heats.get(session.id) ?? null;
          const label = tr("historyThermogramCell", {
            date: formatDate(session.startedAt, language),
            temperature: heat ? tr(TEMPERATURE_KEYS[heat.temperature]) : tr("tempNone"),
          });
          return (
            <li
              key={session.id}
              className="thermogram-cell temp-mark"
              aria-label={label}
              title={label}
              {...heatProps(heat)}
            />
          );
        })}
      </ol>
      {/* The axis counts sessions, not days: "30 sesji temu" … "ostatnia sesja". */}
      <p className="thermogram-ends">
        <span>{sessionsAgo(oldestFirst.length, language, tr)}</span>
        <span>{tr("historyThermogramLatest")}</span>
      </p>
    </div>
  );
}

const PAGE = 50;
const NO_SWEEP: ReadonlySet<string> = new Set();

/** The record of the rows shown (most damage, the newest of equal ones): the hot spot. */
export function recordId(items: readonly BattleHistoryItem[]): string | null {
  let best: BattleHistoryItem | null = null;
  for (const item of items)
    if (item.damage !== null && (best === null || item.damage > (best.damage ?? -1))) best = item;
  return best?.battleId ?? null;
}

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
  // Spec 33 §3: a new row gets one sweep (none with reduced motion).
  const swept = moving ? new Set(arrivals) : NO_SWEEP;
  // Spec 33 §3: each row against the player's own average damage on its tank.
  const averages = new Map(
    (tanks.state.status === "ready" ? tanks.state.data.vehicles : []).map((tank) => [
      tank.vehicleId,
      tank.averageDamage,
    ]),
  );
  const rowHeat = (item: BattleHistoryItem) =>
    item.vehicleId === null ? null : heatAgainst(item.damage, averages.get(item.vehicleId));
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
                <HistoryRows
                  items={items}
                  onOpenBattle={onOpenBattle}
                  entering={shownEntering}
                  heat={rowHeat}
                  hotId={recordId(items)}
                  swept={swept}
                />
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

export function HistoryPage({
  snapshot,
  progress,
  onOpenBattle,
  onOpenSession,
  initialMapKey = null,
  initialMapName = null,
  initialTab = "battles",
  entranceMotion = true,
}: {
  snapshot: AppSnapshot;
  progress: ImportProgressEvent | null;
  onOpenBattle: (battleId: string) => void;
  onOpenSession: (sessionId: string) => void;
  initialMapKey?: string | null;
  initialMapName?: string | null;
  /** The tab shown first (Sessions after "Do historii" under a Damast plate). */
  initialTab?: "battles" | "sessions";
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
  const [tab, setTab] = useState<"battles" | "sessions">(initialTab);
  const heats = useSessionHeats(snapshot.recentSessions);
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
      <Thermogram sessions={snapshot.recentSessions} heats={heats} />
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
        // Spec 34 §3: the sessions as Damast plates (ended sessions only).
        <DamastSessions xvmOn={snapshot.advancedStatus.xvmEnabled} onOpenSession={onOpenSession} />
      )}
    </>
  );
}
