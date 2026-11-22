/**
 * One damage reference for the Session screen (E5-ui §4): the hero card and the insights both read
 * it, so they can never disagree. Rules follow the backend insights (D7, `session_insights.rs`):
 * a tank counts with 30 or more baseline battles, the compared session needs 3 or more battles,
 * and a deviation of 10% or more is worth an insight.
 */
import type { SessionBaselineComparison, SessionInsight } from "../types";

export const MIN_BASELINE_BATTLES = 30;
export const MIN_SESSION_BATTLES = 3;
export const DAMAGE_THRESHOLD_PERCENT = 10;

export type ReferenceState = "ready" | "session_too_small" | "baseline_too_small" | "no_data";

export interface SessionDamageReference {
  /** Session damage on the compared tanks, weighted by their session battles. */
  current: number | null;
  /** The player's earlier damage on the same tanks, with the same weights. */
  baseline: number | null;
  /** Whole percent, signed; null unless `state` is "ready". */
  change: number | null;
  /** Session battles on the compared tanks. */
  sessionBattles: number;
  /** Smallest baseline among the session's tanks (for "12 of 30"). */
  baselineBattles: number;
  /** Tanks that entered the comparison. */
  vehicles: number;
  state: ReferenceState;
}

export function sessionDamageReference(
  comparison: SessionBaselineComparison | null,
): SessionDamageReference {
  const empty: SessionDamageReference = {
    current: null,
    baseline: null,
    change: null,
    sessionBattles: 0,
    baselineBattles: 0,
    vehicles: 0,
    state: "no_data",
  };
  if (!comparison || comparison.vehicles.length === 0) return empty;
  let current = 0;
  let baseline = 0;
  let weight = 0;
  let vehicles = 0;
  let smallest = Number.POSITIVE_INFINITY;
  for (const vehicle of comparison.vehicles) {
    const reference = vehicle.baseline.averageDamage;
    smallest = Math.min(smallest, reference.eligibleBattles);
    const battles = vehicle.sessionMetricBattles?.damage ?? vehicle.sessionEligibleBattles;
    const own = vehicle.damage.currentValue;
    if (
      reference.value === null ||
      reference.eligibleBattles < MIN_BASELINE_BATTLES ||
      own === null ||
      battles <= 0
    )
      continue;
    current += own * battles;
    baseline += reference.value * battles;
    weight += battles;
    vehicles += 1;
  }
  const baselineBattles = Number.isFinite(smallest) ? smallest : 0;
  if (vehicles === 0) {
    const anyBaseline = comparison.vehicles.some(
      (vehicle) => vehicle.baseline.averageDamage.eligibleBattles >= MIN_BASELINE_BATTLES,
    );
    return {
      ...empty,
      baselineBattles,
      state: anyBaseline ? "session_too_small" : "baseline_too_small",
    };
  }
  const result = {
    current: current / weight,
    baseline: baseline / weight,
    change: null,
    sessionBattles: weight,
    baselineBattles,
    vehicles,
  };
  if (weight < MIN_SESSION_BATTLES) return { ...result, state: "session_too_small" };
  const change = result.baseline > 0 ? Math.round((result.current / result.baseline - 1) * 100) : 0;
  return { ...result, change, state: "ready" };
}

/**
 * Temperature of a session (spec 33 §3): its damage against the player's own average on the same
 * tanks, the reference of the hero note; none until that comparison is ready. Session, Overview
 * and the History thermogram all read it here.
 */
export type InsightLine =
  | {
      kind: "session_damage";
      direction: "positive" | "negative";
      percent: number;
      baseline: number;
    }
  | {
      kind: "tank";
      key: string;
      signal: SessionInsight;
      vehicleKey: string | null;
      tankName: string | null;
    };

/** Why there is no insight line: the only case that may say "close to your average" is the first. */
export type InsightFallback =
  | "close_to_average"
  | "damage_close_tanks_too_few"
  | "session_too_small"
  | "baseline_too_small"
  | "no_data";

export function sessionInsightLines(comparison: SessionBaselineComparison | null): {
  lines: InsightLine[];
  fallback: InsightFallback | null;
} {
  const reference = sessionDamageReference(comparison);
  const tankLines: InsightLine[] = (comparison?.insights.summaries ?? []).flatMap((summary) =>
    [summary.strongestPositive, summary.strongestNegative]
      .filter((signal): signal is SessionInsight => signal !== null)
      .map((signal) => ({
        kind: "tank" as const,
        key: `${summary.vehicleId}-${signal.metric}`,
        signal,
        vehicleKey: summary.vehicleKey,
        tankName: summary.tankName,
      })),
  );
  const lines: InsightLine[] = [];
  const damageDeviates =
    reference.state === "ready" &&
    reference.change !== null &&
    Math.abs(reference.change) >= DAMAGE_THRESHOLD_PERCENT;
  const repeatsTankSignal =
    reference.vehicles === 1 &&
    tankLines.some((line) => line.kind === "tank" && line.signal.metric === "damage");
  if (damageDeviates && !repeatsTankSignal && reference.baseline !== null)
    lines.push({
      kind: "session_damage",
      direction: (reference.change ?? 0) > 0 ? "positive" : "negative",
      percent: Math.abs(reference.change ?? 0),
      baseline: reference.baseline,
    });
  lines.push(...tankLines);
  if (lines.length > 0) return { lines, fallback: null };
  if (reference.state !== "ready") return { lines, fallback: reference.state };
  return {
    lines,
    fallback:
      comparison?.insights.state === "available"
        ? "close_to_average"
        : "damage_close_tanks_too_few",
  };
}
