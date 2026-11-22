import { useTankLabel } from "../components/TankName";
import { Recap } from "../components/Recap";
import { WeeklyReportView, closestGoal, trackStyle } from "../components/WeeklyReport";
import { EmberNumber } from "../components/Ember";
import { prefersReducedMotion } from "../lib/motion";
import { useState } from "react";
import { api } from "../api";
import { Async, Card, Dialog, Empty, ErrorState, VehicleImage } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { formatNumber, formatPercent } from "../lib/format";
import { weekFactLabel, weekSentences, weekTarget } from "../lib/recap/week";
import type {
  AppSnapshot,
  ChallengePresentation,
  ChallengeSuggestion,
  ChallengeType,
  WeeklyGoal,
} from "../types";

const NONE: ReadonlySet<string> = new Set();

/**
 * Weekly goals completed while the screen is open (spec 33 §3): none on entering it, then the
 * kinds that turned "achieved" since the last report; kept until the report changes again.
 */
export function useCompletions(goals: readonly WeeklyGoal[] | null): ReadonlySet<string> {
  const signature =
    goals === null ? null : goals.map((goal) => `${goal.kind}:${goal.state}`).join();
  const [state, setState] = useState<{
    signature: string | null;
    states: Map<string, string> | null;
    done: ReadonlySet<string>;
  }>({ signature: null, states: null, done: NONE });
  if (goals === null || state.signature === signature) return state.done;
  const states = new Map(goals.map((goal) => [goal.kind, goal.state]));
  const done = new Set(
    state.states === null
      ? []
      : goals
          .filter(
            (goal) => goal.state === "achieved" && state.states?.get(goal.kind) !== "achieved",
          )
          .map((goal) => goal.kind),
  );
  setState({ signature, states, done });
  return done;
}

const titleKeys: Record<ChallengeType, UiKey> = {
  damage_consistency: "goalDamage",
  effective_survival: "goalSurvival",
  spotting_assist_consistency: "goalSpotting",
  stun_assist_consistency: "goalStun",
};

type GoalLike = Pick<
  ChallengePresentation | ChallengeSuggestion,
  | "challengeType"
  | "vehicleName"
  | "thresholdValue"
  | "contributionFloor"
  | "requiredSuccesses"
  | "targetBattles"
  | "frozenBaselineValue"
>;

export function goalTitle(
  goal: GoalLike,
  tr: (key: UiKey, params?: Record<string, string | number>) => string,
  format: (value: number | null) => string,
  tank: string = goal.vehicleName,
): string {
  return tr(titleKeys[goal.challengeType], {
    tank,
    threshold: format(goal.thresholdValue),
    floor: format(goal.contributionFloor),
    required: goal.requiredSuccesses,
    target: goal.targetBattles,
  });
}

/** Goals keep the stored vehicle name; the window shows the game's short name for it. */
function useGoalTank() {
  const label = useTankLabel();
  return (vehicleName: string) =>
    label(vehicleName.toLowerCase().replace(":", "-"), vehicleName).short;
}

/** Small tank icon next to a goal title (E5-flare §6); no map pictures on this screen. */
function GoalTank({ vehicleName }: { vehicleName: string }) {
  const label = useTankLabel();
  const key = vehicleName.toLowerCase().replace(":", "-");
  return (
    <VehicleImage vehicleKey={key} label={label(key, vehicleName).full} width={40} height={22} />
  );
}

function baselineValue(goal: GoalLike, language: "pl" | "en") {
  return goal.challengeType === "effective_survival"
    ? formatPercent(goal.frozenBaselineValue, language)
    : formatNumber(goal.frozenBaselineValue, language);
}

