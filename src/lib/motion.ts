/**
 * Motion rules (E5-flare §7a): motion explains what changed. The main number counts up only when
 * a new battle arrived in the session that is still running; a plain visit shows it still.
 */
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";

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
/** A changed number keeps its direction color this long (spec 29 §6) … */
export const DELTA_SHOW_MS = 1_000;
/** … then the color fades out this long (opacity only). */
export const DELTA_FADE_MS = 400;

export type Direction = "up" | "down";

/** Which way a number moved; `null` without a change or without both values. */
export function changeDirection(from: number | null, to: number | null): Direction | null {
  if (from === null || to === null || Number.isNaN(from) || Number.isNaN(to) || from === to)
    return null;
  return to > from ? "up" : "down";
}

/** Characters to reserve while a number counts, so its width never jumps (with tabular digits). */
export function reservedChars(texts: readonly string[]): number {
  return texts.reduce((longest, text) => Math.max(longest, [...text].length), 0);
}

/**
 * Battles that arrived while the screen is open (spec 29 §6): none on entering the screen, the
 * new ones of the same running session after that. Kept until the battles change again.
 */
export function useArrivals(
  sessionId: string,
  /** The battles shown, or `null` while they load (ignored: the last list stays the baseline). */
  ids: readonly string[] | null,
  active: boolean,
): string[] {
  const signature = ids === null ? null : `${sessionId}|${ids.join(",")}`;
  const [state, setState] = useState<{
    signature: string | null;
    seen: SessionSeen | null;
    arrivals: string[];
  }>({ signature: null, seen: null, arrivals: [] });
  if (ids === null) return [];
  if (state.signature === signature) return state.arrivals;
  const seen = { sessionId, battleIds: [...ids] };
  const arrivals = newBattleIds(state.seen, seen, active);
  setState({ signature, seen, arrivals });
  return arrivals;
}

/**
 * A number on screen (spec 29 §6): when it changes with `animate`, it counts from the old to the
 * new value over {@link COUNT_MS} and carries its direction for {@link DELTA_SHOW_MS} plus the
 * fade; otherwise it shows the new value at once. `key` grows with every animated change.
 */
export function useCountedNumber(
  value: number | null,
  animate: boolean,
): {
  shown: number | null;
  /** The value the count started from (the same as the shown value without a count). */
  from: number | null;
  direction: Direction | null;
  key: number;
} {
  const [state, setState] = useState({
    target: value,
    from: value,
    shown: value,
    direction: null as Direction | null,
    key: 0,
  });
  let current = state;
  if (!Object.is(state.target, value)) {
    const direction = animate ? changeDirection(state.target, value) : null;
    current = {
      target: value,
      from: state.target,
      shown: direction ? state.target : value,
      direction,
      key: direction ? state.key + 1 : state.key,
    };
    setState(current);
  }
  const { key, from, target, direction } = current;
  useEffect(() => {
    if (!direction || from === null || target === null) return;
    let frame = 0;
    let start: number | null = null;
    const step = (now: number) => {
      start ??= now;
      const shown = countValue(from, target, now - start, COUNT_MS);
      setState((current) => ({ ...current, shown }));
      if (shown !== target) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    // A newer change cancels both (cleanup), so they always belong to the shown change.
    const timer = setTimeout(
      () => setState((current) => ({ ...current, direction: null })),
      DELTA_SHOW_MS + DELTA_FADE_MS,
    );
    return () => {
      cancelAnimationFrame(frame);
      clearTimeout(timer);
    };
  }, [key, from, target, direction]);
  return current;
}

/** How far an element moved, as the offset that puts it back; `null` under half a pixel. */
export function flipOffset(
  before: { left: number; top: number } | undefined,
  after: { left: number; top: number },
): { x: number; y: number } | null {
  if (!before) return null;
  const x = before.left - after.left;
  const y = before.top - after.top;
  if (Math.abs(x) < 0.5 && Math.abs(y) < 0.5) return null;
  return { x, y };
}

function cssTime(name: string, fallback: number): number {
  const value = parseFloat(getComputedStyle(document.documentElement).getPropertyValue(name));
  return Number.isFinite(value) ? value : fallback;
}

/**
 * FLIP (spec 29 §6): when `trigger` changes with `enabled`, the elements marked `data-flip`
 * inside `container` that moved on screen glide from where they were to where they are
 * (`transform` only). Positions are kept current on scroll and resize, so only a move caused by
 * the change animates (a row added above the view, kept still by scroll anchoring, moves nothing).
 */
export function useFlip(
  container: RefObject<HTMLElement | null>,
  trigger: unknown,
  enabled: boolean,
): void {
  const positions = useRef(new Map<string, { left: number; top: number }>());
  const measure = useCallback(() => {
    const map = new Map<string, { left: number; top: number }>();
    for (const element of container.current?.querySelectorAll<HTMLElement>("[data-flip]") ?? []) {
      const rect = element.getBoundingClientRect();
      map.set(element.dataset.flip ?? "", { left: rect.left, top: rect.top });
    }
    return map;
  }, [container]);
  useEffect(() => {
    let frame = 0;
    const update = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => (positions.current = measure()));
    };
    window.addEventListener("scroll", update, { capture: true, passive: true });
    window.addEventListener("resize", update);
    return () => {
      window.removeEventListener("scroll", update, { capture: true });
      window.removeEventListener("resize", update);
      cancelAnimationFrame(frame);
    };
  }, [measure]);
  useLayoutEffect(() => {
    const now = measure();
    if (enabled)
      for (const element of container.current?.querySelectorAll<HTMLElement>("[data-flip]") ?? []) {
        const key = element.dataset.flip ?? "";
        const after = now.get(key)!;
        // Rows below the window before and after the change are not seen moving.
        if (
          after.top >= window.innerHeight &&
          (positions.current.get(key)?.top ?? 0) >= window.innerHeight
        )
          continue;
        const offset = flipOffset(positions.current.get(key), after);
        if (offset && typeof element.animate === "function")
          element.animate(
            [{ transform: `translate(${offset.x}px, ${offset.y}px)` }, { transform: "none" }],
            {
              duration: cssTime("--motion-enter", 250),
              easing:
                getComputedStyle(document.documentElement).getPropertyValue("--ease-out").trim() ||
                "ease-out",
            },
          );
      }
    positions.current = now;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trigger]);
}

