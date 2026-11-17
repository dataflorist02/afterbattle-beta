import { describe, expect, it } from "vitest";
import {
  NUMBERS,
  checkCitations,
  checkNeutral,
  checkReference,
  checkSentence,
  random,
  withoutNames,
} from "../../test/recapInvariants";
import type { BattleFact, BattleFactValue } from "../../types";
import { formatNumber } from "../format";
import { ADDRESSES, fitsAddress, type Address } from "./address";
import { legacyTexts } from "../../i18n/legacy";
import { neutralPl, uiTexts } from "../../i18n/ui";
import { battleFactLabel, battlePlans, battleSentences } from "./battle";
import { fnv1a, finish, variantsFor } from "./compose";

const context = (address: Address) => ({
  address,
  tank: (key: string, name: string | null) => name ?? key,
});

function numbered(values: BattleFactValue[], important: string[] = []): BattleFact[] {
  return values.map((fact, index) => ({
    id: `F${index + 1}`,
    important: important.includes(fact.key),
    fact,
  }));
}

const IMPORTANT = ["wynik", "przetrwanie", "obrazenia"];

function full(): BattleFact[] {
  return numbered(
    [
      { key: "wynik", outcome: "victory" },
      { key: "mapa", mapName: "Test Valley" },
      { key: "czolg", vehicleKey: "synth-a", tankName: "Alpha", tier: 8 },
      { key: "przetrwanie", survived: true },
      { key: "obrazenia", damage: 2_028, reference: 1_959 },
      { key: "zniszczone", frags: 2 },
      { key: "miejsce", place: 2 },
      { key: "pancerz", blocked: 1_850 },
      { key: "strzaly", shots: 18, hits: 12, penetrations: 8 },
    ],
    [...IMPORTANT, "zniszczone", "miejsce"],
  );
}

const text = (facts: BattleFact[], seed = 3, address: Address = "neutral") =>
  battleSentences({ seed, facts }, context(address));

describe("battle summary (spec 30 §4.1, §5)", () => {
  it("opens with result, map, tank and survival, then damage, then the rest", () => {
    const sentences = text(full());
    expect(sentences.map((sentence) => sentence.facts)).toEqual([
      ["F1", "F2", "F3", "F4"],
      ["F5"],
      ["F6"],
      ["F7"],
      ["F8"],
    ]);
  });

  it("never says the tank survived when it was destroyed, nor a win after a loss", () => {
    for (let seed = 0; seed < 40; seed++) {
      const lost = full().map((item) =>
        item.fact.key === "przetrwanie"
          ? { ...item, fact: { key: "przetrwanie" as const, survived: false } }
          : item.fact.key === "wynik"
            ? { ...item, fact: { key: "wynik" as const, outcome: "defeat" as const } }
            : item,
      );
      const first = text(lost, seed)[0].text;
      expect(first).not.toMatch(/przeży|przetrwał|wygra/iu);
      expect(first).toMatch(/zniszczony/u);
      expect(first).toMatch(/przegra/iu);
    }
  });

  it("gives the same text for the same battle and varies over 10 battles", () => {
    expect(text(full(), 99)).toEqual(text(full(), 99));
    const texts = Array.from({ length: 10 }, (_, index) =>
      text(full(), fnv1a(String(4_000 + index)))
        .map((sentence) => sentence.text)
        .join(" "),
    );
    expect(new Set(texts).size).toBeGreaterThanOrEqual(6);
    // The opening has one wording since 2026-11-16, so one repeat in a row is allowed.
    const repeats = texts.filter((value, index) => index > 0 && value === texts[index - 1]);
    expect(repeats.length).toBeLessThanOrEqual(1);
  });

  it("gives survival its own sentence when the summary would be too short", () => {
    const short = numbered(
      [
        { key: "wynik", outcome: "draw" },
        { key: "czolg", vehicleKey: "synth-a", tankName: "Alpha", tier: null },
        { key: "przetrwanie", survived: false },
        { key: "obrazenia", damage: 0, reference: null },
      ],
      IMPORTANT,
    );
    expect(text(short).map((sentence) => sentence.facts)).toEqual([["F1", "F2"], ["F3"], ["F4"]]);
    expect(text(short.slice(1, 2))).toEqual([]);
  });

  it("says the place in words, in its own sentence after destroyed vehicles", () => {
    const sentences = text(full(), 5, "masculine");
    expect(sentences[2].text).toMatch(
      /2 pojazdy przeciwnika|przeciwnika( zniszczone w tej bitwie)?: 2/u,
    );
    expect(sentences[3].text).toMatch(/drugie miejsce w drużynie/u);
    expect(sentences[3].facts).toEqual(["F7"]);
  });
});

