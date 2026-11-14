import type {
  Battle,
  BattleOutcome,
  BattlePaceContext,
  BattlePaceReason,
  BattlePaceRecord,
  Language,
} from "../types";
import { translate } from "../i18n/legacy";

export type Wn8Category =
  | "very_bad"
  | "bad"
  | "below_average"
  | "average"
  | "above_average"
  | "good"
  | "very_good"
  | "great"
  | "unicum"
  | "super_unicum";

export function battleCountLabel(count: number, language: Language): string {
  if (language === "en") return `${count} ${count === 1 ? "battle" : "battles"}`;
  if (count === 1) return "1 bitwa";
  const lastTwo = Math.abs(count) % 100;
  const last = Math.abs(count) % 10;
  return `${count} ${last >= 2 && last <= 4 && !(lastTwo >= 12 && lastTwo <= 14) ? "bitwy" : "bitew"}`;
}

export function sessionStatusLabel(status: string, language: Language): string {
  if (status === "active") return translate(language, "currentSessionActive");
  if (status === "timed_out") return translate(language, "sessionEndedAfterInactivity");
  return translate(language, "sessionEnded");
}

export function battleOutcomePresentation(
  outcome: BattleOutcome,
  language: Language,
): { label: string; className: "result-win" | "result-loss" | "result-draw" | "result-unknown" } {
  switch (outcome) {
    case "victory":
      return { label: translate(language, "battleResultWin"), className: "result-win" };
    case "defeat":
      return { label: translate(language, "battleResultLoss"), className: "result-loss" };
    case "draw":
      return { label: translate(language, "battleResultDraw"), className: "result-draw" };
    case "unknown":
      return { label: translate(language, "battleResultUnknown"), className: "result-unknown" };
  }
}

const battlePaceContextKeys: Record<BattlePaceContext, Parameters<typeof translate>[1]> = {
  obvious_steamroll: "paceObvious",
  borderline_steamroll: "paceBorderline",
  standard: "paceStandard",
  unknown: "paceUnknown",
};

const battlePaceReasonKeys: Record<BattlePaceReason, Parameters<typeof translate>[1]> = {
  missing_duration: "paceReasonMissingDuration",
  missing_final_roster: "paceReasonMissingFinalRoster",
  missing_team_assignment: "paceReasonMissingTeamAssignment",
  missing_finish_reason: "paceReasonMissingFinishReason",
  unsupported_mode: "paceReasonUnsupportedMode",
  unsupported_finish_reason: "paceReasonUnsupportedFinishReason",
  unsupported_schema: "paceReasonUnsupportedSchema",
  invalid_team_size: "paceReasonInvalidTeamSize",
  asymmetric_teams: "paceReasonAsymmetricTeams",
  invalid_destroyed_score: "paceReasonInvalidDestroyedScore",
  conflicting_data: "paceReasonConflictingData",
};

export function battlePacePresentation(
  pace: BattlePaceRecord,
  language: Language,
): {
  context: BattlePaceContext;
  label: string;
  reason: string | null;
  className: string;
  showBadge: boolean;
} {
  const context = pace.context ?? "unknown";
  return {
    context,
    label: translate(language, battlePaceContextKeys[context]),
    reason: pace.reason ? translate(language, battlePaceReasonKeys[pace.reason]) : null,
    className: `pace-${context.replaceAll("_", "-")}`,
    showBadge: context === "obvious_steamroll" || context === "borderline_steamroll",
  };
}

export function wn8Category(value: number | null): Wn8Category | null {
  if (value === null || !Number.isFinite(value)) return null;
  if (value < 300) return "very_bad";
  if (value < 450) return "bad";
  if (value < 650) return "below_average";
  if (value < 900) return "average";
  if (value < 1200) return "above_average";
  if (value < 1600) return "good";
  if (value < 2000) return "very_good";
  if (value < 2450) return "great";
  if (value < 2900) return "unicum";
  return "super_unicum";
}

const basename = (value: string) => value.split(/[\\/]/).pop() ?? "";
const replayName = (value: string) => /\.wotreplay$/i.test(basename(value));
const VEHICLE_DISPLAY_NAME_MAX_LENGTH = 80;

export type VehicleDisplayName = string;