/**
 * Entrance wave (E5-flare-fix2, E8-ui-fix, spec 29 §1): the rows visible at the first display
 * enter with a soft cascade, once per visit, after the data is there; rows below, rows loaded
 * later and rows after a filter change appear without motion. The wave covers the rows whose top
 * is inside the window, at most 20, measured after the fonts and the first layout.
 */
export const WAVE_MAX_ROWS = 20;
/** Rows in the wave when nothing is laid out yet (about the rows of 2560×1440). */
export const WAVE_DEFAULT_ROWS = 14;
export const ROW_ENTRANCE_STEP_MS = 36;
/** The longest wait for the fonts before measuring: a slow font never holds the rows back. */
export const FONT_WAIT_MS = 300;

export function rowEntranceDelay(index: number, count: number): number | null {
  if (index < 0 || index >= count) return null;
  return index * ROW_ENTRANCE_STEP_MS;
}

export function entranceIds(ids: readonly string[], count: number): Set<string> {
  return new Set(ids.slice(0, Math.max(0, count)));
}

/**
 * Rows in the wave from the rows' own boxes: the laid-out rows whose top is above the bottom of
 * the window, at most `limit`. Positions, not a step between two rows, so a day header between
 * rows never shortens the wave (spec 29 §1). Nothing laid out yet (no rows, a zero window, every
 * row 0 px high): the default of 14 rows.
 */
export function visibleWave(
  rows: readonly { top: number; height: number }[],
  viewportHeight: number,
  limit = WAVE_MAX_ROWS,
): number {
  const laidOut = rows.filter((row) => row.height > 0);
  if (!(viewportHeight > 0) || laidOut.length === 0) return Math.min(limit, WAVE_DEFAULT_ROWS);
  return Math.min(limit, laidOut.filter((row) => row.top < viewportHeight).length);
}

/** The wave for rows already in the page, measured against the window. */
export function measuredWave(rows: readonly Element[]): number {
  return visibleWave(
    rows.map((row) => row.getBoundingClientRect()),
    window.innerHeight,
  );
}

/**
 * Runs `measure` once the fonts are ready (at most `fontWaitMs`) and the next frame is laid out,
 * so a measurement never sees fallback fonts or a layout before the first paint. Returns a cancel
 * function.
 */
export function afterFontsAndLayout(measure: () => void, fontWaitMs = FONT_WAIT_MS): () => void {
  let cancelled = false;
  let frame = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const fonts = (document as { fonts?: { ready?: Promise<unknown> } }).fonts?.ready;
  const limit = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, fontWaitMs);
  });
  void Promise.race([fonts ?? Promise.resolve(), limit]).then(() => {
    clearTimeout(timer);
    if (cancelled) return;
    frame = requestAnimationFrame(measure);
  });
  return () => {
    cancelled = true;
    clearTimeout(timer);
    cancelAnimationFrame(frame);
  };
}
