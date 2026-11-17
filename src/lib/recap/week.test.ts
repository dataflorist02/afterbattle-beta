import { describe, expect, it } from "vitest";
import {
  NUMBERS,
  checkCitations,
  checkNeutral,
  checkSentence,
  random,
  withoutNames,
} from "../../test/recapInvariants";
import type { GoalKind, GoalState, WeekFact, WeekFactValue, WeeklyMetric } from "../../types";
import { formatNumber } from "../format";
import { ADDRESSES, type Address } from "./address";
import { finish, fnv1a, variantsFor } from "./compose";
import { weekPlans, weekSentences } from "./week";

const context = (address: Address) => ({
  address,
  tank: (key: string, name: string | null) => name ?? key,
});

function numbered(values: WeekFactValue[]): WeekFact[] {
  const important = ["liczba_bitew", "wskaznik", "cel"];
  return values.map((fact, index) => ({
    id: `F${index + 1}`,
    important: important.includes(fact.key),
    fact,
  }));
}

function full(): WeekFact[] {
  return numbered([
    { key: "liczba_bitew", battles: 39, confidence: "good", minimum: 15 },
    { key: "odsetek_wygranych", percent: 55 },
    { key: "wskaznik", metric: "damage_assist", percent: 105, usual: 100 },
    { key: "wskaznik", metric: "survival", percent: 92, usual: 100 },
    { key: "cel", kind: "survival", state: "achieved", vehicleKey: null, tankName: null },
    {
      key: "cel",
      kind: "tank_damage_assist",
      state: "close",
      vehicleKey: "synth-a",
      tankName: "Alpha",
    },
    { key: "poprzedni_tydzien", achieved: 1, goals: 3 },
    { key: "wyzwania", completed: 1, failed: 0, abandoned: 1, battles: 5 },
  ]);
}

const text = (facts: WeekFact[], seed = 1, address: Address = "neutral") =>
  weekSentences({ seed, facts }, context(address));

describe("week summary (spec 30 §4.3)", () => {
  it("says only that the week has too few battles under 15", () => {
    const sentences = text(
      numbered([{ key: "liczba_bitew", battles: 6, confidence: "too_few", minimum: 15 }]),
    );
    expect(sentences).toHaveLength(1);
    expect(sentences[0].text).toMatch(/^Za mało bitew w tym tygodniu/u);
    expect(sentences[0].text).toMatch(/6/u);
  });

  it("groups indicators and goals into one sentence each", () => {
    const sentences = text(full());
    expect(sentences.map((sentence) => sentence.facts)).toEqual([
      ["F1", "F2"],
      ["F3", "F4"],
      ["F5", "F6"],
      ["F7"],
      ["F8"],
    ]);
    expect(sentences[2].text).toMatch(/Alpha/u);
  });

  it("varies over 10 weeks and is stable for one", () => {
    expect(text(full(), 5)).toEqual(text(full(), 5));
    const texts = Array.from({ length: 10 }, (_, index) =>
      text(full(), fnv1a(String(1_800_000_000_000 + index * 604_800_000)))
        .map((sentence) => sentence.text)
        .join(" "),
    );
    expect(new Set(texts).size).toBeGreaterThanOrEqual(6);
    for (let index = 1; index < texts.length; index++)
      expect(texts[index]).not.toBe(texts[index - 1]);
  });

  it("never calls the win rate a goal (D12)", () => {
    for (let seed = 0; seed < 30; seed++) {
      const wins = text(full(), seed).find((sentence) => sentence.facts.includes("F2"))!;
      expect(wins.text).not.toMatch(/cel/u);
    }
  });
});

const METRIC_LIST: WeeklyMetric[] = ["damage_assist", "survival", "xp"];
const KINDS: GoalKind[] = ["survival", "tank_damage_assist", "xp"];
const STATES: GoalState[] = ["achieved", "close", "below"];

