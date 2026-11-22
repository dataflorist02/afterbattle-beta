import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import type { DamastSession } from "../../types";
import {
  MIN_BEST_BATTLES,
  MIN_TIER_BATTLES,
  damageReference,
  damastBestBattleId,
  damastStats,
} from "./stats";
import { fromSynthetic, type SyntheticSession } from "./synthetic";

const SESSIONS = JSON.parse(
  readFileSync("docs/design/damast/sesje-syntetyczne.json", "utf8"),
) as Record<"hot" | "flat" | "cool", SyntheticSession>;

/** The mockup's reference algorithm (`wzor-referencyjny.js`), loaded as it is. */
function reference() {
  const source = readFileSync("docs/design/damast/wzor-referencyjny.js", "utf8");
  return new Function(`${source}; return { damast, damastStats };`)() as {
    damastStats: (session: SyntheticSession) => {
      r: number[];
      T: number;
      best: number;
      wins: number;
      n: number;
      tier: number;
    };
  };
}

function session(
  battles: { damage: number | null; wn8: number | null; tier: number | null; win?: boolean }[],
  {
    wn8 = 2_000 as number | null,
    damage = 2_000 as number | null,
    tiers = [
      { tier: 8, battles: 30, averageDamage: 1_800 },
      { tier: 10, battles: 30, averageDamage: 3_000 },
    ],
  } = {},
): DamastSession {
  const plate = fromSynthetic("synthetic-a", { avgWN8: 1, avgDmg: { 8: 1 }, b: [] });
  return {
    ...plate,
    battles: battles.map((battle, index) => ({
      battleId: `b${index}`,
      battleTime: index,
      tier: battle.tier,
      damage: battle.damage,
      wn8: battle.wn8,
      win: battle.win ?? false,
    })),
    reference: {
      ...plate.reference,
      wn8,
      averageDamage: damage,
      tiers,
    },
  };
}

