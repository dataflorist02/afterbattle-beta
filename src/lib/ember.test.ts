import { describe, expect, it } from "vitest";
import {
  FULL_HEAT_PERCENT,
  SPARKS_MAX,
  SPARKS_MIN,
  USUAL_PERCENT,
  heatAgainst,
  heatOf,
  heatProps,
  sparkCount,
} from "./ember";

describe("temperature against the player's own average (spec 33 §2)", () => {
  it("keeps the thresholds of the spec: ±5% as usual, full heat at ±30%, sparks 3 to 18", () => {
    expect(USUAL_PERCENT).toBe(5);
    expect(FULL_HEAT_PERCENT).toBe(30);
    expect(SPARKS_MIN).toBe(3);
    expect(SPARKS_MAX).toBe(18);
  });

  it("is as usual strictly inside ±5%", () => {
    for (const change of [0, 4.99, -4.99, 2, -3])
      expect(heatOf(change), String(change)).toEqual({
        temperature: "usual",
        intensity: 0,
        change,
      });
  });

  it("is warmer from +5% and cooler from −5%, at the lowest intensity on the boundary", () => {
    expect(heatOf(5)).toEqual({ temperature: "warmer", intensity: 0, change: 5 });
    expect(heatOf(-5)).toEqual({ temperature: "cooler", intensity: 0, change: -5 });
  });

  it("grows linearly up to ±30% and stays there beyond it", () => {
    expect(heatOf(17.5)?.intensity).toBeCloseTo(0.5);
    expect(heatOf(-17.5)?.intensity).toBeCloseTo(0.5);
    expect(heatOf(30)?.intensity).toBe(1);
    expect(heatOf(-30)?.intensity).toBe(1);
    expect(heatOf(29.99)?.intensity).toBeLessThan(1);
    expect(heatOf(80)).toEqual({ temperature: "warmer", intensity: 1, change: 80 });
    expect(heatOf(-95)).toEqual({ temperature: "cooler", intensity: 1, change: -95 });
  });

  it("has no temperature without a comparison", () => {
    for (const change of [null, undefined, Number.NaN, Number.POSITIVE_INFINITY])
      expect(heatOf(change)).toBeNull();
  });

  it("compares a value with the player's own average, never with a missing or zero one", () => {
    expect(heatAgainst(1_050, 1_000)?.temperature).toBe("warmer");
    expect(heatAgainst(950, 1_000)?.temperature).toBe("cooler");
    expect(heatAgainst(1_049, 1_000)?.temperature).toBe("usual");
    expect(heatAgainst(951, 1_000)?.temperature).toBe("usual");
    expect(heatAgainst(1_300, 1_000)?.intensity).toBe(1);
    expect(heatAgainst(1_000, 0)).toBeNull();
    expect(heatAgainst(1_000, null)).toBeNull();
    expect(heatAgainst(null, 1_000)).toBeNull();
  });

  it("throws sparks on the session card only when warmer: 3 at +5% up to 18 at +30%", () => {
    expect(sparkCount(heatOf(5))).toBe(3);
    expect(sparkCount(heatOf(17.5))).toBe(11);
    expect(sparkCount(heatOf(30))).toBe(18);
    expect(sparkCount(heatOf(300))).toBe(18);
    expect(sparkCount(heatOf(4.99))).toBe(0);
    expect(sparkCount(heatOf(-30))).toBe(0);
    expect(sparkCount(null)).toBe(0);
    for (let change = 5; change <= 30; change += 0.5) {
      const count = sparkCount(heatOf(change));
      expect(count).toBeGreaterThanOrEqual(SPARKS_MIN);
      expect(count).toBeLessThanOrEqual(SPARKS_MAX);
    }
  });

  it("hands the temperature to CSS as an attribute and the intensity as a custom property", () => {
    expect(heatProps(heatOf(17.5))).toEqual({
      "data-temp": "warmer",
      style: { "--heat": "0.5" },
    });
    expect(heatProps(heatOf(0))).toEqual({ "data-temp": "usual", style: { "--heat": "0" } });
    expect(heatProps(null)).toEqual({});
  });
});
