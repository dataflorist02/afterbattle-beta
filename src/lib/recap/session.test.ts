import { describe, expect, it } from "vitest";
import type { SessionFact, SessionFactValue } from "../../types";
import type { Address } from "./address";
import { ADDRESSES, fitsAddress } from "./address";
import { finish, fnv1a } from "./compose";
import { plural } from "./plural";
import { sessionPlans, sessionSentences } from "./session";

const tank = (_key: string, name: string | null) => name ?? "—";

let next = 0;
function fact(value: SessionFactValue, important = true): SessionFact {
  next += 1;
  return { id: `F${next}`, important, fact: value };
}

function full(): SessionFact[] {
  next = 0;
  return [
    fact({ key: "liczba_bitew", battles: 9 }),
    fact({
      key: "odsetek_wygranych",
      wins: 5,
      battles: 9,
      percent: 56,
      reference: { percent: 52, battles: 100, difference: 4 },
    }),
    fact({ key: "obrazenia", current: 2_397, baseline: 2_140, percent: 12 }),
    fact({
      key: "najlepsza",
      battleId: "b-8",
      damage: 4_480,
      mapKey: "91_test_valley",
      mapName: "Test Valley",
    }),
    fact({ key: "wn8", value: 1_850, reference: 1_700, referenceDays: 30 }),
    fact(
      {
        key: "przetrwanie_odsetek",
        survived: 3,
        battles: 9,
        percent: 33,
        reference: { percent: 45, battles: 100, difference: -12 },
      },
      false,
    ),
    fact({ key: "czolg", vehicleKey: "synth-a", tankName: "Alpha", battles: 5, total: 9 }, false),
  ];
}

const text = (facts: SessionFact[], seed = 7, address: Address = "neutral") =>
  sessionSentences({ seed, facts }, { address, tank });

describe("Polish plural forms (Intl.PluralRules)", () => {
  it("picks one, few and many like the language does", () => {
    const forms = ["pojazd", "pojazdy", "pojazdów"] as const;
    expect(
      [0, 1, 2, 4, 5, 12, 14, 22, 25, 101, 102, 1_000, 1_002].map((n) => plural(n, forms)),
    ).toEqual([
      "pojazdów",
      "pojazd",
      "pojazdy",
      "pojazdy",
      "pojazdów",
      "pojazdów",
      "pojazdów",
      "pojazdy",
      "pojazdów",
      "pojazdów",
      "pojazdy",
      "pojazdów",
      "pojazdy",
    ]);
    expect(plural(6.4, ["punkt", "punkty", "punktów"], "punktu")).toBe("punktu");
  });
});

