/**
 * Invariants of the template summaries (spec 30 §7) on synthetic sessions: every number comes
 * from a cited fact, directions follow the sign, no word outside the SJP.PL list, no repeated
 * sense, no dropped fact, no judging word, every important fact said, 3–5 sentences of at most
 * 25 words. Each sentence variant is checked, not only the chosen one, in all three forms.
 */
import { describe, expect, it, vi } from "vitest";
import {
  NUMBERS,
  checkCitations,
  checkNeutral,
  checkReference,
  checkSentence,
  random,
  withoutNames as stripNames,
  type ReferenceShape,
} from "../../test/recapInvariants";
import type { SessionFact, SessionFactValue } from "../../types";
import { formatNumber, formatPercent } from "../format";
import { ADDRESSES, type Address } from "./address";
import { finish, type RecapSentence, variantsFor } from "./compose";
import { sessionPlans, sessionSentences, type SessionTextContext } from "./session";

const TANKS = ["Alpha", "Synth 44 M", "Kolos IV", "Bravo-2", "Delta Ausf. B"];
const MAPS = ["Test Valley", "Synthetic Hills", "Dolina 9", "Port Delta"];
function session(seed: number): SessionFact[] {
  const next = random(seed);
  const pick = <T>(items: readonly T[]) => items[Math.floor(next() * items.length)];
  const chance = (share: number) => next() < share;
  // The first values of each session walk through NUMBERS, so every form is seen.
  let forced = seed;
  const value = (max: number) => {
    forced += 1;
    if (forced - seed <= 3) return NUMBERS[(seed + forced) % NUMBERS.length];
    return chance(0.3) ? pick(NUMBERS) : Math.floor(next() * max);
  };
  const battles = Math.max(1, value(30));
  const facts: SessionFactValue[] = [{ key: "liczba_bitew", battles }];
  const rate = (part: number, total: number) => Math.round((part / total) * 100);
  const reference = (percent: number, minimum = 0) => {
    const earlier = chance(0.5) ? percent : Math.floor(next() * 101);
    const difference = percent - earlier;
    return Math.abs(difference) < minimum
      ? null
      : { percent: earlier, battles: pick([30, 31, 99, 100]), difference };
  };
  if (chance(0.9)) {
    const known = chance(0.8) ? battles : Math.max(1, battles - 1);
    const wins = chance(0.15) ? 0 : Math.floor(next() * (known + 1));
    const percent = rate(wins, known);
    facts.push({
      key: "odsetek_wygranych",
      wins,
      battles: known,
      percent,
      reference: chance(0.3) ? null : reference(percent),
    });
  }
  if (chance(0.7)) {
    const percent = chance(0.15) ? 0 : Math.floor(next() * 121) - 60;
    const baseline = value(5_000);
    facts.push({ key: "obrazenia", current: baseline, baseline, percent });
  }
  if (battles >= 2 && chance(0.85))
    facts.push({
      key: "najlepsza",
      battleId: "b-1",
      damage: value(9_000),
      mapKey: "91_test_valley",
      mapName: chance(0.9) ? pick(MAPS) : null,
    });
  if (chance(0.6)) {
    const own = value(4_000);
    const earlier = chance(0.2) ? null : chance(0.2) ? own : value(4_000);
    facts.push({ key: "wn8", value: own, reference: earlier, referenceDays: 30 });
  }
  if (chance(0.6)) {
    const survived = chance(0.25) ? 0 : Math.floor(next() * (battles + 1));
    const percent = rate(survived, battles);
    const earlier = reference(percent, 5);
    if (earlier)
      facts.push({ key: "przetrwanie_odsetek", survived, battles, percent, reference: earlier });
  }
  if (chance(0.95)) {
    // Rust keeps the tank only above half of the battles (decision 2026-11-16).
    const half = Math.floor(battles / 2) + 1;
    const own = chance(0.4) ? battles : Math.min(battles, half + Math.floor(next() * battles));
    facts.push({
      key: "czolg",
      vehicleKey: "synth-a",
      tankName: chance(0.9) ? pick(TANKS) : null,
      battles: own,
      total: battles,
    });
  }
  const important = new Set(["liczba_bitew", "odsetek_wygranych", "obrazenia", "najlepsza", "wn8"]);
  return facts.map((fact, index) => ({
    id: `F${index + 1}`,
    important: important.has(fact.key),
    fact,
  }));
}

