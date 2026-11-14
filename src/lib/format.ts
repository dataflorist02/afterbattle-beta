import type { Language } from "../types";

/** Group every number ("3 184"), also in Polish where CLDR skips 4-digit numbers. */
// The TypeScript DOM library still types useGrouping as boolean; browsers accept "always".
const ALWAYS = "always" as unknown as boolean;

const locale = (language: Language) => (language === "pl" ? "pl-PL" : "en-GB");

/** Whole number with a non-breaking thin grouping, e.g. "3 184". */
export function formatNumber(value: number | null | undefined, language: Language): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return new Intl.NumberFormat(locale(language), {
    maximumFractionDigits: 0,
    useGrouping: ALWAYS,
  }).format(Math.round(value));
}

export function formatPercent(value: number | null | undefined, language: Language): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  return `${new Intl.NumberFormat(locale(language), {
    maximumFractionDigits: 0,
    useGrouping: ALWAYS,
  }).format(Math.round(value))}%`;
}

/** Thousands with one decimal, e.g. "21,7 tys." / "21.7k". */
export function formatThousands(value: number | null | undefined, language: Language): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "—";
  const thousands = new Intl.NumberFormat(locale(language), {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
    useGrouping: ALWAYS,
  }).format(value / 1000);
  return language === "pl" ? `${thousands} tys.` : `${thousands}k`;
}

const ROMAN_TIERS = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI"];

/** Vehicle tier 1..11 as the game writes it ("X"), or null when unknown or out of range. */
export function formatTier(tier: number | null | undefined): string | null {
  if (tier === null || tier === undefined || !Number.isInteger(tier)) return null;
  return ROMAN_TIERS[tier - 1] ?? null;
}

export function formatTime(timestamp: number, language: Language): string {
  return new Intl.DateTimeFormat(locale(language), { hour: "2-digit", minute: "2-digit" }).format(
    new Date(timestamp),
  );
}

export function formatDate(timestamp: number, language: Language): string {
  return new Intl.DateTimeFormat(locale(language), {
    day: "numeric",
    month: "long",
    year: "numeric",
  }).format(new Date(timestamp));
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || seconds < 0) return "—";
  const minutes = Math.floor(seconds / 60);
  return `${minutes}:${String(Math.floor(seconds % 60)).padStart(2, "0")}`;
}

/** Calendar days between two moments in local time (0 = same day). */
export function daysBetween(earlier: number, later: number): number {
  const start = new Date(earlier);
  const end = new Date(later);
  const day = (value: Date) => Date.UTC(value.getFullYear(), value.getMonth(), value.getDate());
  return Math.round((day(end) - day(start)) / 86_400_000);
}

/** Signed relative change in whole percent, or null when there is no baseline. */
export function relativeChange(current: number | null, baseline: number | null): number | null {
  if (current === null || baseline === null || baseline === 0) return null;
  return Math.round(((current - baseline) / baseline) * 100);
}
