import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BattlePaceSummaryPanel } from "../components/session/BattlePace";
import { DamageChart, limitPoints, rollingAverage } from "../components/DamageChart";
import { LanguageProvider } from "../i18n/language";
import * as fx from "../test/fixtures";
import { expectNoGluedNumbers } from "../test/glued";
import type { BackendEvent } from "../types";
import { isPublicBuild } from "./buildFlags";
import { Navigation } from "./Navigation";

const mocks = vi.hoisted(() => ({
  handlers: [] as ((event: BackendEvent) => void)[],
  snapshot: vi.fn(),
  sessionSummary: vi.fn(),
  sessionBaselineComparison: vi.fn(),
  cancelImport: vi.fn(),
  gameImage: vi.fn(),
  attemptsConversionSession: vi.fn(() => new Promise(() => {})),
  attemptsConversionRecent: vi.fn(() => new Promise(() => {})),
}));
vi.mock("../api", () => ({
  api: {
    ...mocks,
    onEvents: (handler: (event: BackendEvent) => void) => {
      mocks.handlers.push(handler);
      return () => undefined;
    },
  },
}));
const { App } = await import("./App");

async function flush() {
  for (let index = 0; index < 6; index++) await act(async () => Promise.resolve());
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.handlers.length = 0;
  mocks.snapshot.mockResolvedValue(fx.snapshot());
  mocks.sessionSummary.mockResolvedValue(fx.summary());
  mocks.sessionBaselineComparison.mockResolvedValue(fx.comparison());
  mocks.cancelImport.mockResolvedValue(undefined);
  mocks.gameImage.mockResolvedValue(new ArrayBuffer(0));
});
afterEach(cleanup);

function renderApp() {
  return render(
    <LanguageProvider initial="en">
      <App />
    </LanguageProvider>,
  );
}

describe("backend events (D6, A6)", () => {
  it("shows import progress from the event and stops the import on request", async () => {
    renderApp();
    await flush();
    await act(async () =>
      mocks.handlers.forEach((handler) =>
        handler({
          name: "import-progress",
          payload: { done: 300, total: 2000, finished: false, cancelled: false },
        }),
      ),
    );
    const status = screen
      .getAllByRole("status")
      .find((element) => element.textContent?.includes("300"));
    expect(status).toHaveTextContent("Reading battle files: 300 of 2,000");
    fireEvent.click(screen.getByRole("button", { name: "Stop reading" }));
    expect(mocks.cancelImport).toHaveBeenCalledTimes(1);
  });

  it("reloads on data-changed and session-ended instead of polling", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    renderApp();
    await flush();
    const before = mocks.snapshot.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(60_000);
    });
    expect(mocks.snapshot.mock.calls.length).toBe(before);
    await act(async () => mocks.handlers.forEach((handler) => handler({ name: "data-changed" })));
    await act(async () =>
      mocks.handlers.forEach((handler) =>
        handler({ name: "session-ended", payload: { sessionId: "session-1" } }),
      ),
    );
    await flush();
    expect(mocks.snapshot.mock.calls.length).toBe(before + 2);
    vi.useRealTimers();
  });
});

describe("build flag for Maps (D18)", () => {
  it("reads the public flag", () => {
    expect(isPublicBuild({ VITE_PUBLIC_BUILD: "1" })).toBe(true);
    expect(isPublicBuild({ VITE_PUBLIC_BUILD: "true" })).toBe(true);
    expect(isPublicBuild({})).toBe(false);
    expect(isPublicBuild({ VITE_PUBLIC_BUILD: "0" })).toBe(false);
  });

  it.each([
    [false, true],
    [true, false],
  ])("public build %s shows Maps: %s", (publicBuild, visible) => {
    render(
      <LanguageProvider initial="pl">
        <Navigation page="session" onNavigate={() => undefined} publicBuild={publicBuild} />
      </LanguageProvider>,
    );
    const nav = screen.getByRole("navigation");
    expect(within(nav).queryByRole("button", { name: /Mapy/ }) !== null).toBe(visible);
  });
});

describe("damage chart (old E6)", () => {
  it("handles 0, 1 and 200 points and caps longer series at 200", () => {
    for (const [count, rendered] of [
      [0, false],
      [1, false],
      [200, true],
    ] as const) {
      cleanup();
      render(
        <LanguageProvider initial="en">
          <DamageChart values={Array.from({ length: count }, (_, index) => 1000 + index)} />
        </LanguageProvider>,
      );
      expect(screen.queryByRole("img") !== null, String(count)).toBe(rendered);
    }
    const long = Array.from({ length: 1000 }, (_, index) => index);
    const limited = limitPoints(long);
    expect(limited).toHaveLength(200);
    expect(limited[0]).toBe(0);
    expect(limited[199]).toBe(999);
    expect(rollingAverage([10, 20, 30], 2)).toEqual([10, 15, 25]);
  });
});

describe("battle pace panel (T4)", () => {
  it("says in one line how many battles were one-sided and splits them in words and a bar", () => {
    const { container } = render(
      <BattlePaceSummaryPanel summary={fx.summary(8).battlePace} language="pl" />,
    );
    expect(screen.getByText("2 z 8 bitew miało jednostronny przebieg")).toBeInTheDocument();
    expect(
      screen.getByRole("img", {
        name: "Bitwy według przebiegu: ewidentne 1, graniczne 1, standardowe 6, niedostępne 0",
      }),
    ).toBeInTheDocument();
    const legend = Array.from(container.querySelectorAll(".pace-legend li")).map(
      (item) => item.textContent,
    );
    expect(legend).toEqual(["Ewidentne 1", "Graniczne 1", "Standardowe 6", "Niedostępne 0"]);
    expect(container.querySelectorAll(".pace-part")).toHaveLength(3);
    expect(container.querySelector("details")).toBeNull();
    expectNoGluedNumbers(container);
  });

  it("names partial classification and the empty session", () => {
    render(
      <BattlePaceSummaryPanel
        summary={{
          totalBattles: 8,
          obviousSteamrolls: 0,
          borderlineSteamrolls: 0,
          standardBattles: 5,
          unknownBattles: 3,
          classifiedBattles: 5,
          coverageRatio: 0.625,
        }}
        language="pl"
      />,
    );
    expect(
      screen.getByText("Żadna z 5 sklasyfikowanych bitew nie miała jednostronnego przebiegu"),
    ).toBeInTheDocument();
    cleanup();
    render(<BattlePaceSummaryPanel summary={fx.summary(0).battlePace} language="en" />);
    expect(screen.queryByRole("img")).toBeNull();
  });
});
