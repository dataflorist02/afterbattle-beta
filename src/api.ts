import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BattleDetailView,
  Language,
  MapRecord,
  SessionMaps,
  SessionStory,
  Recap,
  RecapKind,
  VehicleNamesView,
  WeeklyReport,
  ReferenceTables,
  AvailableUpdate,
  UpdateConsent,
  ExpectedValuesStatus,
  AppSnapshot,
  BackendEvent,
  HistoricalSessionWn8Presentation,
  ImportProgressEvent,
  AttemptsConversionFactCoverage,
  AttemptsConversionRate,
  AttemptsConversionRecentWindow,
  AttemptsConversionRejectionCode,
  AttemptsConversionRejectionReason,
  AttemptsConversionReport,
  AttemptsConversionScope,
  BattleHistoryFilter,
  BattleHistoryPage,
  ChallengeHistoryPage,
  ChallengePresentation,
  ChallengeSuggestionResponse,
  CommandError,
  ExpectedValuesRefreshResult,
  PerformanceTrendRange,
  PerformanceTrendReport,
  PlayerBaselinePresentation,
  Session,
  SessionBaselineComparison,
  SessionSummary,
  SourceStatus,
  VehicleBaselinePresentation,
  VehicleClassCatalogRefreshResult,
  VehicleClassCatalogStatus,
  VehicleHistoryOverview,
  VehicleHistoryReport,
} from "./types";

export class AppCommandError extends Error {
  readonly code: string;
  constructor(error: unknown) {
    let value = error as Partial<CommandError> | null;
    if (typeof error === "string") {
      try {
        value = JSON.parse(error);
      } catch {
        value = { message: error };
      }
    }
    super(value?.message ?? "Unexpected application error");
    this.code = value?.code ?? "unknown_error";
  }
}
/**
 * Subscribes to the backend events (`import-progress`, `data-changed`, `session-ended`).
 * Returns the function that unsubscribes.
 */
function onBackendEvents(handler: (event: BackendEvent) => void): () => void {
  const pending = [
    listen<ImportProgressEvent>("import-progress", (event) =>
      handler({ name: "import-progress", payload: event.payload }),
    ),
    listen("data-changed", () => handler({ name: "data-changed" })),
    listen<{ sessionId: string }>("session-ended", (event) =>
      handler({ name: "session-ended", payload: event.payload }),
    ),
  ];
  return () => {
    for (const unlisten of pending) void unlisten.then((stop) => stop()).catch(() => {});
  };
}
async function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(name, args);
  } catch (error) {
    throw new AppCommandError(error);
  }
}

