import { useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { BattleTable } from "../components/BattleTable";
import { GameFolderActions } from "../components/GameFolderActions";
import { AttemptsConversionPanel } from "../components/session/AttemptsConversion";
import { BattlePaceSummaryPanel } from "../components/session/BattlePace";
import { ImportBar } from "../components/ImportBar";
import { Recap } from "../components/Recap";
import {
  BattleStrip,
  Figure,
  SessionHero,
  SessionMoments,
} from "../components/session/SessionStory";
import { useTankLabel } from "../components/TankName";
import { SessionDamast } from "../features/damast/SessionDamast";
import { damastBestBattleId, damastHeat, damastStats } from "../features/damast/stats";
import { Async, Dialog, Empty, ErrorState } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import type { Address } from "../lib/recap/address";
import { formatNumber, formatTime } from "../lib/format";
import { prefersReducedMotion, reservedChars, useArrivals, useCountedNumber } from "../lib/motion";
import { SESSION_TARGETS, sessionFactLabel, sessionSentences } from "../lib/recap/session";
import {
  MIN_SESSION_BATTLES,
  sessionDamageReference,
  sessionInsightLines,
  type InsightFallback,
  type InsightLine,
  type SessionDamageReference,
} from "../lib/sessionReference";
import type {
  AppSnapshot,
  Battle,
  BattleRef,
  DamastSession,
  ImportProgressEvent,
  Language,
  Recap as RecapFacts,
  Session,
  SessionBaselineComparison,
  SessionInsight,
  SessionMoment,
  SessionStory,
  SessionSummary,
} from "../types";

const signalKeys: Record<string, [UiKey, UiKey]> = {
  damage: ["signalDamageAbove", "signalDamageBelow"],
  survival: ["signalSurvivalAbove", "signalSurvivalBelow"],
  assist: ["signalAssistAbove", "signalAssistBelow"],
  wn8: ["signalWn8Above", "signalWn8Below"],
};

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

function signalText(signal: SessionInsight, tank: string, tr: Translate) {
  const [above, below] = signalKeys[signal.metric] ?? signalKeys.damage;
  const size = Math.round(Math.abs(signal.deltaValue));
  return tr(signal.direction === "negative" ? below : above, {
    tank,
    percent: size,
    points: size,
  });
}

const fallbackKeys: Record<InsightFallback, UiKey> = {
  close_to_average: "noHighlights",
  damage_close_tanks_too_few: "insightsCloseTanksTooFew",
  session_too_small: "insightsSessionTooSmall",
  baseline_too_small: "insightsBaselineTooSmall",
  no_data: "insightsNoData",
};

function Highlights({ comparison }: { comparison: SessionBaselineComparison | null }) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  const { lines, fallback } = sessionInsightLines(comparison);
  const text = (line: InsightLine) =>
    line.kind === "session_damage"
      ? tr(line.direction === "negative" ? "sessionDamageBelow" : "sessionDamageAbove", {
          percent: line.percent,
          baseline: formatNumber(line.baseline, language),
        })
      : signalText(line.signal, label(line.vehicleKey, line.tankName).short, tr);
  return (
    <section className="section" aria-labelledby="signals-h">
      <h2 id="signals-h" className="section-title">
        {tr("highlights")}
      </h2>
      {fallback !== null ? (
        <p className="muted">{tr(fallbackKeys[fallback])}</p>
      ) : (
        <div className="insight-list">
          {lines.map((line) => (
            <div key={line.kind === "tank" ? line.key : "session"} className="card signal">
              <p>{text(line)}</p>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

/** "Podsumowanie sesji" (spec 30 §8): Polish only, hidden in English and without 3 sentences. */
function SessionRecap({ recap }: { recap: RecapFacts | null }) {
  const { language, address, tr } = useLanguage();
  const label = useTankLabel();
  if (language !== "pl" || recap?.kind !== "session") return null;
  const context = { address, tank: (key: string, name: string | null) => label(key, name).short };
  const sentences = sessionSentences(recap, context);
  if (sentences.length === 0) return null;
  return (
    <Recap
      id="session-recap"
      title={tr("recapSessionTitle")}
      sentences={sentences}
      facts={recap.facts.map((fact) => ({
        id: fact.id,
        label: sessionFactLabel(fact.fact, context),
        target: SESSION_TARGETS[fact.fact.key] ?? null,
      }))}
    />
  );
}

/** Hero note: the same reference as the insights (E5-ui §4). */
function heroNote(
  reference: SessionDamageReference,
  analyzedBattles: number,
  language: Language,
  tr: Translate,
): string[] {
  if (analyzedBattles < MIN_SESSION_BATTLES) return [tr("compareAfterThree")];
  if (reference.state === "baseline_too_small" || reference.state === "no_data")
    return [tr("baselineTooSmall", { count: reference.baselineBattles })];
  if (reference.state === "session_too_small" || reference.baseline === null)
    return [tr("heroComparedTooFew", { count: reference.sessionBattles })];
  const change = reference.change ?? 0;
  const baseline = formatNumber(reference.baseline, language);
  const note = [
    change === 0
      ? tr("damageSameAsBaseline", { baseline })
      : tr("damageVsBaseline", {
          percent: Math.abs(change),
          direction: tr(change > 0 ? "more" : "less"),
          baseline,
        }),
  ];
  if (reference.sessionBattles < analyzedBattles)
    note.push(
      tr("heroComparedPart", { compared: reference.sessionBattles, total: analyzedBattles }),
    );
  return note;
}

function SessionBody({
  session,
  summary,
  comparison,
  story,
  recap,
  damast,
  snapshot,
  meta,
  action,
  onOpenBattle,
  onOpenHistory,
  onOpenHistorySessions,
  onOpenSettings,
}: {
  session: Session;
  summary: SessionSummary;
  comparison: SessionBaselineComparison | null;
  story: SessionStory | null;
  recap: RecapFacts | null;
  /** The plate's data: the session's temperature, and the plate once it has ended. */
  damast: DamastSession | null;
  snapshot: AppSnapshot;
  meta: ReactNode;
  action: ReactNode;
  onOpenBattle: (battleId: string) => void;
  onOpenHistory: () => void;
  onOpenHistorySessions: () => void;
  onOpenSettings: () => void;
}) {
  const { language, tr } = useLanguage();
  const metrics = summary.metrics;
  const battles = [...summary.battles]
    .map((view) => view.battle)
    .sort((a, b) => a.battleTime - b.battleTime || a.arenaId.localeCompare(b.arenaId));
  const active = session.status === "active";
  // Spec 29 §6: only battles that arrive while the screen is open move, never on entering it.
  const fresh = useArrivals(
    session.id,
    battles.map((battle) => battle.arenaId),
    active,
  );
  const animate = fresh.length > 0 && !prefersReducedMotion();
  const xvmOn =
    snapshot.advancedStatus.xvmEnabled &&
    snapshot.wn8Presentation.expectedValues.availability === "available";
  const fallbackTitle = tr(active ? "sessionToday" : "sessionLast");
  if (metrics.analyzedBattles === 0)
    return (
      <>
        <SessionHero
          mapKey={null}
          mapName={null}
          meta={meta}
          headline={null}
          fallbackTitle={fallbackTitle}
          figures={null}
          notes={[]}
          action={action}
        />
        <Empty text={tr("noBattlesInSession")} />
      </>
    );
  const reference = sessionDamageReference(comparison);
  const notes = heroNote(reference, metrics.analyzedBattles, language, tr);
  // The battle the summary's "najwięcej obrażeń" sentence names (spec 30, chosen by damage).
  const mostDamage = story?.bestBattle ?? null;
  // Spec 34 §3: an ended session gets its Damast plate; its spark is then the one hot spot.
  const plated = !active && damast !== null && damastStats(damast).count > 0;
  // Author's decision after E14: the best battle is the highest r everywhere — the strip's mark,
  // the card's map, the moment and (on an ended session) the spark under the plate.
  const bestBattleId = damastBestBattleId(damast);
  const best = battles.find((battle) => battle.arenaId === bestBattleId) ?? null;
  const moments = (story?.moments ?? []).flatMap((moment): SessionMoment[] => {
    if (moment.kind !== "best_battle") return [moment];
    return best ? [{ ...moment, battle: battleRef(best) }] : [];
  });
  return (
    <>
      <SessionHero
        mapKey={best?.mapKey ?? null}
        mapName={best?.mapName ?? null}
        meta={meta}
        headline={story?.headline ?? null}
        fallbackTitle={fallbackTitle}
        notes={notes}
        notesId="recap-target-damage"
        action={action}
        // One session temperature in the app: the Damast one (mean r − 1, spec 34 §2).
        heat={damastHeat(damast)}
        figures={
          <>
            <CountedFigure
              main
              value={metrics.averageDamage}
              animate={animate}
              text={(value) => formatNumber(value, language)}
              label={tr("figureDamage")}
            />
            <CountedFigure
              id="recap-target-wins"
              value={metrics.wins}
              animate={animate}
              text={(value) =>
                tr("figureOf", {
                  value: formatNumber(value, language),
                  count: metrics.outcomeSampleSize,
                })
              }
              label={tr("figureWins")}
            />
            <CountedFigure
              value={metrics.averageAssist}
              animate={animate}
              text={(value) => formatNumber(value, language)}
              label={tr("figureAssist")}
            />
            {xvmOn && summary.wn8.value !== null && (
              <CountedFigure
                id="recap-target-wn8"
                value={summary.wn8.value}
                animate={animate}
                text={(value) => formatNumber(value, language)}
                label={tr("sessionWn8")}
              />
            )}
          </>
        }
      />
      {!xvmOn && (
        <p className="muted">
          {tr("noXvm")}{" "}
          <button type="button" className="link" onClick={onOpenSettings}>
            {tr("openSettings")}
          </button>
        </p>
      )}
      {plated && damast && (
        <SessionDamast
          damast={damast}
          onOpenHistory={onOpenHistorySessions}
          mapOf={(battleId) =>
            battles.find((battle) => battle.arenaId === battleId)?.mapName ?? null
          }
        />
      )}
      <SessionRecap recap={recap} />
      <BattleStrip
        battles={battles}
        bestBattleId={bestBattleId}
        referenceBattleId={mostDamage?.battleId ?? null}
        newBattleIds={fresh}
        onOpenBattle={onOpenBattle}
        hot={!plated}
      />
      <SessionMoments moments={moments} onOpenBattle={onOpenBattle} />
      <div className="session-layout">
        <div className="session-main">
          <Highlights comparison={comparison} />
        </div>
        <div className="session-side">
          <BattleTable
            title={tr("battlesInSession")}
            rows={[...battles].reverse()}
            limit={10}
            totalLabel={tr("allBattles", { count: session.battleCount })}
            onOpenAll={onOpenHistory}
            onOpenBattle={onOpenBattle}
          />
        </div>
      </div>
      <div className="session-extra">
        <BattlePaceSummaryPanel summary={summary.battlePace} language={language} />
        <AttemptsConversionPanel language={language} sessionId={session.id} />
      </div>
    </>
  );
}

/** A battle of the session as the moments show it. */
function battleRef(battle: Battle): BattleRef {
  return {
    battleId: battle.arenaId,
    damage: battle.damage ?? 0,
    mapKey: battle.mapKey,
    mapName: battle.mapName,
    vehicleKey: battle.vehicleKey,
    tankName: battle.tankName,
  };
}

/**
 * A summary number (spec 29 §6): counts from the old to the new value when a battle arrived,
 * with the direction color as a fading overlay; the width is kept for the longer text.
 */
function CountedFigure({
  value,
  animate,
  text,
  label,
  main = false,
  id,
}: {
  value: number | null;
  animate: boolean;
  text: (value: number | null) => string;
  label: string;
  main?: boolean;
  id?: string;
}) {
  const counted = useCountedNumber(value, animate);
  return (
    <Figure
      id={id}
      main={main}
      value={text(counted.shown)}
      label={label}
      delta={counted.direction ? { direction: counted.direction, key: counted.key } : null}
      reserve={counted.direction ? reservedChars([text(counted.from), text(value)]) : undefined}
    />
  );
}

/**
 * The first start without a game folder (hotfix 1.0.0-beta.3): choose it right here, by
 * detection or by hand, instead of an empty screen.
 */
/**
 * The address question of the first run (decision 2026-11-16): he, she or no form, saved to
 * the same setting as in Settings; the texts stay neutral until it is answered.
 */
function AddressQuestion() {
  const { language, address, addressChosen, setAddress, tr } = useLanguage();
  if (language !== "pl") return null;
  const choices: [Address, UiKey][] = [
    ["masculine", "firstRunAddressHe"],
    ["feminine", "firstRunAddressShe"],
    ["neutral", "firstRunAddressNone"],
  ];
  return (
    <div className="card state" role="group" aria-labelledby="first-run-address">
      <p id="first-run-address">{tr("firstRunAddress")}</p>
      <div className="actions" style={{ justifyContent: "flex-start" }}>
        {choices.map(([value, label]) => {
          const pressed = addressChosen && address === value;
          return (
            <button
              key={value}
              type="button"
              className={pressed ? "button button-primary" : "button"}
              aria-pressed={pressed}
              onClick={() => setAddress(value)}
            >
              {tr(label)}
            </button>
          );
        })}
      </div>
    </div>
  );
}

function FirstRun({ onChanged }: { onChanged: () => void }) {
  const { tr } = useLanguage();
  const [error, setError] = useState<string | undefined | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const run = (action: () => Promise<unknown>, done?: string) => {
    setError(null);
    setNotice(null);
    action().then(
      () => {
        if (done) setNotice(done);
        onChanged();
      },
      (failure: { code?: string }) => setError(failure?.code),
    );
  };
  return (
    <>
      <header className="page-header">
        <h1 className="page-title">{tr("firstRunTitle")}</h1>
      </header>
      {error !== null && <ErrorState code={error} />}
      {notice && (
        <p className="muted" role="status">
          {notice}
        </p>
      )}
      <div className="card state">
        <p className="muted">{tr("firstRunText")}</p>
        <p className="muted">{tr("firstRunHint")}</p>
        <GameFolderActions run={run} manualLabel={tr("firstRunPick")} />
      </div>
      <AddressQuestion />
    </>
  );
}

export function SessionPage({
  snapshot,
  sessionId,
  progress,
  onChanged,
  onOpenBattle,
  onOpenHistory,
  onOpenHistorySessions = onOpenHistory,
  onOpenSettings,
}: {
  snapshot: AppSnapshot;
  sessionId: string | null;
  endedSessionId: string | null;
  progress: ImportProgressEvent | null;
  onChanged: () => void;
  onOpenBattle: (battleId: string, sessionId: string) => void;
  onOpenHistory: () => void;
  onOpenHistorySessions?: () => void;
  onOpenSettings: () => void;
}) {
  const { language, tr } = useLanguage();
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [actionError, setActionError] = useState<string | undefined | null>(null);
  const listed = sessionId
    ? (snapshot.recentSessions.find((item) => item.id === sessionId) ?? null)
    : null;
  // Spec 34 §3: a session older than the recent ones (opened from its Damast plate in History)
  // comes with its own summary; it has ended, as only the newest session can run.
  const older = sessionId !== null && listed === null;
  const known = listed ?? snapshot.activeSession ?? snapshot.recentSessions[0] ?? null;
  const id = older ? sessionId : (known?.id ?? null);
  const ended = older || (known !== null && known.status !== "active");
  const version = useDataVersion();
  const data = useAsync(
    async () => {
      if (!id) return null;
      const [summary, comparison, story, recap, damast] = await Promise.all([
        api.sessionSummary(id),
        api.sessionBaselineComparison(id).catch(() => null),
        api.sessionStory(id).catch(() => null),
        api.recap("session", id).catch(() => null),
        // The session's temperature (spec 34 §2); the plate itself only once it has ended.
        Promise.resolve()
          .then(() => api.damastSession(id))
          .catch(() => null),
      ]);
      return { summary, comparison, story, recap, damast };
    },
    [id, ended],
    { refreshKey: version },
  );

  if (!snapshot.configured) return <FirstRun onChanged={onChanged} />;

  const session = older
    ? data.state.status === "ready"
      ? (data.state.data?.summary.session ?? null)
      : null
    : known;

  const act = (run: () => Promise<unknown>) => {
    setActionError(null);
    run().then(onChanged, (error: { code?: string }) => setActionError(error?.code));
  };
  const active = session?.status === "active";
  const action = active ? (
    <button type="button" className="button" onClick={() => setConfirmEnd(true)}>
      {tr("sessionEnd")}
    </button>
  ) : (
    <button type="button" className="button" onClick={() => act(() => api.startSession())}>
      {tr("sessionStart")}
    </button>
  );
  const meta = session && (
    <>
      <p id="recap-target-battles">
        {tr("sessionMetaLine", {
          title: tr(active ? "sessionToday" : "sessionLast"),
          meta: tr("sessionMeta", {
            time: formatTime(session.startedAt, language),
            battles: session.analyzedBattleCount,
            status: tr(active ? "sessionActive" : "sessionEnded"),
          }),
        })}
      </p>
    </>
  );
  const plainHeader = (
    <header className="page-header">
      <div className="section">
        <h1 className="page-title">{tr(active || !session ? "sessionToday" : "sessionLast")}</h1>
        {meta && <div className="muted">{meta}</div>}
      </div>
      {action}
    </header>
  );
  return (
    <>
      {progress && <ImportBar progress={progress} />}
      {actionError !== null && <ErrorState code={actionError} />}
      {!session && !older ? (
        <>
          {plainHeader}
          <Empty text={tr("noSessionYet")} />
        </>
      ) : data.state.status !== "ready" || !data.state.data || !session ? (
        <>
          {plainHeader}
          <Async state={data.state} onRetry={data.reload}>
            {() => null}
          </Async>
        </>
      ) : (
        <SessionBody
          session={session}
          summary={data.state.data.summary}
          comparison={data.state.data.comparison}
          story={data.state.data.story}
          recap={data.state.data.recap}
          damast={data.state.data.damast}
          snapshot={snapshot}
          meta={meta}
          action={action}
          onOpenBattle={(battleId) => onOpenBattle(battleId, session.id)}
          onOpenHistory={onOpenHistory}
          onOpenHistorySessions={onOpenHistorySessions}
          onOpenSettings={onOpenSettings}
        />
      )}
      {confirmEnd && (
        <Dialog title={tr("sessionEnd")} onClose={() => setConfirmEnd(false)}>
          <p className="muted">{tr("sessionEndConfirm")}</p>
          <div className="actions">
            <button type="button" className="button" onClick={() => setConfirmEnd(false)}>
              {tr("cancel")}
            </button>
            <button
              type="button"
              className="button button-primary"
              onClick={() => {
                setConfirmEnd(false);
                act(() => api.endSession());
              }}
            >
              {tr("sessionEnd")}
            </button>
          </div>
        </Dialog>
      )}
    </>
  );
}
