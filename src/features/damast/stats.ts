/**
 * Damast (spec 34 §2): each battle's result against the player's own averages, the session's
 * temperature and its best battle. The tier never raises a result by itself: damage is compared
 * with the player's own average on the same tier, and WN8 already accounts for the tank.
 */
import { heatOf, type Heat } from "../../lib/ember";
import type { DamastReference, DamastSession } from "../../types";

/** The own averages come from this many battles before the session (backend `MAX_WINDOW`). */
export const DAMAST_REFERENCE_BATTLES = 100;
/** Fewer battles on a tier in the reference window: the all-tier average is used instead. */
export const MIN_TIER_BATTLES = 10;
/** r = 0.6 × WN8 / own WN8 + 0.4 × damage / own damage on the tier. */
export const WN8_WEIGHT = 0.6;
export const DAMAGE_WEIGHT = 0.4;
/** A best battle is named from 2 battles (the backend's `MIN_BEST_BATTLES`, session_story.rs). */
export const MIN_BEST_BATTLES = 2;
/** The tier the pattern assumes when no battle of the session has one. */
const DEFAULT_TIER = 8;

/** One battle as the plate sees it, in the order it was played. */
export interface PlateBattle {
  battleId: string;
  /** The battle's result against the own averages; 1 is "as usual". */
  r: number;
  /** Damage against the own average on the tier (1 without damage): the wave's amplitude. */
  damageRatio: number;
  /** Tier for the density of the layers (the session's average tier when unknown). */
  tier: number;
  win: boolean;
  damage: number | null;
  wn8: number | null;
}

export interface DamastStats {
  plate: PlateBattle[];
  /** Mean r minus 1, in percent; null without any reference (the first session ever). */
  change: number | null;
  heat: Heat | null;
  /**
   * Index of the best battle on the plate (highest r, the earlier of equal ones); −1 without an
   * own average to compare with or with fewer than MIN_BEST_BATTLES battles.
   */
  best: number;
  wins: number;
  count: number;
  averageTier: number | null;
}

const positive = (value: number | null | undefined): value is number =>
  value !== null && value !== undefined && Number.isFinite(value) && value > 0;

/** The own average damage to compare a battle on `tier` with (spec 34 §2, "mało danych"). */
export function damageReference(tier: number | null, reference: DamastReference): number | null {
  const own = tier === null ? undefined : reference.tiers.find((item) => item.tier === tier);
  if (own && own.battles >= MIN_TIER_BATTLES && positive(own.averageDamage))
    return own.averageDamage;
  return positive(reference.averageDamage) ? reference.averageDamage : null;
}

/**
 * The temperature of a session, the one of the whole app (author's decision after the E14
 * checkpoint): Session, Overview, the History thermogram and the plate all show this one.
 */
export function damastHeat(session: DamastSession | null | undefined): Heat | null {
  return session ? damastStats(session).heat : null;
}

/**
 * The session's best battle in the whole app (author's decision after E14): the highest r — the
 * spark under the plate, the "najlepsza" tile, the Session card's map, the moment, the Overview
 * card and the Battle screen's hot spot. Null when there is none (see `DamastStats.best`).
 */
export function damastBestBattleId(session: DamastSession | null | undefined): string | null {
  if (!session) return null;
  const stats = damastStats(session);
  return stats.best >= 0 ? stats.plate[stats.best].battleId : null;
}

export function damastStats(session: DamastSession): DamastStats {
  const reference = session.reference;
  const ownWn8 = positive(reference.wn8) ? reference.wn8 : null;
  const noReference = ownWn8 === null && !positive(reference.averageDamage);
  const known = session.battles.filter((battle) => battle.damage !== null || battle.wn8 !== null);
  const tiers = known.map((battle) => battle.tier).filter((tier): tier is number => tier !== null);
  const averageTier =
    tiers.length > 0 ? tiers.reduce((sum, tier) => sum + tier, 0) / tiers.length : null;

  const plate: PlateBattle[] = [];
  for (const battle of known) {
    const ownDamage = damageReference(battle.tier, reference);
    const wn8Part = battle.wn8 !== null && ownWn8 !== null ? battle.wn8 / ownWn8 : null;
    const damagePart =
      battle.damage !== null && ownDamage !== null ? battle.damage / ownDamage : null;
    let r: number;
    if (noReference) r = 1;
    else if (wn8Part !== null && damagePart !== null)
      r = WN8_WEIGHT * wn8Part + DAMAGE_WEIGHT * damagePart;
    else if (wn8Part !== null) r = wn8Part;
    else if (damagePart !== null) r = damagePart;
    else continue;
    plate.push({
      battleId: battle.battleId,
      r,
      damageRatio: damagePart ?? 1,
      tier: battle.tier ?? averageTier ?? DEFAULT_TIER,
      win: battle.win,
      damage: battle.damage,
      wn8: battle.wn8,
    });
  }

  let best = -1;
  if (!noReference && plate.length >= MIN_BEST_BATTLES)
    plate.forEach((battle, index) => {
      if (best < 0 || battle.r > plate[best].r) best = index;
    });
  const change =
    noReference || plate.length === 0
      ? null
      : (plate.reduce((sum, battle) => sum + battle.r, 0) / plate.length - 1) * 100;
  return {
    plate,
    change,
    // The same rounding as `heatAgainst`, so a mean of exactly 1.05 is exactly +5% (E13 rule).
    heat: change === null ? null : heatOf(Math.round(change * 1e6) / 1e6),
    best,
    wins: plate.filter((battle) => battle.win).length,
    count: plate.length,
    averageTier,
  };
}
