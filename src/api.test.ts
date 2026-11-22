import { describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
import { AppCommandError, api, parseAttemptsConversionReport } from "./api";
import * as fx from "./test/fixtures";

describe("backend events", () => {
  it("listens to the three events of spec 20 and stops listening on unsubscribe", async () => {
    const callbacks = new Map<string, (event: { payload: unknown }) => void>();
    const stops = new Map<string, ReturnType<typeof vi.fn>>();
    mocks.listen.mockImplementation(
      async (name: string, callback: (event: { payload: unknown }) => void) => {
        callbacks.set(name, callback);
        const stop = vi.fn();
        stops.set(name, stop);
        return stop;
      },
    );
    const received: unknown[] = [];
    const unsubscribe = api.onEvents((event) => received.push(event));
    expect([...callbacks.keys()]).toEqual(["import-progress", "data-changed", "session-ended"]);
    const progress = { done: 100, total: 2000, finished: false, cancelled: false };
    callbacks.get("import-progress")!({ payload: progress });
    callbacks.get("data-changed")!({ payload: {} });
    callbacks.get("session-ended")!({ payload: { sessionId: "s" } });
    expect(received).toEqual([
      { name: "import-progress", payload: progress },
      { name: "data-changed" },
      { name: "session-ended", payload: { sessionId: "s" } },
    ]);
    unsubscribe();
    await Promise.resolve();
    await Promise.resolve();
    for (const stop of stops.values()) expect(stop).toHaveBeenCalledTimes(1);
  });

  it("starts and cancels the import with the new commands", async () => {
    mocks.invoke.mockResolvedValue(undefined);
    await api.startImport();
    await api.cancelImport();
    expect(mocks.invoke.mock.calls.map((call) => call[0])).toEqual([
      "start_import",
      "cancel_import",
    ]);
    mocks.invoke.mockReset();
  });
});

const commonCodes = [
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
const metricCodes = [
  ...commonCodes,
  "missing_counters",
  "counter_out_of_range",
  "counter_conflict",
];
const reasons = (codes: readonly string[], counts: Record<string, number> = {}) =>
  codes.map((code) => ({ code, count: counts[code] ?? 0 }));
const rate = (overrides = {}) => ({
  selectedBattles: 1,
  commonEligibleBattles: 1,
  metricEligibleBattles: 1,
  metricIneligibleBattles: 0,
  contributingBattles: 1,
  zeroDenominatorBattles: 0,
  missingBattles: 0,
  invalidBattles: 0,
  numeratorSum: 6,
  denominatorSum: 10,
  value: 0.6,
  rejectionReasons: reasons(metricCodes),
  ...overrides,
});
const fact = (valueSum: number | null, overrides = {}) => ({
  selectedBattles: 1,
  commonEligibleBattles: 1,
  factEligibleBattles: valueSum === null ? 0 : 1,
  factIneligibleBattles: valueSum === null ? 1 : 0,
  contributingBattles: valueSum === null ? 0 : 1,
  missingBattles: valueSum === null ? 1 : 0,
  invalidBattles: 0,
  valueSum,
  rejectionReasons: reasons(metricCodes, valueSum === null ? { missing_counters: 1 } : {}),
  ...overrides,
});
const wire = () => ({
  contractVersion: "attempts-conversion-v2",
  scope: { type: "recent", window: 10 },
  selectedBattles: 1,
  commonContext: {
    eligibleBattles: 1,
    ineligibleBattles: 0,
    rejectionReasons: reasons(commonCodes),
  },
  hitRate: rate(),
  penetrationRate: rate({
    metricEligibleBattles: 0,
    metricIneligibleBattles: 1,
    contributingBattles: 0,
    numeratorSum: 0,
    denominatorSum: 0,
    value: null,
    rejectionReasons: reasons(metricCodes, { missing_counters: 1 }),
    missingBattles: 1,
  }),
  facts: { directTeamHits: fact(0), explosionHits: fact(1), damageDealt: fact(2000) },
});

function fullyEligibleWire(selectedBattles: number) {
  const fullRate = () => ({
    selectedBattles,
    commonEligibleBattles: selectedBattles,
    metricEligibleBattles: selectedBattles,
    metricIneligibleBattles: 0,
    contributingBattles: selectedBattles,
    zeroDenominatorBattles: 0,
    missingBattles: 0,
    invalidBattles: 0,
    numeratorSum: selectedBattles,
    denominatorSum: selectedBattles,
    value: 1 as number | null,
    rejectionReasons: reasons(metricCodes),
  });
  const fullFact = () => ({
    selectedBattles,
    commonEligibleBattles: selectedBattles,
    factEligibleBattles: selectedBattles,
    factIneligibleBattles: 0,
    contributingBattles: selectedBattles,
    missingBattles: 0,
    invalidBattles: 0,
    valueSum: 0 as number | null,
    rejectionReasons: reasons(metricCodes),
  });
  return {
    contractVersion: "attempts-conversion-v2",
    scope: { type: "recent", window: 10 },
    selectedBattles,
    commonContext: {
      eligibleBattles: selectedBattles,
      ineligibleBattles: 0,
      rejectionReasons: reasons(commonCodes),
    },
    hitRate: fullRate(),
    penetrationRate: fullRate(),
    facts: { directTeamHits: fullFact(), explosionHits: fullFact(), damageDealt: fullFact() },
  };
}

function rejectRate(
  report: ReturnType<typeof fullyEligibleWire>,
  key: "hitRate" | "penetrationRate",
  counts: Record<string, number>,
  missingBattles: number,
  invalidBattles: number,
) {
  const selectedBattles = Object.values(counts).reduce((total, count) => total + count, 0);
  report.selectedBattles = selectedBattles;
  report.commonContext.eligibleBattles = selectedBattles;
  const full = fullyEligibleWire(selectedBattles);
  report.penetrationRate = full.penetrationRate;
  report.hitRate = full.hitRate;
  report.facts = full.facts;
  report[key] = {
    selectedBattles,
    commonEligibleBattles: selectedBattles,
    metricEligibleBattles: 0,
    metricIneligibleBattles: selectedBattles,
    contributingBattles: 0,
    zeroDenominatorBattles: 0,
    missingBattles,
    invalidBattles,
    numeratorSum: 0,
    denominatorSum: 0,
    value: null,
    rejectionReasons: reasons(metricCodes, counts),
  };
}

function rejectFact(
  report: ReturnType<typeof fullyEligibleWire>,
  key: "directTeamHits" | "explosionHits" | "damageDealt",
  counts: Record<string, number>,
  missingBattles: number,
  invalidBattles: number,
) {
  const selectedBattles = Object.values(counts).reduce((total, count) => total + count, 0);
  report.selectedBattles = selectedBattles;
  report.commonContext.eligibleBattles = selectedBattles;
  const full = fullyEligibleWire(selectedBattles);
  report.hitRate = full.hitRate;
  report.penetrationRate = full.penetrationRate;
  report.facts = full.facts;
  report.facts[key] = {
    selectedBattles,
    commonEligibleBattles: selectedBattles,
    factEligibleBattles: 0,
    factIneligibleBattles: selectedBattles,
    contributingBattles: 0,
    missingBattles,
    invalidBattles,
    valueSum: null,
    rejectionReasons: reasons(metricCodes, counts),
  };
}

describe("attempts conversion v2 API boundary", () => {
  it("uses exact existing commands and parses the metric-specific golden report", async () => {
    mocks.invoke.mockResolvedValue(wire());
    await api.attemptsConversionSession("session-a");
    await api.attemptsConversionRecent(10);
    await api.attemptsConversionRecent(30);
    await api.attemptsConversionRecent(100);
    expect(mocks.invoke).toHaveBeenNthCalledWith(1, "get_attempts_conversion_session_report", {
      sessionId: "session-a",
    });
    expect(parseAttemptsConversionReport(wire()).facts.directTeamHits.valueSum).toBe(0);
    expect(parseAttemptsConversionReport(wire()).penetrationRate.value).toBeNull();
    expect(parseAttemptsConversionReport(fx.attemptsReport()).hitRate.numeratorSum).toBe(41);
  });
  it("accepts only the camelCase session scope field", () => {
    const session = { ...wire(), scope: { type: "session", sessionId: "session-a" } };
    expect(parseAttemptsConversionReport(session).scope).toEqual({
      type: "session",
      sessionId: "session-a",
    });
    expect(() =>
      parseAttemptsConversionReport({
        ...wire(),
        scope: { type: "session", session_id: "session-a" },
      }),
    ).toThrow(AppCommandError);
  });
  it("fails closed for v1, bad reason catalogs, invalid counts, ratios, null coercion, non-finite values, and unsafe reason sums", () => {
    const mutations = [
      (report: ReturnType<typeof wire>) => {
        report.contractVersion = "attempts-conversion-v1";
      },
      (report: ReturnType<typeof wire>) => {
        report.commonContext.rejectionReasons[0].code = "unknown";
      },
      (report: ReturnType<typeof wire>) => {
        report.hitRate.metricEligibleBattles = 2;
      },
      (report: ReturnType<typeof wire>) => {
        report.hitRate.value = Number.NaN;
      },
      (report: ReturnType<typeof wire>) => {
        report.facts.damageDealt.valueSum = 0;
        report.facts.damageDealt.factEligibleBattles = 0;
      },
      (report: ReturnType<typeof wire>) => {
        report.hitRate.rejectionReasons[11].count = Number.MAX_SAFE_INTEGER;
        report.hitRate.rejectionReasons[12].count = 1;
      },
    ];
    for (const mutate of mutations) {
      const invalid = wire();
      mutate(invalid);
      expect(() => parseAttemptsConversionReport(invalid)).toThrow(AppCommandError);
    }
  });
  it("rejects the original missing_counters=1 with missingBattles=0 reproduction", () => {
    const report = fullyEligibleWire(1);
    rejectRate(report, "hitRate", { missing_counters: 1 }, 0, 0);
    expect(() => parseAttemptsConversionReport(report)).toThrow(AppCommandError);
  });
  it("rejects the original counter_conflict=1 with invalidBattles=0 reproduction", () => {
    const report = fullyEligibleWire(1);
    rejectRate(report, "penetrationRate", { counter_conflict: 1 }, 0, 0);
    expect(() => parseAttemptsConversionReport(report)).toThrow(AppCommandError);
  });
  it("rejects every missing and invalid rate mismatch while accepting exact reason counts", () => {
    const rejected = [
      ["hitRate", { counter_out_of_range: 1 }, 0, 0],
      ["penetrationRate", { partial_replay: 1 }, 1, 0],
      ["hitRate", { partial_replay: 1 }, 0, 1],
    ] as const;
    for (const [key, counts, missingBattles, invalidBattles] of rejected) {
      const report = fullyEligibleWire(1);
      rejectRate(report, key, counts, missingBattles, invalidBattles);
      expect(() => parseAttemptsConversionReport(report)).toThrow(AppCommandError);
    }
    const valid = fullyEligibleWire(5);
    rejectRate(
      valid,
      "hitRate",
      { missing_counters: 2, counter_out_of_range: 1, counter_conflict: 2 },
      2,
      3,
    );
    expect(
      parseAttemptsConversionReport(JSON.parse(JSON.stringify(valid))).hitRate.invalidBattles,
    ).toBe(3);
  });
  it("rejects each fact mismatch while accepting exact fact reason counts", () => {
    const rejected = [
      ["directTeamHits", { missing_counters: 1 }, 0, 0],
      ["explosionHits", { counter_out_of_range: 1 }, 0, 0],
      ["damageDealt", { counter_conflict: 1 }, 0, 0],
    ] as const;
    for (const [key, counts, missingBattles, invalidBattles] of rejected) {
      const report = fullyEligibleWire(1);
      rejectFact(report, key, counts, missingBattles, invalidBattles);
      expect(() => parseAttemptsConversionReport(report)).toThrow(AppCommandError);
    }
    const valid = fullyEligibleWire(3);
    rejectFact(
      valid,
      "damageDealt",
      { missing_counters: 1, counter_out_of_range: 1, counter_conflict: 1 },
      1,
      2,
    );
    expect(
      parseAttemptsConversionReport(JSON.parse(JSON.stringify(valid))).facts.damageDealt
        .invalidBattles,
    ).toBe(2);
  });
});

describe("Damast files (spec 34 §3–§4)", () => {
  it("sends a tile and the card as raw bytes with their names in headers, nothing else", async () => {
    mocks.invoke.mockReset();
    mocks.invoke.mockResolvedValue(true);
    const jpeg = new Uint8Array([0xff, 0xd8, 0xff, 0xd9]);
    await api.putDamastTile("damast-1", "0123456789abcdef", jpeg);
    expect(mocks.invoke).toHaveBeenLastCalledWith("put_damast_tile", jpeg, {
      headers: { "x-damast-version": "damast-1", "x-damast-key": "0123456789abcdef" },
    });
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
    await expect(api.exportDamastCard("2026-11-21", png)).resolves.toBe(true);
    expect(mocks.invoke).toHaveBeenLastCalledWith("export_damast_card", png, {
      headers: { "x-damast-date": "2026-11-21" },
    });
    await api.damastTile("damast-1", "0123456789abcdef");
    expect(mocks.invoke).toHaveBeenLastCalledWith("get_damast_tile", {
      version: "damast-1",
      key: "0123456789abcdef",
    });
  });
});
