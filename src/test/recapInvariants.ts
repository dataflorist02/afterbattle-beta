/**
 * Shared checks of the summary invariants (spec 30 §7), used by the session, battle and week
 * tests: numbers from cited facts, directions with their sign, words from the SJP.PL list, no
 * judging words, sentence form and length, citations and coverage.
 */
import { appendFileSync, readFileSync } from "node:fs";
import { expect } from "vitest";
import type { RecapSentence } from "../lib/recap/compose";

export const WORDS = new Set(
  readFileSync("src/lib/recap/words.pl.txt", "utf8")
    .split(/\r?\n/)
    .map((word) => word.trim())
    .filter((word) => word && !word.startsWith("#")),
);
/** Game terms allowed next to the dictionary (spec 28 §6). */
export const TERMS = new Set(["wn", "xp"]);
/** D20 and judging words (spec 30 §1). */
export const FORBIDDEN =
  /^(wpływ\p{L}*|wkład\p{L}*|carry|impact|dobrze|dobr[aey]|słab\p{L}*|niestety|gratul\p{L}*|brawo|świetn\p{L}*|kiepsk\p{L}*|źle|mocn\p{L}*|doskonał\p{L}*|super|porażk\p{L}*|trudn\p{L}*)$/u;
const MORE = /^(więcej|wyższe|wyżej)$/u;
const LESS = /^(mniej|niższe|niżej)$/u;
const NUMBER = /\d+(?:[\u00a0\u202f]\d{3})*(?:,\d+)?/gu;

/** Numbers the dictionary check must see in every form (spec 30 §7 point 4). */
export const NUMBERS = [
  ...Array.from({ length: 26 }, (_, value) => value),
  100,
  101,
  102,
  103,
  104,
  105,
  1_000,
  1_001,
  1_002,
  1_003,
  1_004,
  1_005,
];

/**
 * Impersonal past-tense verbs ("-no/-to") the neutral form must not use (author decision
 * 2026-11-16): every verb the texts ever had in that form, and the ones a stem could give.
 */
export const IMPERSONAL_VERBS = [
  "grano",
  "zagrano",
  "rozegrano",
  "wygrano",
  "przegrano",
  "zremisowano",
  "przeżyto",
  "rozkręcano",
  "zadano",
  "zniszczono",
  "wykryto",
  "oddano",
  "zwolniono",
];

/** Point 1 of the 2026-11-16 corrections: no impersonal verb in a neutral text. */
export function checkNeutral(text: string) {
  const words = text.toLocaleLowerCase("pl-PL").match(/\p{L}+/gu) ?? [];
  for (const word of words) expect(IMPERSONAL_VERBS, `"${word}" in "${text}"`).not.toContain(word);
}

/**
 * The only names a reference may have (last corrections of 2026-11-16): one per kind of fact.
 * The text right before the bracket with the reference ends with one of them.
 */
export const REFERENCE_NAMES: readonly RegExp[] = [
  /w ostatnich \d+ bitw(ie|ach)$/u,
  /zwykle na tych czołgach$/u,
  /zwykle na tym czołgu$/u,
  /w \d+ dni(u|ach) przed sesją$/u,
];

/** What a sentence with a reference must show. */
export type ReferenceShape =
  | {
      /** The current value as the first number ("3 414"; "7" in "7 z 12 bitew, czyli 58%"). */
      current: string;
      /** For a rate: the percent after "czyli" ("58%"). */
      percent?: string;
      /** The reference as written in the brackets ("3 021", "51%"). */
      reference: string;
    }
  | {
      /** The value is zero: the sentence ends with "; zwykle <this>." and has no difference. */
      zero: string;
    };

/**
 * Corrections of 2026-11-16: one syntax for every sentence with a reference — the current value
 * is the first number, the reference is the only number in brackets, at the end, after one of
 * `REFERENCE_NAMES`; a zero value reads "<zero>; zwykle <reference>." without a difference.
 */
