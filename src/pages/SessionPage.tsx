import { useState } from "react";
import type { ReactNode } from "react";
import { api } from "../api";
import { BattleTable } from "../components/BattleTable";
import { AttemptsConversionPanel } from "../components/session/AttemptsConversion";
import { BattlePaceSummaryPanel } from "../components/session/BattlePace";
import { ImportBar } from "../components/ImportBar";
import {
  BattleStrip,
  Figure,
  SessionHero,
  SessionMoments,
} from "../components/session/SessionStory";
import { useTankLabel } from "../components/TankName";
import { Async, Dialog, Empty, ErrorState } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { formatNumber, formatTime } from "../lib/format";
import { prefersReducedMotion, reservedChars, useArrivals, useCountedNumber } from "../lib/motion";
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
  ImportProgressEvent,
  Language,
  Session,
  SessionBaselineComparison,
  SessionInsight,
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
  snapshot,
  meta,
  action,
  onOpenBattle,
  onOpenHistory,
  onOpenSettings,
}: {
  session: Session;
  summary: SessionSummary;
  comparison: SessionBaselineComparison | null;
  story: SessionStory | null;
  snapshot: AppSnapshot;
  meta: ReactNode;
  action: ReactNode;
  onOpenBattle: (battleId: string) => void;
  onOpenHistory: () => void;
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
  const best = story?.bestBattle ?? null;
  return (
    <>
      <SessionHero
        mapKey={best?.mapKey ?? null}
        mapName={best?.mapName ?? null}
        meta={meta}
        headline={story?.headline ?? null}
        fallbackTitle={fallbackTitle}
        notes={notes}
        action={action}
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
      <BattleStrip
        battles={battles}
        bestBattleId={best?.battleId ?? null}
        newBattleIds={fresh}
        onOpenBattle={onOpenBattle}
      />
      <SessionMoments moments={story?.moments ?? []} onOpenBattle={onOpenBattle} />
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
}: {
  value: number | null;
  animate: boolean;
  text: (value: number | null) => string;
  label: string;
  main?: boolean;
}) {
  const counted = useCountedNumber(value, animate);
  return (
    <Figure
      main={main}
      value={text(counted.shown)}
      label={label}
      delta={counted.direction ? { direction: counted.direction, key: counted.key } : null}
      reserve={counted.direction ? reservedChars([text(counted.from), text(value)]) : undefined}
    />
  );
}

export function SessionPage({
  snapshot,
  sessionId,
  progress,
  onChanged,
  onOpenBattle,
  onOpenHistory,
  onOpenSettings,
}: {
  snapshot: AppSnapshot;
  sessionId: string | null;
  endedSessionId: string | null;
  progress: ImportProgressEvent | null;
  onChanged: () => void;
  onOpenBattle: (battleId: string, sessionId: string) => void;
  onOpenHistory: () => void;
  onOpenSettings: () => void;
}) {
  const { language, tr } = useLanguage();
  const [confirmEnd, setConfirmEnd] = useState(false);
  const [actionError, setActionError] = useState<string | undefined | null>(null);
  const session =
    (sessionId ? snapshot.recentSessions.find((item) => item.id === sessionId) : null) ??
    snapshot.activeSession ??
    snapshot.recentSessions[0] ??
    null;
  const id = session?.id ?? null;
  const version = useDataVersion();
  const data = useAsync(
    async () => {
      if (!id) return null;
      const [summary, comparison, story] = await Promise.all([
        api.sessionSummary(id),
        api.sessionBaselineComparison(id).catch(() => null),
        api.sessionStory(id).catch(() => null),
      ]);
      return { summary, comparison, story };
    },
    [id],
    { refreshKey: version },
  );

  if (!snapshot.configured) {
    return (
      <>
        <header className="page-header">
          <h1 className="page-title">{tr("firstRunTitle")}</h1>
        </header>
        <Empty
          text={tr("firstRunText")}
          action={
            <button type="button" className="button button-primary" onClick={onOpenSettings}>
              {tr("openSettings")}
            </button>
          }
        />
      </>
    );
  }

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
      <p>
        {tr("sessionMetaLine", {
          title: tr(active ? "sessionToday" : "sessionLast"),
          meta: tr("sessionMeta", {
            time: formatTime(session.startedAt, language),
            battles: session.analyzedBattleCount,
            status: tr(active ? "sessionActive" : "sessionEnded"),
          }),
        })}
      </p>
      {active && <p>{tr("sessionAutoEnd", { minutes: snapshot.inactivityMinutes })}</p>}
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
      {!session ? (
        <>
          {plainHeader}
          <Empty text={tr("noSessionYet")} />
        </>
      ) : data.state.status !== "ready" || !data.state.data ? (
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
          snapshot={snapshot}
          meta={meta}
          action={action}
          onOpenBattle={(battleId) => onOpenBattle(battleId, session.id)}
          onOpenHistory={onOpenHistory}
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