function readableWords(value: string): string | null {
  const words = value
    .replace(/_/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .split(" ")
    .filter(Boolean)
    .map((word) => (/^[a-z]+$/.test(word) ? word[0].toUpperCase() + word.slice(1) : word));
  const result = words.join(" ").trim();
  return result ? result.slice(0, VEHICLE_DISPLAY_NAME_MAX_LENGTH).trim() || null : null;
}

export function vehicleDisplayName(
  source: { vehicleName?: string | null; tankName?: string | null; vehicleKey?: string | null },
  language: Language,
): VehicleDisplayName {
  const candidate =
    [source.vehicleName, source.tankName, source.vehicleKey].find((value) =>
      Boolean(value?.trim()),
    ) ?? null;
  if (candidate) {
    const safe = basename(candidate)
      .replace(/\.wotreplay$/i, "")
      .trim();
    const looksLikeIdentity =
      /^\d+$/.test(safe) ||
      /^[a-z0-9_-]+:[a-z0-9_.-]+$/i.test(safe) ||
      /^vehicle-\d+(?:-\d+)?$/i.test(safe);
    if (safe && !looksLikeIdentity && !/^\d{8}_\d{4}_/.test(safe)) {
      const withoutNationPrefix = safe.replace(/^[a-z]+-[a-z0-9]+_/i, "");
      const recognized =
        withoutNationPrefix === safe ? safe.replace(/^[a-z]\d+_/i, "") : withoutNationPrefix;
      const readable = readableWords(recognized);
      if (readable) return readable;
    }
  }
  return translate(language, "unknownVehicle");
}

export function readableMapKey(value: string | null): string | null {
  if (!value) return null;
  const safe = basename(value).replace(/\.wotreplay$/i, "");
  if (!safe || /^\d{8}_\d{4}_/.test(safe)) return null;
  return readableWords(safe.replace(/^\d+_/, ""));
}

export function replayFallback(
  value: string | null,
  mapKey: string | null,
): { vehicle: string | null; map: string | null } {
  if (!value) return { vehicle: null, map: readableMapKey(mapKey) };
  let safe = basename(value).replace(/\.wotreplay$/i, "");
  const wasReplay = replayName(value) || /^\d{8}_\d{4}_/.test(safe);
  safe = safe.replace(/^\d{8}_\d{4}_/, "");
  if (wasReplay) {
    if (!mapKey) return { vehicle: null, map: null };
    const suffix = `_${basename(mapKey).replace(/\.wotreplay$/i, "")}`;
    if (!safe.toLowerCase().endsWith(suffix.toLowerCase())) {
      return { vehicle: null, map: readableMapKey(mapKey) };
    }
    safe = safe.slice(0, -suffix.length);
  }
  const withoutNationPrefix = safe.replace(/^[a-z]+-[a-z0-9]+_/i, "");
  safe = withoutNationPrefix === safe ? safe.replace(/^[a-z]\d+_/i, "") : withoutNationPrefix;
  if (
    /^\d+$/.test(safe) ||
    /^[a-z0-9_-]+:[a-z0-9_.-]+$/i.test(safe) ||
    /^vehicle-\d+(?:-\d+)?$/i.test(safe)
  ) {
    return { vehicle: null, map: readableMapKey(mapKey) };
  }
  return { vehicle: readableWords(safe), map: readableMapKey(mapKey) };
}

function safeMetadata(value: string | null): string | null {
  if (!value || replayName(value) || /^\d{8}_\d{4}_/.test(basename(value))) return null;
  return value.trim() || null;
}

export function battleIdentity(
  battle: Pick<Battle, "tankName" | "vehicleKey" | "mapName" | "mapKey">,
  language: Language,
): { vehicle: string; map: string } {
  const unknown = {
    vehicle: translate(language, "unknownVehicle"),
    map: translate(language, "unknownMap"),
  };
  const fallback = replayFallback(battle.tankName ?? battle.vehicleKey, battle.mapKey);
  const vehicleMetadata = safeMetadata(battle.tankName);
  const vehicle = vehicleMetadata
    ? vehicleDisplayName({ tankName: vehicleMetadata }, language)
    : fallback.vehicle;
  const mapMetadata = safeMetadata(battle.mapName);
  const map = mapMetadata
    ? (readableMapKey(mapMetadata) ?? mapMetadata)
    : (readableMapKey(battle.mapKey) ?? fallback.map);
  return {
    vehicle:
      vehicle ?? vehicleDisplayName({ vehicleKey: battle.vehicleKey }, language) ?? unknown.vehicle,
    map: map ?? unknown.map,
  };
}

export function historyBattleIdentity(
  battle: {
    tankName?: string | null;
    vehicleKey?: string | null;
    mapName?: string | null;
    mapKey?: string | null;
  },
  language: Language,
): { vehicle: string; map: string } {
  const unknown = {
    vehicle: translate(language, "unknownVehicle"),
    map: translate(language, "unknownMap"),
  };
  const mapKey = battle.mapKey ?? null;
  const fallback = replayFallback(battle.tankName ?? battle.vehicleKey ?? null, mapKey);
  const vehicleMetadata = safeMetadata(battle.tankName ?? null);
  const vehicle = vehicleMetadata
    ? vehicleDisplayName({ tankName: vehicleMetadata }, language)
    : fallback.vehicle;
  const mapMetadata = safeMetadata(battle.mapName ?? null);
  const map = mapMetadata
    ? (readableMapKey(mapMetadata) ?? mapMetadata)
    : (readableMapKey(mapKey) ?? fallback.map);
  return {
    vehicle:
      vehicle ?? vehicleDisplayName({ vehicleKey: battle.vehicleKey }, language) ?? unknown.vehicle,
    map: map ?? unknown.map,
  };
}