function ActiveGoal({
  goal,
  hot,
  onChanged,
}: {
  goal: ChallengePresentation;
  /** Spec 33 §3: the goal closest to completion is the screen's hot spot. */
  hot: boolean;
  onChanged: () => void;
}) {
  const { language, tr } = useLanguage();
  const tankOf = useGoalTank();
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | undefined | null>(null);
  const percent = Math.round((goal.completedBattles / goal.targetBattles) * 100);
  return (
    <Card hero className={hot ? "section ember-hot" : "section"} labelledBy="goal-h">
      <p className="hero-label">{tr("goalsActive")}</p>
      <h2 id="goal-h" className="section-title goal-title">
        <GoalTank vehicleName={goal.vehicleName} />
        {goalTitle(goal, tr, (value) => formatNumber(value, language), tankOf(goal.vehicleName))}
      </h2>
      <p>
        <EmberNumber hot={hot} className="num stat-value" text={String(goal.successfulBattles)} />
        <span className="stat-suffix">
          {" "}
          {tr("goalsProgress", {
            done: goal.completedBattles,
            target: goal.targetBattles,
            successes: goal.successfulBattles,
          })}
        </span>
      </p>
      <div
        className="track ember-track"
        style={trackStyle(goal.completedBattles / goal.targetBattles)}
        role="img"
        aria-label={tr("goalTrackLabel", {
          done: goal.completedBattles,
          target: goal.targetBattles,
        })}
      >
        <div style={{ width: `${percent}%` }} />
      </div>
      <p className="muted">
        {tr("goalsNeed", { required: goal.requiredSuccesses, target: goal.targetBattles })}{" "}
        {tr("goalsFrozen", { value: baselineValue(goal, language) })}
      </p>
      {error !== null && <ErrorState code={error} />}
      <div className="actions">
        <button type="button" className="button" onClick={() => setConfirm(true)}>
          {tr("goalsAbandon")}
        </button>
      </div>
      {confirm && (
        <Dialog title={tr("goalsAbandon")} onClose={() => setConfirm(false)}>
          <p className="muted">{tr("goalsAbandonConfirm")}</p>
          <div className="actions">
            <button type="button" className="button" onClick={() => setConfirm(false)}>
              {tr("cancel")}
            </button>
            <button
              type="button"
              className="button button-primary"
              onClick={() => {
                setConfirm(false);
                api
                  .abandonActiveChallenge()
                  .then(onChanged, (failure: { code?: string }) => setError(failure?.code));
              }}
            >
              {tr("goalsAbandon")}
            </button>
          </div>
        </Dialog>
      )}
    </Card>
  );
}

function Suggestions({ sessionId, onChanged }: { sessionId: string; onChanged: () => void }) {
  const { language, tr } = useLanguage();
  const tankOf = useGoalTank();
  const version = useDataVersion();
  const { state, reload } = useAsync(() => api.challengeSuggestions(sessionId), [sessionId], {
    refreshKey: version,
  });
  const [error, setError] = useState<string | undefined | null>(null);
  return (
    <section className="section" aria-labelledby="suggest-h">
      <h2 id="suggest-h" className="section-title">
        {tr("goalsSuggestions")}
      </h2>
      {error !== null && <ErrorState code={error} />}
      <Async state={state} onRetry={reload}>
        {(response) =>
          response.suggestions.length === 0 ? (
            <Empty text={tr("goalsEmpty")} />
          ) : (
            <div className="grid">
              {response.suggestions.map((suggestion) => (
                <Card key={suggestion.suggestionId} className="section">
                  <h3 className="section-title goal-title">
                    <GoalTank vehicleName={suggestion.vehicleName} />
                    {goalTitle(
                      suggestion,
                      tr,
                      (value) => formatNumber(value, language),
                      tankOf(suggestion.vehicleName),
                    )}
                  </h3>
                  <p className="muted">
                    {tr("goalsFrozen", { value: baselineValue(suggestion, language) })}
                  </p>
                  <div className="actions">
                    <button
                      type="button"
                      className="button button-primary"
                      onClick={() =>
                        api
                          .startVehicleChallenge(sessionId, suggestion.suggestionId)
                          .then(onChanged, (failure: { code?: string }) => setError(failure?.code))
                      }
                    >
                      {tr("goalsStart")}
                    </button>
                  </div>
                </Card>
              ))}
            </div>
          )
        }
      </Async>
    </section>
  );
}