const commonReasons: readonly AttemptsConversionRejectionCode[] = [
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
const metricReasons: readonly AttemptsConversionRejectionCode[] = [
  ...commonReasons,
  "missing_counters",
  "counter_out_of_range",
  "counter_conflict",
];
const integer = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const sumReasons = (reasons: AttemptsConversionRejectionReason[]) => {
  let total = 0;
  for (const reason of reasons) {
    total += reason.count;
    if (!Number.isSafeInteger(total)) return null;
  }
  return total;
};
const reasonDerivedCounts = (reasons: AttemptsConversionRejectionReason[]) => {
  const missing = reasons.find((reason) => reason.code === "missing_counters");
  const outOfRange = reasons.find((reason) => reason.code === "counter_out_of_range");
  const conflict = reasons.find((reason) => reason.code === "counter_conflict");
  if (!missing || !outOfRange || !conflict) return null;
  const invalid = outOfRange.count + conflict.count;
  return Number.isSafeInteger(invalid) ? { missing: missing.count, invalid } : null;
};
function parseScope(value: unknown): AttemptsConversionScope | null {
  if (!record(value)) return null;
  if (value.type === "session" && typeof value.sessionId === "string")
    return { type: "session", sessionId: value.sessionId };
  if (
    value.type === "recent" &&
    (value.window === 10 || value.window === 30 || value.window === 100)
  )
    return { type: "recent", window: value.window };
  return null;
}
function parseReasons(
  value: unknown,
  catalog: readonly AttemptsConversionRejectionCode[],
): AttemptsConversionRejectionReason[] | null {
  if (!Array.isArray(value) || value.length !== catalog.length) return null;
  const parsed = value.map((reason) =>
    record(reason) && typeof reason.code === "string" && integer(reason.count)
      ? { code: reason.code, count: reason.count }
      : null,
  );
  return parsed.some((reason) => reason === null) ||
    parsed.some((reason, index) => reason!.code !== catalog[index])
    ? null
    : (parsed as AttemptsConversionRejectionReason[]);
}
function parseRate(
  value: unknown,
  selected: number,
  commonEligible: number,
): AttemptsConversionRate | null {
  if (
    !record(value) ||
    ![
      "selectedBattles",
      "commonEligibleBattles",
      "metricEligibleBattles",
      "metricIneligibleBattles",
      "contributingBattles",
      "zeroDenominatorBattles",
      "missingBattles",
      "invalidBattles",
      "numeratorSum",
      "denominatorSum",
    ].every((key) => integer(value[key])) ||
    (value.value !== null &&
      (typeof value.value !== "number" ||
        !Number.isFinite(value.value) ||
        value.value < 0 ||
        value.value > 1))
  )
    return null;
  const numeric = value as Record<string, number | null | unknown>;
  const selectedBattles = numeric.selectedBattles as number,
    commonEligibleBattles = numeric.commonEligibleBattles as number,
    metricEligibleBattles = numeric.metricEligibleBattles as number,
    metricIneligibleBattles = numeric.metricIneligibleBattles as number,
    contributingBattles = numeric.contributingBattles as number,
    zeroDenominatorBattles = numeric.zeroDenominatorBattles as number,
    missingBattles = numeric.missingBattles as number,
    invalidBattles = numeric.invalidBattles as number,
    numeratorSum = numeric.numeratorSum as number,
    denominatorSum = numeric.denominatorSum as number,
    rateValue = numeric.value as number | null;
  const reasons = parseReasons(value.rejectionReasons, metricReasons);
  const derived = reasons && reasonDerivedCounts(reasons);
  if (
    !reasons ||
    !derived ||
    selectedBattles !== selected ||
    commonEligibleBattles !== commonEligible ||
    metricEligibleBattles + metricIneligibleBattles !== selected ||
    contributingBattles + zeroDenominatorBattles !== metricEligibleBattles ||
    numeratorSum > denominatorSum ||
    sumReasons(reasons) !== metricIneligibleBattles ||
    missingBattles !== derived.missing ||
    invalidBattles !== derived.invalid ||
    (denominatorSum === 0) !== (rateValue === null) ||
    (denominatorSum > 0 && Math.abs((rateValue as number) - numeratorSum / denominatorSum) > 1e-12)
  )
    return null;
  return {
    selectedBattles,
    commonEligibleBattles,
    metricEligibleBattles,
    metricIneligibleBattles,
    contributingBattles,
    zeroDenominatorBattles,
    missingBattles,
    invalidBattles,
    numeratorSum,
    denominatorSum,
    value: rateValue,
    rejectionReasons: reasons,
  };
}
function parseFact(
  value: unknown,
  selected: number,
  commonEligible: number,
): AttemptsConversionFactCoverage | null {
  if (
    !record(value) ||
    ![
      "selectedBattles",
      "commonEligibleBattles",
      "factEligibleBattles",
      "factIneligibleBattles",
      "contributingBattles",
      "missingBattles",
      "invalidBattles",
    ].every((key) => integer(value[key])) ||
    (value.valueSum !== null && !integer(value.valueSum))
  )
    return null;
  const numeric = value as Record<string, number | null | unknown>;
  const selectedBattles = numeric.selectedBattles as number,
    commonEligibleBattles = numeric.commonEligibleBattles as number,
    factEligibleBattles = numeric.factEligibleBattles as number,
    factIneligibleBattles = numeric.factIneligibleBattles as number,
    contributingBattles = numeric.contributingBattles as number,
    missingBattles = numeric.missingBattles as number,
    invalidBattles = numeric.invalidBattles as number,
    valueSum = numeric.valueSum as number | null;
  const reasons = parseReasons(value.rejectionReasons, metricReasons);
  const derived = reasons && reasonDerivedCounts(reasons);
  if (
    !reasons ||
    !derived ||
    selectedBattles !== selected ||
    commonEligibleBattles !== commonEligible ||
    factEligibleBattles + factIneligibleBattles !== selected ||
    contributingBattles !== factEligibleBattles ||
    (valueSum === null) !== (contributingBattles === 0) ||
    sumReasons(reasons) !== factIneligibleBattles ||
    missingBattles !== derived.missing ||
    invalidBattles !== derived.invalid
  )
    return null;
  return {
    selectedBattles,
    commonEligibleBattles,
    factEligibleBattles,
    factIneligibleBattles,
    contributingBattles,
    missingBattles,
    invalidBattles,
    valueSum,
    rejectionReasons: reasons,
  };
}
export function parseAttemptsConversionReport(value: unknown): AttemptsConversionReport {
  if (
    !record(value) ||
    value.contractVersion !== "attempts-conversion-v2" ||
    !integer(value.selectedBattles)
  )
    return failAttemptsConversionContract();
  const scope = parseScope(value.scope);
  const common =
    record(value.commonContext) &&
    integer(value.commonContext.eligibleBattles) &&
    integer(value.commonContext.ineligibleBattles)
      ? {
          eligibleBattles: value.commonContext.eligibleBattles,
          ineligibleBattles: value.commonContext.ineligibleBattles,
          rejectionReasons: parseReasons(value.commonContext.rejectionReasons, commonReasons),
        }
      : null;
  if (
    !scope ||
    !common ||
    !common.rejectionReasons ||
    common.eligibleBattles + common.ineligibleBattles !== value.selectedBattles ||
    sumReasons(common.rejectionReasons) !== common.ineligibleBattles ||
    !record(value.facts)
  )
    return failAttemptsConversionContract();
  const hitRate = parseRate(value.hitRate, value.selectedBattles, common.eligibleBattles);
  const penetrationRate = parseRate(
    value.penetrationRate,
    value.selectedBattles,
    common.eligibleBattles,
  );
  const directTeamHits = parseFact(
    value.facts.directTeamHits,
    value.selectedBattles,
    common.eligibleBattles,
  );
  const explosionHits = parseFact(
    value.facts.explosionHits,
    value.selectedBattles,
    common.eligibleBattles,
  );
  const damageDealt = parseFact(
    value.facts.damageDealt,
    value.selectedBattles,
    common.eligibleBattles,
  );
  if (!hitRate || !penetrationRate || !directTeamHits || !explosionHits || !damageDealt)
    return failAttemptsConversionContract();
  return {
    contractVersion: "attempts-conversion-v2",
    scope,
    selectedBattles: value.selectedBattles,
    commonContext: {
      eligibleBattles: common.eligibleBattles,
      ineligibleBattles: common.ineligibleBattles,
      rejectionReasons: common.rejectionReasons,
    },
    hitRate,
    penetrationRate,
    facts: { directTeamHits, explosionHits, damageDealt },
  };
}
function failAttemptsConversionContract(): never {
  throw new AppCommandError({
    code: "report_invariant_violation",
    message: "Attempts conversion report contract is invalid",
  });
}

const attemptsConversionSession = (sessionId: string) =>
  command<unknown>("get_attempts_conversion_session_report", { sessionId }).then(
    parseAttemptsConversionReport,
  );
const attemptsConversionRecent = (window: AttemptsConversionRecentWindow) =>
  command<unknown>("get_attempts_conversion_recent_report", { window }).then(
    parseAttemptsConversionReport,
  );
export const api = {
  snapshot: () => command<AppSnapshot>("get_snapshot"),
  battleView: (battleId: string) => command<BattleDetailView>("get_battle_view", { battleId }),
  weeklyReport: () => command<WeeklyReport>("get_weekly_report"),
  /** A-1 (spec 32 §2): aggregates per tier and class from the player's battles. */
  referenceTables: () => command<ReferenceTables>("get_reference_tables"),
  /** Spec 32 §5: the consent to the update check ("unasked" until the player answers). */
  updateConsent: () => command<UpdateConsent>("get_update_consent"),
  setUpdateConsent: (enabled: boolean) => command<UpdateConsent>("set_update_consent", { enabled }),
  /** One check per start, only with consent (the backend refuses without it). */
  checkForUpdate: () => command<AvailableUpdate | null>("check_for_update"),
  /** Installs the signed update and restarts; only after the player's click. */
  installUpdate: () => command<void>("install_update"),
  sessionStory: (sessionId: string) => command<SessionStory>("get_session_story", { sessionId }),
  /** Facts of a template summary (spec 30); the text is put together in `lib/recap`. */
  recap: (kind: RecapKind, id: string) => command<Recap>("get_recap", { kind, id }),
  mapRecord: (mapKey: string) => command<MapRecord>("get_map_record", { mapKey }),
  sessionMaps: (sessionIds: string[]) => command<SessionMaps[]>("get_session_maps", { sessionIds }),
  vehicleNames: (language: Language) =>
    command<VehicleNamesView>("get_vehicle_names", { language }),
  /** A map with `width`×`height` (device pixels of its box) comes sharp at that size. */
  gameImage: (kind: "vehicle" | "map", key: string, width?: number, height?: number) =>
    command<ArrayBuffer>(
      "get_game_image",
      width && height ? { kind, key, width, height } : { kind, key },
    ),
  completeOnboarding: () => command<void>("complete_onboarding"),
  resetOnboarding: () => command<void>("reset_onboarding"),
  diagnostics: (language: string) => command<string>("get_diagnostics", { language }),
  expectedValuesStatus: () => command<ExpectedValuesStatus>("get_expected_values_status"),
  refreshExpectedValues: () => command<ExpectedValuesRefreshResult>("refresh_expected_values"),
  vehicleClassCatalogStatus: () =>
    command<VehicleClassCatalogStatus>("get_vehicle_class_catalog_status"),
  refreshVehicleClassCatalog: () =>
    command<VehicleClassCatalogRefreshResult>("refresh_vehicle_class_catalog"),
  detectInstallations: () => command<string[]>("detect_wot_installations"),
  /**
   * The system folder picker; `null` when closed without a choice. The path goes to
   * `configureLocal` unchanged — the backend normalizes and checks it.
   */
  pickGameFolder: async (title: string): Promise<string | null> => {
    const chosen = await open({ directory: true, multiple: false, title });
    return typeof chosen === "string" ? chosen : null;
  },
  configureLocal: (gameRoot: string, language: string) =>
    command<SourceStatus>("configure_local_source", { gameRoot, language }),
  pause: () => command<SourceStatus>("pause_tracker"),
  resume: () => command<SourceStatus>("resume_tracker"),
  startImport: () => command<void>("start_import"),
  cancelImport: () => command<void>("cancel_import"),
  recentSessionsWn8: () => command<HistoricalSessionWn8Presentation[]>("get_recent_sessions_wn8"),
  onEvents: onBackendEvents,
  setXvmEnabled: (enabled: boolean) => command<number>("set_xvm_enabled", { enabled }),
  startSession: () => command<Session>("start_session"),
  endSession: () => command<SessionSummary>("end_session"),
  sessionSummary: (sessionId: string) =>
    command<SessionSummary>("get_session_summary", { sessionId }),
  attemptsConversionSession,
  attemptsConversionRecent,
  battleHistory: (filter: BattleHistoryFilter) =>
    command<BattleHistoryPage>("get_battle_history_page", { filter }),
  vehicleHistoryOverview: () => command<VehicleHistoryOverview>("get_vehicle_history_overview"),
  vehicleHistoryReport: (vehicleId: number) =>
    command<VehicleHistoryReport>("get_vehicle_history_report", { vehicleId }),
  playerBaseline: () => command<PlayerBaselinePresentation>("get_player_baseline"),
  vehicleBaseline: (vehicleId: number) =>
    command<VehicleBaselinePresentation>("get_vehicle_baseline", { vehicleId }),
  sessionBaselineComparison: (sessionId: string) =>
    command<SessionBaselineComparison>("get_session_baseline_comparison", { sessionId }),
  performanceTrends: (range: PerformanceTrendRange, vehicleId: number | null = null) =>
    command<PerformanceTrendReport>("get_performance_trends", { range, vehicleId }),
  challengeSuggestions: (sessionId: string) =>
    command<ChallengeSuggestionResponse>("get_challenge_suggestions", { sessionId }),
  startVehicleChallenge: (sessionId: string, suggestionId: string) =>
    command<ChallengePresentation>("start_vehicle_challenge", { sessionId, suggestionId }),
  activeChallenge: () => command<ChallengePresentation | null>("get_active_challenge"),
  challengeHistory: (offset: number, pageSize: number) =>
    command<ChallengeHistoryPage>("get_challenge_history", { offset, pageSize }),
  abandonActiveChallenge: () => command<ChallengePresentation>("abandon_active_challenge"),
  updatePreferences: (language: string, inactivityMinutes: number) =>
    command<AppSnapshot>("update_preferences", { language, inactivityMinutes }),
  exportAnonymousSession: (sessionId: string) =>
    command<string>("export_anonymous_session", { sessionId }),
  deleteAllData: () => command<void>("delete_all_data"),
};
