export type Language = "pl" | "en";
export type Tone = "positive" | "warning" | "neutral";
export type { Confidence } from "./api/generated/Confidence";
export type { SourceKind } from "./api/generated/SourceKind";
export type { DataQuality } from "./api/generated/DataQuality";
export type { BattleOutcome } from "./api/generated/BattleOutcome";
export type { BattlePaceContext } from "./api/generated/BattlePaceContext";
export type { BattlePaceReason } from "./api/generated/BattlePaceReason";
export type { BattlePaceRecord } from "./api/generated/BattlePaceRecord";
export type { BattlePaceSummary } from "./api/generated/BattlePaceSummary";
export type { Battle } from "./api/generated/Battle";
export type { FeedbackItem } from "./api/generated/FeedbackItem";
export type { BattleView } from "./api/generated/BattleView";
export type { Session } from "./api/generated/Session";
export type { SessionMetrics } from "./api/generated/SessionMetrics";
export type { SessionSummary } from "./api/generated/SessionSummary";
export type { HistoryDataQualityFilter } from "./api/generated/HistoryDataQualityFilter";
export type { HistoryWn8Filter } from "./api/generated/HistoryWn8Filter";
export type { BattleHistoryCursor } from "./api/generated/BattleHistoryCursor";
export type { BattleHistoryFilter } from "./api/generated/BattleHistoryFilter";
export type { BattleHistoryItem } from "./api/generated/BattleHistoryItem";
export type { BattleHistoryPage } from "./api/generated/BattleHistoryPage";
export type { VehicleClass } from "./api/generated/VehicleClass";
export type { VehicleClassSource } from "./api/generated/VehicleClassSource";
export type { CompactDescriptorProtocolSource } from "./api/generated/CompactDescriptorProtocolSource";
export type { VehicleClassResolutionState } from "./api/generated/VehicleClassResolutionState";
export type { VehicleKeyCrossCheck } from "./api/generated/VehicleKeyCrossCheck";
export type { VehicleClassResolution } from "./api/generated/VehicleClassResolution";
export type { VehicleClassCatalogAvailability } from "./api/generated/VehicleClassCatalogAvailability";
export type { VehicleClassCatalogRefreshOutcome } from "./api/generated/VehicleClassCatalogRefreshOutcome";
export type { VehicleClassCatalogStatus } from "./api/generated/VehicleClassCatalogStatus";
export type { VehicleClassCatalogRefreshResult } from "./api/generated/VehicleClassCatalogRefreshResult";
export type { VehicleHistorySummary } from "./api/generated/VehicleHistorySummary";
export type { VehicleHistoryOverview } from "./api/generated/VehicleHistoryOverview";
export type { VehicleNameView } from "./api/generated/VehicleNameView";
export type { BattleRef } from "./api/generated/BattleRef";
export type { HeadlineKind } from "./api/generated/HeadlineKind";
export type { MapRecord } from "./api/generated/MapRecord";
export type { SessionHeadline } from "./api/generated/SessionHeadline";
export type { SessionMaps } from "./api/generated/SessionMaps";
export type { SessionMoment } from "./api/generated/SessionMoment";
export type { SessionStory } from "./api/generated/SessionStory";
export type { Recap } from "./api/generated/Recap";
export type { RecapKind } from "./api/generated/RecapKind";
export type { RateReference } from "./api/generated/RateReference";
export type { SessionFact } from "./api/generated/SessionFact";
export type { SessionFactValue } from "./api/generated/SessionFactValue";
export type { BattleFact } from "./api/generated/BattleFact";
export type { BattleFactValue } from "./api/generated/BattleFactValue";
export type { WeekFact } from "./api/generated/WeekFact";
export type { WeekFactValue } from "./api/generated/WeekFactValue";
export type { WeeklyReport } from "./api/generated/WeeklyReport";
export type { WeeklyGoal } from "./api/generated/WeeklyGoal";
export type { WeeklyIndicator } from "./api/generated/WeeklyIndicator";
export type { WeeklyMetric } from "./api/generated/WeeklyMetric";
export type { GoalKind } from "./api/generated/GoalKind";
export type { GoalState } from "./api/generated/GoalState";
export type { VehicleNamesView } from "./api/generated/VehicleNamesView";
export type { VehicleHistorySession } from "./api/generated/VehicleHistorySession";
export type { VehicleHistoryReport } from "./api/generated/VehicleHistoryReport";
export type { VehicleMapSummary } from "./api/generated/VehicleMapSummary";
export type { TeamTotals } from "./api/generated/TeamTotals";
export type { TeamComparison } from "./api/generated/TeamComparison";
export type { BattleDetailView } from "./api/generated/BattleDetailView";
export type { BaselineWindowKind } from "./api/generated/BaselineWindowKind";
export type { BaselineScope } from "./api/generated/BaselineScope";
export type { BaselineConfidence } from "./api/generated/BaselineConfidence";
export type { BaselineMetric } from "./api/generated/BaselineMetric";
export type { NormalizedBaselineMetrics } from "./api/generated/NormalizedBaselineMetrics";
export type { BaselineWindowResult } from "./api/generated/BaselineWindowResult";
export type { PlayerBaselinePresentation } from "./api/generated/PlayerBaselinePresentation";
export type { VehicleBaselinePresentation } from "./api/generated/VehicleBaselinePresentation";
export type { BaselineComparisonState } from "./api/generated/BaselineComparisonState";
export type { BaselineDelta } from "./api/generated/BaselineDelta";
export type { NormalizedBaselineDeltas } from "./api/generated/NormalizedBaselineDeltas";
export type { VehicleSessionBaselineComparison } from "./api/generated/VehicleSessionBaselineComparison";
export type { SessionBaselineKind } from "./api/generated/SessionBaselineKind";
export type { SessionInsightState } from "./api/generated/SessionInsightState";
export type { SessionInsightMetric } from "./api/generated/SessionInsightMetric";
export type { SessionInsightDirection } from "./api/generated/SessionInsightDirection";
export type { SessionInsightDeltaUnit } from "./api/generated/SessionInsightDeltaUnit";
export type { SessionInsightEvidenceTextKey } from "./api/generated/SessionInsightEvidenceTextKey";
export type { SessionInsightUnavailableReason } from "./api/generated/SessionInsightUnavailableReason";
export type { SessionInsight } from "./api/generated/SessionInsight";
export type { SessionInsightSummary } from "./api/generated/SessionInsightSummary";
export type { SessionInsightPresentation } from "./api/generated/SessionInsightPresentation";
export type { SessionBaselineComparison } from "./api/generated/SessionBaselineComparison";
export type { SourceStatus } from "./api/generated/SourceStatus";
/** Payload of the `import-progress` event (spec 20, D6). */
export interface ImportProgressEvent {
  done: number;
  total: number;
  finished: boolean;
  cancelled: boolean;
}
/** Backend events the window listens to instead of polling (spec 20, D6). */
export type BackendEvent =
  | { name: "import-progress"; payload: ImportProgressEvent }
  | { name: "data-changed" }
  | { name: "session-ended"; payload: { sessionId: string } };
