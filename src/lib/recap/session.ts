/**
 * Session summary in Polish (spec 30 §4.2, §5, §6): the facts from `get_recap` as 3–5 sentences.
 * Order: battles with wins, damage, the battle with the most damage, WN8, survival, the most played tank; one fact per
 * sentence (2026-11-16). Every number goes through `format.ts`; the variants of one sentence say
 * the same thing.
 */
import type { RateReference, SessionFact, SessionFactValue } from "../../types";
import { chooseSentences, type RecapSentence, type SentencePlan } from "./compose";
import {
  BATTLES_ACC,
  BATTLES_GEN,
  BATTLES_LOC,
  DAMAGE,
  POINTS,
  count,
  n,
  pct,
  type RecapTextContext,
} from "./common";
import type { Forms } from "./plural";

type Fact<K extends SessionFactValue["key"]> = Extract<SessionFactValue, { key: K }>;

export type SessionTextContext = RecapTextContext;

/** "w 30 dniach" */
const DAYS_LOC: Forms = ["dniu", "dniach", "dniach"];

/**
 * The reference of a rate, always "w ostatnich 100 bitwach" and "pkt proc." (2026-11-16):
 * "o 7 pkt proc. więcej niż w ostatnich 100 bitwach (51%)".
 */
function rateTail(reference: RateReference): string {
  const earlier = `w ostatnich ${count(reference.battles, BATTLES_LOC)}`;
  if (reference.difference === 0) return `tyle samo co ${earlier} (${pct(reference.percent)})`;
  const direction = reference.difference > 0 ? "więcej" : "mniej";
  return `o ${n(Math.abs(reference.difference))} ${POINTS} ${direction} niż ${earlier} (${pct(reference.percent)})`;
}

function battlesVariants(battles: Fact<"liczba_bitew">): string[] {
  const played = count(battles.battles, BATTLES_ACC);
  return [
    `sesja liczy ${played}`,
    `bitwy w tej sesji: ${n(battles.battles)}`,
    `{~rozegra} ${played}`,
    `w sesji {~rozegra} ${played}`,
  ];
}

/**
 * Wins as a count, then the percent, then the reference (2026-11-16): "Wygrałeś 7 z 12 bitew,
 * czyli 58%, o 7 pkt proc. więcej niż w ostatnich 100 bitwach (51%)." Zero: "…; zwykle 49%."
 */
function winsVariants(wins: Fact<"odsetek_wygranych">): string[] {
  const of = `${n(wins.wins)} z ${count(wins.battles, BATTLES_GEN)}`;
  const reference = wins.reference;
  if (wins.wins === 0) {
    const usual = reference ? `; zwykle ${pct(reference.percent)}` : "";
    return [
      `wygrane: ${of}${usual}`,
      `nie {~wygra} żadnej z ${count(wins.battles, BATTLES_GEN)}${usual}`,
    ];
  }
  const tail = `, czyli ${pct(wins.percent)}${reference ? `, ${rateTail(reference)}` : ""}`;
  return [`wygrane: ${of}${tail}`, `zwycięstwa: ${of}${tail}`, `{~wygra} ${of}${tail}`];
}

/**
 * Session damage (2026-11-16): "Średnio 3 414 obrażeń, o 13% więcej niż zwykle na tych czołgach
 * (3 021)."; zero: "Średnio 0 obrażeń; zwykle na tych czołgach 3 021."
 */
function damageVariants(damage: Fact<"obrazenia">): string[] {
  const usual = n(damage.baseline);
  const dealt = count(damage.current, DAMAGE);
  if (damage.current === 0) return [`średnio ${dealt}; zwykle na tych czołgach ${usual}`];
  const size = pct(Math.abs(damage.percent));
  const direction = damage.percent > 0 ? "więcej" : "mniej";
  const tail =
    damage.percent === 0
      ? `tyle samo co zwykle na tych czołgach (${usual})`
      : `o ${size} ${direction} niż zwykle na tych czołgach (${usual})`;
  return [
    `średnio ${dealt}, ${tail}`,
    `obrażenia w tej sesji: średnio ${n(damage.current)}, ${tail}`,
    `średnio {~zada} ${dealt}, ${tail}`,
  ];
}

