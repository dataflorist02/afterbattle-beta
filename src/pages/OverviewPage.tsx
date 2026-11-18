/**
 * Overview (spec 32 §4.2, makieta RC-Przeglad): the last session on the whole width with the
 * best battle's map as its background; trends over the last 100 battles with small line charts
 * next to the week's goals; the best tanks (percentile of the tank's median damage in its
 * tier × class, decision 2026-11-18) next to the A-1 position per tier × class. Only data the
 * app already counts; every block opens its screen. No judging words (D20).
 */
import { api } from "../api";
import { useDataVersion } from "../app/dataVersion";
import { MapBackdrop } from "../components/MapBackdrop";
import { useTankLabel } from "../components/TankName";
import { Card, VehicleImage } from "../components/ui";
import { goalTitle } from "../components/WeeklyReport";
import { useAsync } from "../hooks/useAsync";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { formatDate, formatNumber, formatPercent, formatTier, formatTime } from "../lib/format";
import { sessionSentences } from "../lib/recap/session";
import type {
  AppSnapshot,
  Language,
  PerformanceMetricTrend,
  ReferenceCell,
  VehicleClass,
  WeeklyGoal,
} from "../types";

/** The best tanks shown (decision 2026-11-18: top 3). */
const BEST_SHOWN = 3;
/** Cells of the A-1 grid with values, then cells still collecting their background. */
const CELLS_SHOWN = 8;
const PROGRESS_SHOWN = 2;
/** Columns of the A-1 grid; the last tile widens to the end of its row (no holes). */
const GRID_COLUMNS = 4;
/** The A-1 values start at tier VI (spec 32 §2.2). */
const MIN_TIER = 6;

/** Short group names: "ciężkie X" on a tile, "w ciężkich X" after a percentile. */
const GROUP_KEYS: Partial<Record<VehicleClass, [UiKey, UiKey]>> = {
  light_tank: ["ovGroupLight", "ovInLight"],
  medium_tank: ["ovGroupMedium", "ovInMedium"],
  heavy_tank: ["ovGroupHeavy", "ovInHeavy"],
  tank_destroyer: ["ovGroupTd", "ovInTd"],
  spg: ["ovGroupSpg", "ovInSpg"],
};
/** Singular class name in a tank's line ("średni X"). */
const CLASS_ONE_KEYS: Partial<Record<VehicleClass, UiKey>> = {
  light_tank: "ovClassLight",
  medium_tank: "ovClassMedium",
  heavy_tank: "ovClassHeavy",
  tank_destroyer: "ovClassTd",
  spg: "ovClassSpg",
};

/** "1 bitwa", "64 bitwy", "131 bitew" (Polish plural forms; English "battle(s)"). */
function battlesText(count: number, language: Language): string {
  const number = formatNumber(count, language);
  if (language === "en") return `${number} ${count === 1 ? "battle" : "battles"}`;
  const form = new Intl.PluralRules("pl-PL").select(count);
  return `${number} ${form === "one" ? "bitwa" : form === "few" ? "bitwy" : "bitew"}`;
}

function Block({
  id,
  title,
  aside,
  children,
}: {
  id: string;
  title: string;
  aside?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Card labelledBy={`${id}-h`} className="section overview-block">
      <div className="overview-head">
        <h2 id={`${id}-h`} className="section-title">
          {title}
        </h2>
        {aside}
      </div>
      {children}
    </Card>
  );
}