export type { ImportProgress } from "./api/generated/ImportProgress";
export type { AdvancedStatus } from "./api/generated/AdvancedStatus";
export type { ExpectedValuesAvailability } from "./api/generated/ExpectedValuesAvailability";
export type { ExpectedValuesUpdateOutcome } from "./api/generated/ExpectedValuesUpdateOutcome";
export type { ExpectedValuesStatus } from "./api/generated/ExpectedValuesStatus";
export type { ExpectedValuesRefreshResult } from "./api/generated/ExpectedValuesRefreshResult";
export type { Wn8PresentationState } from "./api/generated/Wn8PresentationState";
export type { Wn8UnavailableReason } from "./api/generated/Wn8UnavailableReason";
export interface Wn8Exclusions {
  partialOrIncomplete: number;
  unsupportedFeatureSchema: number;
  notRandomBattle: number;
  vehicleIdConflict: number;
  missingVehicleId: number;
  vehicleNotInExpectedValues: number;
  missingDamage: number;
  missingFrags: number;
  missingSpots: number;
  missingDefense: number;
  missingResult: number;
  invalidNumericInput: number;
}
export type { SessionWn8Presentation } from "./api/generated/SessionWn8Presentation";
export type { BattleWn8Presentation } from "./api/generated/BattleWn8Presentation";
export type { HistoricalSessionWn8Presentation } from "./api/generated/HistoricalSessionWn8Presentation";
export type { Wn8Presentation } from "./api/generated/Wn8Presentation";
export type { ChallengeType } from "./api/generated/ChallengeType";
export type { ChallengeStatus } from "./api/generated/ChallengeStatus";
export type { ChallengeContributionMetricKind } from "./api/generated/ChallengeContributionMetricKind";
export type { ChallengeEvidenceMetric } from "./api/generated/ChallengeEvidenceMetric";
export type { ChallengeSuggestionUnavailableReason } from "./api/generated/ChallengeSuggestionUnavailableReason";
export type { ChallengeSuggestion } from "./api/generated/ChallengeSuggestion";
export type { ChallengeSuggestionResponse } from "./api/generated/ChallengeSuggestionResponse";
export type { ChallengeBattlePresentation } from "./api/generated/ChallengeBattlePresentation";
export type { ChallengePresentation } from "./api/generated/ChallengePresentation";
export type { ChallengeHistoryPage } from "./api/generated/ChallengeHistoryPage";
export type { PerformanceTrendRange } from "./api/generated/PerformanceTrendRange";
export type { PerformanceTrendMetric } from "./api/generated/PerformanceTrendMetric";
export type { TrendDirection } from "./api/generated/TrendDirection";
export type { TrendScope } from "./api/generated/TrendScope";
export type { PerformanceTrendPoint } from "./api/generated/PerformanceTrendPoint";
export type { PerformanceMetricTrend } from "./api/generated/PerformanceMetricTrend";
export type { PerformanceTrendSignal } from "./api/generated/PerformanceTrendSignal";
export type { PerformanceTrendReport } from "./api/generated/PerformanceTrendReport";
export type { AppSnapshot } from "./api/generated/AppSnapshot";
export type AttemptsConversionRecentWindow = 10 | 30 | 100;
export type AttemptsConversionScope =
  | { type: "session"; sessionId: string }
  | { type: "recent"; window: AttemptsConversionRecentWindow };
