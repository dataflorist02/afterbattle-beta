import { afterEach, describe, expect, it, vi } from "vitest";
import {
  afterFontsAndLayout,
  countValue,
  entranceIds,
  newBattleIds,
  ROW_ENTRANCE_STEP_MS,
  rowEntranceDelay,
  shouldCountUp,
  visibleWave,
  WAVE_DEFAULT_ROWS,
  WAVE_MAX_ROWS,
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

describe("entrance wave (E8-ui-fix, spec 29 §1)", () => {
  const rows = (...tops: number[]) => tops.map((top) => ({ top, height: 56 }));

  it("covers the rows whose top is inside the window, up to 20", () => {
    expect(WAVE_MAX_ROWS).toBe(20);
    expect(visibleWave(rows(200, 260, 320, 380, 440), 400)).toBe(4);
    // A row starting exactly at the bottom edge is not visible.
    expect(visibleWave(rows(200, 300, 400), 400)).toBe(2);
    const many = Array.from({ length: 40 }, (_, index) => 100 + index * 40);
    expect(visibleWave(rows(...many), 2_000)).toBe(20);
    expect(visibleWave(rows(...many), 2_000, 5)).toBe(5);
  });

  it("is not shortened by a day header between the first rows", () => {
    // "Today" with one battle, then the header of "Yesterday" (40 px) before the second row.
    const tops = [200, ...Array.from({ length: 30 }, (_, index) => 300 + index * 60)];
    // 1 row today + rows at 300, 360, …, 780 (9) are above 800.
    expect(visibleWave(rows(...tops), 800)).toBe(10);
  });

  it("falls back to 14 rows when nothing is measured yet", () => {
    expect(WAVE_DEFAULT_ROWS).toBe(14);
    expect(visibleWave([], 800)).toBe(14);
    expect(visibleWave(rows(200, 260), 0)).toBe(14);
    expect(visibleWave(rows(200, 260), Number.NaN)).toBe(14);
    expect(
      visibleWave(
        [
          { top: 0, height: 0 },
          { top: 0, height: 0 },
        ],
        800,
      ),
    ).toBe(14);
    expect(visibleWave([], 800, 5)).toBe(5);
  });

  it("counts only laid-out rows once anything is laid out", () => {
    expect(
      visibleWave(
        [
          { top: 200, height: 56 },
          { top: 0, height: 0 },
          { top: 260, height: 56 },
        ],
        800,
      ),
    ).toBe(2);
    // Rows all below the window: nothing to move.
    expect(visibleWave(rows(900, 960), 800)).toBe(0);
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

describe("measuring after fonts and the first layout (spec 29 §1)", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("runs after the fonts are ready and one animation frame", async () => {
    vi.useFakeTimers();
    let fontsReady: () => void = () => undefined;
    const ready = new Promise<void>((resolve) => (fontsReady = resolve));
    vi.stubGlobal("document", { ...document, fonts: { ready } });
    const run = vi.fn();
    afterFontsAndLayout(run);
    await vi.advanceTimersByTimeAsync(100);
    expect(run).not.toHaveBeenCalled();
    fontsReady();
    await Promise.resolve();
    await Promise.resolve();
    expect(run).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(20);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("does not wait for fonts longer than the limit", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("document", { ...document, fonts: { ready: new Promise(() => {}) } });
    const run = vi.fn();
    afterFontsAndLayout(run, 300);
    await vi.advanceTimersByTimeAsync(299);
    expect(run).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(40);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("leaves no timer or frame behind", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("document", { ...document, fonts: { ready: Promise.resolve() } });
    const run = vi.fn();
    afterFontsAndLayout(run);
    await Promise.resolve();
    await Promise.resolve();
    // Fonts ready: the font limit timer is gone, only the frame waits.
    expect(vi.getTimerCount()).toBe(1);
    await vi.advanceTimersByTimeAsync(20);
    expect(run).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
    const cancel = afterFontsAndLayout(vi.fn());
    cancel();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("never measures when cancelled after the fonts, before the frame", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("document", { ...document, fonts: { ready: Promise.resolve() } });
    const run = vi.fn();
    const cancel = afterFontsAndLayout(run);
    await Promise.resolve();
    await Promise.resolve();
    cancel();
    await vi.advanceTimersByTimeAsync(400);
    expect(run).not.toHaveBeenCalled();
  });

  it("runs after one frame without a font API and never after cancel", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("document", { ...document, fonts: undefined });
    const run = vi.fn();
    afterFontsAndLayout(run);
    await vi.advanceTimersByTimeAsync(20);
    expect(run).toHaveBeenCalledTimes(1);
    const cancelled = vi.fn();
    const cancel = afterFontsAndLayout(cancelled);
    cancel();
    await vi.advanceTimersByTimeAsync(400);
    expect(cancelled).not.toHaveBeenCalled();
    const late = vi.fn();
    const cancelLate = afterFontsAndLayout(late);
    await Promise.resolve();
    await Promise.resolve();
    cancelLate();
    await vi.advanceTimersByTimeAsync(400);
    expect(late).not.toHaveBeenCalled();
  });
});
