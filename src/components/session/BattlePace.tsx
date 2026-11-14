import { translate } from "../../i18n/legacy";
import { battleCountLabel, battlePacePresentation } from "../../lib/presentation";
import type { BattlePaceRecord, BattlePaceSummary, Language } from "../../types";

export function BattlePaceBadge({
  pace,
  language,
}: {
  pace: BattlePaceRecord;
  language: Language;
}) {
  const presentation = battlePacePresentation(pace, language);
  if (!presentation.showBadge) return null;
  return (
    <span className={`battle-pace-badge ${presentation.className}`}>
      <span aria-hidden="true">◆</span>
      {presentation.label}
    </span>
  );
}

export function BattlePaceDetails({
  pace,
  language,
  durationSeconds = null,
}: {
  pace: BattlePaceRecord;
  language: Language;
  durationSeconds?: number | null;
}) {
  const presentation = battlePacePresentation(pace, language);
  const sizes =
    pace.initialTeam1Size !== null && pace.initialTeam2Size !== null
      ? `${pace.initialTeam1Size}v${pace.initialTeam2Size}`
      : "—";
  const score =
    pace.destroyedTeam1Score !== null && pace.destroyedTeam2Score !== null
      ? `${pace.destroyedTeam1Score}:${pace.destroyedTeam2Score}`
      : "—";
  return (
    <section className="battle-pace-details" aria-label={translate(language, "paceContext")}>
      <div>
        <span>{translate(language, "paceContext")}</span>
        <strong>{presentation.label}</strong>
      </div>
      {presentation.reason && <p>{presentation.reason}</p>}
      <dl>
        <div>
          <dt>{translate(language, "paceDuration")}</dt>
          <dd>{durationSeconds === null ? "—" : `${durationSeconds} s`}</dd>
        </div>
        <div>
          <dt>{translate(language, "paceTeams")}</dt>
          <dd>{sizes}</dd>
        </div>
        <div>
          <dt>{translate(language, "paceDestroyedScore")}</dt>
          <dd>{score}</dd>
        </div>
      </dl>
    </section>
  );
}

const paceParts = [
  ["obvious", "paceObviousCount", "obviousSteamrolls"],
  ["borderline", "paceBorderlineCount", "borderlineSteamrolls"],
  ["standard", "paceStandardCount", "standardBattles"],
  ["unknown", "paceUnknownCount", "unknownBattles"],
] as const;

/**
 * Battle pace of a session (E5-ui §3): one summary line and a small horizontal split. Every part
 * has its name and count in words next to the bar, so color is never the only carrier.
 */
export function BattlePaceSummaryPanel({
  summary,
  language,
}: {
  summary: BattlePaceSummary;
  language: Language;
}) {
  const oneSided = summary.obviousSteamrolls + summary.borderlineSteamrolls;
  const partial = summary.classifiedBattles < summary.totalBattles;
  let line: string;
  if (summary.totalBattles === 0) line = translate(language, "paceNoBattles");
  else if (summary.classifiedBattles === 0)
    line = translate(language, "paceUnavailableCount", {
      battles: battleCountLabel(summary.unknownBattles, language),
    });
  else
    line = translate(
      language,
      oneSided > 0
        ? partial
          ? "paceLinePartial"
          : "paceLineFull"
        : partial
          ? "paceLinePartialNone"
          : "paceLineNone",
      { oneSided, total: partial ? summary.classifiedBattles : summary.totalBattles },
    );
  return (
    <section className="card section pace" aria-labelledby="pace-h">
      <h2 id="pace-h" className="section-title">
        {translate(language, "paceTitle")}
      </h2>
      <p className="pace-line">{line}</p>
      {summary.totalBattles > 0 && (
        <>
          <div
            className="pace-bar"
            role="img"
            aria-label={translate(language, "paceBarLabel", {
              obvious: summary.obviousSteamrolls,
              borderline: summary.borderlineSteamrolls,
              standard: summary.standardBattles,
              unknown: summary.unknownBattles,
            })}
          >
            {paceParts.map(([part, , field]) =>
              summary[field] > 0 ? (
                <div
                  key={part}
                  className={`pace-part pace-${part}`}
                  style={{ flexGrow: summary[field] }}
                />
              ) : null,
            )}
          </div>
          <ul className="pace-legend">
            {paceParts.map(([part, label, field]) => (
              <li key={part}>
                <span className={`pace-swatch pace-${part}`} aria-hidden="true" />
                <span>{translate(language, label)}</span>{" "}
                <span className="num">{summary[field]}</span>
              </li>
            ))}
          </ul>
        </>
      )}
      <p className="subtle">{translate(language, "paceNeutralDisclaimer")}</p>
    </section>
  );
}