const TANKS = ["Alpha", "Synth 44 M", "Kolos IV", "Bravo-2"];
const MAPS = ["Test Valley", "Synthetic Hills", "Dolina 9", "Port Delta"];

/** A random battle with the facts a backend could keep (rules of §4.1 already applied). */
function battle(seed: number): BattleFact[] {
  const next = random(seed);
  const pick = <T>(items: readonly T[]) => items[Math.floor(next() * items.length)];
  const chance = (share: number) => next() < share;
  let forced = 0;
  const value = (max: number) => {
    forced += 1;
    if (forced <= 3) return NUMBERS[(seed * 3 + forced) % NUMBERS.length];
    return chance(0.3) ? pick(NUMBERS) : Math.floor(next() * max);
  };
  const facts: BattleFactValue[] = [];
  if (chance(0.95))
    facts.push({ key: "wynik", outcome: pick(["victory", "defeat", "draw"] as const) });
  if (chance(0.9)) facts.push({ key: "mapa", mapName: pick(MAPS) });
  facts.push({
    key: "czolg",
    vehicleKey: "synth-a",
    tankName: chance(0.9) ? pick(TANKS) : null,
    tier: 8,
  });
  if (chance(0.95)) facts.push({ key: "przetrwanie", survived: chance(0.4) });
  if (chance(0.95)) {
    const damage = chance(0.1) ? 0 : value(6_000);
    const reference = chance(0.3) ? null : chance(0.2) ? damage : value(4_000);
    facts.push({ key: "obrazenia", damage, reference });
  }
  const rest: BattleFactValue[] = [];
  if (chance(0.5)) rest.push({ key: "zniszczone", frags: Math.max(1, value(6)) });
  if (chance(0.4)) rest.push({ key: "miejsce", place: 1 + Math.floor(next() * 3) });
  if (chance(0.4))
    rest.push({
      key: "asysta",
      assist: value(3_000),
      reference: chance(0.5) ? null : value(2_000),
    });
  if (chance(0.3)) rest.push({ key: "pancerz", blocked: 500 + value(3_000) });
  if (chance(0.3)) rest.push({ key: "wykryte", spots: value(8) });
  if (!rest.some((item) => item.key === "miejsce") && chance(0.3))
    rest.push({ key: "udzial", percent: 20 + Math.floor(next() * 60) });
  if (chance(0.8)) {
    const shots = value(30);
    const hits = Math.floor(next() * (shots + 1));
    rest.push({ key: "strzaly", shots, hits, penetrations: Math.floor(next() * (hits + 1)) });
  }
  const destroyed = rest.findIndex((item) => item.key === "zniszczone");
  const kept: BattleFactValue[] = [];
  let sentences = 0;
  for (const item of rest) {
    if (sentences >= 3) break;
    if (
      item.key === "miejsce" &&
      destroyed >= 0 &&
      kept.some((fact) => fact.key === "zniszczone")
    ) {
      kept.push(item);
      continue;
    }
    kept.push(item);
    if (!(item.key === "zniszczone" && rest.some((fact) => fact.key === "miejsce"))) sentences += 1;
    else sentences += 1;
  }
  return numbered([...facts, ...kept], [...IMPORTANT, ...(chance(0.5) ? ["zniszczone"] : [])]);
}

function names(facts: BattleFact[]): string[] {
  return facts.flatMap((item) =>
    item.fact.key === "czolg"
      ? [item.fact.tankName ?? item.fact.vehicleKey]
      : item.fact.key === "mapa"
        ? [item.fact.mapName]
        : [],
  );
}

function numbers(fact: BattleFactValue): string[] {
  const values: number[] = [];
  switch (fact.key) {
    case "obrazenia":
    case "asysta": {
      const own = fact.key === "obrazenia" ? fact.damage : fact.assist;
      values.push(own);
      if (fact.reference !== null) values.push(fact.reference, Math.abs(own - fact.reference));
      break;
    }
    case "zniszczone":
      values.push(fact.frags);
      break;
    case "pancerz":
      values.push(fact.blocked);
      break;
    case "wykryte":
      values.push(fact.spots);
      break;
    case "udzial":
      values.push(fact.percent);
      break;
    case "strzaly":
      values.push(fact.shots, fact.hits, fact.penetrations);
      break;
    default:
      break;
  }
  return values.map((value) => formatNumber(value, "pl"));
}

