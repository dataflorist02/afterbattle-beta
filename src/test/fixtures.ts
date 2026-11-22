/**
 * Synthetic data for screen tests and screenshots (no real players, battles or game files).
 * Objects are cast from partial shapes; only the fields the screens read are filled.
 */
import type {
  AppSnapshot,
  ReferenceCell,
  ReferenceMetric,
  ReferenceMetricKind,
  ReferenceTables,
  TankPosition,
  Battle,
  BattleDetailView,
  BattleHistoryPage,
  ChallengePresentation,
  DamastSession,
  DamastSessionPage,
  MapRecord,
  PerformanceTrendReport,
  Session,
  SessionBaselineComparison,
  SessionStory,
  Recap,
  SessionSummary,
  VehicleBaselinePresentation,
  VehicleHistoryOverview,
  VehicleHistoryReport,
  VehicleTemperature,
  WeeklyReport,
} from "../types";

const HOUR = 3_600_000;
export const NOW = Date.UTC(2026, 10, 12, 19, 40);

export function session(overrides: Partial<Session> = {}): Session {
  return {
    id: "session-1",
    startedAt: NOW,
    lastBattleAt: NOW + HOUR,
    endedAt: null,
    status: "active",
    battleCount: 9,
    analyzedBattleCount: 9,
    ...overrides,
  };
}

export function battle(index: number, overrides: Partial<Battle> = {}): Battle {
  return {
    arenaId: `battle-${index}`,
    arenaUniqueId: null,
    battleTime: NOW + index * 600_000,
    battleTimeSource: "metadata",
    vehicleKey: index % 2 ? "synth-s01_alpha" : "synth-s02_bravo",
    wgTankId: index % 2 ? 101 : 102,
    tankName: index % 2 ? "Alpha" : "Bravo",
    mapName: index % 3 ? "Synthetic Hills" : "Test Valley",
    mapKey: index % 3 ? "90_synthetic_hills" : "91_test_valley",
    battleType: "random",
    gameplayId: "ctf",
    sourceKind: "replay",
    dataQuality: "full",
    parserVersion: 9,
    featureSchemaVersion: 3,
    gameVersion: null,
    dataCompleteness: "results",
    replaySource: "replay_results",
    parserErrorCode: null,
    outcome: index % 3 === 1 ? "defeat" : "victory",
    battlePace: {} as Battle["battlePace"],
    playerTeam: 1,
    winnerTeamRaw: 1,
    damage: 2000 + index * 310,
    assist: 300 + index * 40,
    assistSpotting: 200,
    assistTracking: 100,
    assistStun: 0,
    blocked: 1870,
    damageReceived: 900,
    frags: 2,
    spots: 3,
    shots: 18,
    hits: 15,
    penetrations: 13,
    directEnemyHits: 15,
    piercingEnemyHits: 13,
    defensePoints: 0,
    movingAvgDamage: null,
    damageRating: null,
    survived: index % 2 === 0,
    xp: 1200,
    originalXp: 1100,
    durationSeconds: 588,
    sessionId: "session-1",
    ...overrides,
  } as Battle;
}

export function snapshot(overrides: Partial<AppSnapshot> = {}): AppSnapshot {
  return {
    configured: true,
    onboardingComplete: true,
    inactivityMinutes: 60,
    sourceStatus: {
      configured: true,
      active: true,
      paused: false,
      gameRootDisplay: "C:\\Games\\SyntheticGame",
      lastReplayAt: NOW,
      parserState: "ready",
      clientVersion: "test",
      partialMode: false,
      message: null,
      importProgress: null,
    },
    advancedStatus: { xvmEnabled: true, xvmLastUpdatedAt: NOW },
    activeSession: session(),
    recentBattles: [],
    recentSessions: [
      session(),
      session({ id: "session-0", status: "ended", startedAt: NOW - 24 * HOUR }),
    ],
    wn8Presentation: {
      session: null,
      battles: [],
      expectedValues: {
        availability: "available",
        source: "xvm_wargaming",
        sourceVersion: "2026-11-11",
        sha256: null,
        downloadedAt: NOW,
        vehicleCount: 1000,
        skippedRowCount: 0,
        lastAttemptAt: NOW,
        lastSuccessAt: NOW,
        lastErrorCode: null,
        updateInProgress: false,
      },
    },
    activeChallenge: null,
    latestFinishedChallenge: null,
    ...overrides,
  } as AppSnapshot;
}

