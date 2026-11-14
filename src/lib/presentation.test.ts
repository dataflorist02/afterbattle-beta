import { describe, expect, it } from "vitest";

import {
  battleCountLabel,
  battleIdentity,
  battleOutcomePresentation,
  battlePacePresentation,
  readableMapKey,
  replayFallback,
  sessionStatusLabel,
  vehicleDisplayName,
  wn8Category,
} from "./presentation";
import { translate } from "../i18n/legacy";
import type { Battle } from "../types";

const battle = (overrides: Partial<Battle> = {}): Battle => ({
  arenaId: "id",
  arenaUniqueId: null,
  battleTime: 1,
  battleTimeSource: null,
  vehicleKey: "vehicle",
  wgTankId: null,
  tankName: null,
  mapName: null,
  mapKey: null,
  battleType: null,
  gameplayId: null,
  sourceKind: "replay",
  dataQuality: "partial",
  parserVersion: null,
  featureSchemaVersion: null,
  gameVersion: null,
  dataCompleteness: null,
  replaySource: null,
  parserErrorCode: null,
  outcome: "unknown",
  battlePace: {
    context: null,
    reason: null,
    resolverVersion: null,
    finishReasonRaw: null,
    battleTypeRaw: null,
    bonusTypeRaw: null,
    initialTeam1Size: null,
    initialTeam2Size: null,
    destroyedTeam1Score: null,
    destroyedTeam2Score: null,
  },
  playerTeam: null,
  winnerTeamRaw: null,
  damage: null,
  assist: null,
  assistSpotting: null,
  assistTracking: null,
  assistStun: null,
  blocked: null,
  damageReceived: null,
  frags: null,
  spots: null,
  shots: null,
  hits: null,
  penetrations: null,
  directEnemyHits: null,
  directTeamHits: null,
  piercingEnemyHits: null,
  explosionHits: null,
  defensePoints: null,
  movingAvgDamage: null,
  damageRating: null,
  survived: null,
  xp: null,
  originalXp: null,
  durationSeconds: null,
  sessionId: null,
  ...overrides,
});

describe("WN8 presentation scale", () => {
  it.each([
    [995, "above_average"],
    [1271, "good"],
    [2049, "great"],
    [4330, "super_unicum"],
  ] as const)("maps %i to %s", (value, category) => expect(wn8Category(value)).toBe(category));
  it("keeps N/A neutral", () => expect(wn8Category(null)).toBeNull());
  it("uses sentence case for every reviewed Polish and English label", () => {
    expect([
      translate("en", "wn8VeryBad"),
      translate("en", "wn8AboveAverage"),
      translate("en", "wn8Good"),
      translate("en", "wn8Great"),
      translate("en", "wn8SuperUnicum"),
      translate("en", "wn8NotAvailable"),
    ]).toEqual(["Very bad", "Above average", "Good", "Great", "Super unicum", "Unavailable"]);
    expect([
      translate("pl", "wn8VeryBad"),
      translate("pl", "wn8AboveAverage"),
      translate("pl", "wn8Good"),
      translate("pl", "wn8Great"),
      translate("pl", "wn8SuperUnicum"),
      translate("pl", "wn8NotAvailable"),
    ]).toEqual([
      "Bardzo słaby",
      "Powyżej średniej",
      "Dobry",
      "Świetny",
      "Super unicum",
      "Niedostępne",
    ]);
  });
});

describe("localized session language", () => {
  it.each([
    [0, "0 bitew"],
    [1, "1 bitwa"],
    [2, "2 bitwy"],
    [5, "5 bitew"],
    [12, "12 bitew"],
    [22, "22 bitwy"],
    [25, "25 bitew"],
  ] as const)("declines %i battles in Polish", (count, label) =>
    expect(battleCountLabel(count, "pl")).toBe(label),
  );
  it.each([
    [0, "0 battles"],
    [1, "1 battle"],
    [2, "2 battles"],
  ] as const)("pluralizes %i battles in English", (count, label) =>
    expect(battleCountLabel(count, "en")).toBe(label),
  );
  it("maps stored statuses without exposing unknown identifiers", () => {
    expect(sessionStatusLabel("timed_out", "pl")).toBe("Zakończona po bezczynności");
    expect(sessionStatusLabel("ended", "pl")).toBe("Zakończona");
    expect(sessionStatusLabel("timed_out", "en")).toBe("Ended after inactivity");
    expect(sessionStatusLabel("ended", "en")).toBe("Ended");
    expect(sessionStatusLabel("unexpected_raw_value", "pl")).toBe("Zakończona");
  });
});

describe("battle outcome presentation", () => {
  it.each([
    ["victory", "Victory", "Zwycięstwo", "result-win"],
    ["defeat", "Defeat", "Porażka", "result-loss"],
    ["draw", "Draw", "Remis", "result-draw"],
    ["unknown", "Unknown result", "Wynik nieznany", "result-unknown"],
  ] as const)(
    "maps %s centrally without treating draws as losses",
    (outcome, en, pl, className) => {
      expect(battleOutcomePresentation(outcome, "en")).toEqual({ label: en, className });
      expect(battleOutcomePresentation(outcome, "pl")).toEqual({ label: pl, className });
    },
  );
});

