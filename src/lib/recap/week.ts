/**
 * Week summary in Polish (spec 30 §4.3): battles with the win rate (context only, D12), the
 * indicators that moved 5 points or more, the week's goals grouped by state, last week's
 * goals and the 5-battle challenges. Under 15 battles: one sentence, "too few battles".
 */
import type { GoalKind, GoalState, WeekFact, WeekFactValue, WeeklyMetric } from "../../types";
import { chooseSentences, type RecapSentence, type SentencePlan } from "./compose";
import { BATTLES_ACC, BATTLES_NOM, count, n, pct, type RecapTextContext } from "./common";
import type { Forms } from "./plural";

type Fact<K extends WeekFactValue["key"]> = Extract<WeekFactValue, { key: K }>;
type Kept<K extends WeekFactValue["key"]> = { id: string; fact: Fact<K> };

/** "z 1 celu / z 3 celów" */
const GOALS_GEN: Forms = ["celu", "celów", "celów"];
const METRICS: Record<WeeklyMetric, string> = {
  damage_assist: "obrażenia z asystą",
  survival: "przetrwanie",
  xp: "XP",
};
const STATES: Record<Exclude<GoalState, "too_few">, string> = {
  achieved: "osiągnięty",
  close: "blisko celu",
  below: "poniżej celu",
};
const MIN_SENTENCES = 3;

function goalName(fact: Fact<"cel">, context: RecapTextContext): string {
  const names: Record<GoalKind, () => string> = {
    survival: () => "przetrwanie",
    xp: () => "XP",
    tank_damage_assist: () =>
      `obrażenia z asystą na czołgu ${context.tank(fact.vehicleKey ?? "—", fact.tankName)}`,
  };
  return names[fact.kind]();
}

/** "a, b i c" */
function list(items: string[]): string {
  return items.length < 2
    ? (items[0] ?? "")
    : `${items.slice(0, -1).join(", ")} i ${items[items.length - 1]}`;
}

function tooFew({ fact }: Kept<"liczba_bitew">): string[] {
  return [
    `za mało bitew w tym tygodniu na trend: ${n(fact.battles)} z ${n(fact.minimum)}`,
    `za mało bitew w tym tygodniu: ${n(fact.battles)}, a trend liczy się od ${n(fact.minimum)}`,
  ];
}

function battlesVariants(
  battles: Kept<"liczba_bitew">,
  wins: Kept<"odsetek_wygranych"> | null,
): string[] {
  const played = count(battles.fact.battles, BATTLES_ACC);
  const early = battles.fact.confidence === "preliminary" ? ", trend jest wstępny" : "";
  const rate = wins ? `, wygrane: ${pct(wins.fact.percent)}` : "";
  const rateAlt = wins ? `, a odsetek wygranych to ${pct(wins.fact.percent)}` : "";
  return [
    `w tym tygodniu {~rozegra} ${played}${early}${rate}`,
    `tydzień liczy ${played}${early}${rateAlt}`,
    `bitwy w tym tygodniu: ${n(battles.fact.battles)}${early}${rate}`,
    `w tym tygodniu ${count(battles.fact.battles, BATTLES_NOM)}${early}${rateAlt}`,
    `{~zagra} w tym tygodniu ${played}${early}${rate}`,
  ];
}

function winsVariants({ fact }: Kept<"odsetek_wygranych">): string[] {
  return [
    `odsetek wygranych w tym tygodniu: ${pct(fact.percent)}`,
    `wygrane w tym tygodniu: ${pct(fact.percent)}`,
  ];
}

function indicatorVariants(indicators: Kept<"wskaznik">[]): string[] {
  const usual = pct(indicators[0].fact.usual);
  const items = list(indicators.map(({ fact }) => `${METRICS[fact.metric]} ${pct(fact.percent)}`));
  return [
    `wobec Twojej średniej (${usual} to zwykły poziom): ${items}`,
    `wskaźniki tygodnia, gdzie ${usual} to jak zwykle: ${items}`,
    `przy ${usual} jako zwykłym poziomie ten tydzień to ${items}`,
  ];
}

function goalVariants(goals: Kept<"cel">[], context: RecapTextContext): string[] {
  const judged = goals.filter(({ fact }) => fact.state !== "too_few");
  const each = judged.map(
    ({ fact }) =>
      `${goalName(fact, context)} (${STATES[fact.state as Exclude<GoalState, "too_few">]})`,
  );
  return [`cele tygodnia: ${list(each)}`, `stan celów tygodnia: ${list(each)}`];
}

