import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import { api } from "../api";
import { BattleTable } from "../components/BattleTable";
import { DamageChart } from "../components/DamageChart";
import { TankName, useTankLabel } from "../components/TankName";
import { MapBackdrop } from "../components/MapBackdrop";
import { Async, Card, Empty, VehicleImage } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { daysBetween, formatDate, formatNumber, formatPercent, formatTier } from "../lib/format";
import { mergeRefreshedPage } from "../lib/pages";
import { WAVE_MAX_ROWS, prefersReducedMotion, rowEntranceDelay } from "../lib/motion";
import type {
  BaselineMetric,
  BattleHistoryCursor,
  BattleHistoryItem,
  Language,
  VehicleClass,
  VehicleHistorySummary,
  VehicleMapSummary,
} from "../types";

const classKeys: Record<VehicleClass, UiKey> = {
  light_tank: "classLight",
  medium_tank: "classMedium",
  heavy_tank: "classHeavy",
  tank_destroyer: "classTd",
  spg: "classSpg",
  unknown: "classUnknown",
};
export const MIN_TANK_BATTLES = 10;
const PAGE = 50;

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

/** "last yesterday" from the overview's last battle time (mockup header). */
export function lastBattle(
  at: number | null | undefined,
  now: number,
  language: Language,
  tr: Translate,
): string | null {
  if (at === null || at === undefined || !Number.isFinite(at) || at > now) return null;
  const days = daysBetween(at, now);
  if (days === 0) return tr("lastToday");
  if (days === 1) return tr("lastYesterday");
  if (days < 7) return tr("lastDaysAgo", { count: days });
  return tr("lastOn", { date: formatDate(at, language) });
}

function HeaderFigure({
  label,
  recent,
  average,
  percent = false,
}: {
  label: UiKey;
  recent: BaselineMetric | undefined;
  average: BaselineMetric | undefined;
  percent?: boolean;
}) {
  const { language, tr } = useLanguage();
  const format = (value: number | null | undefined) =>
    percent ? formatPercent(value, language) : formatNumber(value, language);
  return (
    <div className="figure">
      <span className="num figure-value">{format(recent?.value)}</span>{" "}
      <span className="figure-label">
        {tr("tankFigure", { label: tr(label), value: format(average?.value) })}
      </span>
    </div>
  );
}

/** Tags of the map grid (spec 26 §5): two best from 3 maps up, the weakest from 3 maps up. */
export function mapTag(index: number, count: number): "best" | "worst" | null {
  if (index === 0 || (index === 1 && count >= 3)) return "best";
  if (count >= 3 && index === count - 1) return "worst";
  return null;
}