describe("battle pace presentation", () => {
  it.each([
    ["obvious_steamroll", "Obvious steamroll", "Ewidentny steamroll", true],
    ["borderline_steamroll", "Possible steamroll", "Możliwy steamroll", true],
    ["standard", "Standard under v1 criteria", "Standardowy przebieg według reguły v1", false],
    ["unknown", "Pace context unavailable", "Brak klasyfikacji przebiegu", false],
  ] as const)("maps %s centrally in English and Polish", (context, en, pl, showBadge) => {
    const pace = { ...battle().battlePace, context };
    expect(battlePacePresentation(pace, "en")).toMatchObject({ label: en, showBadge });
    expect(battlePacePresentation(pace, "pl")).toMatchObject({ label: pl, showBadge });
  });

  it("keeps a borderline result explicitly neutral and localizes closed reasons", () => {
    const pace = {
      ...battle().battlePace,
      context: "borderline_steamroll" as const,
      reason: "unsupported_finish_reason" as const,
    };
    expect(battlePacePresentation(pace, "en")).toMatchObject({
      label: "Possible steamroll",
      reason: "Finish reason is outside the supported v1 scope.",
    });
    expect(battlePacePresentation(pace, "pl")).toMatchObject({
      label: "Możliwy steamroll",
      reason: "Powód zakończenia jest poza wspieranym zakresem v1.",
    });
  });
});

describe("battle identity", () => {
  it("prefers parsed metadata over a technical fallback", () => {
    expect(
      battleIdentity(
        battle({
          tankName: "Official Tank",
          mapName: "Official Map",
          vehicleKey: "20200101_0000_france-F999_Test_99_synthetic_map.wotreplay",
          mapKey: "99_synthetic_map",
        }),
        "en",
      ),
    ).toEqual({ vehicle: "Official Tank", map: "Official Map" });
  });
  it("cleans technical vehicle and map keys", () => {
    expect(
      battleIdentity(
        battle({ vehicleKey: "france-F136_AMX_67_Imbattable", mapKey: "18_cliff" }),
        "en",
      ),
    ).toEqual({ vehicle: "AMX 67 Imbattable", map: "Cliff" });
    expect(readableMapKey("44_north_america")).toBe("North America");
  });
  it("uses basename and a known map key for replay fallback without leaking technical parts", () => {
    const value =
      "C:\\private\\20200101_0000_france-F999_Synthetic_Tank_99_synthetic_map.wotreplay";
    const result = replayFallback(value, "99_synthetic_map");
    expect(result).toEqual({ vehicle: "Synthetic Tank", map: "Synthetic Map" });
    expect(JSON.stringify(result)).not.toMatch(/20200101|0000|france-F999|wotreplay|private/i);
  });
  it("does not guess an ambiguous replay and localizes unknown labels", () => {
    const raw = "20200101_0000_france-F999_Unclear_Name.wotreplay";
    expect(replayFallback(raw, null)).toEqual({ vehicle: null, map: null });
    expect(battleIdentity(battle({ vehicleKey: raw }), "pl")).toEqual({
      vehicle: "Nieznany pojazd",
      map: "Nieznana mapa",
    });
    expect(battleIdentity(battle({ vehicleKey: raw }), "en")).toEqual({
      vehicle: "Unknown vehicle",
      map: "Unknown map",
    });
  });
  it.each([
    ["uk-GB147_FV4025_Contriver", "FV4025 Contriver"],
    ["france-F136_AMX_67_Imbattable", "AMX 67 Imbattable"],
    ["usa-A179_Black_Rock", "Black Rock"],
    ["germany-G177_E65_Zwilling", "E65 Zwilling"],
  ])("uses one safe vehicle display-name fallback for %s", (technical, expected) => {
    expect(vehicleDisplayName({ vehicleKey: technical }, "en")).toBe(expected);
  });
  it("keeps an unknown format readable, bounded and never empty", () => {
    expect(vehicleDisplayName({ vehicleKey: "Prototype_42" }, "en")).toBe("Prototype 42");
    expect(vehicleDisplayName({ vehicleKey: "" }, "en")).toBe("Unknown vehicle");
    expect(vehicleDisplayName({ vehicleKey: "nation:tank" }, "en")).toBe("Unknown vehicle");
    expect(vehicleDisplayName({ vehicleKey: "vehicle-42" }, "en")).toBe("Unknown vehicle");
    expect(vehicleDisplayName({ vehicleKey: "x".repeat(200) }, "en").length).toBeLessThanOrEqual(
      80,
    );
  });
  it("is presentation-only and leaves vehicle identity untouched", () => {
    const source = { vehicleId: 33345, vehicleKey: "usa-A179_Black_Rock" };
    expect(vehicleDisplayName(source, "en")).toBe("Black Rock");
    expect(source).toEqual({ vehicleId: 33345, vehicleKey: "usa-A179_Black_Rock" });
  });
});