const context = (address: Address): SessionTextContext => ({
  address,
  tank: (key, name) => name ?? key,
});
const SESSIONS = Array.from({ length: 100 }, (_, index) => session(index + 1));

/** Proper names of the facts (tank and map), removed before words and numbers are read. */
function names(facts: SessionFact[]): string[] {
  return facts.flatMap((item) => {
    if (item.fact.key === "czolg") return [item.fact.tankName ?? item.fact.vehicleKey];
    if (item.fact.key === "najlepsza" && item.fact.mapName) return [item.fact.mapName];
    return [];
  });
}

function withoutNames(text: string, facts: SessionFact[]): string {
  return stripNames(text, names(facts));
}

/** Every number a fact allows in a sentence, as `format.ts` writes it. */
function factNumbers(fact: SessionFactValue): string[] {
  const values: number[] = [];
  switch (fact.key) {
    case "liczba_bitew":
      values.push(fact.battles);
      break;
    case "odsetek_wygranych":
    case "przetrwanie_odsetek":
      values.push(
        fact.battles,
        fact.percent,
        fact.key === "odsetek_wygranych" ? fact.wins : fact.survived,
      );
      if (fact.reference)
        values.push(
          fact.reference.percent,
          fact.reference.battles,
          Math.abs(fact.reference.difference),
        );
      break;
    case "obrazenia":
      values.push(fact.baseline, fact.current, Math.abs(fact.percent));
      break;
    case "najlepsza":
      values.push(fact.damage);
      break;
    case "wn8":
      values.push(fact.value, fact.referenceDays);
      if (fact.reference !== null)
        values.push(fact.reference, Math.abs(fact.value - fact.reference));
      break;
    case "czolg":
      values.push(fact.battles, fact.total);
      break;
  }
  return values.map((value) => formatNumber(value, "pl"));
}

/** The signed difference of a fact with a reference; null without one. */
function difference(fact: SessionFactValue): number | null {
  if (fact.key === "odsetek_wygranych" || fact.key === "przetrwanie_odsetek")
    return fact.reference?.difference ?? null;
  if (fact.key === "obrazenia") return fact.percent;
  if (fact.key === "wn8" && fact.reference !== null) return fact.value - fact.reference;
  return null;
}

/** The value and the reference a sentence about this fact must show; null without a reference. */
function referenceShape(fact: SessionFactValue): ReferenceShape | null {
  const number = (value: number) => formatNumber(value, "pl");
  switch (fact.key) {
    case "odsetek_wygranych":
    case "przetrwanie_odsetek": {
      if (!fact.reference) return null;
      const part = fact.key === "odsetek_wygranych" ? fact.wins : fact.survived;
      const reference = formatPercent(fact.reference.percent, "pl");
      // "Wygrałeś 7 z 12 bitew, czyli 58%, …"; zero: "…; zwykle 38%."
      return part === 0
        ? { zero: reference }
        : { current: number(part), percent: formatPercent(fact.percent, "pl"), reference };
    }
    case "obrazenia":
      return fact.current === 0
        ? { zero: `na tych czołgach ${number(fact.baseline)}` }
        : { current: number(fact.current), reference: number(fact.baseline) };
    case "wn8":
      if (fact.reference === null) return null;
      return fact.value === 0
        ? { zero: `w ${number(fact.referenceDays)} dniach przed sesją ${number(fact.reference)}` }
        : { current: number(fact.value), reference: number(fact.reference) };
    default:
      return null;
  }
}