function History() {
  const { language, tr } = useLanguage();
  const tankOf = useGoalTank();
  const version = useDataVersion();
  const { state, reload } = useAsync(() => api.challengeHistory(0, 10), [], {
    refreshKey: version,
  });
  const statusKey: Record<string, UiKey> = {
    completed: "goalCompleted",
    failed: "goalFailed",
    abandoned: "goalAbandoned",
  };
  return (
    <section className="section" aria-labelledby="goal-history-h">
      <h2 id="goal-history-h" className="section-title">
        {tr("goalsHistory")}
      </h2>
      <Async state={state} onRetry={reload}>
        {(page) => {
          const finished = page.items.filter((item) => item.status !== "active");
          return finished.length === 0 ? (
            <Empty text={tr("goalsHistoryEmpty")} />
          ) : (
            <div className="card section">
              {finished.map((goal) => (
                <div key={goal.id} className="stat-row">
                  <span>
                    {goalTitle(
                      goal,
                      tr,
                      (value) => formatNumber(value, language),
                      tankOf(goal.vehicleName),
                    )}
                  </span>
                  <span className="subtle">{tr(statusKey[goal.status] ?? "goalAbandoned")}</span>
                </div>
              ))}
            </div>
          );
        }}
      </Async>
    </section>
  );
}

/** "Podsumowanie tygodnia" (spec 30 §2, §4.3): Polish only; under 15 battles one sentence. */
function WeekRecap() {
  const { language, address, tr } = useLanguage();
  const label = useTankLabel();
  const version = useDataVersion();
  const recap = useAsync(() => api.recap("week", "current").catch(() => null), [], {
    refreshKey: version,
  });
  const data = recap.state.status === "ready" ? recap.state.data : null;
  if (language !== "pl" || data?.kind !== "week") return null;
  const context = { address, tank: (key: string, name: string | null) => label(key, name).short };
  const sentences = weekSentences(data, context);
  if (sentences.length === 0) return null;
  return (
    <Recap
      id="week-recap"
      title={tr("recapWeekTitle")}
      sentences={sentences}
      facts={data.facts.map((fact) => ({
        id: fact.id,
        label: weekFactLabel(fact.fact, context),
        target: weekTarget(fact.fact),
      }))}
    />
  );
}

/** Goals in v2.0 are the existing 5-battle tank challenges (spec 25 §1, §5.4). */
export function GoalsPage({
  snapshot,
  onChanged,
}: {
  snapshot: AppSnapshot;
  onChanged: () => void;
}) {
  const { tr } = useLanguage();
  const lastSession =
    snapshot.recentSessions.find((session) => session.status !== "active") ?? null;
  const version = useDataVersion();
  const weekly = useAsync(() => api.weeklyReport(), [], { refreshKey: version });
  const goals = weekly.state.status === "ready" ? weekly.state.data.goals : null;
  const justCompleted = useCompletions(goals);
  const completed = prefersReducedMotion() ? NONE : justCompleted;
  const hotKey = closestGoal(goals ?? [], snapshot.activeChallenge);
  return (
    <>
      {weekly.state.status === "ready" ? (
        <WeeklyReportView
          summary={<WeekRecap />}
          hotKey={hotKey}
          completed={completed}
          report={weekly.state.data}
          xvmOn={
            snapshot.advancedStatus.xvmEnabled &&
            snapshot.wn8Presentation.expectedValues.availability === "available"
          }
        />
      ) : (
        <>
          <header className="page-header">
            <h1 className="page-title">{tr("weeklyTitle")}</h1>
          </header>
          <Async state={weekly.state} onRetry={weekly.reload}>
            {() => null}
          </Async>
        </>
      )}
      <h2 className="section-title">{tr("challengesTitle")}</h2>
      {snapshot.activeChallenge ? (
        <ActiveGoal
          goal={snapshot.activeChallenge}
          hot={hotKey === "challenge"}
          onChanged={onChanged}
        />
      ) : lastSession ? (
        <Suggestions sessionId={lastSession.id} onChanged={onChanged} />
      ) : (
        <Empty text={tr("goalsEmpty")} />
      )}
      <History />
    </>
  );
}
