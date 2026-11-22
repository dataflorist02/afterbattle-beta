import { useId, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { useLanguage } from "../i18n/language";
import { formatNumber } from "../lib/format";
import { prefersReducedMotion } from "../lib/motion";
import type { Language } from "../types";

export const MAX_CHART_POINTS = 200;

/** Keeps at most 200 points, evenly spread, always with the first and the last one. */
export function limitPoints<T>(points: T[], max = MAX_CHART_POINTS): T[] {
  if (points.length <= max) return points;
  const step = (points.length - 1) / (max - 1);
  return Array.from({ length: max }, (_, index) => points[Math.round(index * step)]);
}

/** Round axis values (1, 2 or 5 × 10^n apart) covering `min..max`, lowest first. */
export function axisTicks(min: number, max: number, count = 4): number[] {
  const span = max - min || Math.abs(max) || 1;
  const raw = span / Math.max(1, count - 1);
  const power = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 5, 10].map((factor) => factor * power).find((value) => value >= raw) ?? raw;
  const first = Math.floor(min / step) * step;
  const ticks: number[] = [];
  for (let value = first; value < max + step; value += step) {
    ticks.push(Math.round(value));
    if (value >= max) break;
  }
  return ticks.length >= 2 ? ticks : [first, first + step];
}

const WIDTH = 640;
const HEIGHT = 200;
const LEFT = 52;
const TOP = 12;
const BOTTOM = 12;
/** Room between the reading's tip and its point (the point's radius is 5). */
const TIP_GAP = 12;

/** The battle under one point of the line, for the reading (from the trend's data). */
export interface ChartBattle {
  /** The battle's number in the chart's window. */
  number: number;
  time: number | null;
  damage: number | null;
  map: string | null;
  tank: string | null;
}

/** The x of point `index` of `count`, in the chart's units. */
export function chartX(index: number, count: number): number {
  return LEFT + (index / Math.max(1, count - 1)) * (WIDTH - LEFT - 4);
}

/** "20.11, 21:14": the battle's day and time in the reading. */
export function readingDate(time: number, language: Language): string {
  return new Intl.DateTimeFormat(language === "pl" ? "pl-PL" : "en-GB", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(time));
}

interface Size {
  width: number;
  height: number;
}
export interface TipPlace {
  left: number;
  top: number;
  side: "right" | "left" | "above" | "below";
}

/**
 * Where the reading's tip goes (pixels in the plot): beside its point — right of it, or left
 * near the right edge — centred on it and kept inside vertically; above or below the point when
 * neither side has room. Never outside the chart, never over the point.
 */
export function placeTip(plot: Size, tip: Size, point: { x: number; y: number }): TipPlace {
  const clamp = (value: number, max: number) => Math.max(0, Math.min(value, max));
  const top = clamp(point.y - tip.height / 2, plot.height - tip.height);
  if (point.x + TIP_GAP + tip.width <= plot.width)
    return { left: point.x + TIP_GAP, top, side: "right" };
  if (point.x - TIP_GAP - tip.width >= 0)
    return { left: point.x - TIP_GAP - tip.width, top, side: "left" };
  const left = clamp(point.x - tip.width / 2, plot.width - tip.width);
  if (point.y - TIP_GAP - tip.height >= 0)
    return { left, top: point.y - TIP_GAP - tip.height, side: "above" };
  return { left, top: clamp(point.y + TIP_GAP, plot.height - tip.height), side: "below" };
}

/**
 * Hand-written SVG line chart (D9) with a value axis and, when known, the player's average on
 * this tank as a dashed line (mockup `Czolgi.dc.html`). Author, 22.11: the average is named in
 * a legend above the chart, not on it; the chart can be read — the pointer or the arrow keys
 * (Home, End, Esc) show the battle under a point in a tip that stays inside the chart, and a
 * polite live region says it.
 */
