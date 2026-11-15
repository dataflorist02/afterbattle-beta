/**
 * Loading screen (spec 29 §5, docs/design/brand/afterbattle-loading-reference.svg): only the
 * outline of the mark in the middle of the deep background; the ember rises from the base with
 * the real progress (a mask scaled by `transform`). No text, no separate bar, no invented
 * progress: without progress the ember stays at the base and pulses. At the end the ember fills,
 * holds and the screen fades; with reduced motion it simply goes.
 */
import { useEffect, useState } from "react";
import { useLanguage } from "../i18n/language";
import {
  FADE_MS,
  HOLD_MS,
  PROGRESS_MS,
  emberScale,
  nextLoadingPhase,
  type LoadingPhase,
} from "../lib/loading";
import { prefersReducedMotion } from "../lib/motion";
import { EmberGlow, EmberGradient, MARK_BAR, MARK_OUTLINE, useBrandIds } from "./BrandMark";

function Shape({ width, stroke }: { width: string; stroke: string }) {
  return (
    <>
      <polygon points={MARK_OUTLINE} fill="none" strokeWidth={width} style={{ stroke }} />
      <polyline
        points={MARK_BAR}
        fill="none"
        strokeWidth={width}
        strokeLinecap="round"
        strokeLinejoin="round"
        style={{ stroke }}
      />
    </>
  );
}

export function LoadingScreen({
  needed,
  fraction,
}: {
  /** The start really takes time now (see `loadingNeeded`). */
  needed: boolean;
  /** Real progress 0–1, or `null` when unknown. */
  fraction: number | null;
}) {
  const { tr } = useLanguage();
  const ids = useBrandIds("hot", "glow", "rise");
  const reduced = prefersReducedMotion();
  const [phase, setPhase] = useState<LoadingPhase>("hidden");
  const current = nextLoadingPhase(phase, needed, reduced);
  if (current !== phase) setPhase(current);
  useEffect(() => {
    if (phase !== "finishing" && phase !== "fading") return;
    const timer = setTimeout(
      () => setPhase(phase === "finishing" ? "fading" : "done"),
      phase === "finishing" ? PROGRESS_MS + HOLD_MS : FADE_MS,
    );
    return () => clearTimeout(timer);
  }, [phase]);
  if (current === "hidden" || current === "done") return null;
  const ending = current !== "shown";
  const known = fraction !== null;
  const scale = ending ? 1 : emberScale(fraction);
  const value = ending ? 100 : known ? Math.round(fraction * 100) : undefined;
  const className = [
    "loading-screen",
    !known && !ending && !reduced ? "loading-pulse" : "",
    ending ? "loading-finishing" : "",
    current === "fading" ? "loading-out" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <div
      className={className}
      role="progressbar"
      aria-label={tr("loading")}
      aria-valuemin={value === undefined ? undefined : 0}
      aria-valuemax={value === undefined ? undefined : 100}
      aria-valuenow={value}
    >
      <svg className="loading-mark" viewBox="0 0 64 64" aria-hidden="true" focusable="false">
        <defs>
          <EmberGradient id={ids.hot} />
          <EmberGlow id={ids.glow} />
          {/* From the base (y 57) to the top (y 7), scaled from the bottom. */}
          <clipPath id={ids.rise}>
            <rect
              className="loading-rise"
              x="0"
              y="7"
              width="64"
              height="50"
              style={{ transform: `scaleY(${scale})` }}
            />
          </clipPath>
        </defs>
        <g className="loading-cold">
          <Shape width="0.9" stroke="var(--steel-line)" />
        </g>
        <g className="loading-hot" clipPath={`url(#${ids.rise})`} filter={`url(#${ids.glow})`}>
          <Shape width="1.1" stroke={`url(#${ids.hot})`} />
        </g>
      </svg>
    </div>
  );
}
