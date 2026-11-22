/**
 * A-1 on the Tank screen (spec 32 §3): the tank's tier × class cell against the players in the
 * player's own battles. Percentiles only (p25–p75, median, p90) and the player's median as a
 * marker; per row (makieta RC-Czolg) the name with "Ty X · gracze Y", the bar, and the position
 * of the player's median as a percentile; no single score and no judging words (D20).
 * Static: the bars and the marker do not move. Hovering or focusing a number highlights its mark,
 * like the references of the summaries (spec 30 §8).
 */
import { useState } from "react";
import { api } from "../api";
import { useDataVersion } from "../app/dataVersion";
import { useAsync } from "../hooks/useAsync";
import type { UiKey } from "../i18n/ui";
import { useLanguage } from "../i18n/language";
import { formatNumber, formatTier } from "../lib/format";
import type { Language, ReferenceMetric, ReferenceMetricKind, VehicleClass } from "../types";
import { Card } from "./ui";
import { HelpLink } from "./HelpLink";

const MIN_TIER = 6;

const CLASS_KEYS: Partial<Record<VehicleClass, UiKey>> = {
  light_tank: "refClassLight",
  medium_tank: "refClassMedium",
  heavy_tank: "refClassHeavy",
  tank_destroyer: "refClassTd",
  spg: "refClassSpg",
};

const METRIC_KEYS: Record<ReferenceMetricKind, UiKey> = {
  damage: "refMetricDamage",
  assist_radio_track: "refMetricAssist",
  spotted: "refMetricSpotted",
  kills: "refMetricKills",
  blocked: "refMetricBlocked",
  life_time: "refMetricLifeTime",
  accuracy: "refMetricAccuracy",
};

type Mark = "range" | "median" | "p90" | "own";

/** Whole numbers as on the other screens ("3 184"); small medians such as 1.5 keep a decimal. */
function number(value: number, language: Language): string {
  if (Math.abs(value) >= 10 || Number.isInteger(value)) return formatNumber(value, language);
  return new Intl.NumberFormat(language === "pl" ? "pl-PL" : "en-GB", {
    maximumFractionDigits: 1,
  }).format(value);
}

function MetricRow({ metric }: { metric: ReferenceMetric }) {
  const { language, tr } = useLanguage();
  const [active, setActive] = useState<Mark | null>(null);
  const label = tr(METRIC_KEYS[metric.metric]);
  const background = metric.metric === "accuracy";
  const own = background ? null : metric.ownMedian;
  const position = own !== null ? metric.ownPosition : null;
  const top = Math.max(metric.p90, own ?? 0) * 1.1 || 1;
  const at = (value: number) => `${Math.min(100, Math.max(0, (value / top) * 100))}%`;
  const format = (value: number) => number(value, language);
  const mark = (name: Mark) => (active === name ? " is-active" : "");
  const figure = (name: Mark, text: string) => (
    <span
      className={`ref-num${mark(name)}`}
      tabIndex={0}
      onMouseEnter={() => setActive(name)}
      onMouseLeave={() => setActive(null)}
      onFocus={() => setActive(name)}
      onBlur={() => setActive(null)}
    >
      {text}
    </span>
  );
  // RC-Czolg: the quartiles and p90 are read from the bar's label, not from a line under it.
  const barLabel =
    tr("refBarLabel", {
      label,
      from: format(metric.p25),
      to: format(metric.p75),
      median: format(metric.median),
      p90: format(metric.p90),
    }) + (own !== null ? tr("refBarLabelOwn", { value: format(own) }) : "");
  return (
    <div className="ref-row" role="group" aria-label={label}>
      <div className="ref-text">
        <span className="ref-label">{label}</span>
        <span className="ref-values">
          {background ? (
            tr("refBackgroundOnly")
          ) : own !== null ? (
            <>
              {figure("own", tr("refRowOwn", { value: format(own) }))}
              {" · "}
              {figure("median", tr("refRowPlayers", { value: format(metric.median) }))}
            </>
          ) : (
            figure("median", tr("refRowPlayers", { value: format(metric.median) }))
          )}
        </span>
      </div>
      <div className="ref-track" role="img" aria-label={barLabel}>
        <span
          className={`ref-range${mark("range")}`}
          style={{ left: at(metric.p25), width: `calc(${at(metric.p75)} - ${at(metric.p25)})` }}
        />
        <span className={`ref-median${mark("median")}`} style={{ left: at(metric.median) }} />
        <span className={`ref-p90${mark("p90")}`} style={{ left: at(metric.p90) }} />
        {own !== null && <span className={`ref-own${mark("own")}`} style={{ left: at(own) }} />}
      </div>
      {position !== null ? (
        <span className="ref-percentile">
          <span className="ref-percentile-value">{Math.round(position)}.</span>{" "}
          <span className="ref-percentile-unit">{tr("refPercentile")}</span>
        </span>
      ) : (
        <span className="ref-side muted">
          {background ? tr("refRowPlayers", { value: format(metric.median) }) : null}
        </span>
      )}
    </div>
  );
}

