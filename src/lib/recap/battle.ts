/**
 * Battle summary in Polish (spec 30 §4.1, §5, §6): result with map, tank and survival; damage;
 * then the remaining kept facts in the order Rust gives them, one fact per sentence (2026-11-16).
 * Every number goes through `format.ts`.
 */
import type { BattleFact, BattleFactValue } from "../../types";
import { chooseSentences, type RecapSentence, type SentencePlan } from "./compose";
import { DAMAGE, count, n, pct, type RecapTextContext } from "./common";
import type { Forms } from "./plural";

type Fact<K extends BattleFactValue["key"]> = Extract<BattleFactValue, { key: K }>;
type Kept<K extends BattleFactValue["key"]> = { id: string; fact: Fact<K> };

const VEHICLES: Forms = ["pojazd", "pojazdy", "pojazdów"];
const SHOTS: Forms = ["strzał", "strzały", "strzałów"];
/** "z 1 strzału / z 2 strzałów" */
const SHOTS_GEN: Forms = ["strzału", "strzałów", "strzałów"];
/** Past tense after a number of shots: "1 trafił", "2 trafiły", "5 trafiło". */
const HIT: Forms = ["trafił", "trafiły", "trafiło"];
const PIERCED: Forms = ["przebił", "przebiły", "przebiło"];
const PLACES = ["", "pierwsze", "drugie", "trzecie"];

/** 3–5 sentences, one fact each; past five, trailing facts that are not important are left out. */
const MIN_SENTENCES = 3;
const MAX_SENTENCES = 5;

/**
 * The reference of a battle number, always "zwykle na tym czołgu" (2026-11-16):
 * ", o 69 więcej niż zwykle na tym czołgu (1 959)".
 */
function reference(value: number, average: number | null): string {
  if (average === null) return "";
  const difference = value - average;
  if (difference === 0) return `, tyle samo co zwykle na tym czołgu (${n(average)})`;
  const direction = difference > 0 ? "więcej" : "mniej";
  return `, o ${n(Math.abs(difference))} ${direction} niż zwykle na tym czołgu (${n(average)})`;
}

/** A zero with a reference (2026-11-16): "; zwykle na tym czołgu 2 900", no difference. */
const usualZero = (average: number | null) =>
  average === null ? "" : `; zwykle na tym czołgu ${n(average)}`;

/**
 * The only opening (2026-11-16): the result first, then the map, the tank and its state —
 * "Przegrana na Test Valley, czołg Synth 44 M zniszczony.", "Remis na Port Delta, czołg Kolos IV."
 */
function opening(
  outcome: Kept<"wynik"> | null,
  map: Kept<"mapa"> | null,
  tank: string | null,
  survival: Kept<"przetrwanie"> | null,
): string[] {
  const result = outcome
    ? (({ victory: "wygrana", defeat: "przegrana", draw: "remis" } as Record<string, string>)[
        outcome.fact.outcome
      ] ?? "bitwa")
    : "bitwa";
  const on = map ? ` na ${map.fact.mapName}` : "";
  const state =
    survival === null ? "" : survival.fact.survived ? " przetrwał do końca" : " zniszczony";
  const named = tank ? ` ${tank}` : "";
  const tankPart = tank || survival ? `, czołg${named}${state}` : "";
  return [`${result}${on}${tankPart}`];
}

function damageVariants({ fact }: Kept<"obrazenia">): string[] {
  // "0 obrażeń; zwykle na tym czołgu 2 900." exactly (decision 2026-11-16).
  if (fact.damage === 0)
    return fact.reference === null
      ? [`przeciwnicy otrzymali ${count(0, DAMAGE)}`, `obrażenia zadane przeciwnikom: 0`]
      : [`${count(0, DAMAGE)}${usualZero(fact.reference)}`];
  const dealt = count(fact.damage, DAMAGE);
  const tail = reference(fact.damage, fact.reference);
  return [
    `obrażenia zadane przeciwnikom: ${n(fact.damage)}${tail}`,
    `przeciwnicy otrzymali ${dealt}${tail}`,
    `zadane obrażenia: ${n(fact.damage)}${tail}`,
    `{~zada} przeciwnikom ${dealt}${tail}`,
  ];
}

const placeText = (place: number) => `${PLACES[place]} miejsce w drużynie pod względem obrażeń`;