function check(text: string, cited: SessionFact[], all: SessionFact[]) {
  checkSentence(text, {
    allowed: new Set(cited.flatMap((item) => factNumbers(item.fact))),
    signs: cited.map((item) => difference(item.fact)).filter((value) => value !== null),
    names: names(all),
  });
}

/** Words that may only appear when their fact was kept (spec 30 §7 point 6). */
const ABOUT: Record<SessionFactValue["key"], RegExp> = {
  liczba_bitew: /rozegra|sesja liczy/u,
  odsetek_wygranych: /wygra/u,
  obrazenia: /zwykle na tych|średni/u,
  najlepsza: /najlepsz|najwięcej obrażeń/u,
  wn8: /WN8/u,
  przetrwanie_odsetek: /przeży|przetrwa/u,
  czolg: /czołgiem|czołgu|grany czołg/u,
};

function checkText(sentences: RecapSentence[], facts: SessionFact[]) {
  checkCitations(sentences, facts);
  // 6. Nothing about a fact the rules left out.
  const whole = withoutNames(sentences.map((sentence) => sentence.text).join(" "), facts);
  const kept = new Set(facts.map((item) => item.fact.key));
  for (const [key, pattern] of Object.entries(ABOUT))
    if (!kept.has(key as SessionFactValue["key"])) expect(whole, key).not.toMatch(pattern);
  // 3. No survival claim when no battle was survived.
  const survival = facts.find((item) => item.fact.key === "przetrwanie_odsetek");
  if (survival?.fact.key === "przetrwanie_odsetek" && survival.fact.survived === 0)
    expect(whole).not.toMatch(/(przeży\p{L}*|przetrwał) do końca \d/u);
}

// Checking every variant in all three forms took 5 645 ms once in a full parallel run (E11,
// 2026-11-17), over the default 5 s. An explicit limit for this file only (author's decision).
vi.setConfig({ testTimeout: 20_000 });

describe("session summary invariants (spec 30 §7)", () => {
  it("holds for every variant of every sentence in all three forms", () => {
    let checked = 0;
    for (const facts of SESSIONS)
      for (const address of ADDRESSES)
        for (const plan of sessionPlans(facts, context(address))) {
          const cited = facts.filter((item) => plan.facts.includes(item.id));
          for (const variant of variantsFor(plan, address)) {
            if (address === "neutral") checkNeutral(finish(variant, address));
            check(finish(variant, address), cited, facts);
            checked += 1;
          }
        }
    expect(checked).toBeGreaterThan(3_000);
  });

  it("holds for the chosen text of 100 sessions in all three forms", () => {
    let texts = 0;
    for (const [index, facts] of SESSIONS.entries())
      for (const address of ADDRESSES) {
        const sentences = sessionSentences({ seed: index * 7919, facts }, context(address));
        if (address === "neutral") for (const { text } of sentences) checkNeutral(text);
        if (sentences.length === 0) continue;
        checkText(sentences, facts);
        texts += 1;
      }
    expect(texts).toBeGreaterThan(250);
  });

  it("puts the value first and the reference in brackets (decision 2026-11-16)", () => {
    let checked = 0;
    for (const facts of SESSIONS)
      for (const address of ADDRESSES)
        for (const plan of sessionPlans(facts, context(address))) {
          const shapes = facts
            .filter((item) => plan.facts.includes(item.id))
            .map((item) => referenceShape(item.fact))
            .filter((shape) => shape !== null);
          expect(shapes.length).toBeLessThanOrEqual(1);
          if (shapes.length === 0) continue;
          for (const variant of variantsFor(plan, address)) {
            checkReference(finish(variant, address), names(facts), shapes[0]);
            checked += 1;
          }
        }
    expect(checked).toBeGreaterThan(1_000);
  });

  it("covers every number form the dictionary check asks for", () => {
    const seen = new Set(
      SESSIONS.flatMap((facts) => facts.flatMap((item) => factNumbers(item.fact))),
    );
    for (const value of NUMBERS) expect(seen, String(value)).toContain(formatNumber(value, "pl"));
  });
});