describe("Damast stats (spec 34 §2)", () => {
  it.each(["hot", "flat", "cool"] as const)(
    "matches the mockup's reference on the synthetic %s session",
    (name) => {
      const expected = reference().damastStats(SESSIONS[name]);
      const stats = damastStats(fromSynthetic(name, SESSIONS[name]));
      expect(stats.plate.map((battle) => battle.r)).toEqual(
        expected.r.map((value) => expect.closeTo(value, 12)),
      );
      expect(Math.round(stats.change ?? NaN)).toBe(expected.T);
      expect(stats.best).toBe(expected.best);
      expect(stats.wins).toBe(expected.wins);
      expect(stats.count).toBe(expected.n);
      expect(stats.averageTier).toBeCloseTo(expected.tier, 12);
    },
  );

  it("gives the three synthetic sessions their temperatures", () => {
    const of = (name: "hot" | "flat" | "cool") =>
      damastStats(fromSynthetic(name, SESSIONS[name])).heat;
    expect(of("hot")).toMatchObject({ temperature: "warmer" });
    expect(of("hot")?.change).toBeCloseTo(22.64, 2);
    expect(of("flat")).toMatchObject({ temperature: "usual", intensity: 0 });
    expect(of("cool")).toMatchObject({ temperature: "cooler", intensity: 1 });
  });

  it("weighs WN8 0.6 and damage on the tier 0.4", () => {
    // WN8 2 400 against 2 000 (1.2), damage 1 440 against 1 800 on tier VIII (0.8).
    const stats = damastStats(session([{ damage: 1_440, wn8: 2_400, tier: 8 }]));
    expect(stats.plate[0].r).toBeCloseTo(0.6 * 1.2 + 0.4 * 0.8, 12);
    expect(stats.plate[0].damageRatio).toBeCloseTo(0.8, 12);
  });

  it("uses the same boundary as Żar: exactly ±5% is already warmer or cooler", () => {
    const at = (ratio: number) =>
      damastStats(
        session([{ damage: 3_000 * ratio, wn8: 2_000 * ratio, tier: 10 }], {
          wn8: 2_000,
        }),
      ).heat;
    expect(at(1.05)).toMatchObject({ temperature: "warmer", intensity: 0 });
    expect(at(0.95)).toMatchObject({ temperature: "cooler", intensity: 0 });
    expect(at(1.0499)).toMatchObject({ temperature: "usual" });
    expect(at(0.9501)).toMatchObject({ temperature: "usual" });
    expect(at(1.3)).toMatchObject({ temperature: "warmer", intensity: 1 });
    expect(at(1.6)).toMatchObject({ temperature: "warmer", intensity: 1 });
  });

  it("takes the best battle as the highest r, the first of equal ones", () => {
    const stats = damastStats(
      session([
        { damage: 1_800, wn8: 2_000, tier: 8 },
        { damage: 3_600, wn8: 2_000, tier: 8 },
        { damage: 6_000, wn8: 2_000, tier: 10 },
        { damage: 3_600, wn8: 2_000, tier: 8 },
      ]),
    );
    // 3 600 on VIII (2.0) and 6 000 on X (2.0) are equal; the earlier one wins.
    expect(stats.best).toBe(1);
    expect(
      damastBestBattleId(
        session([
          { damage: 1_800, wn8: 2_000, tier: 8 },
          { damage: 3_600, wn8: 2_000, tier: 8 },
        ]),
      ),
    ).toBe("b1");
    expect(damastStats(session([])).best).toBe(-1);
  });

  it(`names a best battle from ${MIN_BEST_BATTLES} battles and only against an own average`, () => {
    const one = session([{ damage: 3_600, wn8: 2_000, tier: 8 }]);
    expect(damastStats(one).best).toBe(-1);
    expect(damastBestBattleId(one)).toBeNull();
    const withoutReference = session(
      [
        { damage: 1_800, wn8: 2_000, tier: 8 },
        { damage: 3_600, wn8: 2_000, tier: 8 },
      ],
      { wn8: null, damage: null, tiers: [] },
    );
    expect(damastBestBattleId(withoutReference)).toBeNull();
    expect(damastBestBattleId(null)).toBeNull();
  });

  it("never raises a battle for its tier alone", () => {
    // The same result against the own average of each tier gives the same r.
    const low = damastStats(session([{ damage: 2_160, wn8: 2_200, tier: 8 }]));
    const high = damastStats(session([{ damage: 3_600, wn8: 2_200, tier: 10 }]));
    expect(high.plate[0].r).toBeCloseTo(low.plate[0].r, 12);
    // The same damage on a higher tier is a weaker result, never a better one.
    const same = damastStats(
      session([
        { damage: 2_400, wn8: 2_000, tier: 8 },
        { damage: 2_400, wn8: 2_000, tier: 10 },
      ]),
    );
    expect(same.plate[1].r).toBeLessThan(same.plate[0].r);
  });

  it(`falls back to the all-tier average below ${MIN_TIER_BATTLES} battles on the tier`, () => {
    const reference = {
      ...session([]).reference,
      averageDamage: 2_000,
      tiers: [
        { tier: 8, battles: MIN_TIER_BATTLES - 1, averageDamage: 1_000 },
        { tier: 9, battles: MIN_TIER_BATTLES, averageDamage: 2_500 },
      ],
    };
    expect(damageReference(8, reference)).toBe(2_000);
    expect(damageReference(9, reference)).toBe(2_500);
    expect(damageReference(10, reference)).toBe(2_000);
    expect(damageReference(null, reference)).toBe(2_000);
    const stats = damastStats({
      ...session([{ damage: 3_000, wn8: 2_000, tier: 8 }]),
      reference,
    });
    expect(stats.plate[0].damageRatio).toBeCloseTo(1.5, 12);
  });

  it("uses what a battle has: damage alone without WN8, WN8 alone without damage", () => {
    const stats = damastStats(
      session([
        { damage: 2_700, wn8: null, tier: 8 },
        { damage: null, wn8: 1_600, tier: 8 },
        { damage: null, wn8: null, tier: 8 },
      ]),
    );
    expect(stats.count).toBe(2);
    expect(stats.plate.map((battle) => battle.battleId)).toEqual(["b0", "b1"]);
    expect(stats.plate[0].r).toBeCloseTo(1.5, 12);
    expect(stats.plate[1].r).toBeCloseTo(0.8, 12);
    expect(stats.plate[1].damageRatio).toBe(1);
  });

  it("forges a steel plate without any reference: r = 1 and no temperature", () => {
    const stats = damastStats(
      session(
        [
          { damage: 2_000, wn8: 1_500, tier: 8, win: true },
          { damage: 4_000, wn8: 3_000, tier: 10 },
        ],
        { wn8: null, damage: null, tiers: [] },
      ),
    );
    expect(stats.plate.map((battle) => battle.r)).toEqual([1, 1]);
    expect(stats.heat).toBeNull();
    expect(stats.change).toBeNull();
    expect(stats.wins).toBe(1);
  });

  it("counts the average tier from the battles with a tier", () => {
    const stats = damastStats(
      session([
        { damage: 2_000, wn8: 2_000, tier: 8 },
        { damage: 2_000, wn8: 2_000, tier: null },
        { damage: 2_000, wn8: 2_000, tier: 10 },
      ]),
    );
    expect(stats.averageTier).toBe(9);
    expect(stats.plate[1].tier).toBe(9);
    expect(damastStats(session([{ damage: 1, wn8: 1, tier: null }])).averageTier).toBeNull();
  });
});