function destroyedVariants({ fact }: Kept<"zniszczone">): string[] {
  return [
    `zniszczone pojazdy przeciwnika: ${n(fact.frags)}`,
    `liczba zniszczonych pojazdów przeciwnika: ${n(fact.frags)}`,
    `pojazdy przeciwnika zniszczone w tej bitwie: ${n(fact.frags)}`,
    `{~zniszczy} ${count(fact.frags, VEHICLES)} przeciwnika`,
  ];
}

function placeVariants({ fact }: Kept<"miejsce">): string[] {
  return [placeText(fact.place), `pod względem obrażeń ${PLACES[fact.place]} miejsce w drużynie`];
}

function assistVariants({ fact }: Kept<"asysta">): string[] {
  if (fact.assist === 0)
    return [
      `${count(0, DAMAGE)} sojuszników dzięki Twojemu wykrywaniu i zrywaniu gąsienic${usualZero(fact.reference)}`,
    ];
  const dealt = count(fact.assist, DAMAGE);
  const tail = reference(fact.assist, fact.reference);
  return [
    // "Po Twoim wykryciu" read as being spotted by the enemy (decision 2026-11-16).
    `dzięki Twojemu wykrywaniu i zrywaniu gąsienic sojusznicy zadali ${dealt}${tail}`,
    `dzięki temu, że {~wykrywa} i {~zrywa} gąsienice, sojusznicy zadali ${dealt}${tail}`,
  ];
}

function blockedVariants({ fact }: Kept<"pancerz">): string[] {
  return [
    `pancerz czołgu zatrzymał ${count(fact.blocked, DAMAGE)} od przeciwników`,
    `zatrzymane przez pancerz obrażenia od przeciwników: ${n(fact.blocked)}`,
    `obrażenia od przeciwników zatrzymane przez pancerz: ${n(fact.blocked)}`,
  ];
}

function spottedVariants({ fact }: Kept<"wykryte">): string[] {
  if (fact.spots === 0)
    return ["wykryte pojazdy przeciwnika: 0", "nie {~wykry} żadnego pojazdu przeciwnika"];
  return [
    `wykryte pojazdy przeciwnika: ${n(fact.spots)}`,
    `{~wykry} ${count(fact.spots, VEHICLES)} przeciwnika`,
  ];
}

function shareVariants({ fact }: Kept<"udzial">): string[] {
  return [
    `udział w obrażeniach drużyny: ${pct(fact.percent)}`,
    `{~zada} ${pct(fact.percent)} wszystkich obrażeń swojej drużyny`,
  ];
}

function shotsVariants({ fact }: Kept<"strzaly">): string[] {
  const part = (value: number, forms: Forms, none: string) =>
    value === 0 ? none : count(value, forms);
  const hits = part(fact.hits, HIT, "żaden nie trafił");
  const pierced = part(fact.penetrations, PIERCED, "żaden nie przebił pancerza");
  const piercedTail = fact.penetrations === 0 ? pierced : `${pierced} pancerz`;
  return [
    `strzały: ${n(fact.shots)}, trafienia w przeciwników: ${n(fact.hits)}, przebicia: ${n(fact.penetrations)}`,
    `trafienia w przeciwników: ${n(fact.hits)} z ${count(fact.shots, SHOTS_GEN)}, przebicia: ${n(fact.penetrations)}`,
    `{~odda} ${count(fact.shots, SHOTS)}: ${hits} w przeciwników, ${piercedTail}`,
  ];
}