/** "Your maps on this tank": tiles from the best average damage down (E5-flare §3). */
function TankMaps({
  maps,
  average,
  onOpenMap,
  wave = 0,
}: {
  maps: VehicleMapSummary[];
  average: number | null;
  onOpenMap: (mapKey: string) => void;
  /** Tiles that enter with the cascade (E8-ui-fix): the first ones, on the first visit only. */
  wave?: number;
}) {
  const { language, tr } = useLanguage();
  return (
    <section className="section" aria-labelledby="maps-h">
      <div className="card-head">
        <h2 id="maps-h" className="section-title">
          {tr("tankMapsTitle")}
        </h2>
        <p className="muted">{tr("tankMapsNote")}</p>
      </div>
      {maps.length === 0 ? (
        <p className="muted">{tr("whereTooFew")}</p>
      ) : (
        <ol className="map-grid">
          {maps.map((map, index) => {
            const tag = mapTag(index, maps.length);
            const name = map.mapName ?? map.mapKey;
            const share =
              average && average > 0 ? Math.min(1, map.averageDamage / (average * 1.25)) : 0;
            const delay = rowEntranceDelay(index, wave);
            return (
              <li
                key={map.mapKey}
                className={delay === null ? undefined : "row-enter"}
                style={
                  delay === null ? undefined : ({ "--row-delay": `${delay}ms` } as CSSProperties)
                }
              >
                <MapBackdrop
                  as="button"
                  mapKey={map.mapKey}
                  width={320}
                  className={tag === "best" ? "map-tile tile-best" : "map-tile"}
                  onClick={() => onOpenMap(map.mapKey)}
                  aria-label={tr("mapTileLabel", {
                    tag: tag ? tr(tag === "best" ? "mapTagBest" : "mapTagWorst") : "",
                    map: name,
                    damage: formatNumber(map.averageDamage, language),
                    survival: formatPercent(map.survivalRate, language),
                    battles: map.battles,
                  }).trim()}
                >
                  {tag && (
                    <span className={tag === "best" ? "map-tag map-tag-best" : "map-tag"}>
                      {tr(tag === "best" ? "mapTagBest" : "mapTagWorst")}
                    </span>
                  )}
                  <span className="map-tile-name truncate">{name}</span>
                  <span className="map-tile-damage">
                    <span className="num">{formatNumber(map.averageDamage, language)}</span>{" "}
                    <span className="map-tile-unit">{tr("mapTileDamage")}</span>
                  </span>
                  <span className="map-tile-meta">
                    {tr("mapTileMeta", {
                      survival: formatPercent(map.survivalRate, language),
                      battles: map.battles,
                    })}
                  </span>
                  <span className="map-tile-bar" aria-hidden="true">
                    <span
                      className={
                        average !== null && map.averageDamage >= average
                          ? "map-tile-fill map-tile-above"
                          : "map-tile-fill"
                      }
                      style={{ width: `${share * 100}%` }}
                    />
                  </span>
                </MapBackdrop>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}

function TankDetail({
  tank,
  onOpenBattle,
  onOpenMap,
  takeEntrance,
}: {
  tank: VehicleHistorySummary;
  onOpenBattle: (battleId: string) => void;
  onOpenMap: (mapKey: string) => void;
  /** True once per visit: the first tank whose data arrives enters with motion. */
  takeEntrance: () => boolean;
}) {
  const { language, tr } = useLanguage();
  const id = tank.vehicleId;
  const version = useDataVersion();
  const data = useAsync(
    async () => {
      const [baseline, report, trends] = await Promise.all([
        api.vehicleBaseline(id),
        api.vehicleHistoryReport(id),
        api.performanceTrends("last100", id).catch(() => null),
      ]);
      return { baseline, report, trends };
      // Spec 29 §1: choosing another tank keeps the shown data until the new data arrives.
    },
    [id],
    { keepData: true, refreshKey: version },
  );
  // Pages loaded with "Show more", for the tank they were loaded for only.
  const [loaded, setPages] = useState<{
    vehicleId: number;
    items: BattleHistoryItem[];
    next: BattleHistoryCursor | null;
  } | null>(null);
  const pages = loaded?.vehicleId === id ? loaded : null;
  const battles = useAsync(
    () =>
      api.battleHistory({
        vehicleId: id,
        mapKey: null,
        dataQuality: "all",
        wn8Availability: "all",
        dateFrom: null,
        dateTo: null,
        cursor: null,
        pageSize: PAGE,
      }),
    [id],
    { keepData: true, refreshKey: version },
  );
  const name = useTankLabel()(tank.vehicleKey, tank.tankName).full;
  const className = tr(classKeys[tank.vehicleClass.vehicleClass] ?? "classUnknown");
  const tier = formatTier(tank.tier);
  const loadMore = (cursor: BattleHistoryCursor, items: BattleHistoryItem[]) =>
    api
      .battleHistory({
        vehicleId: id,
        mapKey: null,
        dataQuality: "all",
        wn8Availability: "all",
        dateFrom: null,
        dateTo: null,
        cursor,
        pageSize: PAGE,
      })
      .then((page) =>
        setPages({ vehicleId: id, items: [...items, ...page.items], next: page.nextCursor }),
      );
  const ready = data.state.status === "ready" ? data.state.data : null;
  // E8-ui-fix: the entrance starts only when the data is there (never on the loading state),
  // decided before the first paint so nothing shows still and then jumps; it ends after the
  // cascade, and choosing another tank shows its data without motion.
  const [wave, setWave] = useState(0);
  // The panel stays mounted across tanks (spec 29 §1): data of another tank ends any cascade.
  useLayoutEffect(() => {
    if (!ready) return;
    if (!takeEntrance()) {
      setWave(0);
      return;
    }
    setWave(Math.min(ready.report.maps?.length ?? 0, WAVE_MAX_ROWS) || 1);
    const timer = setTimeout(() => setWave(0), 1_100);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready]);
  const recent = ready?.baseline.windows.find((window) => window.window === "last10");
  const longer = ready?.baseline.windows.find((window) => window.window === "last100");
  const counted = longer?.eligibleBattles ?? 0;
  return (
    <div className="section" style={{ gap: "var(--space-6)" }}>
      <header className="tank-header">
        <VehicleImage vehicleKey={tank.vehicleKey} label={name} width={240} height={120} />
        <div className="section">
          <h1 className="page-title truncate" title={name}>
            {name}
          </h1>
          <p className="muted">
            {[
              tier === null
                ? `${className}, ${tr("tankBattles", { count: tank.totalBattles })}`
                : tr("tankMeta", { class: className, tier, battles: tank.totalBattles }),
              lastBattle(tank.lastBattleAt, Date.now(), language, tr),
            ]
              .filter(Boolean)
              .join(", ")}
          </p>
        </div>
        {ready && (
          <div className="tank-figures" role="group" aria-label={tr("tankVsOwnNote")}>
            {counted < MIN_TANK_BATTLES ? (
              <p className="muted">{tr("tankTooFew", { count: counted })}</p>
            ) : (
              <>
                <HeaderFigure
                  label="tankFigureDamage"
                  recent={recent?.averageDamage}
                  average={longer?.averageDamage}
                />
                <HeaderFigure
                  label="tankFigureAssist"
                  recent={recent?.averageAssist}
                  average={longer?.averageAssist}
                />
                <HeaderFigure
                  label="tankFigureSurvival"
                  recent={recent?.survivalRate}
                  average={longer?.survivalRate}
                  percent
                />
              </>
            )}
          </div>
        )}
      </header>
      <div className="tank-layout" aria-busy={data.refreshing || battles.refreshing}>
        <div className={wave > 0 ? "tank-main tank-enter" : "tank-main"}>
          <Async state={data.state} onRetry={data.reload}>
            {({ report, trends }) => {
              const damageTrend = trends?.metrics.find((metric) =>
                String(metric.metric).includes("damage"),
              );
              const values = (damageTrend?.points ?? [])
                .map((point) => point.value)
                .filter((value): value is number => value !== null);
              return (
                <>
                  <TankMaps
                    maps={report.maps ?? []}
                    average={longer?.averageDamage.value ?? null}
                    onOpenMap={onOpenMap}
                    wave={wave}
                  />
                  <Card labelledBy="trend-h" className="section">
                    <h2 id="trend-h" className="section-title">
                      {tr("damageOverTime")}
                    </h2>
                    <p className="muted">{tr("damageOverTimeNote", { count: values.length })}</p>
                    <DamageChart values={values} average={longer?.averageDamage.value ?? null} />
                  </Card>
                </>
              );
            }}
          </Async>
        </div>
        <div className="tank-side">
          <Async state={battles.state} onRetry={battles.reload}>
            {(first) => {
              const items = pages ? mergeRefreshedPage(first.items, pages.items) : first.items;
              const next = pages ? pages.next : first.nextCursor;
              return (
                <>
                  <BattleTable
                    title={tr("historyBattles")}
                    rows={items}
                    onOpenBattle={onOpenBattle}
                    showTank={false}
                  />
                  {next && (
                    <button
                      type="button"
                      className="button"
                      onClick={() => void loadMore(next, items)}
                    >
                      {tr("loadMore")}
                    </button>
                  )}
                </>
              );
            }}
          </Async>
        </div>
      </div>
    </div>
  );
}

export function TanksPage({
  initialVehicleId,
  onOpenBattle,
  onOpenMap = () => undefined,
  entranceMotion = true,
}: {
  initialVehicleId?: number;
  onOpenBattle: (battleId: string, vehicleId: number) => void;
  onOpenMap?: (mapKey: string) => void;
  /** False when returning from a Battle: the screen reappears without motion. */
  entranceMotion?: boolean;
}) {
  const { tr } = useLanguage();
  const label = useTankLabel();
  const version = useDataVersion();
  const { state, reload } = useAsync(() => api.vehicleHistoryOverview(), [], {
    refreshKey: version,
  });
  const [selected, setSelected] = useState<number | undefined>(initialVehicleId);
  const entranceLeft = useRef(entranceMotion);
  const takeEntrance = useCallback(() => {
    const result = entranceLeft.current && !prefersReducedMotion();
    entranceLeft.current = false;
    return result;
  }, []);
  return (
    <Async state={state} onRetry={reload}>
      {(overview) => {
        const tanks = [...overview.vehicles].sort((a, b) => b.totalBattles - a.totalBattles);
        if (tanks.length === 0)
          return (
            <>
              <h1 className="page-title">{tr("tanksTitle")}</h1>
              <Empty text={tr("noTanks")} />
            </>
          );
        const current = tanks.find((tank) => tank.vehicleId === selected) ?? tanks[0];
        return (
          <div className="split">
            <nav className="side-list" aria-label={tr("tanksTitle")}>
              {tanks.map((tank) => (
                <button
                  key={tank.vehicleId}
                  type="button"
                  className="side-item"
                  aria-current={tank.vehicleId === current.vehicleId ? "true" : undefined}
                  onClick={() => setSelected(tank.vehicleId)}
                >
                  <span className="cell">
                    <VehicleImage
                      vehicleKey={tank.vehicleKey}
                      label={label(tank.vehicleKey, tank.tankName).full}
                    />
                    <TankName vehicleKey={tank.vehicleKey} tankName={tank.tankName} />
                  </span>
                  <span className="subtle nowrap">
                    {tr("tankBattles", { count: tank.totalBattles })}
                  </span>
                </button>
              ))}
            </nav>
            {/* No key on the tank: the panel stays mounted when another tank is chosen
                (spec 29 §1), so it never shows a loading frame or enters again. */}
            <TankDetail
              tank={current}
              onOpenBattle={(battleId) => onOpenBattle(battleId, current.vehicleId)}
              onOpenMap={onOpenMap}
              takeEntrance={takeEntrance}
            />
          </div>
        );
      }}
    </Async>
  );
}
