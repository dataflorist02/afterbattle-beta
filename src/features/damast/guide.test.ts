import { describe, expect, it } from "vitest";
import { t as translate } from "../../i18n/ui";
import type { DamastSession, Language } from "../../types";
import { plateGuide } from "./guide";
import { damastStats, MIN_BEST_BATTLES } from "./stats";

/** r of each battle, its tier and win: a synthetic session whose r is exactly these values. */
const SHAPE: [r: number, tier: number, win: boolean][] = [
  [1.0, 8, true],
  [1.2, 9, false],
  [1.3, 10, true],
  [1.0, 10, true],
  [0.8, 9, false],
  [1.0, 8, true],
];
const OWN_WN8 = 2_000;
const OWN_DAMAGE = 2_800;

function synthetic(
  shape = SHAPE,
  { wn8 = true, reference = true }: { wn8?: boolean; reference?: boolean } = {},
): DamastSession {
  return {
    session: {
      id: "session-guide",
      startedAt: Date.UTC(2026, 10, 20, 17),
      endedAt: Date.UTC(2026, 10, 20, 20),
      status: "ended",
    } as DamastSession["session"],
    wn8: wn8 ? 2_200 : null,
    averageDamage: 3_000,
    battles: shape.map(([r, tier, win], index) => ({
      battleId: `guide-${index}`,
      battleTime: Date.UTC(2026, 10, 20, 17, index * 10),
      tier,
      damage: Math.round(OWN_DAMAGE * r),
      wn8: wn8 ? Math.round(OWN_WN8 * r) : null,
      win,
    })),
    reference: {
      cutoff: Date.UTC(2026, 10, 20, 17),
      wn8: reference ? OWN_WN8 : null,
      wn8Battles: reference ? 100 : 0,
      averageDamage: reference ? OWN_DAMAGE : null,
      damageBattles: reference ? 100 : 0,
      tiers: reference
        ? [8, 9, 10].map((tier) => ({ tier, battles: 30, averageDamage: OWN_DAMAGE }))
        : [],
    },
  };
}

const MAPS: Record<string, string> = { "guide-2": "Synthetic Hills" };
const mapOf = (battleId: string) => MAPS[battleId] ?? null;

function guide(session: DamastSession, language: Language = "pl") {
  const tr = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
    translate(language, key, params);
  const rows = plateGuide(session, damastStats(session), mapOf, language, tr);
  return Object.fromEntries(rows.map((row) => [row.kind, row.lines.join(" ")]));
}

