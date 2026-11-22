/**
 * "Jak czytać tę płytkę" (author, 22.11): what this session's plate shows, row by row, from the
 * same numbers the plate is drawn from (stats.ts). Where an average or WN8 is missing, the row
 * says so plainly; nothing is made up.
 */
import type { UiKey } from "../../i18n/ui";
import { USUAL_PERCENT } from "../../lib/ember";
import type { DamastSession, Language } from "../../types";
import { battlesLabel, formatAverageTier } from "./card";
import { MIN_BEST_BATTLES, type DamastStats, type PlateBattle } from "./stats";

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

export type GuideKind = "order" | "tint" | "eyes" | "tiers" | "waves" | "spark";
export interface GuideRow {
  kind: GuideKind;
  lines: string[];
}

const positive = (value: number | null | undefined) =>
  value !== null && value !== undefined && Number.isFinite(value) && value > 0;

/** Warmer or cooler than usual: the same ±5% as every temperature (spec 33 §2). */
const warm = (battle: PlateBattle) => (battle.r - 1) * 100 >= USUAL_PERCENT;
const cool = (battle: PlateBattle) => (battle.r - 1) * 100 <= -USUAL_PERCENT;

/**
 * The run of neighbouring battles on the same side of usual around the extreme one (the highest
 * r for warm, the lowest for cool), as 1-based numbers; null when no battle is on that side.
 */
export function extremeRun(
  plate: readonly PlateBattle[],
  side: "warm" | "cool",
): { from: number; to: number } | null {
  const on = side === "warm" ? warm : cool;
  let extreme = -1;
  plate.forEach((battle, index) => {
    if (!on(battle)) return;
    if (
      extreme < 0 ||
      (side === "warm" ? battle.r > plate[extreme].r : battle.r < plate[extreme].r)
    )
      extreme = index;
  });
  if (extreme < 0) return null;
  let from = extreme;
  let to = extreme;
  while (from > 0 && on(plate[from - 1])) from--;
  while (to < plate.length - 1 && on(plate[to + 1])) to++;
  return { from: from + 1, to: to + 1 };
}

export function plateGuide(
  damast: DamastSession,
  stats: DamastStats,
  mapOf: (battleId: string) => string | null,
  language: Language,
  tr: Translate,
): GuideRow[] {
  const { plate, reference } = { plate: stats.plate, reference: damast.reference };
  const ownWn8 = positive(reference.wn8);
  const ownDamage = positive(reference.averageDamage);
  const withWn8 = ownWn8 && plate.some((battle) => battle.wn8 !== null);
  const withDamage = ownDamage && plate.some((battle) => battle.damage !== null);
  const battles = (run: { from: number; to: number }) =>
    run.from === run.to
      ? tr("damastInfoBattleOne", { number: run.from })
      : tr("damastInfoBattleRange", { from: run.from, to: run.to });

  const tint: string[] = [];
  if (!withWn8 && !withDamage) tint.push(tr("damastInfoNoReference"));
  else {
    const hot = extremeRun(plate, "warm");
    const cold = extremeRun(plate, "cool");
    if (hot && cold)
      tint.push(tr("damastInfoWarmCold", { warm: battles(hot), cold: battles(cold) }));
    else if (hot) tint.push(tr("damastInfoWarmOnly", { warm: battles(hot) }));
    else if (cold) tint.push(tr("damastInfoColdOnly", { cold: battles(cold) }));
    else tint.push(tr("damastInfoAllUsual"));
    tint.push(
      tr(
        withWn8 && withDamage
          ? "damastInfoTintBoth"
          : withWn8
            ? "damastInfoTintNoDamage"
            : "damastInfoTintNoWn8",
      ),
    );
  }

  const best = stats.best >= 0 ? plate[stats.best] : null;
  const map = best ? mapOf(best.battleId) : null;
  const spark = best
    ? map
      ? tr("damastInfoSpark", { number: stats.best + 1, map })
      : tr("damastInfoSparkNoMap", { number: stats.best + 1 })
    : !withWn8 && !withDamage
      ? tr("damastInfoSparkNoReference")
      : tr("damastInfoSparkFew", { min: MIN_BEST_BATTLES });

  return [
    {
      kind: "order",
      lines: [tr("damastInfoOrder", { battles: battlesLabel(stats.count, language, tr) })],
    },
    { kind: "tint", lines: tint },
    { kind: "eyes", lines: [tr("damastInfoEyes", { wins: stats.wins })] },
    {
      kind: "tiers",
      lines: [
        stats.averageTier === null
          ? tr("damastInfoTiersUnknown")
          : tr("damastInfoTiers", { tier: formatAverageTier(stats.averageTier, language) }),
      ],
    },
    {
      kind: "waves",
      lines: [tr(withDamage ? "damastInfoWaves" : "damastInfoWavesFlat")],
    },
    { kind: "spark", lines: [spark] },
  ];
}