function sign(fact: BattleFactValue): number | null {
  if (fact.key === "obrazenia" && fact.reference !== null) return fact.damage - fact.reference;
  if (fact.key === "asysta" && fact.reference !== null) return fact.assist - fact.reference;
  return null;
}

/** Words that may only appear when their fact was kept (§7 point 6). */
const ABOUT: Partial<Record<BattleFactValue["key"], RegExp>> = {
  zniszczone: /zniszczy\p{L}*|zniszczonych/u,
  asysta: /gąsienic/u,
  pancerz: /pancerz czołgu|przez pancerz/u,
  wykryte: /wykryt|wykrył/u,
  miejsce: /miejsce/u,
  udzial: /drużyny/u,
  strzaly: /strzał|strzały/u,
};

const BATTLES = Array.from({ length: 200 }, (_, index) => battle(index + 1));

describe("battle summary invariants (spec 30 §7)", () => {
  it("holds for every variant of every sentence in all three forms", () => {
    let checked = 0;
    for (const facts of BATTLES)
      for (const address of ADDRESSES)
        for (const plan of battlePlans(facts, context(address))) {
          const cited = facts.filter((item) => plan.facts.includes(item.id));
          for (const variant of variantsFor(plan, address)) {
            if (address === "neutral") checkNeutral(finish(variant, address));
            const sentence = finish(variant, address);
            checkSentence(sentence, {
              allowed: new Set(cited.flatMap((item) => numbers(item.fact))),
              signs: cited.map((item) => sign(item.fact)).filter((value) => value !== null),
              names: names(facts),
            });
            // 3. No contradiction with the text facts.
            for (const item of cited) {
              if (item.fact.key === "przetrwanie")
                expect(sentence, sentence).toMatch(
                  item.fact.survived ? /^(?![^]*zniszczony)/u : /^(?![^]*(przeży|przetrwał))/u,
                );
              if (item.fact.key === "wynik") {
                const other: Record<string, RegExp> = {
                  victory: /przegra|remis/iu,
                  defeat: /wygra|remis/iu,
                  draw: /wygra|przegra/iu,
                };
                expect(sentence, sentence).not.toMatch(other[item.fact.outcome]);
              }
            }
            checked += 1;
          }
        }
    expect(checked).toBeGreaterThan(5_000);
  });

  it("puts the value first and the reference in brackets (decision 2026-11-16)", () => {
    let checked = 0;
    for (const facts of BATTLES)
      for (const address of ADDRESSES)
        for (const plan of battlePlans(facts, context(address))) {
          const cited = facts.filter((item) => plan.facts.includes(item.id));
          for (const variant of variantsFor(plan, address)) {
            const sentence = finish(variant, address);
            for (const { fact } of cited) {
              const value =
                fact.key === "obrazenia" ? fact.damage : fact.key === "asysta" ? fact.assist : null;
              if (value === null || !("reference" in fact) || fact.reference === null) continue;
              const usual = formatNumber(fact.reference, "pl");
              // Zero: "0 obrażeń; zwykle na tym czołgu 2 900." exactly (decision 2026-11-16).
              if (value === 0 && fact.key === "obrazenia")
                expect(sentence).toBe(`0 obrażeń; zwykle na tym czołgu ${usual}.`);
              checkReference(
                sentence,
                names(facts),
                value === 0
                  ? { zero: `na tym czołgu ${usual}` }
                  : { current: formatNumber(value, "pl"), reference: usual },
              );
              checked += 1;
            }
          }
        }
    expect(checked).toBeGreaterThan(500);
  });

  it("says zero damage as 0 with the usual damage, not as none", () => {
    const facts = numbered(
      [
        { key: "wynik", outcome: "draw" },
        { key: "mapa", mapName: "Port Delta" },
        { key: "czolg", vehicleKey: "synth-a", tankName: "Kolos IV", tier: null },
        { key: "przetrwanie", survived: true },
        { key: "obrazenia", damage: 0, reference: 2_900 },
      ],
      IMPORTANT,
    );
    for (const address of ADDRESSES)
      for (const variant of battlePlans(facts, context(address)).find(
        (plan) => plan.slot === "damage",
      )!.variants) {
        expect(finish(variant, address)).toBe(
          `0 obrażeń; zwykle na tym czołgu ${formatNumber(2_900, "pl")}.`,
        );
      }
  });

  it("says allies' damage thanks to the player's spotting, never 'po Twoim wykryciu' (2026-11-16)", () => {
    const facts = numbered(
      [
        { key: "wynik", outcome: "defeat" },
        { key: "obrazenia", damage: 1_120, reference: 1_540 },
        { key: "asysta", assist: 2_630, reference: 2_210 },
      ],
      IMPORTANT,
    );
    const said = (address: Address) =>
      battlePlans(facts, context(address))
        .find((plan) => plan.slot === "assist")!
        .variants.filter((variant) => fitsAddress(variant, address))
        .map((variant) => finish(variant, address));
    const tail = `sojusznicy zadali ${formatNumber(2_630, "pl")} obrażeń, o 420 więcej niż zwykle na tym czołgu (${formatNumber(2_210, "pl")}).`;
    for (const address of ADDRESSES) {
      expect(said(address)).toContain(`Dzięki Twojemu wykrywaniu i zrywaniu gąsienic ${tail}`);
      for (const sentence of said(address)) expect(sentence).not.toMatch(/wykryciu/u);
    }
    expect(said("masculine")).toContain(`Dzięki temu, że wykrywałeś i zrywałeś gąsienice, ${tail}`);
    expect(said("feminine")).toContain(`Dzięki temu, że wykrywałaś i zrywałaś gąsienice, ${tail}`);
    // Nowhere in the interface texts either: reference labels, UI texts, older texts.
    const label = battleFactLabel(
      { key: "asysta", assist: 2_630, reference: 2_210 },
      context("neutral"),
    );
    expect(label).toBe(
      `obrażenia sojuszników dzięki Twojemu wykrywaniu i zrywaniu gąsienic ${formatNumber(2_630, "pl")}`,
    );
    const interfaceTexts = [
      ...Object.values(uiTexts.pl),
      ...Object.values(neutralPl),
      ...Object.values(legacyTexts.pl),
    ];
    for (const value of interfaceTexts) expect(value).not.toMatch(/wykryciu/u);
  });

  it("opens with the result only: result, map, tank and its state (2026-11-16)", () => {
    const opening = (values: BattleFactValue[], address: Address) =>
      battlePlans(numbered(values, IMPORTANT), context(address))[0].variants.map((variant) =>
        finish(variant, address),
      );
    const draw: BattleFactValue[] = [
      { key: "wynik", outcome: "draw" },
      { key: "mapa", mapName: "Port Delta" },
      { key: "czolg", vehicleKey: "synth-a", tankName: "Kolos IV", tier: null },
      { key: "przetrwanie", survived: true },
      { key: "obrazenia", damage: 0, reference: 2_900 },
    ];
    const defeat: BattleFactValue[] = [
      { key: "wynik", outcome: "defeat" },
      { key: "mapa", mapName: "Test Valley" },
      { key: "czolg", vehicleKey: "synth-b", tankName: "Synth 44 M", tier: null },
      { key: "przetrwanie", survived: false },
      { key: "obrazenia", damage: 1_120, reference: 1_540 },
      { key: "wykryte", spots: 6 },
    ];
    for (const address of ADDRESSES) {
      expect(opening(draw, address)).toEqual(["Remis na Port Delta, czołg Kolos IV."]);
      expect(opening(defeat, address)).toEqual([
        "Przegrana na Test Valley, czołg Synth 44 M zniszczony.",
      ]);
    }
  });

  it("holds for the chosen text of 200 battles", () => {
    let texts = 0;
    for (const [index, facts] of BATTLES.entries())
      for (const address of ADDRESSES) {
        const sentences = battleSentences({ seed: index * 104_729, facts }, context(address));
        if (address === "neutral") for (const { text } of sentences) checkNeutral(text);
        if (sentences.length === 0) continue;
        checkCitations(sentences, facts);
        const whole = withoutNames(
          sentences.map((sentence) => sentence.text).join(" "),
          names(facts),
        );
        const kept = new Set(facts.map((item) => item.fact.key));
        for (const [key, pattern] of Object.entries(ABOUT))
          if (!kept.has(key as BattleFactValue["key"])) expect(whole, key).not.toMatch(pattern);
        texts += 1;
      }
    expect(texts).toBeGreaterThan(500);
  });
});