describe("how to read this plate (author, 22.11)", () => {
  it("reads the session's own plate, row by row (PL)", () => {
    const rows = guide(synthetic());
    expect(Object.keys(rows)).toEqual(["order", "tint", "eyes", "tiers", "waves", "spark"]);
    expect(rows.order).toBe("Bitwy idą od lewej do prawej: 6 bitew.");
    // Battles 2 and 3 are warmer than usual side by side (1.2, 1.3); battle 5 the coolest.
    expect(rows.tint).toBe(
      "Najcieplej przy bitwach 2–3, najchłodniej przy bitwie 5. " +
        "Ciepło i chłód: wobec Twojej średniej WN8 i obrażeń na tym samym tierze.",
    );
    expect(rows.eyes).toBe("Oczka: 4, jedno za każdą wygraną.");
    expect(rows.tiers).toBe("Gęstsze warstwy to wyższy tier (średnio 9,0).");
    expect(rows.waves).toBe("Wyższe fale to więcej obrażeń.");
    // The highest r (1.3) is battle 3, on its map.
    expect(rows.spark).toBe("Iskra pod płytką: najlepsza bitwa (nr 3, Synthetic Hills).");
  });

  it("says the same in English", () => {
    const rows = guide(synthetic(), "en");
    expect(rows.order).toBe("Battles run from left to right: 6 battles.");
    expect(rows.tint).toBe(
      "Warmest at battles 2–3, coolest at battle 5. " +
        "Warm and cool: against your average WN8 and damage on the same tier.",
    );
    expect(rows.eyes).toBe("Eyes: 4, one for each win.");
    expect(rows.tiers).toBe("Denser layers mean a higher tier (on average 9.0).");
    expect(rows.waves).toBe("Higher waves mean more damage.");
    expect(rows.spark).toBe("The spark under the plate: the best battle (no. 3, Synthetic Hills).");
  });

  it("follows the data: another warmest and coolest battle, another best battle", () => {
    const shape = SHAPE.map((battle) => [...battle] as (typeof SHAPE)[number]);
    // Battle 1 is the warmest on its own (battle 2 back to usual), 5–6 cool side by side.
    shape[0][0] = 1.4;
    shape[1][0] = 1.0;
    shape[5][0] = 0.7;
    shape[4][0] = 0.9;
    const stats = damastStats(synthetic(shape));
    expect(stats.best).toBe(0);
    const rows = guide(synthetic(shape));
    expect(rows.tint).toMatch(/^Najcieplej przy bitwie 1, najchłodniej przy bitwach 5–6\. /u);
    expect(rows.spark).toBe("Iskra pod płytką: najlepsza bitwa (nr 1).");
  });

  it("says in one sentence when every battle is as usual", () => {
    const rows = guide(synthetic(SHAPE.map(([, tier, win]) => [1.02, tier, win])));
    expect(rows.tint).toBe(
      "Wszystkie bitwy jak zwykle, więc nalot bez barwy. " +
        "Ciepło i chłód: wobec Twojej średniej WN8 i obrażeń na tym samym tierze.",
    );
  });

  it("names a one-sided session: warmer only", () => {
    const rows = guide(synthetic(SHAPE.map(([r, tier, win]) => [Math.max(1, r), tier, win])));
    expect(rows.tint).toMatch(
      /^Najcieplej przy bitwach 2–3; żadna bitwa nie jest chłodniejsza niż zwykle\. /u,
    );
  });

  it("says plainly that WN8 is missing, without inventing it", () => {
    const rows = guide(synthetic(SHAPE, { wn8: false }));
    expect(rows.tint).toBe(
      "Najcieplej przy bitwach 2–3, najchłodniej przy bitwie 5. " +
        "Ciepło i chłód: tylko obrażenia wobec Twojej średniej na tym samym tierze, bez WN8.",
    );
    expect(Object.values(rows).join(" ")).not.toMatch(/\bWN8 [\d]/u);
    // The rest of the plate reads the same.
    expect(rows.eyes).toBe("Oczka: 4, jedno za każdą wygraną.");
    expect(rows.spark).toBe("Iskra pod płytką: najlepsza bitwa (nr 3, Synthetic Hills).");
  });

  it("says plainly that there is no average to compare with", () => {
    const rows = guide(synthetic(SHAPE, { reference: false }));
    expect(rows.tint).toBe("Nalot bez barwy: brak Twojej średniej do porównania.");
    expect(rows.waves).toBe("Fale bez różnic: brak Twojej średniej obrażeń do porównania.");
    expect(rows.spark).toBe("Iskry brak: brak Twojej średniej do porównania.");
    // What does not need an average stays.
    expect(rows.eyes).toBe("Oczka: 4, jedno za każdą wygraną.");
    expect(rows.tiers).toBe("Gęstsze warstwy to wyższy tier (średnio 9,0).");
  });

  it("has no spark for a single battle and an unknown tier without a number", () => {
    const one = synthetic([[1.2, 8, true]]);
    one.battles[0].tier = null;
    const rows = guide(one);
    expect(rows.order).toBe("Bitwy idą od lewej do prawej: 1 bitwa.");
    expect(rows.tiers).toBe("Gęstsze warstwy to wyższy tier; tier tej sesji nieznany.");
    expect(rows.spark).toBe(`Iskry brak: najlepszą bitwę wskazujemy od ${MIN_BEST_BATTLES} bitew.`);
  });
});