/** A small line chart of a trend's points; the numbers are in the text next to it. */
function Sparkline({ metric, label }: { metric: PerformanceMetricTrend | null; label: string }) {
  const { language, tr } = useLanguage();
  const values = (metric?.points ?? [])
    .map((point) => point.value)
    .filter((value): value is number => value !== null && Number.isFinite(value));
  if (values.length < 2) return null;
  const width = 200;
  const height = 36;
  const low = Math.min(...values);
  const high = Math.max(...values);
  const span = high - low || 1;
  const points = values
    .map((value, index) => {
      const x = (index / (values.length - 1)) * width;
      const y = height - 3 - ((value - low) / span) * (height - 6);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <svg
      className="overview-spark"
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      role="img"
      aria-label={tr("ovSparkline", {
        metric: label,
        count: values.length,
        from: formatNumber(values[0], language),
        to: formatNumber(values[values.length - 1], language),
      })}
    >
      <polyline points={points} fill="none" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

/** How far a goal is: the week's value against the target, 0–100%. */
function goalShare(goal: WeeklyGoal): number {
  if (goal.current === null || goal.target <= 0) return 0;
  return Math.min(100, Math.max(0, (goal.current / goal.target) * 100));
}

export function OverviewPage({
  snapshot,
  onOpenSession,
  onOpenTank,
  onOpenGoals,
  onOpenHistory,
  onOpenTanks,
}: {
  snapshot: AppSnapshot;
  onOpenSession: (sessionId: string) => void;
  onOpenTank: (vehicleId: number) => void;
  onOpenGoals: () => void;
  onOpenHistory: () => void;
  onOpenTanks: () => void;
}) {
  const { language, address, tr } = useLanguage();
  const label = useTankLabel();
  const version = useDataVersion();
  const options = { keepData: true, refreshKey: version };
  const session = snapshot.activeSession ?? snapshot.recentSessions[0] ?? null;
  const sessionId = session?.id ?? null;
  const summary = useAsync(
    () => (sessionId ? api.sessionSummary(sessionId) : Promise.resolve(null)),
    [sessionId],
    options,
  );
  const recap = useAsync(
    () => (sessionId ? api.recap("session", sessionId).catch(() => null) : Promise.resolve(null)),
    [sessionId],
    options,
  );
  const trends = useAsync(() => api.performanceTrends("last100"), [], options);
  const vehicles = useAsync(() => api.vehicleHistoryOverview(), [], options);
  const tables = useAsync(() => api.referenceTables(), [], options);
  const weekly = useAsync(() => api.weeklyReport(), [], options);
  const list = vehicles.state.status === "ready" ? vehicles.state.data.vehicles : null;

  const noBattles =
    !snapshot.activeSession && snapshot.recentSessions.length === 0 && list?.length === 0;
  if (noBattles)
    return (
      <>
        <header className="page-header">
          <h1 className="page-title">{tr("ovTitle")}</h1>
        </header>
        <div className="card state">
          <p className="muted">{tr("ovEmpty")}</p>
        </div>
      </>
    );

  const number = (value: number | null | undefined) => formatNumber(value, language);
  const lastSession = summary.state.status === "ready" ? summary.state.data : null;
  // The map of the session's best battle (most damage) behind the card (spec 32 §4.2, D19).
  const bestBattle = [...(lastSession?.battles ?? [])].sort(
    (a, b) => (b.battle.damage ?? -1) - (a.battle.damage ?? -1),
  )[0]?.battle;
  const recapData = recap.state.status === "ready" ? recap.state.data : null;
  const firstSentence =
    language === "pl" && recapData?.kind === "session"
      ? (sessionSentences(recapData, {
          address,
          tank: (key: string, name: string | null) => label(key, name).short,
        })[0]?.text ?? null)
      : null;
  const xvmOn = snapshot.advancedStatus.xvmEnabled;
  const sessionWn8 = xvmOn ? (lastSession?.wn8.value ?? null) : null;
  const sessionTime =
    session &&
    tr("ovSessionTime", {
      date: formatDate(session.startedAt, language),
      from: formatTime(session.startedAt, language),
      to: formatTime(session.lastBattleAt ?? session.endedAt ?? session.startedAt, language),
      battles: battlesText(session.analyzedBattleCount, language),
    });

  const trendReport = trends.state.status === "ready" ? trends.state.data : null;
  const trend = (name: string) =>
    trendReport?.metrics.find((metric) => String(metric.metric) === name) ?? null;
  const trendColumn = (
    name: string,
    title: string,
    format: (value: number | null) => string,
    note?: string,
  ) => {
    const metric = trend(name);
    return (
      <div className="overview-trend" key={name}>
        <span className="overview-trend-label">{title}</span>
        <span className="overview-big">{format(metric?.currentValue ?? null)}</span>
        <span className="overview-trend-note">
          {tr("ovPrevious", { value: format(metric?.previousValue ?? null) })}
          {note ? ` · ${note}` : ""}
        </span>
        <Sparkline metric={metric} label={title} />
      </div>
    );
  };
  const wn8 = trend("wn8");

  const tableData = tables.state.status === "ready" ? tables.state.data : null;
  const byId = new Map((list ?? []).map((tank) => [tank.vehicleId, tank]));
  const bestRows = (tableData?.tanks ?? [])
    .filter((tank) => byId.has(tank.vehicleId))
    .slice(0, BEST_SHOWN);
  const mostPlayed = [...(list ?? [])].sort((a, b) => b.totalBattles - a.totalBattles).slice(0, 3);
  const groupName = (cell: { tier: number; vehicleClass: VehicleClass }, index: 0 | 1) => {
    const keys = GROUP_KEYS[cell.vehicleClass];
    const tier = formatTier(cell.tier) ?? String(cell.tier);
    return keys ? tr(keys[index], { tier }) : tier;
  };
  const damageOf = (cell: ReferenceCell) =>
    cell.metrics.find((metric) => metric.metric === "damage") ?? null;
  const valueCells = (tableData?.cells ?? [])
    .filter((cell) => cell.level !== "count_only" && damageOf(cell)?.ownPosition != null)
    .sort((a, b) => b.ownBattles - a.ownBattles)
    .slice(0, CELLS_SHOWN);
  const progressCells = (tableData?.cells ?? [])
    .filter((cell) => cell.tier >= MIN_TIER && cell.level === "count_only")
    .sort((a, b) => b.population - a.population)
    .slice(0, PROGRESS_SHOWN);
  // A value tile takes one column, a collecting one two; the last tile fills its row.
  const spans = [...valueCells.map(() => 1), ...progressCells.map(() => 2)];
  const used = spans.reduce((sum, span) => sum + span, 0) % GRID_COLUMNS;
  if (spans.length > 0 && used !== 0) spans[spans.length - 1] += GRID_COLUMNS - used;
  const spanStyle = (index: number) =>
    spans[index] > 1 ? { gridColumn: `span ${spans[index]}` } : undefined;
  const report = weekly.state.status === "ready" ? weekly.state.data : null;
  const judged = report?.goals.filter((goal) => goal.state !== "too_few") ?? [];

  return (
    <>
      <header className="page-header">
        <h1 className="page-title">{tr("ovTitle")}</h1>
      </header>
      <div className="overview-grid">
        {sessionId && (
          <MapBackdrop
            as="section"
            mapKey={bestBattle?.mapKey ?? null}
            width={1200}
            className="flare-hero overview-session"
            aria-labelledby="ov-session-h"
            label={
              bestBattle?.mapKey
                ? tr("bestMapBackground", { map: bestBattle.mapName ?? bestBattle.mapKey })
                : undefined
            }
          >
            <div className="overview-head">
              <div>
                <h2 id="ov-session-h" className="section-title">
                  {tr("ovLastSession")}
                </h2>
                {sessionTime && <p className="overview-session-time">{sessionTime}</p>}
              </div>
              <button type="button" className="link" onClick={() => onOpenSession(sessionId)}>
                {tr("ovOpenSession")}
              </button>
            </div>
            <div className="overview-session-body">
              {lastSession && (
                <div className="flare-figures overview-session-figures">
                  <div className="figure figure-main">
                    <span className="num figure-value">
                      {number(lastSession.metrics.averageDamage)}
                    </span>
                    <span className="figure-label">{tr("figureDamage")}</span>
                  </div>
                  <div className="figure">
                    <span className="num figure-value">
                      {tr("figureOf", {
                        value: number(lastSession.metrics.wins),
                        count: lastSession.metrics.outcomeSampleSize,
                      })}
                    </span>
                    <span className="figure-label">{tr("figureWins")}</span>
                  </div>
                  {sessionWn8 !== null && (
                    <div className="figure">
                      <span className="num figure-value">{number(sessionWn8)}</span>
                      <span className="figure-label">{tr("sessionWn8")}</span>
                    </div>
                  )}
                </div>
              )}
              {firstSentence && <p className="overview-session-sentence">{firstSentence}</p>}
            </div>
          </MapBackdrop>
        )}

        <div className="overview-row overview-row-wide">
          <Block
            id="ov-trends"
            title={tr("ovTrends")}
            aside={
              <button type="button" className="link" onClick={onOpenHistory}>
                {tr("ovOpenHistory")}
              </button>
            }
          >
            <p className="muted overview-note">{tr("ovTrendsNote")}</p>
            <div className="overview-trends">
              {trendColumn("damage", tr("ovDamage"), (value) => number(value))}
              {wn8?.currentValue === null || wn8?.currentValue === undefined ? (
                <div className="overview-trend">
                  <span className="overview-trend-label">{tr("ovWn8")}</span>
                  <span className="overview-trend-note">{tr("ovWn8Off")}</span>
                </div>
              ) : (
                trendColumn("wn8", tr("ovWn8"), (value) => number(value))
              )}
              {trendColumn(
                "win_rate",
                tr("ovWinRate"),
                (value) => formatPercent(value, language),
                tr("ovWinRateNote"),
              )}
            </div>
          </Block>

          <Block
            id="ov-goals"
            title={tr("ovGoals")}
            aside={
              <button type="button" className="link" onClick={onOpenGoals}>
                {tr("ovOpenGoals")}
              </button>
            }
          >
            {judged.length === 0 ? (
              <p className="muted">{tr("ovGoalsNone")}</p>
            ) : (
              <p className="overview-goals-count">
                <span className="overview-big">
                  {tr("figureOf", {
                    value: judged.filter((goal) => goal.state === "achieved").length,
                    count: report?.goals.length ?? 0,
                  })}
                </span>{" "}
                <span className="muted">{tr("ovGoalsAchieved")}</span>
              </p>
            )}
            {(report?.goals ?? []).map((goal) => (
              <div className="overview-goal" key={`${goal.kind}-${goal.vehicleId ?? ""}`}>
                <span>
                  {goalTitle(goal, label(goal.vehicleKey ?? "", goal.tankName).short, language, tr)}
                </span>
                <span
                  className="overview-goal-bar"
                  role="img"
                  aria-label={tr("ovGoalShare", {
                    share: formatPercent(goalShare(goal), language),
                  })}
                >
                  <span style={{ width: `${goalShare(goal)}%` }} />
                </span>
              </div>
            ))}
            {snapshot.activeChallenge && <p className="muted">{tr("ovChallenge")}</p>}
          </Block>
        </div>

        <div className="overview-row">
          <Block
            id="ov-tanks"
            title={tr("ovTanks")}
            aside={
              <button type="button" className="link" onClick={onOpenTanks}>
                {tr("ovAllTanks")}
              </button>
            }
          >
            <p className="muted overview-note">{tr("ovBestRule")}</p>
            {bestRows.length === 0 ? (
              <p className="muted">{tr("ovBestNone")}</p>
            ) : (
              <ul className="overview-list">
                {bestRows.map((row) => {
                  const tank = byId.get(row.vehicleId)!;
                  const name = label(tank.vehicleKey, tank.tankName).full;
                  const classKey = CLASS_ONE_KEYS[row.vehicleClass];
                  const tier = formatTier(row.tier) ?? String(row.tier);
                  return (
                    <li key={row.vehicleId}>
                      <button
                        type="button"
                        className="overview-link"
                        onClick={() => onOpenTank(row.vehicleId)}
                      >
                        <span className="overview-tank">
                          <VehicleImage vehicleKey={tank.vehicleKey} label={name} />
                          <span>
                            {name}
                            <span className="muted">
                              {" · "}
                              {classKey ? `${tr(classKey)} ${tier}` : tier}
                              {" · "}
                              {battlesText(row.battles, language)}
                            </span>
                          </span>
                        </span>
                        <span className="overview-percentile">
                          <span className="overview-big">{Math.round(row.position)}.</span>{" "}
                          <span className="muted">{groupName(row, 1)}</span>
                        </span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
            {mostPlayed.length > 0 && (
              <p className="muted overview-note">
                {tr("ovMostPlayedLine", {
                  tanks: mostPlayed
                    .map(
                      (tank) =>
                        `${label(tank.vehicleKey, tank.tankName).short} (${number(tank.totalBattles)})`,
                    )
                    .join(", "),
                })}
              </p>
            )}
          </Block>

          <Block
            id="ov-reference"
            title={tr("refTitle")}
            aside={
              <button type="button" className="link" onClick={onOpenTanks}>
                {tr("ovOpenTanks")}
              </button>
            }
          >
            <p className="muted overview-note">{tr("ovCellsNote")}</p>
            {valueCells.length === 0 && progressCells.length === 0 ? (
              <p className="muted">{tr("ovBackgroundNone")}</p>
            ) : (
              <ul className="overview-cells">
                {valueCells.map((cell, index) => (
                  <li
                    key={`${cell.tier}-${cell.vehicleClass}`}
                    className="overview-cell"
                    style={spanStyle(index)}
                  >
                    <span className="overview-cell-name">{groupName(cell, 0)}</span>
                    <span className="overview-big">
                      {Math.round(damageOf(cell)!.ownPosition!)}.
                    </span>
                  </li>
                ))}
                {progressCells.map((cell, index) => (
                  <li
                    key={`${cell.tier}-${cell.vehicleClass}`}
                    className="overview-cell overview-cell-progress"
                    style={spanStyle(valueCells.length + index)}
                  >
                    <span className="overview-cell-name">{groupName(cell, 0)}</span>
                    <span>{tr("refTooFewShort", { count: number(cell.population) })}</span>
                  </li>
                ))}
              </ul>
            )}
          </Block>
        </div>
      </div>
    </>
  );
}