export function summary(battles = 9, averageDamage: number | null = 3184): SessionSummary {
  return {
    session: session({ battleCount: battles, analyzedBattleCount: battles }),
    metrics: {
      battles,
      analyzedBattles: battles,
      wins: Math.round(battles * 0.66),
      winRate: 66,
      outcomeSampleSize: battles,
      averageDamage,
      damageSampleSize: battles,
      averageAssist: 612,
      assistSampleSize: battles,
      averageBlocked: null,
      averageFrags: null,
      averageXp: null,
      xpSampleSize: battles,
      survivalRate: 44.4,
      survivalSampleSize: battles,
    },
    feedback: [],
    wn8: { value: 2150 } as SessionSummary["wn8"],
    battles: Array.from({ length: battles }, (_, index) => ({
      battle: battle(index),
      feedback: [],
    })),
    battleWn8: [],
    battlePace: {
      totalBattles: battles,
      obviousSteamrolls: 1,
      borderlineSteamrolls: 1,
      standardBattles: battles - 2,
      unknownBattles: 0,
      classifiedBattles: battles,
      coverageRatio: 1,
    },
  };
}

/**
 * Damast (spec 34) of an ended session: the nine battles of `summary()`, tiers VIII–X, against
 * own averages (WN8 2 000; damage 2 400 on VIII, 2 900 on IX, 3 000 on X). Damage is 1.18 times
 * the tier's average and WN8 2 360 ± offsets that add up to zero, so the mean r is exactly 1.18:
 * +18% warmer (intensity 0.52, 11 sparks), the same as the damage comparison of `comparison()`.
 * The best battle is the sixth (r 1.27).
 */
export function damastSession(
  id = "session-0",
  startedAt = NOW - 24 * HOUR,
  overrides: Partial<DamastSession> = {},
): DamastSession {
  const offsets = [-120, 150, -120, 150, -120, 300, -120, 150, -270];
  const tierDamage = [2_832, 3_422, 3_540];
  const battles = offsets.map((offset, index) => ({
    // The battles of `summary()`: the plate and the strip name the same battles.
    battleId: `battle-${index}`,
    battleTime: startedAt + index * 600_000,
    tier: 8 + (index % 3),
    damage: tierDamage[index % 3],
    wn8: 2_360 + offset,
    win: index % 3 !== 1,
  }));
  return {
    session: session({ id, status: "ended", startedAt, endedAt: startedAt + 3 * HOUR }),
    wn8: 2_150,
    averageDamage: 3_184,
    battles,
    reference: {
      cutoff: startedAt,
      wn8: 2_000,
      wn8Battles: 100,
      averageDamage: 2_800,
      damageBattles: 100,
      tiers: [
        { tier: 8, battles: 30, averageDamage: 2_400 },
        { tier: 9, battles: 30, averageDamage: 2_900 },
        { tier: 10, battles: 40, averageDamage: 3_000 },
      ],
    },
    ...overrides,
  };
}

/**
 * `damastSession` with its best battle (the highest r) at `index`: the numbers of the sixth
 * battle and of battle `index` swap places, the ids and times stay in order.
 */
export function damastWithBest(index: number, id = "session-0"): DamastSession {
  const plate = damastSession(id);
  const battles = plate.battles.map((battle) => ({ ...battle }));
  const numbers = ({ tier, damage, wn8, win }: (typeof battles)[number]) => ({
    tier,
    damage,
    wn8,
    win,
  });
  const best = numbers(battles[5]);
  Object.assign(battles[5], numbers(battles[index]));
  Object.assign(battles[index], best);
  return { ...plate, battles };
}

/** The same session against a higher own WN8 (3 200): mean r 0.9145, 8.55% cooler. */
export function coolerDamast(id = "session-0", startedAt = NOW - 24 * HOUR): DamastSession {
  const plate = damastSession(id, startedAt);
  return { ...plate, reference: { ...plate.reference, wn8: 3_200 } };
}