function week(seed: number): WeekFact[] {
  const next = random(seed);
  const chance = (share: number) => next() < share;
  const pick = <T>(items: readonly T[]) => items[Math.floor(next() * items.length)];
  const battles = chance(0.25) ? NUMBERS[seed % 15] : 15 + Math.floor(next() * 90);
  const confidence = battles < 15 ? "too_few" : battles < 30 ? "preliminary" : "good";
  const facts: WeekFactValue[] = [{ key: "liczba_bitew", battles, confidence, minimum: 15 }];
  if (confidence === "too_few") return numbered(facts);
  if (chance(0.9)) facts.push({ key: "odsetek_wygranych", percent: Math.floor(next() * 101) });
  for (const metric of METRIC_LIST)
    if (chance(0.6)) {
      const percent = 100 + (chance(0.5) ? 1 : -1) * (5 + Math.floor(next() * 40));
      facts.push({ key: "wskaznik", metric, percent, usual: 100 });
    }
  for (const kind of KINDS)
    if (chance(0.6))
      facts.push({
        key: "cel",
        kind,
        state: pick(STATES),
        vehicleKey: kind === "tank_damage_assist" ? "synth-a" : null,
        tankName: kind === "tank_damage_assist" ? pick(["Alpha", "Kolos IV", "Synth 44 M"]) : null,
      });
  if (chance(0.7)) {
    const goals = 1 + Math.floor(next() * 3);
    facts.push({ key: "poprzedni_tydzien", achieved: Math.floor(next() * (goals + 1)), goals });
  }
  if (chance(0.6))
    facts.push({
      key: "wyzwania",
      completed: pick(NUMBERS.slice(0, 26)),
      failed: Math.floor(next() * 4),
      abandoned: Math.floor(next() * 3),
      battles: 5,
    });
  return numbered(facts);
}

function names(facts: WeekFact[]): string[] {
  return facts.flatMap((item) =>
    item.fact.key === "cel" && item.fact.tankName ? [item.fact.tankName] : [],
  );
}

function numbers(fact: WeekFactValue): string[] {
  const values: number[] = [];
  switch (fact.key) {
    case "liczba_bitew":
      values.push(fact.battles, fact.minimum);
      break;
    case "odsetek_wygranych":
      values.push(fact.percent);
      break;
    case "wskaznik":
      values.push(fact.percent, fact.usual);
      break;
    case "poprzedni_tydzien":
      values.push(fact.achieved, fact.goals);
      break;
    case "wyzwania":
      values.push(fact.completed, fact.failed, fact.abandoned, fact.battles);
      break;
    default:
      break;
  }
  return values.map((value) => formatNumber(value, "pl"));
}

const ABOUT: Partial<Record<WeekFactValue["key"], RegExp>> = {
  odsetek_wygranych: /wygran/u,
  wskaznik: /zwykł|jak zwykle/u,
  cel: /cele tygodnia/u,
  poprzedni_tydzien: /poprzedni/u,
  wyzwania: /wyzwania/u,
};

const WEEKS = Array.from({ length: 50 }, (_, index) => week(index + 1));

describe("week summary invariants (spec 30 §7)", () => {
  it("holds for every variant of every sentence in all three forms", () => {
    let checked = 0;
    for (const facts of WEEKS)
      for (const address of ADDRESSES)
        for (const plan of weekPlans(facts, context(address))) {
          const cited = facts.filter((item) => plan.facts.includes(item.id));
          for (const variant of variantsFor(plan, address)) {
            if (address === "neutral") checkNeutral(finish(variant, address));
            checkSentence(finish(variant, address), {
              allowed: new Set(cited.flatMap((item) => numbers(item.fact))),
              signs: [],
              names: names(facts),
            });
            checked += 1;
          }
        }
    expect(checked).toBeGreaterThan(500);
  });

  it("holds for the chosen text of 50 weeks", () => {
    let texts = 0;
    for (const [index, facts] of WEEKS.entries())
      for (const address of ADDRESSES) {
        const sentences = weekSentences({ seed: index * 31, facts }, context(address));
        if (address === "neutral") for (const { text } of sentences) checkNeutral(text);
        if (sentences.length === 0) continue;
        const tooFew =
          facts[0].fact.key === "liczba_bitew" && facts[0].fact.confidence === "too_few";
        checkCitations(sentences, facts, tooFew ? [1, 1] : [3, 5]);
        const whole = withoutNames(
          sentences.map((sentence) => sentence.text).join(" "),
          names(facts),
        );
        const kept = new Set(facts.map((item) => item.fact.key));
        for (const [key, pattern] of Object.entries(ABOUT))
          if (!kept.has(key as WeekFactValue["key"])) expect(whole, key).not.toMatch(pattern);
        texts += 1;
      }
    expect(texts).toBeGreaterThan(100);
  });
});
