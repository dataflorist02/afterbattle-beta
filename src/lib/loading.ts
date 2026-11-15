/**
 * Loading screen rules (spec 29 §5): only when the start really takes time — the first snapshot
 * is late (cold start) or the first import fills an empty database. Progress comes only from real
 * data (the import's done/total); without it the ember stays at the base and pulses. A fast start
 * shows the app at once (D19).
 */

/** The first snapshot may take this long before the loading screen appears. */
export const LOADING_DELAY_MS = 400;
/** The ember at the base: unknown progress, and the start of known progress. */
export const EMBER_BASE = 0.08;
/** A change of progress moves the ember this long. */
export const PROGRESS_MS = 300;
/** At 100% the full ember stays this long … */
export const HOLD_MS = 150;
/** … and the screen fades out this long. */
export const FADE_MS = 200;

export function loadingNeeded({
  starting,
  late,
  emptyDatabase,
  importing,
}: {
  /** The first snapshot is not there yet. */
  starting: boolean;
  /** {@link LOADING_DELAY_MS} passed since the window started. */
  late: boolean;
  emptyDatabase: boolean;
  importing: boolean;
}): boolean {
  if (starting) return late;
  return emptyDatabase && importing;
}

/**
 * A running or recent session means stored battles. Without one the database may still hold
 * battles (the full import gives a session only to battles newer than the inactivity limit), so
 * the history is asked for one row; `recentBattles` of the snapshot is only the running session's.
 */
export function hasSessions(
  snapshot: { activeSession: unknown; recentSessions: readonly unknown[] } | null,
): boolean {
  return (
    snapshot !== null && (snapshot.activeSession !== null || snapshot.recentSessions.length > 0)
  );
}

/** The share of replays read, 0–1; `null` when the import gives no total. */
export function importFraction(progress: { done: number; total: number } | null): number | null {
  if (!progress || !(progress.total > 0)) return null;
  return Math.min(1, Math.max(0, progress.done / progress.total));
}

/** Height of the ember (scaleY of the rising mask) for a progress, from the base glow to 1. */
export function emberScale(fraction: number | null): number {
  if (fraction === null || Number.isNaN(fraction)) return EMBER_BASE;
  return EMBER_BASE + (1 - EMBER_BASE) * Math.min(1, Math.max(0, fraction));
}

/**
 * `hidden` before it was ever needed, `shown`, `finishing` (the ember fills and holds), `fading`,
 * `done` (gone for the rest of this window's life).
 */
export type LoadingPhase = "hidden" | "shown" | "finishing" | "fading" | "done";

export function nextLoadingPhase(
  phase: LoadingPhase,
  needed: boolean,
  reducedMotion: boolean,
): LoadingPhase {
  if (phase === "done" || phase === "fading") return phase;
  if (needed) return "shown";
  if (phase === "hidden") return "hidden";
  return reducedMotion ? "done" : "finishing";
}