/** `count` ended sessions for History → Sessions, one day apart, newest first. */
export function damastSessions(count: number, from = 0): DamastSession[] {
  return Array.from({ length: count }, (_, index) => {
    const number = from + index;
    const plate = damastSession(`session-d${number}`, NOW - (number + 1) * 24 * HOUR);
    // Every session a little different, so every plate (and tile key) is its own.
    return {
      ...plate,
      wn8: 1_800 + ((number * 37) % 900),
      battles: plate.battles.map((battle, battleIndex) => ({
        ...battle,
        damage: (battle.damage ?? 0) + ((number * 13 + battleIndex * 7) % 400) - 200,
      })),
    };
  });
}

export function damastPage(items: DamastSession[], next = false): DamastSessionPage {
  const last = items[items.length - 1];
  return {
    items,
    nextCursor:
      next && last ? { startedAt: last.session.startedAt, sessionId: last.session.id } : null,
  };
}

export function comparison(baselineBattles = 100): SessionBaselineComparison {
  const vehicle = (vehicleId: number, tankName: string, baseline: number) => ({
    vehicleId,
    vehicleKey: null,
    tankName,
    sessionBattles: 4,
    sessionEligibleBattles: 4,
    baseline: {
      averageDamage: {
        value: baseline,
        eligibleBattles: baselineBattles,
        candidateBattles: baselineBattles,
      },
    },
    damage: {
      currentValue: 3184,
      baselineValue: baseline,
      absoluteDelta: null,
      relativeDeltaPercent: null,
      comparisonState: "above",
    },
  });
  return {
    vehicles: [vehicle(101, "Alpha", 2702), vehicle(102, "Bravo", 2702)],
    insights: {
      version: "v1",
      state: "available",
      unavailableReason: null,
      summaries: [
        {
          vehicleId: 101,
          tankName: "Alpha",
          strongestPositive: { metric: "damage", direction: "positive", deltaValue: 31 },
          strongestNegative: { metric: "survival", direction: "negative", deltaValue: -12 },
          supportingSignals: [],
        },
      ],
    },
  } as unknown as SessionBaselineComparison;
}

export function battleDetail(overrides: Partial<BattleDetailView> = {}): BattleDetailView {
  return {
    view: { battle: battle(4, { damage: 5210, outcome: "victory", survived: true }), feedback: [] },
    teams: {
      own: {
        damage: 21700,
        spotted: 18,
        assist: 8900,
        kills: 15,
        survivors: 9,
        tierMin: 10,
        tierMax: 10,
        platoons: 1,
      },
      enemy: {
        damage: 14400,
        spotted: 11,
        assist: 7300,
        kills: 6,
        survivors: 0,
        tierMin: 10,
        tierMax: 10,
        platoons: 1,
      },
    },
    healthRemaining: 312,
    maxHealth: 1950,
    mostDamageInTeam: true,
    ...overrides,
  };
}

export function overview(count = 3, tier: number | null = 10): VehicleHistoryOverview {
  return {
    unidentifiedBattleCount: 0,
    vehicles: Array.from({ length: count }, (_, index) => ({
      vehicleId: 101 + index,
      vehicleKey: `synth-s0${index + 1}_tank`,
      tankName: ["Alpha", "Bravo", "Charlie"][index] ?? `Tank ${index}`,
      vehicleClass: { vehicleClass: "medium_tank" },
      tier,
      totalBattles: 214 - index * 40,
      sessionCount: 10,
      firstBattleAt: NOW - 100 * HOUR,
      lastBattleAt: NOW,
    })),
  } as unknown as VehicleHistoryOverview;
}

/** One dot of the tank list (`get_vehicle_temperatures`): last 10 against last 100 damage. */
export function vehicleTemperature(
  vehicleId: number,
  recent: number,
  battles = 100,
  average = 2702,
): VehicleTemperature {
  const metric = (value: number, eligible: number) => ({
    value,
    eligibleBattles: eligible,
    candidateBattles: eligible,
  });
  return {
    vehicleId,
    recentDamage: metric(recent, Math.min(10, battles)),
    averageDamage: metric(average, battles),
    averageBattles: battles,
  };
}

