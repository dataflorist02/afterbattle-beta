/**
 * Żar (spec 33 §1–§2): one temperature scale for the whole app. A number is warmer or cooler
 * than the player's own average in its context, never than other players; a direction, not a
 * judgement (D20). The thresholds are shared with the "Jak to działa" texts in Settings.
 */
import type { CSSProperties } from "react";

/** Inside ±5% of the own average a result is "as usual". */
export const USUAL_PERCENT = 5;
/** The intensity grows linearly up to ±30% and stays the same beyond it. */
export const FULL_HEAT_PERCENT = 30;
/** Sparks on the session card only: 3 at +5%, 18 at +30% and more. */
export const SPARKS_MIN = 3;
export const SPARKS_MAX = 18;

export type Temperature = "cooler" | "usual" | "warmer";

export interface Heat {
  temperature: Temperature;
  /** 0 on the ±5% boundary (and as usual), 1 from ±30%. */
  intensity: number;
  /** The signed difference from the own average, in percent. */
  change: number;
}

/** Temperature of a difference from the own average, given in percent; null without one. */
export function heatOf(change: number | null | undefined): Heat | null {
  if (change === null || change === undefined || !Number.isFinite(change)) return null;
  const size = Math.abs(change);
  if (size < USUAL_PERCENT) return { temperature: "usual", intensity: 0, change };
  const intensity = Math.min(1, (size - USUAL_PERCENT) / (FULL_HEAT_PERCENT - USUAL_PERCENT));
  return { temperature: change > 0 ? "warmer" : "cooler", intensity, change };
}

/** Temperature of `value` against the player's own `average` (no comparison without one). */
export function heatAgainst(
  value: number | null | undefined,
  average: number | null | undefined,
): Heat | null {
  if (value === null || value === undefined || average === null || average === undefined)
    return null;
  if (!(average > 0) || !Number.isFinite(value)) return null;
  // Rounded to a millionth of a percent, so 1050 against 1000 is exactly +5%.
  return heatOf(Math.round((value / average - 1) * 100 * 1e6) / 1e6);
}

/** Sparks over the session card: none unless warmer, then 3 to 18 by the intensity. */
export function sparkCount(heat: Heat | null): number {
  if (heat?.temperature !== "warmer") return 0;
  return Math.round(SPARKS_MIN + (SPARKS_MAX - SPARKS_MIN) * heat.intensity);
}

/**
 * What CSS needs: the temperature as `data-temp` and the intensity as `--heat` (0–1). The colors
 * come from the ember tokens; the element keeps its temperature with reduced motion as well.
 */
export function heatProps(heat: Heat | null): { "data-temp"?: Temperature; style?: CSSProperties } {
  if (!heat) return {};
  return {
    "data-temp": heat.temperature,
    style: { "--heat": String(Math.round(heat.intensity * 1000) / 1000) } as CSSProperties,
  };
}
