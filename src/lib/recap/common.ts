/** Words and number helpers shared by the Polish summaries (spec 30 §5). */
import type { Address } from "./address";
import { formatNumber, formatPercent } from "../format";
import { plural, type Forms } from "./plural";

export interface RecapTextContext {
  address: Address;
  /** The tank name as the screen shows it (the game's own name when known). */
  tank: (vehicleKey: string, tankName: string | null) => string;
}

/** "rozegrano 1 bitwę / 2 bitwy / 5 bitew" */
export const BATTLES_ACC: Forms = ["bitwę", "bitwy", "bitew"];
/** "z 1 bitwy / z 2 bitew / z 5 bitew" */
export const BATTLES_GEN: Forms = ["bitwy", "bitew", "bitew"];
/** "w sesji 1 bitwa / 2 bitwy / 5 bitew" */
export const BATTLES_NOM: Forms = ["bitwa", "bitwy", "bitew"];
/** "w ostatnich 100 bitwach" */
export const BATTLES_LOC: Forms = ["bitwie", "bitwach", "bitwach"];
export const DAMAGE: Forms = ["obrażenie", "obrażenia", "obrażeń"];
/** Percentage points, always abbreviated (2026-11-16): "o 7 pkt proc. więcej". */
export const POINTS = "pkt proc.";

/** A whole number as the screens write it ("3 184"). */
export const n = (value: number) => formatNumber(value, "pl");
/** A whole percent as the screens write it ("56%"). */
export const pct = (value: number) => formatPercent(value, "pl");
/** A number with its noun in the agreeing form ("2 pojazdy"). */
export const count = (value: number, forms: Forms) => `${n(value)} ${plural(value, forms)}`;