export function checkReference(text: string, names: string[], shape: ReferenceShape) {
  const plain = withoutNames(text, names);
  if ("zero" in shape) {
    expect(text.endsWith(`; zwykle ${shape.zero}.`), text).toBe(true);
    expect(plain, text).not.toMatch(/więcej|mniej|tyle samo|\(/u);
    return;
  }
  expect(plain.match(NUMBER)?.[0], text).toBe(shape.current);
  if (shape.percent) expect(text, text).toContain(`, czyli ${shape.percent},`);
  const brackets = [...plain.matchAll(/\(([^()]*)\)/gu)].map((match) => match[1]);
  expect(brackets, text).toEqual([shape.reference]);
  expect(plain, text).toMatch(/\)\.$/u);
  const named = text.slice(0, text.lastIndexOf(" ("));
  expect(
    REFERENCE_NAMES.some((name) => name.test(named)),
    text,
  ).toBe(true);
}

/**
 * One fact, one sentence (2026-11-16): at most one semicolon, and only in the zero format
 * "<zero>; zwykle <reference>".
 */
export function checkSemicolons(text: string) {
  const count = text.split(";").length - 1;
  expect(count, text).toBeLessThanOrEqual(1);
  if (count === 1) expect(text, text).toMatch(/^[^;]+; zwykle [^;]+\.$/u);
}

/** A small seeded generator (mulberry32), so the data is the same in every run. */
export function random(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

/** Proper names removed before words and numbers are read; "WN8" keeps only its letters. */
export function withoutNames(text: string, names: string[]): string {
  return [...names]
    .sort((left, right) => right.length - left.length)
    .reduce((rest, name) => rest.replaceAll(name, " "), text)
    .replaceAll("WN8", "WN");
}

export interface SentenceFacts {
  /** Every number the cited facts allow, as `format.ts` writes it. */
  allowed: Set<string>;
  /** Signed differences of the cited facts that have a reference. */
  signs: number[];
  /** Proper names of all facts of the summary. */
  names: string[];
}

/** Points 1, 2, 4, 7 and 9 (length and form) for one sentence. */
export function checkSentence(text: string, facts: SentenceFacts) {
  const plain = withoutNames(text, facts.names);
  for (const number of plain.match(NUMBER) ?? [])
    expect(facts.allowed, `${number} in "${text}"`).toContain(number);
  const words = plain.toLocaleLowerCase("pl-PL").match(/\p{L}+/gu) ?? [];
  if (words.some((word) => MORE.test(word)))
    expect(
      facts.signs.some((value) => value > 0),
      text,
    ).toBe(true);
  if (words.some((word) => LESS.test(word)))
    expect(
      facts.signs.some((value) => value < 0),
      text,
    ).toBe(true);
  for (const word of words) {
    // RECAP_MISSING_WORDS=<file> collects missing words instead of failing, to extend the list
    // (then check it with scripts/check-recap-words.py).
    const missing = !WORDS.has(word) && !TERMS.has(word);
    if (missing && process.env.RECAP_MISSING_WORDS)
      appendFileSync(process.env.RECAP_MISSING_WORDS, word + String.fromCharCode(10));
    else expect(missing, `"${word}" in "${text}"`).toBe(false);
  }
  for (const word of words) expect(word, text).not.toMatch(FORBIDDEN);
  expect(text.split(" ").length, text).toBeLessThanOrEqual(25);
  // A capital letter, or the zero of "0 obrażeń; zwykle …" (2026-11-16).
  expect(text, text).toMatch(/^(\p{Lu}|0 )[^]*\.$/u);
  checkSemicolons(text);
}

/** Points 1 (citations), 5, 8 and 9 (3–5 sentences) for a whole text. */
export function checkCitations(
  sentences: RecapSentence[],
  facts: { id: string; important: boolean }[],
  sentenceRange: [number, number] = [3, 5],
) {
  expect(sentences.length).toBeGreaterThanOrEqual(sentenceRange[0]);
  expect(sentences.length).toBeLessThanOrEqual(sentenceRange[1]);
  const ids = new Set(facts.map((item) => item.id));
  const cited = sentences.flatMap((sentence) => sentence.facts);
  for (const id of cited) expect(ids.has(id)).toBe(true);
  expect(new Set(cited).size).toBe(cited.length);
  for (const item of facts) if (item.important) expect(cited).toContain(item.id);
}
