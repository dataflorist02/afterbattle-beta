import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  AttemptsConversionPanel,
  contributingLabel,
  zeroDenominatorMessage,
} from "./AttemptsConversion";
import { expectNoGluedNumbers } from "../../test/glued";
import type { AttemptsConversionReport } from "../../types";

const mocks = vi.hoisted(() => ({
  attemptsConversionSession: vi.fn(),
  attemptsConversionRecent: vi.fn(),
  AppCommandError: class AppCommandError extends Error {
    code: string;
    constructor(code = "mock_error") {
      super("mock");
      this.code = code;
    }
  },
}));
vi.mock("../../api", () => ({ AppCommandError: mocks.AppCommandError, api: mocks }));
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
] as const;
const metricCodes = [
  ...commonCodes,
  "missing_counters",
  "counter_out_of_range",
  "counter_conflict",
] as const;
const reasons = (
  codes: readonly (typeof metricCodes)[number][],
  active?: (typeof metricCodes)[number],
) => codes.map((code) => ({ code, count: code === active ? 1 : 0 }));
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
  rejectionReasons: reasons(metricCodes, valueSum === null ? "missing_counters" : undefined),
  ...overrides,
});
function report(overrides: Partial<AttemptsConversionReport> = {}): AttemptsConversionReport {
  return {
    contractVersion: "attempts-conversion-v2",
    scope: { type: "session", sessionId: "session-a" },
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
      missingBattles: 1,
      rejectionReasons: reasons(metricCodes, "missing_counters"),
    }),
    facts: { directTeamHits: fact(0), explosionHits: fact(1), damageDealt: fact(2_000) },
    ...overrides,
  };
}
function contributingReport(count: number): AttemptsConversionReport {
  const result = report();
  const completeRate = {
    ...rate(),
    selectedBattles: count,
    commonEligibleBattles: count,
    metricEligibleBattles: count,
    metricIneligibleBattles: 0,
    contributingBattles: count,
    numeratorSum: count,
    denominatorSum: count,
    value: 1,
    rejectionReasons: reasons(metricCodes),
  };
  const completeFact = (valueSum: number) => ({
    ...fact(valueSum),
    selectedBattles: count,
    commonEligibleBattles: count,
    factEligibleBattles: count,
    factIneligibleBattles: 0,
    contributingBattles: count,
    rejectionReasons: reasons(metricCodes),
  });
  result.selectedBattles = count;
  result.commonContext = {
    eligibleBattles: count,
    ineligibleBattles: 0,
    rejectionReasons: reasons(commonCodes),
  };
  result.hitRate = completeRate;
  result.penetrationRate = completeRate;
  result.facts = {
    directTeamHits: completeFact(0),
    explosionHits: completeFact(0),
    damageDealt: completeFact(count),
  };
  return result;
}
describe("AttemptsConversionPanel v2", () => {
  beforeEach(() => {
    mocks.attemptsConversionSession.mockReset().mockResolvedValue(report());
    mocks.attemptsConversionRecent
      .mockReset()
      .mockResolvedValue(report({ scope: { type: "recent", window: 10 } }));
  });
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });
  it("renders the three-step funnel without a false penetration zero", async () => {
    const { container } = render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
    expect(await screen.findByText(/Supported context: 1 of 1 battles/)).toBeInTheDocument();
    const funnel = screen.getByRole("list", { name: "Shots 10, hits 6, penetrations 0" });
    const steps = Array.from(funnel.querySelectorAll(".funnel-step")).map((step) =>
      Array.from(step.querySelectorAll("p")).map((line) => line.textContent),
    );
    expect(steps).toEqual([
      ["10", "Shots"],
      ["6", "Hits"],
      ["—", "Penetrations"],
    ]);
    expect(screen.getByText("60% hit")).toBeInTheDocument();
    expect(screen.getByText("no rate")).toBeInTheDocument();
    expect(
      screen.getByText("Team hits: 0 · Explosion hits: 1 · Damage: 2,000"),
    ).toBeInTheDocument();
    expectNoGluedNumbers(container);
  });

  it("never glues a number to its label, in Polish either", async () => {
    mocks.attemptsConversionSession.mockResolvedValue(contributingReport(57));
    const { container } = render(<AttemptsConversionPanel language="pl" sessionId="session-a" />);
    expect(await screen.findByText("100% trafia")).toBeInTheDocument();
    expect(screen.getByText("100% przebija")).toBeInTheDocument();
    expectNoGluedNumbers(container);
    fireEvent.click(screen.getByRole("button", { name: "Jak to liczymy" }));
    expectNoGluedNumbers(screen.getByRole("dialog"));
  });
  it("does not render calculated-from prose for zero contributing battles", async () => {
    render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
    await screen.findByText("Shots");
    expect(screen.queryByText("Calculated from 0 battles")).toBeNull();
  });
  it("explains every skipped battle in one dialog, per indicator, without zero-count reasons", async () => {
    render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
    await screen.findByText("Shots");
    expect(screen.queryByText(/Why were some battles skipped/)).toBeNull();
    expect(document.querySelector("details")).toBeNull();
    expect(screen.getAllByRole("button", { name: "How we count this" })).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "How we count this" }));
    const dialog = screen.getByRole("dialog", { name: "How we count this" });
    expect(dialog).toHaveTextContent("Penetration rate among direct hits 1 left out");
    expect(dialog).toHaveTextContent("Required counters unavailable: 1");
    expect(screen.queryByTestId("attempts-skipped-hits")).toBeNull();
    expect(screen.queryByText(/Older data version/)).toBeNull();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });
  it.each([
    [1, "Policzono z 1 bitwy"],
    [2, "Policzono z 2 bitew"],
    [5, "Policzono z 5 bitew"],
    [12, "Policzono z 12 bitew"],
    [22, "Policzono z 22 bitew"],
  ])("uses Polish contributing grammar for %i", (count, expected) => {
    expect(contributingLabel(count, "pl")).toBe(expected);
  });
  it.each([
    [1, "Calculated from 1 battle"],
    [2, "Calculated from 2 battles"],
  ])("uses English contributing grammar for %i", (count, expected) => {
    expect(contributingLabel(count, "en")).toBe(expected);
  });
  it.each([
    [1, "Policzono z 1 bitwy"],
    [2, "Policzono z 2 bitew"],
    [5, "Policzono z 5 bitew"],
    [12, "Policzono z 12 bitew"],
    [22, "Policzono z 22 bitew"],
  ])("renders the Polish contributing form for %i in both rate panels", async (count, expected) => {
    mocks.attemptsConversionSession.mockResolvedValue(contributingReport(count));
    render(<AttemptsConversionPanel language="pl" sessionId="session-a" />);
    expect(await screen.findAllByText(expected)).toHaveLength(2);
    if (count === 2) expect(screen.queryByText("Policzono z 2 bitwy")).toBeNull();
    if (count === 22) expect(screen.queryByText("Policzono z 22 bitwy")).toBeNull();
  });
  it.each([
    [1, "Calculated from 1 battle"],
    [2, "Calculated from 2 battles"],
  ])(
    "renders the English contributing form for %i in both rate panels",
    async (count, expected) => {
      mocks.attemptsConversionSession.mockResolvedValue(contributingReport(count));
      render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
      expect(await screen.findAllByText(expected)).toHaveLength(2);
      if (count === 1) expect(screen.queryByText("Calculated from 1 battles")).toBeNull();
    },
  );
  it("does not render zero contributing forms", async () => {
    render(<AttemptsConversionPanel language="pl" sessionId="session-a" />);
    await screen.findByText("Strzały");
    expect(screen.queryByText("Policzono z 0 bitew")).toBeNull();
    expect(contributingLabel(2, "pl")).not.toBe("Policzono z 2 bitwy");
    expect(contributingLabel(22, "pl")).not.toBe("Policzono z 22 bitwy");
    expect(contributingLabel(1, "en")).not.toBe("Calculated from 1 battles");
  });
  it.each(["shots", "enemyHits"] as const)(
    "keeps zero-denominator prose independent for %s",
    (kind) => {
      expect(zeroDenominatorMessage(kind, 1, "en")).toContain("1 battle");
    },
  );
  it("renders the sanitized invalid-report state instead of a rejected payload", async () => {
    mocks.attemptsConversionSession.mockRejectedValue(
      new mocks.AppCommandError("report_invariant_violation"),
    );
    render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
    expect(await screen.findByText("The report has an invalid format.")).toBeInTheDocument();
    expect(screen.queryByText("missing_counters")).toBeNull();
  });
  it("retains empty, scope switching, and stale response protection", async () => {
    let resolveOld!: (value: AttemptsConversionReport) => void;
    mocks.attemptsConversionSession.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveOld = resolve;
        }),
    );
    render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
    await waitFor(() => expect(mocks.attemptsConversionSession).toHaveBeenCalled());
    fireEvent.click(screen.getByRole("radio", { name: "Last 30 battles" }));
    await waitFor(() => expect(mocks.attemptsConversionRecent).toHaveBeenCalledWith(30));
    resolveOld(report());
    expect(screen.getByRole("radio", { name: "Last 30 battles" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("is a segmented radio group moved with the arrow keys", async () => {
    render(<AttemptsConversionPanel language="en" sessionId="session-a" />);
    await screen.findByText("Shots");
    const group = screen.getByRole("radiogroup", { name: "Analysis scope" });
    const radios = screen.getAllByRole("radio");
    expect(group).toContainElement(radios[0]);
    expect(radios.map((radio) => radio.getAttribute("tabindex"))).toEqual(["0", "-1", "-1", "-1"]);
    fireEvent.keyDown(radios[0], { key: "ArrowRight" });
    await waitFor(() => expect(mocks.attemptsConversionRecent).toHaveBeenCalledWith(10));
    const ten = screen.getByRole("radio", { name: "Last 10 battles" });
    expect(ten).toHaveAttribute("aria-checked", "true");
    expect(document.activeElement).toBe(ten);
    fireEvent.keyDown(ten, { key: "ArrowLeft" });
    fireEvent.keyDown(screen.getByRole("radio", { name: "Session" }), { key: "ArrowLeft" });
    expect(screen.getByRole("radio", { name: "Last 100 battles" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    fireEvent.keyDown(screen.getByRole("radio", { name: "Last 100 battles" }), { key: "Home" });
    expect(screen.getByRole("radio", { name: "Session" })).toHaveAttribute("aria-checked", "true");
  });
});
