/**
 * How the Polish texts address the player (spec 30 §2, §5; decisions 2026-11-16): masculine
 * ("zadałeś"), feminine ("zadałaś") or neutral, the default. A gendered text writes a past-tense
 * verb once, as its stem in a `{~stem}` marker; this module makes the form. The neutral form has
 * no verb in the second person and no impersonal "-no/-to" verb either ("Wygrana na …, czołg
 * przetrwał do końca."): texts with a marker are not used in it, they have a neutral variant.
 */

export type Address = "neutral" | "masculine" | "feminine";

export const ADDRESSES: readonly Address[] = ["neutral", "masculine", "feminine"];
export const DEFAULT_ADDRESS: Address = "neutral";

/** The past-tense form of a verb stem ("zada" → "zadałeś" / "zadałaś"). */
export function pastVerb(stem: string, address: Address): string {
  if (address === "masculine") return `${stem}łeś`;
  if (address === "feminine") return `${stem}łaś`;
  throw new Error(`The neutral form has no verb for the stem "${stem}"`);
}

const MARKER = /\{~([^{}]+)\}/g;

/** Every `{~stem}` in `text` in the chosen form; the neutral form refuses a text with one. */
export function applyAddress(text: string, address: Address): string {
  return text.replace(MARKER, (_, stem: string) => pastVerb(stem, address));
}

/** Stems of every `{~stem}` marker in `text`. */
export function verbStems(text: string): string[] {
  return [...text.matchAll(MARKER)].map((match) => match[1]);
}

/** Whether a text can be said in this form: the neutral form takes only texts without a verb. */
export function fitsAddress(text: string, address: Address): boolean {
  return address !== "neutral" || verbStems(text).length === 0;
}