/** Rust keeps only a tank of more than half of the battles (decision 2026-11-16). */
function tankVariants(tank: Fact<"czolg">, name: string): string[] {
  if (tank.battles === tank.total) return [`cała sesja na czołgu ${name}`];
  const share = `${n(tank.battles)} z ${count(tank.total, BATTLES_GEN)}`;
  return [
    `najczęściej grany czołg: ${name}, ${share}`,
    `najwięcej bitew na czołgu ${name}: ${share}`,
    `najczęściej {~gra} czołgiem ${name}: ${share}`,
  ];
}

/**
 * The battle with the most damage (the backend's choice, unchanged). Author's decision after
 * E14: "najlepsza bitwa" in the app is the highest r of the Damast plate, so the summary calls
 * this one "najwięcej obrażeń", never "najlepsza".
 */
function bestVariants(best: Fact<"najlepsza">): string[] {
  const damage = n(best.damage);
  const where = best.mapName ? ` na mapie ${best.mapName}` : "";
  return [
    `najwięcej obrażeń w jednej bitwie sesji: ${damage}${where}`,
    `najwięcej obrażeń w jednej bitwie: ${damage}${where}`,
    `najwięcej obrażeń w bitwie tej sesji: ${damage}${where}`,
    best.mapName
      ? `najwięcej obrażeń {~zada} na mapie ${best.mapName}: ${damage}`
      : `najwięcej obrażeń w jednej bitwie {~zada}: ${damage}`,
  ];
}

function wn8Variants(wn8: Fact<"wn8">): string[] {
  const value = n(wn8.value);
  if (wn8.reference === null)
    return [
      `WN8 sesji: ${value}`,
      `WN8 w tej sesji wyniosło ${value}`,
      `wynik WN8 w tej sesji: ${value}`,
    ];
  const days = `w ${count(wn8.referenceDays, DAYS_LOC)} przed sesją`;
  if (wn8.value === 0) return [`WN8 sesji: 0; zwykle ${days} ${n(wn8.reference)}`];
  const difference = wn8.value - wn8.reference;
  const tail =
    difference === 0
      ? `, tyle samo co ${days} (${n(wn8.reference)})`
      : `, o ${n(Math.abs(difference))} ${difference > 0 ? "więcej" : "mniej"} niż ${days} (${n(wn8.reference)})`;
  return [
    `WN8 sesji: ${value}${tail}`,
    `WN8 w tej sesji wyniosło ${value}${tail}`,
    `wynik WN8 w tej sesji: ${value}${tail}`,
  ];
}

/**
 * Survival (2026-11-16): "Czołg przetrwał do końca 5 z 12 bitew, czyli 42%, …"; none survived:
 * "Czołg nie przetrwał do końca żadnej bitwy sesji; zwykle 38%."
 */
function survivalVariants(survival: Fact<"przetrwanie_odsetek">): string[] {
  const reference = survival.reference;
  if (survival.survived === 0) {
    const usual = `; zwykle ${pct(reference.percent)}`;
    return [
      `czołg nie przetrwał do końca żadnej bitwy sesji${usual}`,
      `nie {~przeży} do końca żadnej bitwy sesji${usual}`,
    ];
  }
  const tail = `${n(survival.survived)} z ${count(survival.battles, BATTLES_GEN)}, czyli ${pct(survival.percent)}, ${rateTail(reference)}`;
  return [
    `czołg przetrwał do końca ${tail}`,
    `przetrwanie do końca: ${tail}`,
    `{~przeży} do końca ${tail}`,
  ];
}

/** 3–5 sentences, one fact each; past five, trailing facts that are not important are left out. */
const MAX_SENTENCES = 5;
const MIN_SENTENCES = 3;

