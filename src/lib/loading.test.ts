import { describe, expect, it } from "vitest";
import {
  EMBER_BASE,
  FADE_MS,
  HOLD_MS,
  LOADING_DELAY_MS,
  PROGRESS_MS,
  emberScale,
  importFraction,
  hasSessions,
  loadingNeeded,
  nextLoadingPhase,
} from "./loading";

describe("when the loading screen shows (spec 29 §5)", () => {
  it("waits 400 ms for the first snapshot before showing anything", () => {
    expect(LOADING_DELAY_MS).toBe(400);
    const starting = { starting: true, emptyDatabase: false, importing: false };
    expect(loadingNeeded({ ...starting, late: false })).toBe(false);
    expect(loadingNeeded({ ...starting, late: true })).toBe(true);
  });

  it("shows during the first import into an empty database only", () => {
    const ready = { starting: false, late: true };
    expect(loadingNeeded({ ...ready, emptyDatabase: true, importing: true })).toBe(true);
    // Battles to show: the import goes on in the background with the import bar.
    expect(loadingNeeded({ ...ready, emptyDatabase: false, importing: true })).toBe(false);
    // An empty database without an import (no game folder yet, no replays): the app itself.
    expect(loadingNeeded({ ...ready, emptyDatabase: true, importing: false })).toBe(false);
    expect(loadingNeeded({ ...ready, emptyDatabase: false, importing: false })).toBe(false);
    expect(
      loadingNeeded({ starting: false, late: false, emptyDatabase: true, importing: true }),
    ).toBe(true);
  });
});

describe("stored battles", () => {
  it("are certain with a running or recent session, else the history must be asked", () => {
    expect(hasSessions({ activeSession: null, recentSessions: [] })).toBe(false);
    expect(hasSessions({ activeSession: { id: "s" }, recentSessions: [] })).toBe(true);
    expect(hasSessions({ activeSession: null, recentSessions: [{ id: "s" }] })).toBe(true);
    expect(hasSessions(null)).toBe(false);
  });
});

describe("progress of the loading screen", () => {
  it("is the share of replays read, or unknown", () => {
    expect(importFraction({ done: 25, total: 100 })).toBe(0.25);
    expect(importFraction({ done: 100, total: 100 })).toBe(1);
    expect(importFraction({ done: 120, total: 100 })).toBe(1);
    expect(importFraction({ done: -5, total: 100 })).toBe(0);
    expect(importFraction({ done: 0, total: 0 })).toBeNull();
    expect(importFraction({ done: 3, total: -1 })).toBeNull();
    expect(importFraction(null)).toBeNull();
  });

  it("maps progress to the height of the ember, from the base glow to the full mark", () => {
    expect(EMBER_BASE).toBe(0.08);
    expect(emberScale(0)).toBe(EMBER_BASE);
    expect(emberScale(1)).toBe(1);
    expect(emberScale(0.5)).toBeCloseTo(0.54, 10);
    expect(emberScale(-1)).toBe(EMBER_BASE);
    expect(emberScale(7)).toBe(1);
    // Unknown progress: only the glow at the base (it pulses), never invented progress.
    expect(emberScale(null)).toBe(EMBER_BASE);
    expect(emberScale(Number.NaN)).toBe(EMBER_BASE);
  });
});

describe("leaving the loading screen", () => {
  it("fills the ember, holds, then fades; never shows again in this window", () => {
    expect([PROGRESS_MS, HOLD_MS, FADE_MS]).toEqual([300, 150, 200]);
    expect(nextLoadingPhase("hidden", true, false)).toBe("shown");
    expect(nextLoadingPhase("shown", true, false)).toBe("shown");
    expect(nextLoadingPhase("shown", false, false)).toBe("finishing");
    expect(nextLoadingPhase("finishing", false, false)).toBe("finishing");
    expect(nextLoadingPhase("finishing", true, false)).toBe("shown");
    expect(nextLoadingPhase("fading", true, false)).toBe("fading");
    expect(nextLoadingPhase("hidden", false, false)).toBe("hidden");
    expect(nextLoadingPhase("done", true, false)).toBe("done");
  });

  it("leaves at once with reduced motion", () => {
    expect(nextLoadingPhase("shown", false, true)).toBe("done");
    expect(nextLoadingPhase("hidden", true, true)).toBe("shown");
  });
});
