import { describe, expect, it } from "vitest";
import {
  countValue,
  entranceIds,
  newBattleIds,
  ROW_ENTRANCE_STEP_MS,
  rowEntranceDelay,
  shouldCountUp,
  WAVE_MAX_ROWS,
  waveRowCount,
} from "./motion";

const seen = (sessionId: string, battleIds: string[]) => ({ sessionId, battleIds });

describe("motion rules (E5-flare §7a)", () => {
  it("counts the main number up only when a new battle arrived in the running session", () => {
    const before = seen("s1", ["a", "b"]);
    const after = seen("s1", ["a", "b", "c"]);
    expect(shouldCountUp(before, after, true, false)).toBe(true);
    expect(newBattleIds(before, after, true)).toEqual(["c"]);
    expect(shouldCountUp(null, after, true, false)).toBe(false);
    expect(shouldCountUp(before, before, true, false)).toBe(false);
    expect(shouldCountUp(before, after, false, false)).toBe(false);
    expect(shouldCountUp(seen("s0", ["a"]), after, true, false)).toBe(false);
    expect(shouldCountUp(before, after, true, true)).toBe(false);
  });

  it("eases the count from the old to the new value and ends exactly on it", () => {
    expect(countValue(1_000, 2_000, 0, 600)).toBe(1_000);
    expect(countValue(1_000, 2_000, 300, 600)).toBeGreaterThan(1_500);
    expect(countValue(1_000, 2_000, 600, 600)).toBe(2_000);
    expect(countValue(1_000, 2_000, 9_999, 600)).toBe(2_000);
    expect(countValue(1_000, 2_000, 10, 0)).toBe(2_000);
  });
});

describe("entrance wave (E8-ui-fix)", () => {
  it("covers the rows visible at first display, up to 20", () => {
    expect(WAVE_MAX_ROWS).toBe(20);
    // 2560×1440: about 1 000 px of list under the header, 72 px rows → 14 rows.
    expect(waveRowCount(1_000, 72)).toBe(14);
    expect(waveRowCount(720, 72)).toBe(10);
    expect(waveRowCount(721, 72)).toBe(11);
    expect(waveRowCount(5_000, 60)).toBe(20);
    expect(waveRowCount(5_000, 60, 5)).toBe(5);
  });

  it("is empty without a measured area or row", () => {
    expect(waveRowCount(0, 72)).toBe(0);
    expect(waveRowCount(-10, 72)).toBe(0);
    expect(waveRowCount(1_000, 0)).toBe(0);
    expect(waveRowCount(1_000, -1)).toBe(0);
    expect(waveRowCount(Number.NaN, 72)).toBe(0);
    expect(waveRowCount(1_000, Number.POSITIVE_INFINITY)).toBe(0);
  });

  it("staggers the wave rows by the History step and nothing below", () => {
    const delays = Array.from({ length: 30 }, (_, index) => rowEntranceDelay(index, 14));
    expect(delays.slice(0, 14)).toEqual(
      Array.from({ length: 14 }, (_, index) => index * ROW_ENTRANCE_STEP_MS),
    );
    expect(delays.slice(14).every((delay) => delay === null)).toBe(true);
    expect(ROW_ENTRANCE_STEP_MS).toBe(36);
    expect(rowEntranceDelay(0, 0)).toBeNull();
    expect(rowEntranceDelay(-1, 14)).toBeNull();
  });

  it("picks the ids of the wave rows only", () => {
    const ids = Array.from({ length: 50 }, (_, index) => `b${index}`);
    expect([...entranceIds(ids, 14)]).toEqual(ids.slice(0, 14));
    expect(entranceIds(ids, 0).size).toBe(0);
    expect(entranceIds(ids.slice(0, 3), 14).size).toBe(3);
  });
});
