import { describe, expect, it } from "vitest";
import type { SessionBaselineComparison } from "../types";
import { sessionDamageReference, sessionInsightLines } from "./sessionReference";

type Vehicle = {
  id: number;
  name: string;
  sessionDamage: number;
  sessionBattles: number;
  baseline: number | null;
  baselineBattles: number;
};

/** Synthetic comparison shaped like `get_session_baseline_comparison` (only the read fields). */
function comparison(
  vehicles: Vehicle[],
  insights: { state: string; signals?: { vehicleId: number; deltaValue: number }[] },
): SessionBaselineComparison {
  return {
    vehicles: vehicles.map((vehicle) => ({
      vehicleId: vehicle.id,
      tankName: vehicle.name,
      sessionEligibleBattles: vehicle.sessionBattles,
      sessionMetricBattles: { wn8: 0, damage: vehicle.sessionBattles, survival: 0, assist: 0 },
      baseline: {
        averageDamage: {
          value: vehicle.baseline,
          eligibleBattles: vehicle.baselineBattles,
          candidateBattles: vehicle.baselineBattles,
        },
      },
      damage: { currentValue: vehicle.sessionDamage, baselineValue: vehicle.baseline },
    })),
    insights: {
      version: "session-deviation-v1",
      state: insights.state,
      unavailableReason: null,
      summaries: (insights.signals ?? []).map((signal) => ({
        vehicleId: signal.vehicleId,
        tankName: vehicles.find((vehicle) => vehicle.id === signal.vehicleId)?.name ?? null,
        strongestPositive: null,
        strongestNegative: {
          metric: "damage",
          direction: "negative",
          deltaValue: signal.deltaValue,
        },
        supportingSignals: [],
      })),
    },
  } as unknown as SessionBaselineComparison;
}

// The author's case (2026-11-13), synthetic: 8 battles spread over four tanks, two on each, so no
// tank reaches the 3 session battles of a per-tank insight; 1 528 damage against 3 244.
const spread: Vehicle[] = [101, 102, 103, 104].map((id, index) => ({
  id,
  name: `Synth ${index + 1}`,
  sessionDamage: 1528,
  sessionBattles: 2,
  baseline: 3244,
  baselineBattles: 100,
}));

describe("session damage reference (hero card and insights share it)", () => {
  it("weights each tank's session and baseline damage by its session battles", () => {
    const reference = sessionDamageReference(
      comparison(
        [
          { ...spread[0], sessionDamage: 2000, baseline: 2500, sessionBattles: 3 },
          { ...spread[1], sessionDamage: 1000, baseline: 1500, sessionBattles: 1 },
        ],
        { state: "available" },
      ),
    );
    expect(reference.current).toBe(1750);
    expect(reference.baseline).toBe(2250);
    expect(reference.change).toBe(-22);
    expect(reference.sessionBattles).toBe(4);
    expect(reference.vehicles).toBe(2);
    expect(reference.state).toBe("ready");
  });

  it("leaves out tanks with fewer than 30 baseline battles and says why when none is left", () => {
    const reference = sessionDamageReference(
      comparison([{ ...spread[0], baselineBattles: 12 }], { state: "insufficient_baseline" }),
    );
    expect(reference.baseline).toBeNull();
    expect(reference.change).toBeNull();
    expect(reference.state).toBe("baseline_too_small");
    expect(reference.baselineBattles).toBe(12);
  });

  it("needs 3 session battles on the compared tanks", () => {
    const reference = sessionDamageReference(
      comparison([{ ...spread[0], sessionBattles: 2 }], { state: "insufficient_session" }),
    );
    expect(reference.state).toBe("session_too_small");
    expect(reference.change).toBeNull();
  });

  it("has no reference without a comparison", () => {
    expect(sessionDamageReference(null).state).toBe("no_data");
  });
});

describe("session insights (regression: 'close to your average' at -53%)", () => {
  it("names the 53% drop instead of calling the session close to the average", () => {
    const data = comparison(spread, { state: "insufficient_session" });
    const result = sessionInsightLines(data);
    expect(sessionDamageReference(data).change).toBe(-53);
    expect(result.lines.map((line) => line.kind)).toEqual(["session_damage"]);
    expect(result.lines[0]).toMatchObject({ direction: "negative", percent: 53 });
    expect(result.fallback).toBeNull();
  });

  it("says plainly that confidence is too low when nothing can be compared", () => {
    const fewSession = comparison(
      spread.map((vehicle) => ({ ...vehicle, sessionBattles: 0 })),
      { state: "insufficient_session" },
    );
    expect(sessionInsightLines(fewSession)).toEqual({ lines: [], fallback: "session_too_small" });
    const fewBaseline = comparison(
      spread.map((vehicle) => ({ ...vehicle, baselineBattles: 10 })),
      { state: "insufficient_baseline" },
    );
    expect(sessionInsightLines(fewBaseline)).toEqual({ lines: [], fallback: "baseline_too_small" });
    expect(sessionInsightLines(null)).toEqual({ lines: [], fallback: "no_data" });
  });

  it("calls a session close to the average only when everything was compared and is close", () => {
    const close = comparison(
      spread.map((vehicle) => ({ ...vehicle, sessionBattles: 3, sessionDamage: 3100 })),
      { state: "available" },
    );
    expect(sessionInsightLines(close)).toEqual({ lines: [], fallback: "close_to_average" });
  });

  it("says the damage is close but per-tank comparison needs more battles", () => {
    const closeButSpread = comparison(
      spread.map((vehicle) => ({ ...vehicle, sessionDamage: 3100 })),
      { state: "insufficient_session" },
    );
    expect(sessionInsightLines(closeButSpread)).toEqual({
      lines: [],
      fallback: "damage_close_tanks_too_few",
    });
  });

  it("does not repeat the per-tank damage signal of a one-tank session", () => {
    const one = comparison([{ ...spread[0], sessionBattles: 4 }], {
      state: "available",
      signals: [{ vehicleId: 101, deltaValue: -53 }],
    });
    expect(sessionInsightLines(one).lines.map((line) => line.kind)).toEqual(["tank"]);
  });

  it("puts the session line before the per-tank signals of a multi-tank session", () => {
    const mixed = comparison(
      [
        { ...spread[0], sessionBattles: 4 },
        { ...spread[1], sessionBattles: 2 },
      ],
      { state: "available", signals: [{ vehicleId: 101, deltaValue: -53 }] },
    );
    expect(sessionInsightLines(mixed).lines.map((line) => line.kind)).toEqual([
      "session_damage",
      "tank",
    ]);
  });
});
