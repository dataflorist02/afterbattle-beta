/**
 * Weekly report (spec 20 E7, spec 27, mockup `Cele.dc.html` in direction C): goals for this
 * week, this week against the four before, last week's result. Rules come from Rust
 * (`get_weekly_report`); win rate is context only, never a goal (D12).
 */
import type { ReactNode } from "react";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { formatNumber, formatPercent } from "../lib/format";
import type { Language, WeeklyGoal, WeeklyIndicator, WeeklyReport } from "../types";
import { useTankLabel } from "./TankName";
import { VehicleImage } from "./ui";

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

/** "9–15 listopada" / "9–15 November" (the end is the Sunday before the exclusive week end). */
export function weekRange(start: number, end: number, language: Language): string {
  const locale = language === "pl" ? "pl-PL" : "en-GB";
  const last = new Date(Math.min(end, start + WEEK_MS) - 1);
  const first = new Date(start);
  const day = new Intl.DateTimeFormat(locale, { day: "numeric" });
  const dayMonth = new Intl.DateTimeFormat(locale, { day: "numeric", month: "long" });
  return first.getMonth() === last.getMonth()
    ? `${day.format(first)}–${dayMonth.format(last)}`
    : `${dayMonth.format(first)} – ${dayMonth.format(last)}`;
}

function goalValue(goal: WeeklyGoal, value: number | null, language: Language): string {
  return goal.kind === "survival" ? formatPercent(value, language) : formatNumber(value, language);
}

function goalTitle(goal: WeeklyGoal, tank: string, language: Language, tr: Translate): string {
  const target = goalValue(goal, goal.target, language);
  if (goal.kind === "survival") return tr("weeklyGoalSurvival", { target });
  if (goal.kind === "xp") return tr("weeklyGoalXp", { target });
  return tr("weeklyGoalTank", { tank, target });
}

function goalStatus(goal: WeeklyGoal, language: Language, tr: Translate): string {
  switch (goal.state) {
    case "achieved":
      return tr("weeklyStateAchieved", { battles: goal.battles });
    case "close":
      return tr("weeklyStateClose", {
        share: formatPercent(((goal.current ?? 0) / goal.target) * 100, language),
      });
    case "below":
      return goal.kind === "survival"
        ? tr("weeklyStateNowSurvival", { survived: goal.survived ?? 0, battles: goal.battles })
        : tr("weeklyStateNow", { battles: goal.battles });
    default:
      return tr(
        goal.kind === "tank_damage_assist" ? "weeklyStateTooFewTank" : "weeklyStateTooFew",
        {
          battles: goal.battles,
        },
      );
  }
}

function goalExplanation(goal: WeeklyGoal, language: Language, tr: Translate): string {
  return goal.kind === "survival"
    ? tr("weeklyWhySurvival", { basis: formatPercent(goal.basis, language) })
    : tr("weeklyWhyAverage", { basis: formatNumber(goal.basis, language) });
}

function GoalCard({ goal }: { goal: WeeklyGoal }) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  const tank = label(goal.vehicleKey, goal.tankName);
  const share = goal.current === null ? 0 : Math.min(1, goal.current / goal.target);
  return (
    <div
      id={`recap-target-goal-${goal.kind}`}
      className="card section weekly-goal"
      data-state={goal.state}
    >
      <h3 className="section-title goal-title">
        {goal.kind === "tank_damage_assist" && (
          <VehicleImage vehicleKey={goal.vehicleKey} label={tank.full} width={40} height={22} />
        )}
        {goalTitle(goal, tank.short, language, tr)}
      </h3>
      <p className="weekly-goal-value">
        <span className="num">{goalValue(goal, goal.current, language)}</span>{" "}
        <span className="muted">{goalStatus(goal, language, tr)}</span>
      </p>
      <div
        className="track"
        role="img"
        aria-label={
          goal.state === "achieved"
            ? tr("weeklyTrackDone")
            : tr("weeklyTrackShare", { share: Math.round(share * 100) })
        }
      >
        <div style={{ width: `${share * 100}%` }} />
      </div>
      <p className="muted">{goalExplanation(goal, language, tr)}</p>
    </div>
  );
}

const metricKeys: Record<WeeklyIndicator["metric"], UiKey> = {
  damage_assist: "weeklyMetricDamage",
  survival: "weeklyMetricSurvival",
  xp: "weeklyMetricXp",
};

