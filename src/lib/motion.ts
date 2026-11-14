/**
 * Motion rules (E5-flare §7a): motion explains what changed. The main number counts up only when
 * a new battle arrived in the session that is still running; a plain visit shows it still.
 */
import { useEffect, useRef, useState } from "react";

export interface SessionSeen {
  sessionId: string;
  battleIds: readonly string[];
}

export function prefersReducedMotion(): boolean {
  try {
    return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  } catch {
    return false;
  }
}

/** New battles of the same running session since the last time it was seen. */
export function newBattleIds(
  previous: SessionSeen | null,
  next: SessionSeen,
  active: boolean,
): string[] {
  if (!active || !previous || previous.sessionId !== next.sessionId) return [];
  const known = new Set(previous.battleIds);
  return next.battleIds.filter((id) => !known.has(id));
}

export function shouldCountUp(
  previous: SessionSeen | null,
  next: SessionSeen,
  active: boolean,
  reducedMotion: boolean,
): boolean {
  return !reducedMotion && newBattleIds(previous, next, active).length > 0;
}

/** Value at `elapsed` of a count from `from` to `to` over `duration` ms, eased out. */
export function countValue(from: number, to: number, elapsed: number, duration: number): number {
  if (duration <= 0 || elapsed >= duration) return to;
  const progress = Math.max(0, elapsed) / duration;
  const eased = 1 - (1 - progress) ** 3;
  return from + (to - from) * eased;
}

export const COUNT_MS = 600;

/** The number shown: counts from the previous value when `animate` turns true, else static. */
export function useCountUp(value: number | null, animate: boolean): number | null {
  const [shown, setShown] = useState(value);
  const previous = useRef(value);
  useEffect(() => {
    const from = previous.current;
    previous.current = value;
    if (!animate || value === null || from === null || from === value) {
      setShown(value);
      return;
    }
    let frame = 0;
    const start = performance.now();
    const step = (now: number) => {
      const next = countValue(from, value, now - start, COUNT_MS);
      setShown(next);
      if (next !== value) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [value, animate]);
  return shown;
}

/**
 * Entrance wave (E5-flare-fix2, E8-ui-fix): the rows visible at the first display enter with a
 * soft cascade, once per visit, after the data is there; rows below, rows loaded later and rows
 * after a filter change appear without motion. The wave covers
 * `ceil(list area height / row height)` rows, at most 20 (about 14 at 2560×1440).
 */
export const WAVE_MAX_ROWS = 20;
export const ROW_ENTRANCE_STEP_MS = 36;

/** Rows in the wave: the rows that fit the visible list area, at most `limit`; 0 unmeasured. */
export function waveRowCount(areaHeight: number, rowHeight: number, limit = WAVE_MAX_ROWS): number {
  if (!Number.isFinite(areaHeight) || !Number.isFinite(rowHeight)) return 0;
  if (areaHeight <= 0 || rowHeight <= 0) return 0;
  return Math.min(limit, Math.ceil(areaHeight / rowHeight));
}

export function rowEntranceDelay(index: number, count: number): number | null {
  if (index < 0 || index >= count) return null;
  return index * ROW_ENTRANCE_STEP_MS;
}

export function entranceIds(ids: readonly string[], count: number): Set<string> {
  return new Set(ids.slice(0, Math.max(0, count)));
}

/**
 * The wave size for rows already in the page: the visible area from the first row's top to the
 * bottom of the window, over the distance between the first two rows (or the first row's
 * height). Read before the first paint, so the rows never show still and then jump.
 */
export function measuredWave(rows: readonly Element[]): number {
  const first = rows[0]?.getBoundingClientRect();
  if (!first) return 0;
  const second = rows[1]?.getBoundingClientRect();
  const step = second && second.top > first.top ? second.top - first.top : first.height;
  return waveRowCount(window.innerHeight - first.top, step);
}