export function vehicleBaseline(eligible = 100): VehicleBaselinePresentation {
  const metric = (value: number) => ({
    value,
    eligibleBattles: eligible,
    candidateBattles: eligible,
  });
  return {
    windows: [
      {
        window: "last10",
        eligibleBattles: Math.min(10, eligible),
        averageDamage: metric(2940),
        averageAssist: metric(520),
        survivalRate: metric(31),
      },
      {
        window: "last100",
        eligibleBattles: eligible,
        averageDamage: metric(2702),
        averageAssist: metric(610),
        survivalRate: metric(34),
      },
    ],
  } as unknown as VehicleBaselinePresentation;
}

export function vehicleReport(): VehicleHistoryReport {
  return {
    summary: overview(1).vehicles[0],
    sessions: [],
    maps: [
      {
        mapKey: "90_synthetic_hills",
        mapName: "Synthetic Hills",
        battles: 31,
        averageDamage: 3480,
        survivalRate: 41,
      },
      {
        mapKey: "91_test_valley",
        mapName: "Test Valley",
        battles: 24,
        averageDamage: 2150,
        survivalRate: 18,
      },
    ],
  };
}

export function trends(points = 120): PerformanceTrendReport {
  return {
    metrics: [
      {
        metric: "damage",
        points: Array.from({ length: points }, (_, index) => ({
          ordinal: index,
          battleTime: NOW + index,
          value: 2500 + Math.sin(index / 6) * 400,
          sampleSize: 10,
          coverage: 1,
          vehicleId: 101,
          // The battle under the point (the chart's reading, E14 polish 2).
          damage: 1_800 + ((index * 337) % 2_400),
          mapName: index % 3 ? "Synthetic Hills" : "Test Valley",
          vehicleKey: "synth-s01_tank",
          tankName: "Alpha",
        })),
      },
    ],
  } as unknown as PerformanceTrendReport;
}

export function historyPage(count = 12): BattleHistoryPage {
  return {
    items: Array.from({ length: count }, (_, index) => {
      const value = battle(index);
      return {
        battleId: value.arenaId,
        sessionId: value.sessionId,
        sessionStartedAt: NOW,
        sessionStatus: "active",
        vehicleId: value.wgTankId,
        vehicleKey: value.vehicleKey,
        tankName: value.tankName,
        mapName: value.mapName,
        mapKey: value.mapKey,
        battleTime: value.battleTime,
        outcome: value.outcome,
        battlePace: value.battlePace,
        durationSeconds: value.durationSeconds,
        damage: value.damage,
        assist: value.assist,
        frags: value.frags,
        xp: value.xp,
        survived: value.survived,
        dataQuality: value.dataQuality,
        wn8: {} as never,
      };
    }),
    nextCursor: null,
  };
}

export function activeChallenge(): ChallengePresentation {
  return {
    id: "goal-1",
    status: "active",
    challengeType: "damage_consistency",
    vehicleId: 101,
    vehicleName: "Alpha",
    targetBattles: 5,
    requiredSuccesses: 3,
    frozenBaselineValue: 2702,
    thresholdValue: 1900,
    contributionFloor: null,
    completedBattles: 2,
    successfulBattles: 1,
    battles: [],
  } as unknown as ChallengePresentation;
}

const COMMON_CODES = [
  "partial_replay",
  "results_provenance_missing",
  "parser_too_old",
  "unsupported_schema",
  "unsupported_mode",
  "invalid_team_structure",
  "vehicle_class_catalog_unavailable",
  "vehicle_class_unknown",
  "vehicle_class_conflict",
  "unsupported_vehicle_mechanics",
];
const METRIC_CODES = [
  ...COMMON_CODES,
  "missing_counters",
  "counter_out_of_range",
  "counter_conflict",
];
const zeroReasons = (codes: string[]) => codes.map((code) => ({ code, count: 0 }));

