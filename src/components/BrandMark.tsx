/**
 * The Afterbattle mark (spec 29 §4, docs/design/brand): the letter A as an outline with a cross
 * bar shaped like a rise; steel outline, only the bar heated. One component for the navigation
 * rail, the title bar and the loading screen, so the shapes have one source (the coordinates of
 * the brand files, viewBox 64). Colors come from the tokens; the brand files' gradient stops map
 * to the nearest steel and navy tokens.
 */
import { useId } from "react";
import type { CSSProperties } from "react";

/** Outline of the A (viewBox 64). */
export const MARK_OUTLINE = "32,9 53,55 43.5,55 32,29 20.5,55 11,55";
/** The bar shaped like a rise. */
export const MARK_BAR = "24,47 32,40.5 40,47";
/** Below this size the icon drops its gradients and inner edge (spec 29 §4). */
export const ICON_SIMPLE_BELOW = 32;

export type BrandVariant = "icon" | "mark" | "mark-small";

const ROUND = { strokeLinecap: "round", strokeLinejoin: "round" } as const;

/** Ids for gradients and filters: unique per mark, letters and digits only. */
export function useBrandIds(...names: string[]): Record<string, string> {
  const base = `brand${useId().replace(/[^a-z0-9]/gi, "")}`;
  return Object.fromEntries(names.map((name) => [name, `${base}${name}`]));
}

function stop(offset: string, token: string) {
  return <stop key={offset} offset={offset} style={{ stopColor: `var(--${token})` }} />;
}

/** The ember gradient of the heated bar and the loading screen (bottom white-hot, top deep). */
export function EmberGradient({ id }: { id: string }) {
  return (
    <linearGradient id={id} gradientUnits="userSpaceOnUse" x1="0" y1="56" x2="0" y2="8">
      {stop("0", "ember-white")}
      {stop("0.3", "accent-soft")}
      {stop("0.65", "ember-lo")}
      {stop("1", "ember-deep")}
    </linearGradient>
  );
}

/** The soft glow around heated lines (blur about 1.1 units). */
export function EmberGlow({ id }: { id: string }) {
  return (
    <filter id={id} x="-30%" y="-30%" width="160%" height="160%">
      <feGaussianBlur in="SourceGraphic" stdDeviation="1.1" result="b" />
      <feMerge>
        <feMergeNode in="b" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
  );
}

function Icon({ simple }: { simple: boolean }) {
  const ids = useBrandIds("navy", "silver");
  const lines = (outline: CSSProperties) => (
    <g transform="translate(5.12 5.4) scale(0.84)">
      <polygon points={MARK_OUTLINE} fill="none" strokeWidth="4.4" style={outline} />
      <polyline
        points={MARK_BAR}
        fill="none"
        strokeWidth="4"
        {...ROUND}
        style={{ stroke: "var(--accent)" }}
      />
    </g>
  );
  if (simple)
    return (
      <>
        <rect width="64" height="64" rx="15" style={{ fill: "var(--navy)" }} />
        {lines({ stroke: "var(--text-soft)" })}
      </>
    );
  return (
    <>
      <defs>
        <radialGradient id={ids.navy} cx="0.5" cy="0.42" r="0.75">
          {stop("0", "navy-hi")}
          {stop("0.6", "navy")}
          {stop("1", "bg-deep")}
        </radialGradient>
        <linearGradient
          id={ids.silver}
          gradientUnits="userSpaceOnUse"
          x1="10"
          y1="8"
          x2="54"
          y2="56"
        >
          {stop("0", "text")}
          {stop("0.35", "text-2")}
          {stop("0.5", "text")}
          {stop("0.75", "text-3")}
          {stop("1", "text-soft")}
        </linearGradient>
      </defs>
      <rect width="64" height="64" rx="15" style={{ fill: `url(#${ids.navy})` }} />
      <rect
        x="0.75"
        y="0.75"
        width="62.5"
        height="62.5"
        rx="14.3"
        fill="none"
        strokeWidth="1.5"
        style={{ stroke: "var(--text)", strokeOpacity: 0.1 }}
      />
      {lines({ stroke: `url(#${ids.silver})` })}
    </>
  );
}

function Mark() {
  const ids = useBrandIds("hot", "glow");
  return (
    <>
      <defs>
        <EmberGradient id={ids.hot} />
        <EmberGlow id={ids.glow} />
      </defs>
      <polygon
        points={MARK_OUTLINE}
        fill="none"
        strokeWidth="1.1"
        style={{ stroke: "var(--text-soft)" }}
      />
      <polyline
        points={MARK_BAR}
        fill="none"
        strokeWidth="1.6"
        {...ROUND}
        filter={`url(#${ids.glow})`}
        style={{ stroke: `url(#${ids.hot})` }}
      />
    </>
  );
}

function MarkSmall() {
  return (
    <>
      <polygon
        points={MARK_OUTLINE}
        fill="none"
        strokeWidth="3.4"
        style={{ stroke: "var(--text-soft)" }}
      />
      <polyline
        points={MARK_BAR}
        fill="none"
        strokeWidth="3.6"
        {...ROUND}
        style={{ stroke: "var(--accent)" }}
      />
    </>
  );
}

export function BrandMark({
  variant = "mark",
  size,
  className,
}: {
  variant?: BrandVariant;
  /** Width and height in CSS pixels. */
  size: number;
  className?: string;
}) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      className={className}
      width={size}
      height={size}
      viewBox="0 0 64 64"
    >
      {variant === "icon" ? (
        <Icon simple={size < ICON_SIMPLE_BELOW} />
      ) : variant === "mark" ? (
        <Mark />
      ) : (
        <MarkSmall />
      )}
    </svg>
  );
}
