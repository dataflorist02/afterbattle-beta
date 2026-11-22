/**
 * Test data of Damast (spec 34 §5): the synthetic sessions of the mockup
 * (`docs/design/damast/sesje-syntetyczne.json`) in the shape the backend sends.
 * Synthetic only: no names, no ids of real players or battles.
 */
import type { DamastSession } from "../../types";

/** One session of `sesje-syntetyczne.json`: damage, tier, WN8 and win per battle. */
export interface SyntheticSession {
  avgWN8: number;
  avgDmg: Record<string, number>;
  b: { d: number; t: number; wn8: number; w: number }[];
}

const MINUTE = 60_000;

/**
 * The backend's shape of a synthetic session; every tier of `avgDmg` has `tierBattles` battles
 * in the reference window (20 by default: above the fallback limit of 10).
 */
export function fromSynthetic(
  id: string,
  source: SyntheticSession,
  { startedAt = Date.UTC(2026, 10, 21, 18), tierBattles = 20 } = {},
): DamastSession {
  const battles = source.b.map((battle, index) => ({
    battleId: `${id}-battle-${index + 1}`,
    battleTime: startedAt + index * 8 * MINUTE,
    tier: battle.t,
    damage: battle.d,
    wn8: battle.wn8,
    win: battle.w === 1,
  }));
  const tiers = Object.entries(source.avgDmg).map(([tier, averageDamage]) => ({
    tier: Number(tier),
    battles: tierBattles,
    averageDamage,
  }));
  const allTiers = tiers.reduce((sum, tier) => sum + tier.averageDamage, 0) / tiers.length;
  const lastBattle = battles[battles.length - 1]?.battleTime ?? startedAt;
  return {
    session: {
      id,
      startedAt,
      lastBattleAt: lastBattle,
      endedAt: lastBattle + 60 * MINUTE,
      status: "timed_out",
      battleCount: battles.length,
      analyzedBattleCount: battles.length,
    },
    wn8: Math.round(battles.reduce((sum, battle) => sum + (battle.wn8 ?? 0), 0) / battles.length),
    averageDamage: battles.reduce((sum, battle) => sum + (battle.damage ?? 0), 0) / battles.length,
    battles,
    reference: {
      cutoff: startedAt,
      wn8: source.avgWN8,
      wn8Battles: 100,
      averageDamage: allTiers,
      damageBattles: 100,
      tiers,
    },
  };
}
