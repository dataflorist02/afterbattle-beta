import { useEffect, useRef, useState } from "react";

import { api, AppCommandError } from "../../api";
import { translate, type TranslationKey } from "../../i18n/legacy";
import { battleCountLabel } from "../../lib/presentation";
import { Dialog, SegmentedControl } from "../ui";
import type {
  AttemptsConversionRate,
  AttemptsConversionRecentWindow,
  AttemptsConversionRejectionCode,
  AttemptsConversionRejectionReason,
  AttemptsConversionReport,
  Language,
} from "../../types";

type ScopeSelection = "session" | "10" | "30" | "100";
const recentWindows: readonly AttemptsConversionRecentWindow[] = [10, 30, 100];
const reasonKeys: Record<AttemptsConversionRejectionCode, TranslationKey> = {
  partial_replay: "attemptsReasonPartialReplay",
  results_provenance_missing: "attemptsReasonResultsProvenanceMissing",
  parser_too_old: "attemptsReasonParserTooOld",
  unsupported_schema: "attemptsReasonUnsupportedSchema",
  unsupported_mode: "attemptsReasonUnsupportedMode",
  invalid_team_structure: "attemptsReasonInvalidTeamStructure",
  vehicle_class_catalog_unavailable: "attemptsReasonVehicleClassCatalogUnavailable",
  vehicle_class_unknown: "attemptsReasonVehicleClassUnknown",
  vehicle_class_conflict: "attemptsReasonVehicleClassConflict",
  unsupported_vehicle_mechanics: "attemptsReasonUnsupportedVehicleMechanics",
  missing_counters: "attemptsReasonMissingCounters",
  counter_out_of_range: "attemptsReasonCounterOutOfRange",
  counter_conflict: "attemptsReasonCounterConflict",
};
const locale = (language: Language) => (language === "pl" ? "pl-PL" : "en-US");
const integer = (value: number, language: Language) =>
  new Intl.NumberFormat(locale(language)).format(value);
const percent = (value: number, language: Language) =>
  new Intl.NumberFormat(locale(language), { style: "percent", maximumFractionDigits: 0 }).format(
    value,
  );
function calculatedFromBattleCount(count: number, language: Language) {
  if (language === "pl") return count === 1 ? "1 bitwy" : `${count} bitew`;
  return battleCountLabel(count, language);
}
export function contributingLabel(count: number, language: Language) {
  return translate(language, "attemptsConversionCalculatedFromV2", {
    count: calculatedFromBattleCount(count, language),
  });
}
export function zeroDenominatorMessage(
  kind: "shots" | "enemyHits",
  count: number,
  language: Language,
) {
  const key = kind === "shots" ? "attemptsConversionNoShots" : "attemptsConversionNoEnemyHits";
  const plural = Math.abs(count) % 100;
  const last = Math.abs(count) % 10;
  const verb =
    language === "en"
      ? count === 1
        ? "was"
        : "were"
      : count === 1
        ? "weszła"
        : last >= 2 && last <= 4 && !(plural >= 12 && plural <= 14)
          ? "weszły"
          : "weszło";
  return translate(language, key, { battles: battleCountLabel(count, language), verb });
}
function errorKey(error: unknown): TranslationKey {
  const code = error instanceof AppCommandError ? error.code : "";
  if (code === "session_not_found") return "attemptsConversionSessionNotFound";
  if (code === "invalid_window") return "attemptsConversionInvalidWindow";
  if (code === "database_error") return "attemptsConversionDatabaseError";
  if (code === "report_invariant_violation") return "attemptsConversionInvalidReport";
  return "attemptsConversionUnavailable";
}

type Indicator = {
  key: string;
  label: string;
  skipped: number;
  reasons: AttemptsConversionRejectionReason[];
  note?: string;
};

function indicators(report: AttemptsConversionReport, language: Language): Indicator[] {
  const zero = (rate: AttemptsConversionRate, kind: "shots" | "enemyHits") =>
    rate.zeroDenominatorBattles > 0
      ? zeroDenominatorMessage(kind, rate.zeroDenominatorBattles, language)
      : undefined;
  const facts = [
    ["teamHits", "attemptsConversionTeamHits", report.facts.directTeamHits],
    ["explosionHits", "attemptsConversionExplosionHits", report.facts.explosionHits],
    ["damage", "attemptsConversionDamage", report.facts.damageDealt],
  ] as const;
  return [
    {
      key: "common",
      label: translate(language, "attemptsIndicatorCommon"),
      skipped: report.commonContext.ineligibleBattles,
      reasons: report.commonContext.rejectionReasons,
    },
    {
      key: "hits",
      label: translate(language, "attemptsConversionHitRate"),
      skipped: report.hitRate.metricIneligibleBattles,
      reasons: report.hitRate.rejectionReasons,
      note: zero(report.hitRate, "shots"),
    },
    {
      key: "penetrations",
      label: translate(language, "attemptsConversionPenetrationRate"),
      skipped: report.penetrationRate.metricIneligibleBattles,
      reasons: report.penetrationRate.rejectionReasons,
      note: zero(report.penetrationRate, "enemyHits"),
    },
    ...facts.map(([key, label, fact]) => ({
      key,
      label: translate(language, label),
      skipped: fact.factIneligibleBattles,
      reasons: fact.rejectionReasons,
    })),
  ];
}

