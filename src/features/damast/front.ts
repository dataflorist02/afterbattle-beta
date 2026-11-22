/**
 * Fair Play for Damast (spec 34 §3, spec 33 §1): plates are forged and drawn only with the app's
 * window in front (visible and focused), never while the game has the focus.
 */

/** The window is in front: visible and focused (the game is not). */
export function windowInFront(): boolean {
  if (typeof document === "undefined") return true;
  if (document.visibilityState === "hidden") return false;
  return typeof document.hasFocus !== "function" || document.hasFocus();
}

/** Calls `start` once the window is in front; returns the clean-up. */
export function whenInFront(start: () => void): () => void {
  if (windowInFront()) {
    start();
    return () => {};
  }
  const check = () => {
    if (!windowInFront()) return;
    stop();
    start();
  };
  const stop = () => {
    window.removeEventListener("focus", check);
    document.removeEventListener("visibilitychange", check);
  };
  window.addEventListener("focus", check);
  document.addEventListener("visibilitychange", check);
  return stop;
}

/** Resolves once the window is in front. */
export function inFront(): Promise<void> {
  return new Promise((resolve) => {
    whenInFront(resolve);
  });
}