/** A wire-format attempts and conversion report (contract v2), every battle eligible. */
export function attemptsReport(
  battles = 9,
  sums = { shots: 57, hits: 41, penetrations: 32, explosions: 2, damage: 28_656 },
): unknown {
  const rate = (numeratorSum: number, denominatorSum: number) => ({
    selectedBattles: battles,
    commonEligibleBattles: battles,
    metricEligibleBattles: battles,
    metricIneligibleBattles: 0,
    contributingBattles: battles,
    zeroDenominatorBattles: 0,
    missingBattles: 0,
    invalidBattles: 0,
    numeratorSum,
    denominatorSum,
    value: denominatorSum > 0 ? numeratorSum / denominatorSum : null,
    rejectionReasons: zeroReasons(METRIC_CODES),
  });
  const fact = (valueSum: number) => ({
    selectedBattles: battles,
    commonEligibleBattles: battles,
    factEligibleBattles: battles,
    factIneligibleBattles: 0,
    contributingBattles: battles,
    missingBattles: 0,
    invalidBattles: 0,
    valueSum,
    rejectionReasons: zeroReasons(METRIC_CODES),
  });
  return {
    contractVersion: "attempts-conversion-v2",
    scope: { type: "recent", window: 10 },
    selectedBattles: battles,
    commonContext: {
      eligibleBattles: battles,
      ineligibleBattles: 0,
      rejectionReasons: zeroReasons(COMMON_CODES),
    },
    hitRate: rate(sums.hits, sums.shots),
    penetrationRate: rate(sums.penetrations, sums.hits),
    facts: {
      directTeamHits: fact(0),
      explosionHits: fact(sums.explosions),
      damageDealt: fact(sums.damage),
    },
  };
}

/** A session story as `get_session_story` returns it (synthetic). */
export function sessionStory(overrides: Partial<SessionStory> = {}): SessionStory {
  const best = battle(8);
  return {
    headline: { kind: "strong", percent: 18, battles: 9 },
    bestBattle: {
      battleId: best.arenaId,
      damage: best.damage ?? 0,
      mapKey: best.mapKey,
      mapName: best.mapName,
      vehicleKey: best.vehicleKey,
      tankName: best.tankName,
    },
    moments: [
      {
        kind: "damage_trend",
        rising: true,
        from: 2_000,
        to: 4_480,
        battlesPerThird: 3,
        everyBattle: true,
      },
      {
        kind: "weakest_tank",
        vehicleId: 101,
        vehicleKey: "synth-s01_alpha",
        tankName: "Alpha",
        metric: "survival",
        current: 0,
        baseline: 33,
        survived: 0,
        battles: 3,
        mapKey: "91_test_valley",
        mapName: "Test Valley",
      },
    ],
    ...overrides,
  };
}

/** Battle summary facts as `get_recap` returns them (synthetic, matching `battleDetail()`). */
export function battleRecap(): Recap {
  const own = battle(4);
  return {
    kind: "battle",
    seed: 11,
    facts: [
      { id: "F1", important: true, fact: { key: "wynik", outcome: "victory" } },
      { id: "F2", important: false, fact: { key: "mapa", mapName: own.mapName ?? "Test Valley" } },
      {
        id: "F3",
        important: false,
        fact: { key: "czolg", vehicleKey: own.vehicleKey, tankName: own.tankName, tier: 8 },
      },
      { id: "F4", important: true, fact: { key: "przetrwanie", survived: true } },
      { id: "F5", important: true, fact: { key: "obrazenia", damage: 5_210, reference: 3_244 } },
      { id: "F6", important: true, fact: { key: "zniszczone", frags: 2 } },
      { id: "F7", important: true, fact: { key: "miejsce", place: 1 } },
      {
        id: "F8",
        important: false,
        fact: { key: "strzaly", shots: 18, hits: 15, penetrations: 13 },
      },
    ],
  };
}

/**
 * Week summary facts as `get_recap` returns them, the way `recap::week_recap` derives them from
 * `weeklyReport()` (indicators 104% and 101% are within 5 points, last week has no goals).
 */
export function weekRecap(battles = 39): Recap {
  if (battles < 15)
    return {
      kind: "week",
      seed: 5,
      facts: [
        {
          id: "F1",
          important: true,
          fact: { key: "liczba_bitew", battles, confidence: "too_few", minimum: 15 },
        },
      ],
    };
  return {
    kind: "week",
    seed: 5,
    facts: [
      {
        id: "F1",
        important: true,
        fact: { key: "liczba_bitew", battles, confidence: "good", minimum: 15 },
      },
      { id: "F2", important: false, fact: { key: "odsetek_wygranych", percent: 55 } },
      {
        id: "F3",
        important: true,
        fact: { key: "wskaznik", metric: "survival", percent: 92, usual: 100 },
      },
      {
        id: "F4",
        important: true,
        fact: { key: "cel", kind: "survival", state: "below", vehicleKey: null, tankName: null },
      },
      {
        id: "F5",
        important: true,
        fact: {
          key: "cel",
          kind: "tank_damage_assist",
          state: "achieved",
          vehicleKey: "synth-s01_alpha",
          tankName: "Alpha",
        },
      },
      {
        id: "F6",
        important: true,
        fact: { key: "cel", kind: "xp", state: "close", vehicleKey: null, tankName: null },
      },
      {
        id: "F7",
        important: false,
        fact: { key: "wyzwania", completed: 1, failed: 0, abandoned: 0, battles: 5 },
      },
    ],
  };
}

