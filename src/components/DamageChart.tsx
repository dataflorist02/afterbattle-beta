import { useLanguage } from "../i18n/language";
import { formatNumber } from "../lib/format";

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

/**
 * Hand-written SVG line chart (D9) with a value axis and, when known, the player's average on
 * this tank as a dashed line (mockup `Czolgi.dc.html`).
 */
export function DamageChart({
  values,
  average = null,
}: {
  values: number[];
  average?: number | null;
}) {
  const { language, tr } = useLanguage();
  const points = limitPoints(values);
  if (points.length < 2) return <p className="muted">{tr("chartTooFew")}</p>;
  const all = average === null ? points : [...points, average];
  const ticks = axisTicks(Math.min(...all), Math.max(...all));
  const low = ticks[0];
  const high = ticks[ticks.length - 1];
  const y = (value: number) =>
    TOP + (1 - (value - low) / (high - low || 1)) * (HEIGHT - TOP - BOTTOM);
  const x = (index: number) => LEFT + (index / (points.length - 1)) * (WIDTH - LEFT - 4);
  const path = points
    .map(
      (value, index) => `${index === 0 ? "M" : "L"}${x(index).toFixed(1)},${y(value).toFixed(1)}`,
    )
    .join(" ");
  const last = points[points.length - 1];
  const averageText =
    average === null ? null : tr("chartAverage", { value: formatNumber(average, language) });
  return (
    <svg
      className="chart"
      role="img"
      aria-label={
        averageText
          ? `${tr("chartLabel", { count: points.length })}. ${averageText}`
          : tr("chartLabel", { count: points.length })
      }
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      data-points={points.length}
    >
      {ticks.map((tick) => (
        <g key={tick} className="chart-tick">
          <line x1={LEFT} x2={WIDTH} y1={y(tick)} y2={y(tick)} />
          <text x={0} y={y(tick) + 4}>
            {formatNumber(tick, language)}
          </text>
        </g>
      ))}
      {average !== null && averageText && (
        <g className="chart-average">
          <line x1={LEFT} x2={WIDTH} y1={y(average)} y2={y(average)} />
          <text x={LEFT + 8} y={y(average) > TOP + 20 ? y(average) - 8 : y(average) + 18}>
            {averageText}
          </text>
        </g>
      )}
      <path className="chart-line" d={path} />
      <circle className="chart-dot" cx={x(points.length - 1)} cy={y(last)} r={5} />
    </svg>
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