/** "How we count this": the one place that explains skipped battles (E5-ui §3). */
function HowCounted({
  report,
  language,
}: {
  report: AttemptsConversionReport;
  language: Language;
}) {
  const [open, setOpen] = useState(false);
  const shown = indicators(report, language).filter(
    (indicator) => indicator.skipped > 0 || indicator.note,
  );
  return (
    <>
      <button type="button" className="link" onClick={() => setOpen(true)}>
        {translate(language, "attemptsHowCounted")}
      </button>
      {open && (
        <Dialog title={translate(language, "attemptsHowCounted")} onClose={() => setOpen(false)}>
          <p className="muted">{translate(language, "attemptsHowIntro")}</p>
          <h3 className="stat-label">{translate(language, "attemptsHowSkippedTitle")}</h3>
          {shown.length > 0 ? (
            <ul className="how-list">
              {shown.map((indicator) => (
                <li key={indicator.key} data-testid={`attempts-skipped-${indicator.key}`}>
                  <p className="how-row">
                    <span>{indicator.label}</span>{" "}
                    <span className="subtle">
                      {translate(language, "attemptsHowSkippedCount", {
                        count: integer(indicator.skipped, language),
                      })}
                    </span>
                  </p>
                  <ul className="how-reasons">
                    {indicator.reasons
                      .filter((reason) => reason.count > 0)
                      .map((reason) => (
                        <li key={reason.code} data-testid={`attempts-reason-${reason.code}`}>
                          {translate(language, reasonKeys[reason.code])}:{" "}
                          {integer(reason.count, language)}
                        </li>
                      ))}
                    {indicator.note && <li>{indicator.note}</li>}
                  </ul>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted">{translate(language, "attemptsHowNoneSkipped")}</p>
          )}
          <div className="actions">
            <button type="button" className="button" onClick={() => setOpen(false)}>
              {translate(language, "attemptsHowClose")}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}

function FunnelStep({ value, label, share }: { value: string; label: string; share: number }) {
  return (
    <li className="funnel-step">
      <p className="num funnel-value">{value}</p>
      <p className="funnel-label">{label}</p>
      <div className="track" aria-hidden="true">
        <div style={{ width: `${Math.max(0, Math.min(1, share)) * 100}%` }} />
      </div>
    </li>
  );
}

function FunnelRate({
  rate,
  textKey,
  language,
}: {
  rate: AttemptsConversionRate;
  textKey: "attemptsFunnelHitRate" | "attemptsFunnelPenetrationRate";
  language: Language;
}) {
  return (
    <li className="funnel-rate">
      <p>
        {rate.value === null
          ? translate(language, "attemptsFunnelNoRate")
          : translate(language, textKey, { percent: percent(rate.value, language) })}
      </p>
      {rate.contributingBattles > 0 && (
        <p className="subtle">{contributingLabel(rate.contributingBattles, language)}</p>
      )}
    </li>
  );
}

function isSpgOnly(report: AttemptsConversionReport) {
  const nonZero = report.commonContext.rejectionReasons.filter((reason) => reason.count > 0);
  return (
    report.selectedBattles > 0 &&
    nonZero.length === 1 &&
    nonZero[0].code === "unsupported_vehicle_mechanics" &&
    nonZero[0].count === report.selectedBattles
  );
}

function ReportBody({
  report,
  language,
}: {
  report: AttemptsConversionReport;
  language: Language;
}) {
  if (report.selectedBattles === 0)
    return (
      <p className="muted">
        {translate(
          language,
          report.scope.type === "session"
            ? "attemptsConversionEmptySession"
            : "attemptsConversionEmptyRecent",
        )}
      </p>
    );
  const coverage = (
    <p className="muted conversion-coverage">
      {translate(language, "attemptsConversionSupportedContext", {
        eligible: report.commonContext.eligibleBattles,
        selected: report.selectedBattles,
      })}{" "}
      · <HowCounted report={report} language={language} />
    </p>
  );
  if (report.commonContext.eligibleBattles === 0)
    return (
      <>
        <p className="muted">
          {translate(
            language,
            isSpgOnly(report) ? "attemptsConversionSpgOnly" : "attemptsConversionNoEligible",
          )}
        </p>
        {coverage}
      </>
    );
  const shots = report.hitRate.denominatorSum;
  const hits = report.hitRate.numeratorSum;
  const penetrations = report.penetrationRate.numeratorSum;
  const hitShare = report.hitRate.value ?? 0;
  const facts = (
    [
      ["attemptsConversionTeamHits", report.facts.directTeamHits.valueSum],
      ["attemptsConversionExplosionHits", report.facts.explosionHits.valueSum],
      ["attemptsConversionDamage", report.facts.damageDealt.valueSum],
    ] as const
  ).map(([label, value]) =>
    translate(language, "attemptsSecondaryFact", {
      label: translate(language, label),
      value: value === null ? "—" : integer(value, language),
    }),
  );
  return (
    <>
      <ol
        className="funnel"
        aria-label={translate(language, "attemptsFunnelLabel", {
          shots: integer(shots, language),
          hits: integer(hits, language),
          penetrations: integer(penetrations, language),
        })}
      >
        <FunnelStep
          value={integer(shots, language)}
          label={translate(language, "attemptsFunnelShots")}
          share={shots > 0 ? 1 : 0}
        />
        <FunnelRate rate={report.hitRate} textKey="attemptsFunnelHitRate" language={language} />
        <FunnelStep
          value={integer(hits, language)}
          label={translate(language, "attemptsFunnelHits")}
          share={hitShare}
        />
        <FunnelRate
          rate={report.penetrationRate}
          textKey="attemptsFunnelPenetrationRate"
          language={language}
        />
        <FunnelStep
          value={report.penetrationRate.value === null ? "—" : integer(penetrations, language)}
          label={translate(language, "attemptsFunnelPenetrations")}
          share={hitShare * (report.penetrationRate.value ?? 0)}
        />
      </ol>
      <p className="subtle conversion-facts">{facts.join(" · ")}</p>
      {coverage}
    </>
  );
}

export function AttemptsConversionPanel({
  language,
  sessionId,
}: {
  language: Language;
  sessionId: string | null;
}) {
  const [scope, setScope] = useState<ScopeSelection>(sessionId ? "session" : "10");
  const [report, setReport] = useState<AttemptsConversionReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<TranslationKey | null>(null);
  const [retry, setRetry] = useState(0);
  const request = useRef(0);
  useEffect(() => {
    const selection = scope === "session" && sessionId === null ? "10" : scope;
    const generation = ++request.current;
    setLoading(true);
    setError(null);
    setReport(null);
    const response =
      selection === "session"
        ? api.attemptsConversionSession(sessionId!)
        : api.attemptsConversionRecent(Number(selection) as AttemptsConversionRecentWindow);
    void response
      .then((value) => {
        if (generation === request.current) setReport(value);
      })
      .catch((value) => {
        if (generation === request.current) setError(errorKey(value));
      })
      .finally(() => {
        if (generation === request.current) setLoading(false);
      });
    return () => {
      request.current += 1;
    };
  }, [scope, sessionId, retry]);
  const options = (
    [
      { value: "session", label: translate(language, "attemptsConversionSession") },
      ...recentWindows.map((window) => ({
        value: String(window) as ScopeSelection,
        label: String(window),
        ariaLabel: translate(language, "attemptsConversionRecentRange", { count: window }),
      })),
    ] as const
  ).filter((option) => option.value !== "session" || sessionId !== null);
  return (
    <section className="card section conversion" aria-labelledby="conversion-h">
      <div className="card-head">
        <div>
          <h2 id="conversion-h" className="section-title">
            {translate(language, "attemptsConversion")}
          </h2>
        </div>
        <SegmentedControl
          label={translate(language, "attemptsConversionScope")}
          value={scope === "session" && !sessionId ? "10" : scope}
          options={options}
          onChange={setScope}
        />
      </div>
      {loading ? (
        <p className="muted" role="status">
          {translate(language, "attemptsConversionLoading")}
        </p>
      ) : error ? (
        <div className="state-inline" role="alert">
          <p className="state-error">{translate(language, error)}</p>
          <button type="button" className="button" onClick={() => setRetry((value) => value + 1)}>
            {translate(language, "attemptsConversionRetry")}
          </button>
        </div>
      ) : report ? (
        <ReportBody report={report} language={language} />
      ) : null}
    </section>
  );
}