function range(values: (number | null)[], format: (value: number) => string): string | null {
  const known = values.filter((value): value is number => value !== null);
  if (known.length === 0) return null;
  const low = Math.min(...known);
  const high = Math.max(...known);
  return format(low) === format(high) ? format(low) : `${format(low)}–${format(high)}`;
}

export function WeeklyReportView({
  report,
  xvmOn,
  summary = null,
}: {
  report: WeeklyReport;
  xvmOn: boolean;
  /** The week's summary card (spec 30 §2), right under the header. */
  summary?: ReactNode;
}) {
  const { language, tr } = useLanguage();
  const percent = (value: number) => formatPercent(value, language);
  // "97–101%": the unit once, after the range.
  const percentRange = (values: (number | null)[]) => {
    const plain = range(values, (value) => formatNumber(value, language));
    return plain && `${plain}%`;
  };
  const lastAchieved = report.previousWeek.goals.filter((goal) => goal.state === "achieved").length;
  return (
    <>
      <header className="page-header">
        <div className="section">
          <h1 className="page-title">{tr("weeklyTitle")}</h1>
          <p id="recap-target-week-battles" className="muted">
            {tr("weeklySubtitle", {
              range: weekRange(report.weekStart, report.weekEnd, language),
              battles: report.battles,
            })}
          </p>
        </div>
      </header>
      {summary}
      {report.goals.length === 0 ? (
        <p className="muted">{tr("weeklyNoGoals")}</p>
      ) : (
        <section className="weekly-goals" aria-label={tr("weeklyGoalsLabel")}>
          {report.goals.map((goal) => (
            <GoalCard key={goal.kind} goal={goal} />
          ))}
        </section>
      )}
      <section className="card section" aria-labelledby="week-h">
        <div className="card-head">
          <div>
            <h2 id="week-h" className="section-title">
              {tr("weeklyVsTitle")}
            </h2>
            <p className="muted">{tr("weeklyVsNote")}</p>
          </div>
        </div>
        {report.confidence === "too_few" && (
          <p className="muted">{tr("weeklyTooFew", { battles: report.battles })}</p>
        )}
        {report.confidence === "preliminary" && (
          <p className="muted">{tr("weeklyPreliminary", { battles: report.battles })}</p>
        )}
        <div className="weekly-rows">
          {report.indicators.map((indicator) => {
            const before = percentRange(indicator.previous);
            return (
              <div
                key={indicator.metric}
                id={`recap-target-indicator-${indicator.metric}`}
                className="weekly-row"
              >
                <span>{tr(metricKeys[indicator.metric])}</span>{" "}
                <span className="num weekly-value">
                  {indicator.current === null ? "—" : percent(indicator.current)}
                </span>{" "}
                <span className="muted">
                  {before ? tr("weeklyBefore", { range: before }) : tr("weeklyBeforeNone")}
                </span>
              </div>
            );
          })}
          {xvmOn && report.wn8 && (
            <div className="weekly-row">
              <span>{tr("weeklyWn8")}</span>{" "}
              <span className="num weekly-value">{formatNumber(report.wn8.current, language)}</span>{" "}
              <span className="muted">
                {range(report.wn8.previous, (value) => formatNumber(value, language))
                  ? tr("weeklyBefore", {
                      range: range(report.wn8.previous, (value) =>
                        formatNumber(value, language),
                      ) as string,
                    })
                  : tr("weeklyBeforeNone")}
              </span>
            </div>
          )}
          <div id="recap-target-week-wins" className="weekly-row">
            <span>{tr("weeklyWins")}</span>{" "}
            <span className="num weekly-value">{formatPercent(report.winRate, language)}</span>{" "}
            <span className="muted">{tr("weeklyWinsContext")}</span>
          </div>
        </div>
      </section>
      <section className="section" aria-labelledby="last-h">
        <h2 id="last-h" className="section-title">
          {tr("weeklyLastTitle")}
        </h2>
        <p id="recap-target-previous-week" className="muted">
          {report.previousWeek.goals.length === 0
            ? tr("weeklyLastNone", { battles: report.previousWeek.battles })
            : tr("weeklyLastLine", {
                achieved: lastAchieved,
                goals: report.previousWeek.goals.length,
                battles: report.previousWeek.battles,
              })}
        </p>
        {(report.challenges.completed > 0 ||
          report.challenges.failed > 0 ||
          report.challenges.abandoned > 0) && (
          <p id="recap-target-challenges" className="muted">
            {tr("weeklyChallenges", {
              completed: report.challenges.completed,
              failed: report.challenges.failed,
              abandoned: report.challenges.abandoned,
            })}
          </p>
        )}
      </section>
    </>
  );
}