export function TankReference({
  tier,
  vehicleClass,
}: {
  tier: number | null;
  vehicleClass: VehicleClass;
}) {
  const { language, tr } = useLanguage();
  const version = useDataVersion();
  const tables = useAsync(() => api.referenceTables(), [], { keepData: true, refreshKey: version });
  const classKey = CLASS_KEYS[vehicleClass];
  const data = tables.state.status === "ready" ? tables.state.data : null;
  const cell =
    data?.cells.find((item) => item.tier === tier && item.vehicleClass === vehicleClass) ?? null;
  let body;
  if (tier === null || formatTier(tier) === null || !classKey)
    body = <p className="muted">{tr("refNoCell")}</p>;
  else if (tier < MIN_TIER) body = <p className="muted">{tr("refLowTier")}</p>;
  else if (!data) body = null;
  else if (!cell || cell.level === "count_only")
    body = <p className="muted">{tr("refTooFew", { count: cell?.population ?? 0 })}</p>;
  else {
    const count = number(cell.population, language);
    const ownShown = cell.metrics.some((metric) => metric.ownMedian !== null);
    body = (
      <>
        <p className="muted">
          {tr("refLead", { class: tr(classKey), tier: formatTier(tier) ?? tier, count })}
        </p>
        {cell.level === "approximate" && <p className="muted">{tr("refApproximate", { count })}</p>}
        {!ownShown && <p className="muted">{tr("refOwnTooFew", { count: cell.ownBattles })}</p>}
        {/* Spec 33 §4: the legend stays as signs (bar, line, dot); the words are for readers
            and in "Jak to działa", not on the screen. */}
        <ul className="ref-legend" aria-label={tr("refLegend")}>
          <li title={tr("refLegendRange")}>
            <span className="ref-legend-range" aria-hidden="true" />
            <span className="visually-hidden">{tr("refLegendRange")}</span>
          </li>
          <li title={tr("refLegendMedian")}>
            <span className="ref-legend-median" aria-hidden="true" />
            <span className="visually-hidden">{tr("refLegendMedian")}</span>
          </li>
          {ownShown && (
            <li title={tr("refLegendOwn")}>
              <span className="ref-legend-own" aria-hidden="true" />
              <span aria-hidden="true">{tr("refLegendYou")}</span>
              <span className="visually-hidden">{tr("refLegendOwn")}</span>
            </li>
          )}
          <li title={tr("refLegendP90")}>
            <span className="ref-legend-p90" aria-hidden="true" />
            <span className="visually-hidden">{tr("refLegendP90")}</span>
          </li>
        </ul>
        <div className="ref-rows">
          {cell.metrics.map((metric) => (
            <MetricRow key={metric.metric} metric={metric} />
          ))}
        </div>
      </>
    );
  }
  return (
    <Card labelledBy="reference-h" className="section">
      <div className="title-with-help">
        <h2 id="reference-h" className="section-title">
          {tr("refTitle")}
        </h2>
        <HelpLink entry="players" />
      </div>
      {body}
    </Card>
  );
}