export type AttemptsConversionRejectionCode =
  | "partial_replay"
  | "results_provenance_missing"
  | "parser_too_old"
  | "unsupported_schema"
  | "unsupported_mode"
  | "invalid_team_structure"
  | "vehicle_class_catalog_unavailable"
  | "vehicle_class_unknown"
  | "vehicle_class_conflict"
  | "unsupported_vehicle_mechanics"
  | "missing_counters"
  | "counter_out_of_range"
  | "counter_conflict";
export interface AttemptsConversionRejectionReason {
  code: AttemptsConversionRejectionCode;
  count: number;
}
export interface CommonContextCoverage {
  eligibleBattles: number;
  ineligibleBattles: number;
  rejectionReasons: AttemptsConversionRejectionReason[];
}
export interface AttemptsConversionRate {
  selectedBattles: number;
  commonEligibleBattles: number;
  metricEligibleBattles: number;
  metricIneligibleBattles: number;
  contributingBattles: number;
  zeroDenominatorBattles: number;
  missingBattles: number;
  invalidBattles: number;
  numeratorSum: number;
  denominatorSum: number;
  value: number | null;
  rejectionReasons: AttemptsConversionRejectionReason[];
}
export interface AttemptsConversionFactCoverage {
  selectedBattles: number;
  commonEligibleBattles: number;
  factEligibleBattles: number;
  factIneligibleBattles: number;
  contributingBattles: number;
  missingBattles: number;
  invalidBattles: number;
  valueSum: number | null;
  rejectionReasons: AttemptsConversionRejectionReason[];
}
export interface AttemptsConversionFacts {
  directTeamHits: AttemptsConversionFactCoverage;
  explosionHits: AttemptsConversionFactCoverage;
  damageDealt: AttemptsConversionFactCoverage;
}
export interface AttemptsConversionReport {
  contractVersion: "attempts-conversion-v2";
  scope: AttemptsConversionScope;
  selectedBattles: number;
  commonContext: CommonContextCoverage;
  hitRate: AttemptsConversionRate;
  penetrationRate: AttemptsConversionRate;
  facts: AttemptsConversionFacts;
}
export type { LocalImportResult } from "./api/generated/LocalImportResult";
export type { CommandError } from "./api/generated/CommandError";