function previousVariants({ fact }: Kept<"poprzedni_tydzien">): string[] {
  const share = `${n(fact.achieved)} z ${count(fact.goals, GOALS_GEN)}`;
  return [
    `w poprzednim tygodniu osiągnięte ${share}`,
    `poprzedni tydzień: osiągnięte ${share}`,
    `z poprzedniego tygodnia osiągnięte ${share}`,
  ];
}

function challengesVariants({ fact }: Kept<"wyzwania">): string[] {
  const of = `wyzwania na ${n(fact.battles)} bitew`;
  return [
    `${of} w tym tygodniu: ukończone ${n(fact.completed)}, nieudane ${n(fact.failed)}, porzucone ${n(fact.abandoned)}`,
    `zakończone ${of}: ukończone ${n(fact.completed)}, nieudane ${n(fact.failed)}, porzucone ${n(fact.abandoned)}`,
  ];
}

/** Every sentence of a week summary with all its variants (tests read every variant). */
export function weekPlans(facts: WeekFact[], context: RecapTextContext): SentencePlan[] {
  const all = <K extends WeekFactValue["key"]>(key: K): Kept<K>[] =>
    facts
      .filter((item) => item.fact.key === key)
      .map((item) => ({ id: item.id, fact: item.fact as Fact<K> }));
  const battles = all("liczba_bitew")[0] ?? null;
  if (!battles) return [];
  if (battles.fact.confidence === "too_few")
    return [{ slot: "too_few", facts: [battles.id], variants: tooFew(battles) }];
  const wins = all("odsetek_wygranych")[0] ?? null;
  const indicators = all("wskaznik");
  const goals = all("cel");
  const previous = all("poprzedni_tydzien")[0] ?? null;
  const challenges = all("wyzwania")[0] ?? null;
  const rest: SentencePlan[] = [];
  if (indicators.length > 0)
    rest.push({
      slot: "indicators",
      facts: indicators.map((item) => item.id),
      variants: indicatorVariants(indicators),
    });
  if (goals.length > 0)
    rest.push({
      slot: "goals",
      facts: goals.map((item) => item.id),
      variants: goalVariants(goals, context),
    });
  if (previous)
    rest.push({ slot: "previous", facts: [previous.id], variants: previousVariants(previous) });
  if (challenges)
    rest.push({
      slot: "challenges",
      facts: [challenges.id],
      variants: challengesVariants(challenges),
    });
  const split = wins !== null && 1 + rest.length < MIN_SENTENCES;
  const plans: SentencePlan[] = [
    {
      slot: "battles",
      facts: wins && !split ? [battles.id, wins.id] : [battles.id],
      variants: battlesVariants(battles, split ? null : wins),
    },
  ];
  if (wins && split) plans.push({ slot: "wins", facts: [wins.id], variants: winsVariants(wins) });
  plans.push(...rest);
  return plans.length < MIN_SENTENCES ? [] : plans;
}

/** The week summary: one variant per sentence, chosen by the seed (spec 30 §6). */
export function weekSentences(
  recap: { seed: number; facts: WeekFact[] },
  context: RecapTextContext,
): RecapSentence[] {
  return chooseSentences(recap.seed, weekPlans(recap.facts, context), context.address);
}

/** What a reference button says about its fact. */
export function weekFactLabel(fact: WeekFactValue, context: RecapTextContext): string {
  switch (fact.key) {
    case "liczba_bitew":
      return `bitwy w tym tygodniu ${n(fact.battles)}`;
    case "odsetek_wygranych":
      return `wygrane ${pct(fact.percent)}, tylko kontekst`;
    case "wskaznik":
      return `${METRICS[fact.metric]} ${pct(fact.percent)} zwykłego poziomu`;
    case "cel":
      return `cel: ${goalName(fact, context)}, ${fact.state === "too_few" ? "za mało bitew" : STATES[fact.state]}`;
    case "poprzedni_tydzien":
      return `poprzedni tydzień: ${n(fact.achieved)} z ${n(fact.goals)} celów osiągnięte`;
    case "wyzwania":
      return `wyzwania: ukończone ${n(fact.completed)}, nieudane ${n(fact.failed)}, porzucone ${n(fact.abandoned)}`;
  }
}

/** Element id on the Goals screen that shows a fact's number. */
export function weekTarget(fact: WeekFactValue): string {
  switch (fact.key) {
    case "liczba_bitew":
      return "recap-target-week-battles";
    case "odsetek_wygranych":
      return "recap-target-week-wins";
    case "wskaznik":
      return `recap-target-indicator-${fact.metric}`;
    case "cel":
      return `recap-target-goal-${fact.kind}`;
    case "poprzedni_tydzien":
      return "recap-target-previous-week";
    case "wyzwania":
      return "recap-target-challenges";
  }
}