/** Every sentence of a battle summary with all its variants (tests read every variant). */
export function battlePlans(facts: BattleFact[], context: RecapTextContext): SentencePlan[] {
  const find = <K extends BattleFactValue["key"]>(key: K): Kept<K> | null => {
    const item = facts.find((fact) => fact.fact.key === key);
    return item ? { id: item.id, fact: item.fact as Fact<K> } : null;
  };
  const outcome = find("wynik");
  const map = find("mapa");
  const tank = find("czolg");
  const survival = find("przetrwanie");
  const damage = find("obrazenia");
  const tankName = tank ? context.tank(tank.fact.vehicleKey, tank.fact.tankName) : null;
  const opened = [outcome, map, tank].filter((item) => item !== null);
  const rest: SentencePlan[] = [];
  const own = (slot: string, id: string, variants: string[]) =>
    rest.push({ slot, facts: [id], variants });
  for (const item of facts) {
    const kept = { id: item.id, fact: item.fact };
    switch (item.fact.key) {
      case "zniszczone":
        own("destroyed", item.id, destroyedVariants(kept as Kept<"zniszczone">));
        break;
      case "miejsce":
        own("place", item.id, placeVariants(kept as Kept<"miejsce">));
        break;
      case "asysta":
        own("assist", item.id, assistVariants(kept as Kept<"asysta">));
        break;
      case "pancerz":
        own("blocked", item.id, blockedVariants(kept as Kept<"pancerz">));
        break;
      case "wykryte":
        own("spotted", item.id, spottedVariants(kept as Kept<"wykryte">));
        break;
      case "udzial":
        own("share", item.id, shareVariants(kept as Kept<"udzial">));
        break;
      case "strzaly":
        own("shots", item.id, shotsVariants(kept as Kept<"strzaly">));
        break;
      default:
        break;
    }
  }
  const restCount = rest.length + (damage ? 1 : 0);
  // Fewer than 3 sentences: survival gets its own sentence.
  const splitSurvival = survival !== null && opened.length > 0 && 1 + restCount < MIN_SENTENCES;
  const plans: SentencePlan[] = [];
  if (opened.length > 0 || survival) {
    const withSurvival = splitSurvival ? null : survival;
    plans.push({
      slot: "opening",
      facts: [...opened, withSurvival].filter((item) => item !== null).map((item) => item.id),
      variants: opening(outcome, map, tankName, withSurvival),
    });
  }
  if (splitSurvival && survival)
    plans.push({
      slot: "survival",
      facts: [survival.id],
      variants: survival.fact.survived
        ? ["czołg przetrwał do końca bitwy", "{~przeży} do końca bitwy"]
        : [
            "czołg został zniszczony przed końcem bitwy",
            "przed końcem bitwy czołg został zniszczony",
          ],
    });
  if (damage) plans.push({ slot: "damage", facts: [damage.id], variants: damageVariants(damage) });
  plans.push(...rest);
  const important = new Set(facts.filter((item) => item.important).map((item) => item.id));
  while (
    plans.length > MAX_SENTENCES &&
    plans[plans.length - 1].facts.every((id) => !important.has(id))
  )
    plans.pop();
  return plans.length < MIN_SENTENCES ? [] : plans;
}

/** The battle summary: one variant per sentence, chosen by the seed (spec 30 §6). */
export function battleSentences(
  recap: { seed: number; facts: BattleFact[] },
  context: RecapTextContext,
): RecapSentence[] {
  return chooseSentences(recap.seed, battlePlans(recap.facts, context), context.address);
}

/** What a reference button says about its fact ("Fakt F5: obrażenia 2 028"). */
export function battleFactLabel(fact: BattleFactValue, context: RecapTextContext): string {
  switch (fact.key) {
    case "wynik":
      return `wynik: ${fact.outcome === "victory" ? "wygrana" : fact.outcome === "defeat" ? "przegrana" : "remis"}`;
    case "mapa":
      return `mapa ${fact.mapName}`;
    case "czolg":
      return `czołg ${context.tank(fact.vehicleKey, fact.tankName)}`;
    case "przetrwanie":
      return fact.survived ? "czołg przetrwał do końca" : "czołg zniszczony";
    case "obrazenia":
      return `obrażenia ${n(fact.damage)}${fact.reference === null ? "" : `, średnia na tym czołgu ${n(fact.reference)}`}`;
    case "zniszczone":
      return `zniszczone pojazdy ${n(fact.frags)}`;
    case "asysta":
      return `obrażenia sojuszników dzięki Twojemu wykrywaniu i zrywaniu gąsienic ${n(fact.assist)}`;
    case "pancerz":
      return `obrażenia zatrzymane przez pancerz ${n(fact.blocked)}`;
    case "wykryte":
      return `wykryte pojazdy ${n(fact.spots)}`;
    case "miejsce":
      return `${PLACES[fact.place]} miejsce w drużynie pod względem obrażeń`;
    case "udzial":
      return `udział w obrażeniach drużyny ${pct(fact.percent)}`;
    case "strzaly":
      return `strzały ${n(fact.shots)}, trafienia ${n(fact.hits)}, przebicia ${n(fact.penetrations)}`;
  }
}

/** Element ids on the Battle screen that show a fact's number; the rest get a tooltip. */
export const BATTLE_TARGETS: Partial<Record<BattleFactValue["key"], string>> = {
  wynik: "recap-target-headline",
  mapa: "recap-target-headline",
  czolg: "recap-target-tank",
  przetrwanie: "recap-target-survival",
  obrazenia: "recap-target-damage",
  udzial: "recap-target-damage",
  miejsce: "recap-target-damage",
  zniszczone: "recap-target-destroyed",
  asysta: "recap-target-assist",
  pancerz: "recap-target-blocked",
  wykryte: "recap-target-spotted",
  strzaly: "recap-target-hits",
};