describe("session summary (spec 30 §4.2, §5, §6)", () => {
  it("seeds like the backend (FNV-1a)", () => {
    expect(fnv1a("")).toBe(0x811c9dc5);
    expect(fnv1a("a")).toBe(0xe40c292c);
  });

  it("says every kept fact once, with 3 to 5 sentences ending in a full stop", () => {
    const sentences = text(full());
    expect(sentences.length).toBe(5);
    const cited = sentences.flatMap((sentence) => sentence.facts);
    // The tank (not important) is the sixth fact and stays out (2026-11-16).
    expect([...cited].sort()).toEqual(["F1", "F2", "F3", "F4", "F5", "F6"]);
    for (const sentence of sentences) {
      expect(sentence.text).toMatch(/^\p{Lu}.*\.$/u);
      expect(sentence.text.split(" ").length).toBeLessThanOrEqual(25);
    }
  });

  it("says one fact per sentence and leaves the tank out past five sentences (2026-11-16)", () => {
    for (let seed = 0; seed < 30; seed++) {
      const sentences = text(full(), seed);
      expect(sentences.map((sentence) => sentence.facts)).toEqual([
        ["F1", "F2"],
        ["F3"],
        ["F4"],
        ["F5"],
        ["F6"],
      ]);
      for (const sentence of sentences) expect(sentence.text).not.toContain(";");
    }
    const withoutWn8 = full().filter((item) => item.fact.key !== "wn8");
    const own = text(withoutWn8).find((sentence) => sentence.facts.includes("F7"));
    expect(own?.facts).toEqual(["F7"]);
  });

  it("says a rate as count, then the percent, then the reference (2026-11-16)", () => {
    const said = (address: Address, slot: string) =>
      sessionPlans(full(), { address, tank })
        .find((plan) => plan.slot === slot)!
        .variants.filter((variant) => fitsAddress(variant, address))
        .map((variant) => finish(variant, address));
    expect(said("masculine", "battles_wins")).toContain(
      "Wygrałeś 5 z 9 bitew, czyli 56%, o 4 pkt proc. więcej niż w ostatnich 100 bitwach (52%).",
    );
    expect(said("feminine", "battles_wins")).toContain(
      "Wygrałaś 5 z 9 bitew, czyli 56%, o 4 pkt proc. więcej niż w ostatnich 100 bitwach (52%).",
    );
    for (const address of ADDRESSES)
      expect(said(address, "survival")).toContain(
        "Czołg przetrwał do końca 3 z 9 bitew, czyli 33%, o 12 pkt proc. mniej niż w ostatnich 100 bitwach (45%).",
      );
  });

  it("gives the same text for the same session every time", () => {
    expect(text(full(), 123)).toEqual(text(full(), 123));
  });

  it("does not sound the same over 10 sessions in a row", () => {
    const texts = Array.from({ length: 10 }, (_, index) =>
      text(full(), fnv1a(`synthetic-session-${index + 1}`))
        .map((sentence) => sentence.text)
        .join(" "),
    );
    expect(new Set(texts).size).toBeGreaterThanOrEqual(6);
    for (let index = 1; index < texts.length; index++)
      expect(texts[index]).not.toBe(texts[index - 1]);
    const firsts = new Set(texts.map((value) => value.split(".")[0]));
    expect(firsts.size).toBeGreaterThanOrEqual(2);
  });

  it("never starts two sentences with the same word", () => {
    for (let seed = 0; seed < 200; seed++) {
      const firsts = text(full(), seed).map((sentence) => sentence.text.split(" ")[0]);
      expect(new Set(firsts).size, String(seed)).toBe(firsts.length);
    }
  });

  it("writes the verbs in each address form", () => {
    for (let seed = 0; seed < 30; seed++) {
      const join = (address: Address) =>
        text(full(), seed, address)
          .map((sentence) => sentence.text)
          .join(" ");
      expect(join("neutral")).not.toMatch(/(łeś|łaś)(?!\p{L})/u);
      expect(join("feminine")).not.toMatch(/łeś(?!\p{L})/u);
      expect(join("masculine")).not.toMatch(/łaś(?!\p{L})/u);
    }
    const feminine = Array.from({ length: 10 }, (_, seed) =>
      text(full(), seed, "feminine")
        .map((sentence) => sentence.text)
        .join(" "),
    ).join(" ");
    expect(feminine).toMatch(/łaś(?!\p{L})/u);
  });

  it("says no battle survived instead of 0%", () => {
    const facts = full().map((item) =>
      item.fact.key === "przetrwanie_odsetek"
        ? {
            ...item,
            fact: {
              ...item.fact,
              survived: 0,
              percent: 0,
              reference: { percent: 45, battles: 100, difference: -45 },
            },
          }
        : item,
    );
    for (let seed = 0; seed < 20; seed++) {
      const survival = text(facts, seed).find((sentence) => sentence.facts.includes("F6"))!;
      expect(survival.text).toMatch(/żadnej bitw/u);
      expect(survival.text).not.toMatch(/\b0%/);
    }
    expect(text(facts, 3).find((sentence) => sentence.facts.includes("F6"))!.text).toBe(
      "Czołg nie przetrwał do końca żadnej bitwy sesji; zwykle 45%.",
    );
  });

  it("splits battles and wins when the summary would be shorter than 3 sentences", () => {
    next = 0;
    const few = [
      fact({ key: "liczba_bitew", battles: 1 }),
      fact({ key: "odsetek_wygranych", wins: 1, battles: 1, percent: 100, reference: null }),
      fact({ key: "czolg", vehicleKey: "synth-a", tankName: "Alpha", battles: 1, total: 1 }, false),
    ];
    const sentences = text(few);
    expect(sentences.map((sentence) => sentence.facts)).toEqual([["F1"], ["F2"], ["F3"]]);
  });

  it("is empty when fewer than 3 sentences are possible", () => {
    next = 0;
    const tooFew = [
      fact({ key: "liczba_bitew", battles: 1 }),
      fact({ key: "czolg", vehicleKey: "synth-a", tankName: "Alpha", battles: 1, total: 1 }, false),
    ];
    expect(text(tooFew)).toEqual([]);
    expect(text([])).toEqual([]);
  });
});
