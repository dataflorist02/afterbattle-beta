/**
 * "Jak to działa" (spec 33 §4): the explanations of how the app counts, in one place in Settings
 * instead of sentences next to the numbers. Every number in the texts comes from a constant of
 * the code; the screen tests compare them with the Rust sources where the rule lives there.
 */
import type { UiKey } from "../i18n/ui";
import { FULL_HEAT_PERCENT, SPARKS_MAX, SPARKS_MIN, USUAL_PERCENT } from "./ember";
import { CARD_HEIGHT, CARD_WIDTH } from "../features/damast/card";
import {
  DAMAGE_WEIGHT,
  DAMAST_REFERENCE_BATTLES,
  MIN_TIER_BATTLES,
  WN8_WEIGHT,
} from "../features/damast/stats";
import { formatTier } from "./format";
import {
  DAMAGE_THRESHOLD_PERCENT,
  MIN_BASELINE_BATTLES,
  MIN_SESSION_BATTLES,
} from "./sessionReference";
import { MIN_TANK_BATTLES } from "../pages/TanksPage";

/** A-1 limits of `src-tauri/src/reference.rs` (MIN_ROWS, MIN_ROWS_APPROXIMATE, MIN_OWN_BATTLES). */
export const A1_MIN_ROWS = 200;
export const A1_MIN_ROWS_APPROXIMATE = 50;
export const A1_MIN_OWN_BATTLES = 10;
export const A1_MIN_TIER = 6;

export type HowId =
  "battles" | "average" | "wn8" | "players" | "summaries" | "temperature" | "damast" | "privacy";

export interface HowEntry {
  id: HowId;
  title: UiKey;
  paragraphs: { key: UiKey; params?: Record<string, string | number> }[];
}

/** The entries in their order; `minutes` is the player's inactivity limit (a setting). */
export function howItWorks(minutes: number): HowEntry[] {
  return [
    {
      id: "battles",
      title: "howBattlesTitle",
      paragraphs: [{ key: "howBattlesText" }, { key: "howBattlesSession", params: { minutes } }],
    },
    {
      id: "average",
      title: "howAverageTitle",
      paragraphs: [
        { key: "howAverageText" },
        {
          key: "howAverageSession",
          params: {
            session: MIN_SESSION_BATTLES,
            baseline: MIN_BASELINE_BATTLES,
            threshold: DAMAGE_THRESHOLD_PERCENT,
          },
        },
        { key: "howAverageTank", params: { tank: MIN_TANK_BATTLES } },
        { key: "howAverageTrends" },
        { key: "howAverageGoals" },
        { key: "howAverageAttempts" },
      ],
    },
    { id: "wn8", title: "howWn8Title", paragraphs: [{ key: "howWn8Text" }] },
    {
      id: "players",
      title: "howPlayersTitle",
      paragraphs: [
        { key: "howPlayersText" },
        {
          key: "howPlayersLimits",
          params: {
            rows: A1_MIN_ROWS,
            approximate: A1_MIN_ROWS_APPROXIMATE,
            own: A1_MIN_OWN_BATTLES,
            tier: formatTier(A1_MIN_TIER) ?? String(A1_MIN_TIER),
          },
        },
        { key: "howPlayersPercentile" },
        { key: "howPlayersLegend" },
      ],
    },
    { id: "summaries", title: "howSummariesTitle", paragraphs: [{ key: "howSummariesText" }] },
    {
      id: "temperature",
      title: "howTemperatureTitle",
      paragraphs: [
        { key: "howTemperatureText" },
        {
          key: "howTemperatureLimits",
          params: {
            usual: USUAL_PERCENT,
            full: FULL_HEAT_PERCENT,
            sparksMin: SPARKS_MIN,
            sparksMax: SPARKS_MAX,
          },
        },
        { key: "howTemperatureMotion" },
      ],
    },
    {
      id: "damast",
      title: "howDamastTitle",
      paragraphs: [
        { key: "howDamastText" },
        {
          key: "howDamastResult",
          params: {
            wn8: Math.round(WN8_WEIGHT * 100),
            damage: Math.round(DAMAGE_WEIGHT * 100),
            window: DAMAST_REFERENCE_BATTLES,
            tier: MIN_TIER_BATTLES,
            usual: USUAL_PERCENT,
          },
        },
        { key: "howDamastForge", params: { width: CARD_WIDTH, height: CARD_HEIGHT } },
      ],
    },
    {
      id: "privacy",
      title: "howPrivacyTitle",
      paragraphs: [{ key: "howPrivacyLocal" }, { key: "howPrivacyUpdates" }],
    },
  ];
}