/** Session summary facts as `get_recap` returns them (synthetic, matching `summary()`). */
export function sessionRecap(): Recap {
  const best = battle(8);
  return {
    kind: "session",
    seed: 7,
    facts: [
      { id: "F1", important: true, fact: { key: "liczba_bitew", battles: 9 } },
      {
        id: "F2",
        important: true,
        fact: {
          key: "odsetek_wygranych",
          wins: 6,
          battles: 9,
          percent: 67,
          reference: { percent: 52, battles: 100, difference: 15 },
        },
      },
      {
        id: "F3",
        important: true,
        fact: { key: "obrazenia", current: 3_184, baseline: 2_702, percent: 18 },
      },
      {
        id: "F4",
        important: true,
        fact: {
          key: "najlepsza",
          battleId: best.arenaId,
          damage: best.damage ?? 0,
          mapKey: best.mapKey,
          mapName: best.mapName,
        },
      },
      {
        id: "F5",
        important: true,
        fact: { key: "wn8", value: 2_150, reference: 1_980, referenceDays: 30 },
      },
      {
        id: "F6",
        important: false,
        fact: {
          key: "przetrwanie_odsetek",
          survived: 3,
          battles: 9,
          percent: 33,
          reference: { percent: 45, battles: 100, difference: -12 },
        },
      },
      {
        id: "F7",
        important: false,
        fact: {
          key: "czolg",
          vehicleKey: "synth-s01_alpha",
          tankName: "Alpha",
          battles: 5,
          total: 9,
        },
      },
    ],
  };
}

export function mapRecord(overrides: Partial<MapRecord> = {}): MapRecord {
  return {
    mapKey: "92_synthetic_hills",
    battles: 31,
    wins: 19,
    outcomeBattles: 31,
    winRate: 61.3,
    averageDamage: 3_010,
    ...overrides,
  };
}

/** A weekly report as `get_weekly_report` returns it (synthetic week 9–15 November 2026). */
export function weeklyReport(overrides: Partial<WeeklyReport> = {}): WeeklyReport {
  const start = new Date(2026, 10, 9).getTime();
  return {
    weekStart: start,
    weekEnd: start + 7 * 24 * HOUR,
    battles: 39,
    confidence: "good",
    indicators: [
      { metric: "damage_assist", current: 104, previous: [97, 101, 99, 100] },
      { metric: "survival", current: 92, previous: [95, 108, null, 100] },
      { metric: "xp", current: 101, previous: [96, 103, 100, 99] },
    ],
    winRate: 55,
    goals: [
      {
        kind: "survival",
        target: 35,
        basis: 30,
        vehicleId: null,
        vehicleKey: null,
        tankName: null,
        current: 30.8,
        battles: 39,
        survived: 12,
        state: "below",
      },
      {
        kind: "tank_damage_assist",
        target: 3_300,
        basis: 3_000,
        vehicleId: 101,
        vehicleKey: "synth-s01_alpha",
        tankName: "Alpha",
        current: 3_460,
        battles: 14,
        survived: null,
        state: "achieved",
      },
      {
        kind: "xp",
        target: 1_450,
        basis: 1_320,
        vehicleId: null,
        vehicleKey: null,
        tankName: null,
        current: 1_390,
        battles: 39,
        survived: null,
        state: "close",
      },
    ],
    previousWeek: { weekStart: start - 7 * 24 * HOUR, battles: 46, goals: [] },
    challenges: { completed: 1, failed: 0, abandoned: 0 },
    wn8: { current: 2_150, previous: [2_010, 2_080, null, 1_990] },
    ...overrides,
  };
}