/** Every sentence of a session summary with all its variants (tests read every variant). */
export function sessionPlans(facts: SessionFact[], context: SessionTextContext): SentencePlan[] {
  const find = <K extends SessionFactValue["key"]>(key: K) => {
    const item = facts.find((fact) => fact.fact.key === key);
    return item ? { id: item.id, fact: item.fact as Fact<K> } : null;
  };
  const battles = find("liczba_bitew");
  const wins = find("odsetek_wygranych");
  const damage = find("obrazenia");
  const best = find("najlepsza");
  const wn8 = find("wn8");
  const survival = find("przetrwanie_odsetek");
  const tank = find("czolg");
  const tankName = tank ? context.tank(tank.fact.vehicleKey, tank.fact.tankName) : "";
  const rest = [damage, best, wn8, survival, tank].filter(Boolean).length;
  // "Wygrałeś 7 z 12 bitew" says the number of battles too, when every battle has a result.
  const joinable = battles !== null && wins !== null && wins.fact.battles === battles.fact.battles;
  const joined = joinable && 1 + rest >= MIN_SENTENCES;
  const plans: SentencePlan[] = [];
  if (joined)
    plans.push({
      slot: "battles_wins",
      facts: [battles.id, wins.id],
      variants: winsVariants(wins.fact),
    });
  else {
    if (battles)
      plans.push({ slot: "battles", facts: [battles.id], variants: battlesVariants(battles.fact) });
    if (wins) plans.push({ slot: "wins", facts: [wins.id], variants: winsVariants(wins.fact) });
  }
  if (damage)
    plans.push({ slot: "damage", facts: [damage.id], variants: damageVariants(damage.fact) });
  if (best) plans.push({ slot: "best", facts: [best.id], variants: bestVariants(best.fact) });
  if (wn8) plans.push({ slot: "wn8", facts: [wn8.id], variants: wn8Variants(wn8.fact) });
  if (survival)
    plans.push({
      slot: "survival",
      facts: [survival.id],
      variants: survivalVariants(survival.fact),
    });
  if (tank)
    plans.push({ slot: "tank", facts: [tank.id], variants: tankVariants(tank.fact, tankName) });
  const important = new Set(facts.filter((item) => item.important).map((item) => item.id));
  while (
    plans.length > MAX_SENTENCES &&
    plans[plans.length - 1].facts.every((id) => !important.has(id))
  )
    plans.pop();
  return plans.length < MIN_SENTENCES ? [] : plans;
}

/** The session summary: one variant per sentence, chosen by the seed (spec 30 §6). */
export function sessionSentences(
  recap: { seed: number; facts: SessionFact[] },
  context: SessionTextContext,
): RecapSentence[] {
  return chooseSentences(recap.seed, sessionPlans(recap.facts, context), context.address);
}

/** What a reference button says about its fact ("Fakt F2: wygrane 6 z 9 (67%)"). */
export function sessionFactLabel(fact: SessionFactValue, context: SessionTextContext): string {
  switch (fact.key) {
    case "liczba_bitew":
      return `liczba bitew ${n(fact.battles)}`;
    case "odsetek_wygranych":
      return `wygrane ${n(fact.wins)} z ${n(fact.battles)} (${pct(fact.percent)})`;
    case "obrazenia":
      return `obrażenia na czołgach z tej sesji ${fact.percent > 0 ? "+" : fact.percent < 0 ? "−" : ""}${pct(Math.abs(fact.percent))} wobec Twojej średniej ${n(fact.baseline)}`;
    case "najlepsza":
      return `najwięcej obrażeń w bitwie ${n(fact.damage)}${fact.mapName ? `, ${fact.mapName}` : ""}`;
    case "wn8":
      return `WN8 sesji ${n(fact.value)}`;
    case "przetrwanie_odsetek":
      return `przetrwanie do końca ${n(fact.survived)} z ${count(fact.battles, BATTLES_GEN)} (${pct(fact.percent)})`;
    case "czolg":
      return `najczęściej grany czołg ${context.tank(fact.vehicleKey, fact.tankName)}: ${n(fact.battles)} z ${count(fact.total, BATTLES_GEN)}`;
  }
}

/** Element ids on the Session screen that show a fact's number; the rest get a tooltip. */
export const SESSION_TARGETS: Partial<Record<SessionFactValue["key"], string>> = {
  liczba_bitew: "recap-target-battles",
  odsetek_wygranych: "recap-target-wins",
  obrazenia: "recap-target-damage",
  najlepsza: "recap-target-best",
  wn8: "recap-target-wn8",
};