export function DamageChart({
  values,
  average = null,
  battles,
}: {
  values: number[];
  average?: number | null;
  /** The battles under the values, in the same order (the reading's data). */
  battles?: readonly ChartBattle[];
}) {
  const { language, tr } = useLanguage();
  const gradientId = `heat-${useId().replace(/[^\w-]/g, "")}`;
  const plot = useRef<HTMLDivElement>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState<number | null>(null);
  const [place, setPlace] = useState<TipPlace | null>(null);
  // Points and their battles thinned together (at most 200).
  const shown = limitPoints(values.map((value, index) => ({ value, battle: battles?.[index] })));
  const points = shown.map((point) => point.value);
  const count = points.length;
  const current = active !== null && active < count ? active : null;
  const all = average === null ? points : [...points, average];
  const ticks = count >= 2 ? axisTicks(Math.min(...all), Math.max(...all)) : [0, 1];
  const low = ticks[0];
  const high = ticks[ticks.length - 1];
  const y = (value: number) =>
    TOP + (1 - (value - low) / (high - low || 1)) * (HEIGHT - TOP - BOTTOM);
  const x = (index: number) => chartX(index, count);
  const currentValue = current === null ? null : points[current];

  // The tip is placed once its size is known: beside the point, inside the plot.
  useLayoutEffect(() => {
    if (current === null || currentValue === null) {
      setPlace(null);
      return;
    }
    const box = plot.current;
    const tip = tipRef.current;
    if (!box || !tip) return;
    const width = box.clientWidth || WIDTH;
    const height = box.clientHeight || HEIGHT;
    setPlace(
      placeTip(
        { width, height },
        { width: tip.offsetWidth, height: tip.offsetHeight },
        {
          x: (chartX(current, count) / WIDTH) * width,
          y: (y(currentValue) / HEIGHT) * height,
        },
      ),
    );
    // y() follows the axis of the same values; language changes the tip's size.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current, currentValue, count, low, high, language]);

  if (count < 2) return <p className="muted">{tr("chartTooFew")}</p>;
  const path = points
    .map(
      (value, index) => `${index === 0 ? "M" : "L"}${x(index).toFixed(1)},${y(value).toFixed(1)}`,
    )
    .join(" ");
  const last = points[count - 1];
  const averageText =
    average === null ? null : tr("chartAverage", { value: formatNumber(average, language) });
  // Spec 33 §3: the line warms above the player's own average and stays steel below it; the
  // switch sits at the height of the average line (a still gradient, nothing moves).
  const split =
    average === null
      ? null
      : `${(Math.min(1, Math.max(0, (y(average) - TOP) / (HEIGHT - TOP - BOTTOM))) * 100).toFixed(1)}%`;

  const reading = current === null ? null : readingOf(current);
  function readingOf(index: number) {
    const battle = shown[index].battle;
    const head = [
      tr("chartReadingBattle", { number: battle?.number ?? index + 1 }),
      ...(battle?.time ? [readingDate(battle.time, language)] : []),
    ].join(" · ");
    const lines = [
      ...(battle && battle.damage !== null
        ? [tr("chartReadingDamage", { value: formatNumber(battle.damage, language) })]
        : []),
      tr("chartReadingLine", { value: formatNumber(points[index], language) }),
      ...(battle?.map ? [battle.map] : []),
      ...(battle?.tank ? [battle.tank] : []),
    ];
    return { head, lines, said: [head, ...lines].join(", ") };
  }

  const indexAt = (event: MouseEvent<HTMLDivElement>) => {
    const box = event.currentTarget.getBoundingClientRect();
    if (box.width <= 0) return null;
    const at = ((event.clientX - box.left) / box.width) * WIDTH;
    const index = Math.round(((at - LEFT) / (WIDTH - LEFT - 4)) * (count - 1));
    return Math.max(0, Math.min(count - 1, index));
  };
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const moves: Record<string, () => number | null> = {
      // The first arrow starts at the newest battle, the highlighted end of the line.
      ArrowLeft: () => (current === null ? count - 1 : Math.max(0, current - 1)),
      ArrowRight: () => (current === null ? count - 1 : Math.min(count - 1, current + 1)),
      Home: () => 0,
      End: () => count - 1,
      Escape: () => null,
    };
    const move = moves[event.key];
    if (!move) return;
    event.preventDefault();
    setActive(move());
  };

  return (
    <div className="chart-figure">
      {averageText && (
        <p className="chart-legend">
          <svg className="chart-legend-sample" viewBox="0 0 28 8" aria-hidden="true">
            <line className="chart-legend-dash" x1={0} x2={28} y1={4} y2={4} />
          </svg>
          <span>{averageText}</span>
        </p>
      )}
      <div
        ref={plot}
        className="chart-plot"
        tabIndex={0}
        onMouseMove={(event) => setActive(indexAt(event))}
        onMouseLeave={() => setActive(null)}
        onBlur={() => setActive(null)}
        onKeyDown={onKeyDown}
      >
        <svg
          className="chart"
          role="img"
          aria-label={
            averageText
              ? `${tr("chartLabel", { count })}. ${averageText}`
              : tr("chartLabel", { count })
          }
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          data-points={count}
        >
          {ticks.map((tick) => (
            <g key={tick} className="chart-tick">
              <line x1={LEFT} x2={WIDTH} y1={y(tick)} y2={y(tick)} />
              <text x={0} y={y(tick) + 4}>
                {formatNumber(tick, language)}
              </text>
            </g>
          ))}
          {average !== null && (
            <g className="chart-average">
              <line x1={LEFT} x2={WIDTH} y1={y(average)} y2={y(average)} />
            </g>
          )}
          {split && (
            <defs>
              <linearGradient
                id={gradientId}
                className="chart-heat"
                gradientUnits="userSpaceOnUse"
                x1="0"
                y1={TOP}
                x2="0"
                y2={HEIGHT - BOTTOM}
              >
                <stop className="chart-heat-warm" offset="0%" />
                <stop className="chart-heat-warm" offset={split} />
                <stop className="chart-heat-steel" offset={split} />
                <stop className="chart-heat-steel" offset="100%" />
              </linearGradient>
            </defs>
          )}
          <path
            className="chart-line"
            d={path}
            style={split ? { stroke: `url(#${gradientId})` } : undefined}
          />
          <circle className="chart-dot" cx={x(count - 1)} cy={y(last)} r={5} />
          {current !== null && (
            <g aria-hidden="true">
              <line
                className="chart-cursor"
                x1={x(current).toFixed(1)}
                x2={x(current).toFixed(1)}
                y1={TOP}
                y2={HEIGHT - BOTTOM}
              />
              <circle
                className="chart-point"
                cx={x(current).toFixed(1)}
                cy={y(points[current]).toFixed(1)}
                r={5}
              />
            </g>
          )}
        </svg>
        {reading && (
          <div
            ref={tipRef}
            className={prefersReducedMotion() ? "chart-tip" : "chart-tip chart-tip-in"}
            data-side={place?.side}
            style={
              place
                ? { left: `${place.left}px`, top: `${place.top}px` }
                : { left: 0, top: 0, visibility: "hidden" }
            }
            aria-hidden="true"
          >
            <span className="chart-tip-head">{reading.head}</span>
            {reading.lines.map((line) => (
              <span key={line}>{line}</span>
            ))}
          </div>
        )}
      </div>
      <p className="visually-hidden" aria-live="polite">
        {reading?.said ?? ""}
      </p>
    </div>
  );
}

/** Rolling average of `window` battles over damage values, oldest first. */
export function rollingAverage(values: number[], window = 10): number[] {
  const result: number[] = [];
  let sum = 0;
  for (let index = 0; index < values.length; index++) {
    sum += values[index];
    if (index >= window) sum -= values[index - window];
    result.push(sum / Math.min(index + 1, window));
  }
  return result;
}