/** A-1 (spec 32 §2): one metric of a reference cell (synthetic numbers). */
/**
 * Different numbers for every metric (author's fixup after STOP 2 preparation, 2026-11-18): with
 * one set for all, a row showing another metric's numbers could not be seen on screen or in tests.
 */
const REFERENCE_VALUES: Record<
  ReferenceMetricKind,
  [
    p25: number,
    median: number,
    p75: number,
    p90: number,
    own: number | null,
    position: number | null,
  ]
> = {
  damage: [1100, 1650, 2300, 3050, 1870, 58.4],
  assist_radio_track: [180, 320, 520, 760, 410, 63.2],
  spotted: [0, 1, 2, 3, 1.5, 61],
  kills: [0, 1, 2, 3, 1, 52.7],
  blocked: [300, 860, 1500, 2200, 1200, 66.1],
  life_time: [180, 277, 380, 440, 269, 47.3],
  accuracy: [62, 71, 80, 86, null, null],
};

export function referenceMetric(
  metric: ReferenceMetricKind,
  over: Partial<ReferenceMetric> = {},
): ReferenceMetric {
  const [p25, median, p75, p90, ownMedian, ownPosition] = REFERENCE_VALUES[metric];
  return {
    metric,
    count: 5354,
    p25,
    median,
    p75,
    p90,
    ownCount: 131,
    ownMedian,
    ownPosition,
    ...over,
  };
}

const REFERENCE_KINDS: ReferenceMetricKind[] = [
  "damage",
  "assist_radio_track",
  "spotted",
  "kills",
  "blocked",
  "life_time",
  "accuracy",
];

/** Reference tables with one cell, tier X medium tanks (the tanks of `overview()`). */
export function referenceTables(cell: Partial<ReferenceCell> = {}): ReferenceTables {
  const level = cell.level ?? "values";
  return {
    battles: 2012,
    minRows: 200,
    minRowsApproximate: 50,
    minOwnBattles: 10,
    cells: [
      {
        tier: 10,
        vehicleClass: "medium_tank",
        level,
        population: 5354,
        ownBattles: 131,
        metrics: level === "count_only" ? [] : REFERENCE_KINDS.map((kind) => referenceMetric(kind)),
        ...cell,
      },
    ],
    // The tanks of `overview()` in their cell (Overview, decision 2026-11-18).
    tanks:
      level === "values"
        ? [
            tankPosition(102, 3100, 88.4, 64),
            tankPosition(101, 2900, 81.2, 131),
            tankPosition(103, 2400, 79, 41),
          ]
        : [],
  };
}

export function tankPosition(
  vehicleId: number,
  medianDamage: number,
  position: number,
  battles: number,
): TankPosition {
  return { vehicleId, tier: 10, vehicleClass: "medium_tank", battles, medianDamage, position };
}

/** Trends over 100 battles with current and previous values (Overview, spec 32 §4.2). */
export function overviewTrends(wn8 = true): PerformanceTrendReport {
  const metric = (name: string, current: number | null, previous: number | null) => ({
    metric: name,
    currentValue: current,
    previousValue: previous,
    currentSampleSize: 100,
    previousSampleSize: 100,
    // Ten points of a slow rise with a little wobble, for the small line charts.
    points:
      current === null || previous === null
        ? []
        : Array.from({ length: 10 }, (_, index) => ({
            ordinal: index + 1,
            battleTime: NOW - (10 - index) * HOUR,
            value:
              previous +
              ((current - previous) * index) / 9 +
              (index % 3 === 1 ? 1 : -1) * 0.01 * current,
            sampleSize: 10,
            coverage: 1,
            vehicleId: null,
            damage: 2_000 + index * 100,
            mapName: "Synthetic Hills",
            vehicleKey: "synth-s01_alpha",
            tankName: "Alpha",
          })),
  });
  return {
    range: "last100",
    currentBattleCount: 100,
    previousBattleCount: 100,
    metrics: [
      metric("damage", 2702, 2600),
      metric("win_rate", 52, 50),
      metric("wn8", wn8 ? 2150 : null, wn8 ? 2050 : null),
    ],
  } as unknown as PerformanceTrendReport;
}
