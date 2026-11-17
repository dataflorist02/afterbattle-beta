import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";
import { sessionSentences } from "../lib/recap/session";
import * as fx from "../test/fixtures";
import { expectNoGluedNumbers } from "../test/glued";
import type { Language } from "../types";

const mocks = vi.hoisted(() => ({
  snapshot: vi.fn(),
  sessionSummary: vi.fn(),
  sessionBaselineComparison: vi.fn(),
  battleView: vi.fn(),
  vehicleHistoryOverview: vi.fn(),
  vehicleBaseline: vi.fn(),
  vehicleHistoryReport: vi.fn(),
  performanceTrends: vi.fn(),
  battleHistory: vi.fn(),
  recentSessionsWn8: vi.fn(),
  challengeSuggestions: vi.fn(),
  challengeHistory: vi.fn(),
  abandonActiveChallenge: vi.fn(),
  vehicleClassCatalogStatus: vi.fn(),
  diagnostics: vi.fn(),
  deleteAllData: vi.fn(),
  gameImage: vi.fn(),
  vehicleNames: vi.fn(),
  sessionStory: vi.fn(),
  recap: vi.fn(),
  mapRecord: vi.fn(),
  sessionMaps: vi.fn(),
  weeklyReport: vi.fn(),
  endSession: vi.fn(),
  startSession: vi.fn(),
  cancelImport: vi.fn(),
  detectInstallations: vi.fn(),
  pickGameFolder: vi.fn(),
  configureLocal: vi.fn(),
  startVehicleChallenge: vi.fn(),
  attemptsConversionSession: vi.fn(() => new Promise(() => {})),
  attemptsConversionRecent: vi.fn(() => new Promise(() => {})),
  onEvents: vi.fn<(handler: (event: { name: string; payload?: unknown }) => void) => () => void>(
    () => () => {},
  ),
}));
vi.mock("../api", () => ({ api: mocks }));

const { App } = await import("./App");
const { lastBattle, mapTag } = await import("../pages/TanksPage");
const { clearGameImages } = await import("../hooks/useGameImage");

function setDefaults() {
  mocks.snapshot.mockResolvedValue(fx.snapshot());
  mocks.sessionSummary.mockResolvedValue(fx.summary());
  mocks.sessionBaselineComparison.mockResolvedValue(fx.comparison());
  mocks.battleView.mockResolvedValue(fx.battleDetail());
  mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview());
  mocks.vehicleBaseline.mockResolvedValue(fx.vehicleBaseline());
  mocks.vehicleHistoryReport.mockResolvedValue(fx.vehicleReport());
  mocks.performanceTrends.mockResolvedValue(fx.trends());
  mocks.battleHistory.mockResolvedValue(fx.historyPage());
  mocks.recentSessionsWn8.mockResolvedValue([]);
  mocks.challengeSuggestions.mockResolvedValue({ suggestions: [], unavailableReason: null });
  mocks.challengeHistory.mockResolvedValue({ items: [], nextOffset: null });
  mocks.vehicleClassCatalogStatus.mockResolvedValue({ vehicleCount: 1270, sourceVersion: "2.0" });
  mocks.diagnostics.mockResolvedValue("{}");
  mocks.deleteAllData.mockResolvedValue(undefined);
  mocks.gameImage.mockResolvedValue(new ArrayBuffer(0));
  mocks.vehicleNames.mockResolvedValue({ language: null, names: {} });
  mocks.sessionStory.mockResolvedValue(fx.sessionStory());
  mocks.recap.mockResolvedValue(fx.sessionRecap());
  mocks.mapRecord.mockResolvedValue(fx.mapRecord());
  mocks.weeklyReport.mockResolvedValue(fx.weeklyReport());
  mocks.sessionMaps.mockImplementation((ids: string[]) =>
    Promise.resolve(
      ids.map((sessionId) => ({ sessionId, mapKeys: ["91_test_valley", "92_synthetic_hills"] })),
    ),
  );
  mocks.abandonActiveChallenge.mockResolvedValue({});
  mocks.detectInstallations.mockResolvedValue([]);
  mocks.pickGameFolder.mockResolvedValue(null);
  mocks.configureLocal.mockResolvedValue(fx.snapshot().sourceStatus);
}

/** A battle tile or row by its tank; summary references ("Fakt F7: … Alpha") are not battles. */
const BATTLE_BUTTON = /^(?!Fakt ).*(Alpha|Bravo)/;

async function flush() {
  for (let index = 0; index < 6; index++) await act(async () => Promise.resolve());
}

async function renderApp(language: Language = "pl", strict = false) {
  const app = (
    <LanguageProvider initial={language}>
      <App />
    </LanguageProvider>
  );
  const view = render(strict ? <StrictMode>{app}</StrictMode> : app);
  await flush();
  return view;
}

/** One animation frame (and the microtasks after it) inside act. */
async function frame() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
  await flush();
}

async function go(label: string) {
  fireEvent.click(screen.getByRole("button", { name: label }));
  await flush();
  await frame();
}

function expectNoCodes(container: HTMLElement) {
  expect(container.textContent ?? "").not.toMatch(/\b[a-z]+_[a-z_]+\b/);
}

beforeEach(() => {
  vi.clearAllMocks();
  // A test that chose an address form must not leak it into the next one.
  localStorage.removeItem("afterbattle-address");
  setDefaults();
});
afterEach(cleanup);

describe("navigation and footer", () => {
  it("shows the app icon at the top of the rail, not a letter (spec 29 §4)", async () => {
    const { container } = await renderApp();
    const logo = container.querySelector(".rail-logo")!;
    expect(logo).toHaveAttribute("aria-hidden", "true");
    expect(logo.textContent).toBe("");
    const icon = logo.querySelector("svg")!;
    expect(icon).toHaveAttribute("width", "40");
    expect(icon.querySelector("radialGradient")).not.toBeNull();
  });

  it("shows the six destinations with Maps greyed out in the private build", async () => {
    await renderApp();
    const nav = screen.getByRole("navigation", { name: "Główna nawigacja" });
    const names = within(nav)
      .getAllByRole("button")
      .map((button) => button.textContent?.trim());
    expect(names[0]).toContain("Sesja");
    expect(names.some((name) => name?.startsWith("Mapy"))).toBe(true);
    const maps = within(nav).getByRole("button", { name: /Mapy/ });
    expect(maps).toHaveAttribute("aria-disabled", "true");
    expect(within(nav).getByRole("button", { name: "Sesja" })).toHaveAttribute(
      "aria-current",
      "page",
    );
  });

  it("shows the XVM version once, in the footer", async () => {
    await renderApp();
    expect(screen.getByRole("contentinfo")).toHaveTextContent("XVM · 2026-11-11");
    expect(screen.getAllByText(/2026-11-11/)).toHaveLength(1);
  });
});

describe("Session screen", () => {
  it("shows the session summary under the numbers with a reference after each sentence (spec 30 §8)", async () => {
    await renderApp();
    expect(mocks.recap).toHaveBeenCalledWith("session", fx.summary().session.id);
    const card = screen.getByRole("region", { name: "Podsumowanie sesji" });
    const sentences = within(card).getAllByRole("listitem");
    expect(sentences.length).toBeGreaterThanOrEqual(3);
    expect(sentences.length).toBeLessThanOrEqual(5);
    expect(card.textContent).toMatch(/9 bitew/i);
    expect(card.textContent).not.toMatch(/łeś|łaś/);
    expect(
      within(card).getByText(/Złożone z Twoich liczb według stałych reguł\./),
    ).toBeInTheDocument();
    const wins = within(card).getByRole("button", { name: "Fakt F2: wygrane 6 z 9 (67%)" });
    const figure = screen.getByText("6 z 9").closest(".figure")!;
    fireEvent.mouseEnter(wins);
    expect(figure).toHaveClass("recap-highlight");
    fireEvent.mouseLeave(wins);
    expect(figure).not.toHaveClass("recap-highlight");
    // Survival has no number on the screen, so its reference shows a tooltip.
    const survival = within(card).getByRole("button", { name: /^Fakt F6: przetrwanie do końca/ });
    fireEvent.focus(survival);
    expect(within(card).getByRole("tooltip")).toHaveTextContent(/3 z 9 bitew/);
    fireEvent.blur(survival);
    expect(within(card).queryByRole("tooltip")).not.toBeInTheDocument();
    expectNoCodes(document.body);
  });

  it("follows the address setting in the summary", async () => {
    localStorage.setItem("afterbattle-address", "feminine");
    // A seed whose text has a verb (the neutral variants have none).
    const recap = fx.sessionRecap();
    const facts = recap.kind === "session" ? recap.facts : [];
    const seed = Array.from({ length: 50 }, (_, value) => value).find((value) =>
      sessionSentences({ seed: value, facts }, { address: "feminine", tank: (key) => key }).some(
        (sentence) => /łaś/u.test(sentence.text),
      ),
    )!;
    mocks.recap.mockResolvedValue({ ...recap, seed });
    await renderApp();
    const card = screen.getByRole("region", { name: "Podsumowanie sesji" });
    expect(card.textContent).toMatch(/łaś/);
    expect(card.textContent).not.toMatch(/łeś/);
  });

  it("hides the summary card in English (spec 30 §2)", async () => {
    await renderApp("en");
    expect(screen.queryByRole("region", { name: /summary/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Podsumowanie sesji" })).not.toBeInTheDocument();
  });

  it("leaves the summary out when the facts cannot be read", async () => {
    mocks.recap.mockRejectedValue({ code: "database_error" });
    await renderApp();
    expect(screen.queryByRole("region", { name: "Podsumowanie sesji" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Momenty sesji" })).toBeInTheDocument();
  });

  it("shows the hero average, the comparison and the battles", async () => {
    await renderApp();
    expect(
      screen.getByRole("heading", { level: 1, name: /Mocna sesja|Dzisiejsza sesja/ }),
    ).toBeInTheDocument();
    expect(screen.getByText(/^3\s184$/)).toBeInTheDocument();
    expect(
      screen.getByText(/O 18% więcej niż Twoja średnia na tych samych czołgach/),
    ).toBeInTheDocument();
    expect(screen.getByText(/Na Alpha zadajesz o 31% więcej obrażeń/)).toBeInTheDocument();
    expect(screen.getAllByRole("img", { name: /Alpha|Bravo/ }).length).toBeGreaterThan(0);
    expectNoCodes(document.body);
  });

  it("tells the session as a headline on the best map, a strip of battles and moments (E5-flare §2)", async () => {
    await renderApp();
    const hero = screen
      .getByRole("heading", { level: 1, name: "Mocna sesja: obrażenia o 18% wyżej niż zwykle" })
      .closest("section")!;
    expect(hero).toHaveClass("map-backdrop");
    expect(
      within(hero).getByText(/Tło: grafika mapy najlepszej bitwy \(Synthetic Hills\)/),
    ).toBeInTheDocument();
    expect(within(hero).getByText("o 18% wyżej")).toHaveClass("flare-accent");
    expect(within(hero).getByText("6 z 9")).toBeInTheDocument();
    expect(within(hero).getByText("średnie obrażenia")).toBeInTheDocument();
    const strip = screen.getByRole("list", { name: "Sesja bitwa po bitwie" });
    const tiles = within(strip).getAllByRole("button");
    expect(tiles).toHaveLength(9);
    const best = tiles.filter((tile) => tile.classList.contains("tile-best"));
    expect(best).toHaveLength(1);
    expect(best[0]).toHaveAccessibleName(/^Najlepsza, /);
    for (const tile of tiles.filter((item) => item.classList.contains("tile-loss")))
      expect(within(tile).getByText(/Porażka/)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Momenty sesji" })).toBeInTheDocument();
    expect(screen.getByText("Coraz lepiej z każdą bitwą")).toBeInTheDocument();
    expect(screen.getByText(/2\s000 → 4\s480/)).toBeInTheDocument();
    expect(screen.getByText("Do poprawy")).toBeInTheDocument();
    expect(screen.getByText("0 z 3 przeżytych")).toBeInTheDocument();
    fireEvent.click(tiles[0]);
    await flush();
    expect(screen.getByRole("heading", { level: 1, name: /na mapie/ })).toBeInTheDocument();
    expectNoCodes(document.body);
  });

  it("names the first and last battle when every battle rose, else the averages of the thirds", async () => {
    // Every battle: the numbers equal the first and the last tile of the strip.
    await renderApp();
    const strip = screen.getByRole("list", { name: "Sesja bitwa po bitwie" });
    const tiles = within(strip).getAllByRole("button");
    const trend = screen.getByText("Coraz lepiej z każdą bitwą").closest(".moment")!;
    expect(trend).toHaveTextContent(/2\s000 → 4\s480/);
    expect(trend).toHaveTextContent("Każda kolejna bitwa z większymi obrażeniami niż poprzednia");
    expect(tiles[0]).toHaveAccessibleName(/2\s000 obrażeń/);
    expect(tiles[tiles.length - 1]).toHaveAccessibleName(/4\s480 obrażeń/);
    cleanup();
    // Not every battle: averages, said plainly next to the numbers.
    mocks.sessionStory.mockResolvedValue(
      fx.sessionStory({
        moments: [
          {
            kind: "damage_trend",
            rising: true,
            from: 2_310,
            to: 4_170,
            battlesPerThird: 3,
            everyBattle: false,
          },
        ],
      }),
    );
    await renderApp();
    const averages = screen.getByText("Coraz lepiej z każdą bitwą").closest(".moment")!;
    expect(averages).toHaveTextContent(/2\s310 → 4\s170/);
    expect(averages).toHaveTextContent("Średnio z pierwszych 3 bitew → z ostatnich 3");
    expect(averages).not.toHaveTextContent(/Każda kolejna/);
  });

  it("falls back to a plain title and no moments without a story", async () => {
    mocks.sessionStory.mockRejectedValue({ code: "database_error" });
    await renderApp();
    expect(screen.getByRole("heading", { level: 1, name: "Dzisiejsza sesja" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Momenty sesji" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /obrażeń/ }).length).toBeGreaterThan(0);
    cleanup();
    mocks.sessionStory.mockResolvedValue(
      fx.sessionStory({ headline: { kind: "neutral", percent: null, battles: 9 }, moments: [] }),
    );
    await renderApp();
    expect(
      screen.getByRole("heading", { level: 1, name: "Sesja: 9 bitew losowych" }),
    ).toBeInTheDocument();
    cleanup();
    for (const [kind, title] of [
      ["tough", "Trudna sesja: obrażenia o 18% niżej niż zwykle"],
      ["calm", "Spokojna sesja: wyniki blisko Twojej średniej"],
    ] as const) {
      mocks.sessionStory.mockResolvedValue(
        fx.sessionStory({ headline: { kind, percent: kind === "calm" ? null : 18, battles: 9 } }),
      );
      await renderApp();
      expect(screen.getByRole("heading", { level: 1, name: title })).toBeInTheDocument();
      cleanup();
    }
  });

  it("states the inactivity limit from the settings (U5)", async () => {
    await renderApp();
    expect(screen.getByText(/Sesja kończy się sama po 60 min bez bitwy/)).toBeInTheDocument();
  });

  it("is loading before the data arrives", async () => {
    mocks.sessionSummary.mockReturnValue(new Promise(() => {}));
    await renderApp();
    expect(screen.getByRole("status")).toHaveAttribute("aria-busy", "true");
  });

  it("explains an error and retries", async () => {
    mocks.sessionSummary.mockRejectedValueOnce({ code: "database_error" });
    await renderApp();
    expect(screen.getByRole("alert")).toHaveTextContent("Zapis lub odczyt danych się nie powiódł");
    await go("Spróbuj ponownie");
    expect(screen.getByText(/^3\s184$/)).toBeInTheDocument();
  });

  it("is empty without a session and asks for the folder before the first run", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null, recentSessions: [] }));
    await renderApp();
    expect(screen.getByText(/Nie ma jeszcze żadnej sesji/)).toBeInTheDocument();
    cleanup();
    mocks.snapshot.mockResolvedValue(fx.snapshot({ configured: false }));
    await renderApp();
    expect(screen.getByRole("heading", { name: "Wskaż folder gry" })).toBeInTheDocument();
  });

  it("asks on the first run how to address the player, neutral until answered (2026-11-16)", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ configured: false }));
    await renderApp();
    const group = screen.getByRole("group", { name: "Jak się do Ciebie zwracać?" });
    const choices = within(group).getAllByRole("button");
    expect(choices.map((choice) => choice.textContent)).toEqual(["On", "Ona", "Bez formy"]);
    for (const choice of choices) expect(choice).toHaveAttribute("aria-pressed", "false");
    expect(localStorage.getItem("afterbattle-address")).toBeNull();
    fireEvent.click(within(group).getByRole("button", { name: "Ona" }));
    expect(localStorage.getItem("afterbattle-address")).toBe("feminine");
    expect(within(group).getByRole("button", { name: "Ona" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    fireEvent.click(within(group).getByRole("button", { name: "Bez formy" }));
    expect(localStorage.getItem("afterbattle-address")).toBe("neutral");
    cleanup();
    // The forms exist only in Polish.
    await renderApp("en");
    expect(screen.queryByRole("group")).not.toBeInTheDocument();
  });

  it("says when there are too few battles to compare and when XVM is off", async () => {
    mocks.sessionSummary.mockResolvedValue(fx.summary(2, 2500));
    mocks.snapshot.mockResolvedValue(
      fx.snapshot({ advancedStatus: { xvmEnabled: false, xvmLastUpdatedAt: null } }),
    );
    await renderApp();
    expect(screen.getByText("Porównanie ze średnią pojawi się po 3 bitwach.")).toBeInTheDocument();
    expect(screen.getByText(/WN8 nie jest pokazywane/)).toBeInTheDocument();
    expect(screen.queryByText("WN8 sesji")).not.toBeInTheDocument();
    cleanup();
    mocks.sessionSummary.mockResolvedValue(fx.summary());
    mocks.sessionBaselineComparison.mockResolvedValue(fx.comparison(12));
    await renderApp();
    expect(
      screen.getByText(/Za mało bitew na tych czołgach do porównania \(12 z 30\)/),
    ).toBeInTheDocument();
  });

  it("never calls a session 53% below the average close to it (E5-ui §4, regression)", async () => {
    // Synthetic: 8 battles on four tanks, two each, so no tank has a per-tank insight.
    const spread = fx.comparison();
    spread.vehicles = [101, 102, 103, 104].map((vehicleId) => ({
      ...fx.comparison().vehicles[0],
      vehicleId,
      sessionEligibleBattles: 2,
      sessionMetricBattles: { wn8: 0, damage: 2, survival: 2, assist: 2 },
      baseline: {
        ...fx.comparison().vehicles[0].baseline,
        averageDamage: { value: 3244, eligibleBattles: 100, candidateBattles: 100 },
      } as (typeof spread.vehicles)[number]["baseline"],
      damage: { ...fx.comparison().vehicles[0].damage, currentValue: 1528, baselineValue: 3244 },
    }));
    spread.insights = { ...spread.insights, state: "insufficient_session", summaries: [] };
    mocks.sessionSummary.mockResolvedValue(fx.summary(8, 1528));
    mocks.sessionBaselineComparison.mockResolvedValue(spread);
    await renderApp();
    expect(screen.getByText(/O 53% mniej niż Twoja średnia na tych samych czołgach/)).toBeVisible();
    expect(
      screen.getByText(/W tej sesji zadajesz o 53% mniej obrażeń niż zwykle na tych czołgach/),
    ).toBeVisible();
    expect(screen.queryByText(/bliskie Twojej średniej/)).not.toBeInTheDocument();
  });

  it("ends the session only after confirmation, and Escape closes the dialog", async () => {
    mocks.endSession.mockResolvedValue({});
    await renderApp();
    await go("Zakończ sesję");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(mocks.endSession).not.toHaveBeenCalled();
    await go("Zakończ sesję");
    fireEvent.click(
      within(screen.getByRole("dialog")).getByRole("button", { name: "Zakończ sesję" }),
    );
    await flush();
    expect(mocks.endSession).toHaveBeenCalledTimes(1);
  });
});

describe("Battle screen", () => {
  it("shows the battle summary with references to the numbers (spec 30 §2, §8)", async () => {
    mocks.recap.mockImplementation((kind: string) =>
      Promise.resolve(kind === "battle" ? fx.battleRecap() : fx.sessionRecap()),
    );
    await renderApp();
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    expect(mocks.recap).toHaveBeenCalledWith("battle", expect.any(String));
    const card = screen.getByRole("region", { name: "Podsumowanie bitwy" });
    const sentences = within(card).getAllByRole("listitem");
    expect(sentences.length).toBeGreaterThanOrEqual(3);
    expect(sentences[0].textContent).toMatch(/wygran/iu);
    expect(card.textContent).toMatch(/pierwsze miejsce w drużynie/u);
    const damage = within(card).getByRole("button", { name: /^Fakt F5: obrażenia 5\s210/u });
    const target = document.getElementById("recap-target-damage")!;
    fireEvent.mouseEnter(damage);
    expect(target).toHaveClass("recap-highlight");
    fireEvent.mouseLeave(damage);
    expect(target).not.toHaveClass("recap-highlight");
    expectNoCodes(document.body);
  });

  it("hides the battle summary in English", async () => {
    mocks.recap.mockImplementation((kind: string) =>
      Promise.resolve(kind === "battle" ? fx.battleRecap() : fx.sessionRecap()),
    );
    await renderApp("en");
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    expect(
      screen.queryByRole("region", { name: /Podsumowanie|summary/iu }),
    ).not.toBeInTheDocument();
  });

  it("puts the title on the map image header, or on a placeholder that says where it comes from", async () => {
    clearGameImages();
    const created = vi.fn(() => "blob:synthetic-map");
    const original = URL.createObjectURL;
    URL.createObjectURL = created;
    mocks.gameImage.mockImplementation((kind: string) =>
      Promise.resolve(kind === "map" ? new Uint8Array([1, 2, 3]).buffer : new ArrayBuffer(0)),
    );
    try {
      await renderApp();
      fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
      await flush();
      const header = screen.getByRole("heading", { level: 1, name: /na mapie/ }).closest("header")!;
      expect(header).toHaveClass("map-backdrop");
      expect(header).toHaveAttribute("data-loaded", "true");
      expect(header.style.getPropertyValue("--map-picture")).toContain("blob:synthetic-map");
      expect(within(header).getByText(/Tło: grafika mapy .* z plików gry/)).toBeInTheDocument();
    } finally {
      URL.createObjectURL = original;
    }
    cleanup();
    clearGameImages();
    mocks.gameImage.mockResolvedValue(new ArrayBuffer(0));
    await renderApp();
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    const placeholder = screen
      .getByRole("heading", { level: 1, name: /na mapie/ })
      .closest("header")!;
    expect(placeholder).toHaveAttribute("data-loaded", "false");
    expect(placeholder.className).toMatch(/map-ph-[0-5]/);
    expect(within(placeholder).getByText(/Tło: grafika mapy .* z plików gry/)).toBeInTheDocument();
  });

  it("compares both teams with totals only and shows remaining health", async () => {
    await renderApp();
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    expect(screen.getByRole("heading", { level: 1, name: /na mapie/ })).toBeInTheDocument();
    expect(screen.getByText("obrażeń, najwięcej w drużynie, 24% jej obrażeń")).toBeInTheDocument();
    expect(screen.getByText("312 HP")).toBeInTheDocument();
    expect(screen.getByText(/^czołg przetrwał, z 1\s950$/)).toBeInTheDocument();
    const you = screen.getByRole("heading", { name: "Twoja bitwa" }).closest("section")!;
    expect(within(you).getByText("15 z 18")).toBeInTheDocument();
    expectNoGluedNumbers(you);
    const record = screen
      .getByRole("heading", { name: "Ty na mapie Synthetic Hills" })
      .closest("section")!;
    expect(within(record).getByText("31")).toBeInTheDocument();
    expect(within(record).getByText("61%")).toBeInTheDocument();
    expect(within(record).getByText(/^3\s010$/)).toBeInTheDocument();
    expect(mocks.mapRecord).toHaveBeenCalledWith("90_synthetic_hills");
    expect(screen.getByRole("heading", { name: "Drużyny" })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/slot|gracz \d/i);
    await go(`← Wróć`);
    expect(
      screen.getByRole("heading", { level: 1, name: /Mocna sesja|Dzisiejsza sesja/ }),
    ).toBeInTheDocument();
  });

  it("opens History filtered by the battle's map from the map card", async () => {
    await renderApp();
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    await go("Wszystkie bitwy na tej mapie");
    expect(screen.getByRole("heading", { level: 1, name: "Historia" })).toBeInTheDocument();
    expect(mocks.battleHistory).toHaveBeenLastCalledWith(
      expect.objectContaining({ mapKey: "90_synthetic_hills" }),
    );
    expect(screen.getByLabelText("Mapa")).toHaveValue("90_synthetic_hills");
  });

  it("says when a battle does not count and when teams are unavailable", async () => {
    mocks.battleView.mockResolvedValue(
      fx.battleDetail({
        teams: null,
        view: { battle: fx.battle(1), feedback: [{ code: "battle_not_counted" } as never] },
      }),
    );
    await renderApp();
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    expect(screen.getByRole("note")).toHaveTextContent("Ta bitwa nie liczy się do statystyk");
    mocks.battleView.mockResolvedValue(fx.battleDetail({ teams: null }));
    cleanup();
    await renderApp();
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    expect(
      screen.getByText("Porównanie drużyn jest niedostępne dla tej bitwy."),
    ).toBeInTheDocument();
  });
});

describe("Tanks screen", () => {
  it("compares the tank with the player's own average and shows best and worst maps", async () => {
    await renderApp();
    await go("Czołgi");
    expect(screen.getByRole("heading", { name: "Alpha" })).toBeInTheDocument();
    const figures = screen.getByRole("group", {
      name: "Ostatnie 10 bitew wobec średniej ze 100 ostatnich",
    });
    expect(within(figures).getByText(/2\s940/)).toBeInTheDocument();
    expect(within(figures).getByText(/^obrażenia, średnio 2\s702$/)).toBeInTheDocument();
    expect(within(figures).getByText("przetrwanie, średnio 34%")).toBeInTheDocument();
    const grid = screen
      .getByRole("heading", { name: "Twoje mapy na tym czołgu" })
      .closest("section")!;
    const tiles = within(grid).getAllByRole("button");
    expect(tiles[0]).toHaveAccessibleName(/^Najlepiej Synthetic Hills: średnio 3\s480 obrażeń/);
    expect(tiles[0]).toHaveClass("tile-best");
    expect(tiles[1]).toHaveAccessibleName(/^Test Valley: średnio 2\s150 obrażeń/);
    expect(
      screen.getByRole("img", { name: /Wykres obrażeń w czasie, 120 punktów/ }),
    ).toBeInTheDocument();
    expectNoCodes(document.body);
  });

  it("shows class, tier and battle count in the header", async () => {
    await renderApp();
    await go("Czołgi");
    expect(screen.getByText(/^Czołg średni, X poziom, 214 bitew, ostatnia /)).toBeInTheDocument();
    cleanup();
    await renderApp("en");
    await go("Tanks");
    expect(screen.getByText(/^Medium tank, tier X, 214 battles, last /)).toBeInTheDocument();
  });

  it("names the last battle relative to today (E5-ui §6)", () => {
    const tr = (key: string, params?: Record<string, string | number>) =>
      `${key}${params ? JSON.stringify(params) : ""}`;
    const now = new Date(2026, 10, 14, 9, 0).getTime();
    const at = (day: number, hour = 21) => new Date(2026, 10, day, hour, 0).getTime();
    expect(lastBattle(at(14, 1), now, "pl", tr as never)).toBe("lastToday");
    expect(lastBattle(at(13, 23), now, "pl", tr as never)).toBe("lastYesterday");
    expect(lastBattle(at(9), now, "pl", tr as never)).toBe('lastDaysAgo{"count":5}');
    expect(lastBattle(at(1), now, "pl", tr as never)).toBe('lastOn{"date":"1 listopada 2026"}');
    expect(lastBattle(null, now, "pl", tr as never)).toBeNull();
    expect(lastBattle(now + 1, now, "pl", tr as never)).toBeNull();
  });

  it("has no tank column in its battle table and a chart with values and the player's average", async () => {
    await renderApp();
    await go("Czołgi");
    const table = screen.getByRole("heading", { name: "Bitwy" }).closest("section")!;
    const head = table.querySelector(".table-head")!;
    expect(head.textContent).not.toMatch(/Czołg/);
    expect(head.children).toHaveLength(5);
    const chart = screen.getByRole("img", { name: /Wykres obrażeń w czasie/ });
    expect(chart.getAttribute("aria-label")).toMatch(/Twoja średnia 2\s\d{3}/);
    const ticks = Array.from(chart.querySelectorAll(".chart-tick text")).map((t) => t.textContent);
    expect(ticks.length).toBeGreaterThanOrEqual(2);
    expect(ticks.every((tick) => /^\d[\d\s]*$/.test(tick ?? ""))).toBe(true);
    expect(chart.querySelector(".chart-average line")).not.toBeNull();
  });

  it("leaves the tier out of the header when it is unknown or out of range", async () => {
    for (const tier of [null, 0, 12]) {
      mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview(3, tier));
      await renderApp();
      await go("Czołgi");
      expect(screen.getByText(/^Czołg średni, 214 bitew/)).toBeInTheDocument();
      expect(document.body.textContent).not.toMatch(/poziom/);
      cleanup();
    }
  });

  it("has a readable empty state for a tank with few battles and no tanks at all", async () => {
    mocks.vehicleBaseline.mockResolvedValue(fx.vehicleBaseline(1));
    mocks.performanceTrends.mockResolvedValue(fx.trends(1));
    await renderApp();
    await go("Czołgi");
    expect(screen.getByText("Za mało bitew na tym czołgu (1 z 10).")).toBeInTheDocument();
    expect(screen.getByText("Wykres pojawi się po 2 bitwach na tym czołgu.")).toBeInTheDocument();
    cleanup();
    mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview(0));
    await renderApp();
    await go("Czołgi");
    expect(screen.getByText(/Brak bitew losowych/)).toBeInTheDocument();
  });
});

describe("wide layouts (E5-ui §1, §5)", () => {
  it("Session: hero and insights on the left, the battle table on the right", async () => {
    const { container } = await renderApp();
    const layout = container.querySelector(".session-layout")!;
    const main = layout.querySelector(":scope > .session-main")!;
    const side = layout.querySelector(":scope > .session-side")!;
    expect(
      within(main as HTMLElement).getByRole("heading", { name: "Co wyróżnia tę sesję" }),
    ).toBeInTheDocument();
    expect(
      within(side as HTMLElement).getByRole("heading", { name: "Bitwy w tej sesji" }),
    ).toBeInTheDocument();
    expect(container.querySelector(".session-extra .pace")).not.toBeNull();
    expect(container.querySelector(".session-extra .conversion")).not.toBeNull();
  });

  it("Tank: cards and chart in one column, maps and battles in the other", async () => {
    const { container } = await renderApp();
    await go("Czołgi");
    const main = container.querySelector(".tank-layout > .tank-main") as HTMLElement;
    const side = container.querySelector(".tank-layout > .tank-side") as HTMLElement;
    expect(within(main).getByRole("heading", { name: "Obrażenia w czasie" })).toBeInTheDocument();
    expect(
      within(main).getByRole("heading", { name: "Twoje mapy na tym czołgu" }),
    ).toBeInTheDocument();
    expect(within(side).getByRole("heading", { name: "Bitwy" })).toBeInTheDocument();
  });

  it("Battle: result and statistics next to the team comparison", async () => {
    const { container } = await renderApp();
    fireEvent.click(container.querySelector(".table-row")!);
    await flush();
    const layout = container.querySelector(".battle-layout") as HTMLElement;
    expect(within(layout).getByRole("heading", { name: "Twoja bitwa" })).toBeInTheDocument();
    expect(within(layout).getByRole("heading", { name: "Drużyny" })).toBeInTheDocument();
  });

  it("History: rows on a map strip with survival in words and destroyed for wide screens", async () => {
    const { container } = await renderApp();
    await go("Historia");
    const head = container.querySelector(".hrow-head")!;
    expect(Array.from(head.querySelectorAll(".col-extra")).map((cell) => cell.textContent)).toEqual(
      ["Zniszczone"],
    );
    expect(head.textContent).toContain("Przetrwanie");
    const row = container.querySelector(".hrow")!;
    expect(row).toHaveClass("map-backdrop");
    expect(row.querySelector(".hrow-survival")!.textContent).toMatch(/^(Przeżył|Zniszczony|—)$/);
  });
});

describe("vehicle names (E5-ui §2)", () => {
  const names = {
    language: "pl",
    names: {
      "synth-s01_alpha": { name: "Synthetic Alpha Mk. II Long Name", shortName: "Alpha II" },
      "synth-s01_tank": { name: "Synthetic Alpha Mk. II Long Name", shortName: "Alpha II" },
    },
  };

  it("shows the game's short name in lists and the full name in the title and the header", async () => {
    mocks.vehicleNames.mockResolvedValue(names);
    await renderApp();
    expect(mocks.vehicleNames).toHaveBeenCalledWith("pl");
    const table = screen.getByRole("heading", { name: "Bitwy w tej sesji" }).closest("section")!;
    const short = within(table).getAllByText("Alpha II");
    expect(short.length).toBeGreaterThan(0);
    for (const cell of short) {
      expect(cell).toHaveAttribute("title", "Synthetic Alpha Mk. II Long Name");
      expect(cell).toHaveClass("truncate");
    }
    await go("Czołgi");
    expect(
      screen.getByRole("heading", { level: 1, name: "Synthetic Alpha Mk. II Long Name" }),
    ).toHaveAttribute("title", "Synthetic Alpha Mk. II Long Name");
    const list = screen.getByRole("navigation", { name: "Czołgi" });
    expect(within(list).getByText("Alpha II")).toHaveAttribute(
      "title",
      "Synthetic Alpha Mk. II Long Name",
    );
  });

  it("asks again in the other language and falls back to the readable code", async () => {
    mocks.vehicleNames.mockResolvedValue({ language: null, names: {} });
    mocks.vehicleHistoryOverview.mockResolvedValue({
      unidentifiedBattleCount: 0,
      vehicles: [
        {
          ...fx.overview().vehicles[0],
          vehicleKey: "synth-sx9_vz_60s_test",
          tankName: "synth-SX9_Vz_60S_Test",
        },
      ],
    });
    await renderApp("en");
    expect(mocks.vehicleNames).toHaveBeenCalledWith("en");
    await go("Tanks");
    expect(screen.getByRole("heading", { level: 1, name: "Vz 60S Test" })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/synth-SX9/);
  });

  it("are never sent anywhere but the window: the request carries only the language", async () => {
    mocks.vehicleNames.mockResolvedValue(names);
    await renderApp();
    for (const [name, mock] of Object.entries(mocks)) {
      if (name === "vehicleNames" || typeof mock !== "function" || !("mock" in mock)) continue;
      expect(JSON.stringify(mock.mock.calls), name).not.toMatch(/Synthetic Alpha|Alpha II/);
    }
  });
});

describe("tank map tags (spec 26 §5)", () => {
  it("marks two best maps from three maps up and the weakest one", () => {
    expect([0, 1].map((index) => mapTag(index, 2))).toEqual(["best", null]);
    expect([0, 1, 2].map((index) => mapTag(index, 3))).toEqual(["best", "best", "worst"]);
    expect([0, 1, 2, 3].map((index) => mapTag(index, 4))).toEqual(["best", "best", null, "worst"]);
    expect(mapTag(0, 1)).toBe("best");
  });
});

/**
 * jsdom has no layout: rows are laid out here every 60 px from y = 200 (E8-ui-fix wave). Other
 * elements keep the empty rectangle.
 */
function layOutRows(windowHeight: number) {
  const original = Element.prototype.getBoundingClientRect;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const rows = Array.from(document.querySelectorAll(".hrow-list > li, .map-grid > li"));
    const index = rows.indexOf(this);
    if (index < 0) return original.call(this);
    const top = 200 + index * 60;
    return {
      top,
      bottom: top + 56,
      height: 56,
      left: 0,
      right: 0,
      width: 0,
      x: 0,
      y: top,
    } as DOMRect;
  });
  vi.stubGlobal("innerHeight", windowHeight);
}

/** Like `layOutRows`, with each day header taking 40 px before its rows (spec 29 §1). */
function layOutWithHeaders(windowHeight: number) {
  const original = Element.prototype.getBoundingClientRect;
  vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
    const items = Array.from(document.querySelectorAll(".day-header, .hrow-list > li"));
    const index = items.indexOf(this);
    if (index < 0) return original.call(this);
    let top = 200;
    for (const item of items.slice(0, index)) top += item.matches(".day-header") ? 40 : 60;
    const height = this.matches(".day-header") ? 40 : 56;
    return {
      top,
      bottom: top + height,
      height,
      left: 0,
      right: 0,
      width: 0,
      x: 0,
      y: top,
    } as DOMRect;
  });
  vi.stubGlobal("innerHeight", windowHeight);
}

describe("History motion (E5-flare-fix2, E8-ui-fix)", () => {
  const entering = (container: HTMLElement) =>
    Array.from(container.querySelectorAll(".hrow-list li.row-enter"));
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("cascades the rows visible at the first visit, up to 20, by the History step", async () => {
    layOutRows(800);
    mocks.battleHistory.mockResolvedValue(fx.historyPage(30));
    const { container } = await renderApp();
    await go("Historia");
    const rows = Array.from(container.querySelectorAll(".hrow-list li"));
    expect(rows).toHaveLength(30);
    // (800 − 200) / 60 = 10 rows fit the window.
    const moving = entering(container);
    expect(moving).toEqual(rows.slice(0, 10));
    const delays = moving.map((row) =>
      parseInt((row as HTMLElement).style.getPropertyValue("--row-delay"), 10),
    );
    expect(delays).toEqual(Array.from({ length: 10 }, (_, index) => index * 36));
    expect(container.querySelector(".screen")).toHaveClass("screen-still");
  });

  it("stops the wave at 20 rows on a tall window and moves 14 rows unmeasured", async () => {
    layOutRows(1_440);
    mocks.battleHistory.mockResolvedValue(fx.historyPage(30));
    const { container } = await renderApp();
    await go("Historia");
    expect(entering(container)).toHaveLength(20);
    cleanup();
    vi.restoreAllMocks();
    clearGameImages();
    // jsdom without a layout mock: every box is 0 x 0, so the default wave of 14 rows enters.
    const unmeasured = await renderApp();
    await go("Historia");
    expect(entering(unmeasured.container)).toHaveLength(14);
  });

  it("is not shortened by a day header in the middle of the wave", async () => {
    layOutWithHeaders(800);
    const day = 86_400_000;
    const page = fx.historyPage(30);
    const now = Date.now();
    page.items = page.items.map((item, index) => ({
      ...item,
      battleTime: index === 0 ? now : now - day - index * 60_000,
    }));
    mocks.battleHistory.mockResolvedValue(page);
    const { container } = await renderApp();
    await go("Historia");
    expect(container.querySelectorAll(".day-header")).toHaveLength(2);
    // Header 200-240, row 240; header 300-340; rows from 340 every 60 px: 340 ... 760 (8) < 800.
    expect(entering(container)).toHaveLength(9);
  });

  it("measures only after the fonts are ready, rows hidden until then", async () => {
    layOutRows(800);
    let fontsReady: () => void = () => undefined;
    const ready = new Promise<void>((resolve) => (fontsReady = resolve));
    Object.defineProperty(document, "fonts", { value: { ready }, configurable: true });
    try {
      mocks.battleHistory.mockResolvedValue(fx.historyPage(30));
      const { container } = await renderApp();
      await go("Historia");
      expect(container.querySelector(".wave-pending")).not.toBeNull();
      expect(entering(container)).toHaveLength(0);
      fontsReady();
      await flush();
      await frame();
      expect(container.querySelector(".wave-pending")).toBeNull();
      expect(entering(container)).toHaveLength(10);
    } finally {
      delete (document as { fonts?: unknown }).fonts;
    }
  });

  it("keeps the wave when effects run twice (StrictMode, as in the dev build)", async () => {
    layOutRows(800);
    mocks.battleHistory.mockResolvedValue(fx.historyPage(30));
    const { container } = await renderApp("pl", true);
    await go("Historia");
    expect(entering(container)).toHaveLength(10);
  });

  it("measures the rows of data that arrives frames later", async () => {
    layOutRows(800);
    mocks.battleHistory.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve(fx.historyPage(30)), 50)),
    );
    const { container } = await renderApp();
    await go("Historia");
    await frame();
    await frame();
    await frame();
    expect(container.querySelectorAll(".hrow-list li")).toHaveLength(30);
    expect(entering(container)).toHaveLength(10);
  });

  it("does not move rows after switching Battles/Sessions, on a filter change or back from Battle", async () => {
    layOutRows(800);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mocks.battleHistory.mockResolvedValue(fx.historyPage(30));
      const { container } = await renderApp();
      await go("Historia");
      expect(entering(container)).toHaveLength(10);
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      fireEvent.click(screen.getByRole("radio", { name: "Bitwy" }));
      await flush();
      expect(entering(container)).toHaveLength(0);
      mocks.battleHistory.mockResolvedValue(fx.historyPage(12));
      fireEvent.change(screen.getByLabelText("Mapa"), { target: { value: "91_test_valley" } });
      await flush();
      expect(entering(container)).toHaveLength(0);
      fireEvent.click(container.querySelector(".hrow")!);
      await flush();
      expect(container.querySelector(".screen")).not.toHaveClass("screen-still");
      await go("← Wróć");
      expect(screen.getByRole("heading", { level: 1, name: "Historia" })).toBeInTheDocument();
      expect(entering(container)).toHaveLength(0);
      expect(container.querySelector(".screen")).toHaveClass("screen-still");
    } finally {
      vi.useRealTimers();
    }
  });

  it("drops the entrance class after the cascade", async () => {
    layOutRows(800);
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { container } = await renderApp();
      await go("Historia");
      expect(entering(container).length).toBeGreaterThan(0);
      await act(async () => {
        vi.advanceTimersByTime(1_100);
      });
      expect(entering(container)).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("Tanks motion (E8-ui-fix)", () => {
  const tiles = (container: HTMLElement) =>
    Array.from(container.querySelectorAll(".map-grid > li"));
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("does not move the screen; the map tiles cascade once, after the data, by the History step", async () => {
    const { container } = await renderApp();
    await go("Czołgi");
    expect(container.querySelector(".screen")).toHaveClass("screen-still");
    const main = container.querySelector(".tank-main") as HTMLElement;
    expect(main).toHaveClass("tank-enter");
    const moving = tiles(container).filter((tile) => tile.classList.contains("row-enter"));
    expect(moving).toEqual(tiles(container));
    expect(
      moving.map((tile) => (tile as HTMLElement).style.getPropertyValue("--row-delay")),
    ).toEqual(moving.map((_, index) => `${index * 36}ms`));
  });

  it("does not move again when another tank is chosen or after the cascade", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { container } = await renderApp();
      await go("Czołgi");
      fireEvent.click(screen.getAllByRole("button", { name: /Bravo/ })[0]);
      await flush();
      expect(container.querySelector(".tank-main")).not.toHaveClass("tank-enter");
      expect(container.querySelectorAll(".map-grid > li.row-enter")).toHaveLength(0);
      cleanup();
      clearGameImages();
      const again = await renderApp();
      await go("Czołgi");
      await act(async () => {
        vi.advanceTimersByTime(1_100);
      });
      expect(again.container.querySelector(".tank-main")).not.toHaveClass("tank-enter");
      expect(again.container.querySelectorAll(".map-grid > li.row-enter")).toHaveLength(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the entrance when effects run twice (StrictMode, as in the dev build)", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { container } = await renderApp("pl", true);
      await go("Czołgi");
      expect(container.querySelector(".tank-main")).toHaveClass("tank-enter");
      await act(async () => {
        vi.advanceTimersByTime(1_100);
      });
      expect(container.querySelector(".tank-main")).not.toHaveClass("tank-enter");
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the detail panel and its data on screen while another tank loads (spec 29 §1)", async () => {
    const { container } = await renderApp();
    await go("Czołgi");
    const layout = container.querySelector(".tank-layout");
    const grid = container.querySelector(".map-grid");
    expect(grid).not.toBeNull();
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    function later<T>(value: T): Promise<T> {
      return gate.then(() => value);
    }
    mocks.vehicleBaseline.mockImplementation(() => later(fx.vehicleBaseline()));
    mocks.vehicleHistoryReport.mockImplementation(() => later(fx.vehicleReport()));
    mocks.performanceTrends.mockImplementation(() => later(fx.trends()));
    mocks.battleHistory.mockImplementation(() => later(fx.historyPage()));
    fireEvent.click(screen.getAllByRole("button", { name: /Bravo/ })[0]);
    await flush();
    await frame();
    // Same panel, old data still there, no loading skeleton, marked as refreshing.
    expect(container.querySelector(".tank-layout")).toBe(layout);
    expect(container.querySelector(".map-grid")).toBe(grid);
    expect(container.querySelector(".tank-layout [role='status'][aria-busy='true']")).toBeNull();
    expect(container.querySelector(".tank-layout")).toHaveAttribute("aria-busy", "true");
    release();
    await flush();
    await frame();
    expect(container.querySelector(".tank-layout")).toBe(layout);
    expect(container.querySelector(".tank-layout")).toHaveAttribute("aria-busy", "false");
    expect(container.querySelector(".tank-main")).not.toHaveClass("tank-enter");
    expect(container.querySelectorAll(".map-grid > li.row-enter")).toHaveLength(0);
    expect(screen.getByRole("heading", { level: 1, name: /Bravo/ })).toBeInTheDocument();
  });

  it("starts the battle list of a newly chosen tank from its first page", async () => {
    mocks.battleHistory.mockResolvedValue({ ...fx.historyPage(3), nextCursor: { t: 1 } });
    const { container } = await renderApp();
    await go("Czołgi");
    const rows = () => container.querySelectorAll(".tank-side .table-row").length;
    const before = rows();
    mocks.battleHistory.mockResolvedValueOnce({
      items: fx.historyPage(5).items.slice(3),
      nextCursor: null,
    });
    fireEvent.click(screen.getByRole("button", { name: "Pokaż kolejne" }));
    await flush();
    expect(rows()).toBe(before + 2);
    mocks.battleHistory.mockResolvedValue({ ...fx.historyPage(1), nextCursor: null });
    fireEvent.click(screen.getAllByRole("button", { name: /Bravo/ })[0]);
    await flush();
    await frame();
    expect(rows()).toBe(1);
  });

  it("has no motion at all with reduced motion", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("reduce"),
      media: query,
      addEventListener() {},
      removeEventListener() {},
    }));
    const { container } = await renderApp();
    await go("Czołgi");
    expect(container.querySelector(".tank-main")).not.toHaveClass("tank-enter");
    expect(container.querySelectorAll(".map-grid > li.row-enter")).toHaveLength(0);
  });
});

/**
 * Live refresh (spec 29 §2): one backend event refreshes the page that is open, keeps its data
 * on screen until the new data arrives, and keeps the chosen tank, filters and scroll.
 */
describe("live refresh after a backend event (spec 29 §2)", () => {
  type Handler = (event: { name: string; payload?: unknown }) => void;
  let emit: Handler = () => undefined;
  beforeEach(() => {
    mocks.onEvents.mockImplementation((handler: Handler) => {
      emit = handler;
      return () => {};
    });
  });

  /** Holds every response of `mock` until `release` is called. */
  function hold<T>(mock: { mockImplementation: (fn: () => Promise<T>) => unknown }, value: T) {
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    mock.mockImplementation(() => gate.then(() => value));
    return () => release();
  }

  async function event(name = "data-changed") {
    await act(async () => emit({ name }));
    await flush();
  }

  const busy = (container: HTMLElement) =>
    container.querySelector("main [role='status'][aria-busy='true']");

  it("shows a new battle on Session without re-entering and without a loading frame", async () => {
    const { container } = await renderApp();
    const strip = () => container.querySelectorAll(".battle-strip > li").length;
    expect(strip()).toBe(9);
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    const release = hold(mocks.sessionSummary, fx.summary(10));
    await event();
    expect(strip()).toBe(9);
    expect(busy(container)).toBeNull();
    release();
    await flush();
    expect(strip()).toBe(10);
    expect(busy(container)).toBeNull();
  });

  it("refreshes after the end of a session and after a finished import as well", async () => {
    await renderApp();
    const calls = () => mocks.sessionSummary.mock.calls.length;
    const before = calls();
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    await act(async () => emit({ name: "session-ended", payload: { sessionId: "s1" } }));
    await flush();
    expect(calls()).toBe(before + 1);
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    await act(async () =>
      emit({
        name: "import-progress",
        payload: { done: 3, total: 3, finished: true, cancelled: false },
      }),
    );
    await flush();
    expect(calls()).toBe(before + 2);
  });

  it("shows a new battle in History, keeps the filter and the scroll", async () => {
    const scrolled = vi.fn();
    vi.stubGlobal("scrollTo", scrolled);
    try {
      const { container } = await renderApp();
      await go("Historia");
      await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 1_200));
      });
      fireEvent.change(screen.getByLabelText("Mapa"), { target: { value: "91_test_valley" } });
      await flush();
      const rows = () => container.querySelectorAll(".hrow-list > li").length;
      expect(rows()).toBe(12);
      scrolled.mockClear();
      mocks.snapshot.mockResolvedValue(fx.snapshot());
      const release = hold(mocks.battleHistory, fx.historyPage(13));
      await event();
      expect(rows()).toBe(12);
      expect(busy(container)).toBeNull();
      release();
      await flush();
      await frame();
      expect(rows()).toBe(13);
      // Only the battle that arrived enters (spec 29 §6); the rows that were there stay put.
      const entering = Array.from(container.querySelectorAll(".hrow-list li.row-enter"));
      expect(entering.map((row) => row.getAttribute("data-flip"))).toEqual(["battle-12"]);
      expect(screen.getByLabelText("Mapa")).toHaveValue("91_test_valley");
      expect(mocks.battleHistory).toHaveBeenLastCalledWith(
        expect.objectContaining({ mapKey: "91_test_valley", cursor: null }),
      );
      expect(scrolled).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("keeps the rows loaded with Show more after a refresh, with the new battle on top", async () => {
    const first = fx.historyPage(3);
    mocks.battleHistory.mockResolvedValue({ ...first, nextCursor: { t: 1 } });
    const { container } = await renderApp();
    await go("Historia");
    const more = fx.historyPage(6);
    more.items = more.items.slice(3);
    mocks.battleHistory.mockResolvedValueOnce({ ...more, nextCursor: null });
    fireEvent.click(screen.getByRole("button", { name: "Pokaż kolejne" }));
    await flush();
    const ids = () =>
      Array.from(container.querySelectorAll(".hrow-list > li .hrow")).map((row) =>
        row.getAttribute("aria-label"),
      );
    expect(container.querySelectorAll(".hrow-list > li")).toHaveLength(6);
    const fresh = fx.historyPage(7);
    const newest = { ...fresh.items[6], battleId: "brand-new", battleTime: Date.now() };
    mocks.battleHistory.mockResolvedValue({
      items: [newest, ...first.items],
      nextCursor: { t: 1 },
    });
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    await event();
    await frame();
    expect(container.querySelectorAll(".hrow-list > li")).toHaveLength(7);
    expect(ids()).toHaveLength(7);
    expect(screen.queryByRole("button", { name: "Pokaż kolejne" })).toBeNull();
  });

  it("keeps the chosen tank on Tanks and refreshes its data without a loading frame", async () => {
    const { container } = await renderApp();
    await go("Czołgi");
    fireEvent.click(screen.getAllByRole("button", { name: /Bravo/ })[0]);
    await flush();
    await frame();
    const reports = mocks.vehicleHistoryReport.mock.calls.length;
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    const release = hold(mocks.vehicleHistoryReport, fx.vehicleReport());
    await event();
    expect(busy(container)).toBeNull();
    expect(container.querySelector(".map-grid")).not.toBeNull();
    release();
    await flush();
    expect(mocks.vehicleHistoryReport.mock.calls.length).toBeGreaterThan(reports);
    expect(mocks.vehicleHistoryReport).toHaveBeenLastCalledWith(102);
    expect(screen.getByRole("heading", { level: 1, name: /Bravo/ })).toBeInTheDocument();
    expect(container.querySelector(".tank-main")).not.toHaveClass("tank-enter");
  });

  it("refreshes Goals and Battle as well", async () => {
    await renderApp();
    await go("Cele");
    const weekly = mocks.weeklyReport.mock.calls.length;
    const history = mocks.challengeHistory.mock.calls.length;
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    await event();
    expect(mocks.weeklyReport.mock.calls.length).toBe(weekly + 1);
    expect(mocks.challengeHistory.mock.calls.length).toBe(history + 1);
    await go("Sesja");
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    const views = mocks.battleView.mock.calls.length;
    const records = mocks.mapRecord.mock.calls.length;
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    await event();
    expect(mocks.battleView.mock.calls.length).toBe(views + 1);
    expect(mocks.mapRecord.mock.calls.length).toBe(records + 1);
  });
});

describe("loading screen (spec 29 §5)", () => {
  type Handler = (event: { name: string; payload?: unknown }) => void;
  let emit: Handler = () => undefined;
  beforeEach(() => {
    mocks.onEvents.mockImplementation((handler: Handler) => {
      emit = handler;
      return () => {};
    });
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });
  const bar = () => screen.queryByRole("progressbar", { name: "Wczytywanie" });

  it("never appears when the first snapshot is there within 400 ms", async () => {
    vi.useFakeTimers();
    mocks.snapshot.mockImplementation(
      () => new Promise((resolve) => setTimeout(() => resolve(fx.snapshot()), 300)),
    );
    render(
      <LanguageProvider initial="pl">
        <App />
      </LanguageProvider>,
    );
    await act(async () => {
      vi.advanceTimersByTime(350);
    });
    expect(bar()).toBeNull();
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    await flush();
    expect(bar()).toBeNull();
    expect(screen.getByRole("navigation", { name: "Główna nawigacja" })).toBeInTheDocument();
  });

  it("shows the outline on a slow start without a number, then fills, holds and fades", async () => {
    vi.useFakeTimers();
    let release: () => void = () => undefined;
    mocks.snapshot.mockImplementation(
      () => new Promise((resolve) => (release = () => resolve(fx.snapshot()))),
    );
    const { container } = render(
      <LanguageProvider initial="pl">
        <App />
      </LanguageProvider>,
    );
    await act(async () => {
      vi.advanceTimersByTime(399);
    });
    expect(bar()).toBeNull();
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    const shown = bar()!;
    expect(shown).not.toHaveAttribute("aria-valuenow");
    expect(shown).toHaveClass("loading-pulse");
    expect(shown.textContent).toBe("");
    const rise = () => container.querySelector(".loading-rise")!.getAttribute("style") ?? "";
    expect(rise()).toContain("scaleY(0.08)");
    release();
    await flush();
    expect(bar()).not.toBeNull();
    expect(rise()).toContain("scaleY(1)");
    expect(bar()).not.toHaveClass("loading-pulse");
    await act(async () => {
      vi.advanceTimersByTime(449);
    });
    expect(bar()).not.toHaveClass("loading-out");
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(bar()).toHaveClass("loading-out");
    await act(async () => {
      vi.advanceTimersByTime(200);
    });
    expect(bar()).toBeNull();
  });

  it("follows the first import into an empty database and leaves with the first battles", async () => {
    // No session and no battle in the history: an empty database.
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null, recentSessions: [] }));
    mocks.battleHistory.mockResolvedValue({ items: [], nextCursor: null });
    await renderApp();
    expect(bar()).toBeNull();
    await act(async () =>
      emit({
        name: "import-progress",
        payload: { done: 0, total: 400, finished: false, cancelled: false },
      }),
    );
    expect(bar()).toHaveAttribute("aria-valuenow", "0");
    expect(bar()).toHaveAttribute("aria-valuemin", "0");
    expect(bar()).toHaveAttribute("aria-valuemax", "100");
    await act(async () =>
      emit({
        name: "import-progress",
        payload: { done: 100, total: 400, finished: false, cancelled: false },
      }),
    );
    expect(bar()).toHaveAttribute("aria-valuenow", "25");
    expect(bar()).not.toHaveClass("loading-pulse");
    // The first batch stored old battles without a session: the history is no longer empty.
    // While the history is asked again, the last answer holds (no flicker).
    let answer: () => void = () => undefined;
    mocks.battleHistory.mockImplementation(
      () => new Promise((resolve) => (answer = () => resolve(fx.historyPage(3)))),
    );
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null, recentSessions: [] }));
    await act(async () => emit({ name: "data-changed" }));
    await flush();
    expect(bar()).not.toHaveClass("loading-finishing");
    answer();
    await flush();
    expect(bar()).toHaveClass("loading-finishing");
  });

  it("stays away for a history without sessions during a background scan", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null, recentSessions: [] }));
    await renderApp();
    await act(async () =>
      emit({
        name: "import-progress",
        payload: { done: 0, total: 5, finished: false, cancelled: false },
      }),
    );
    expect(bar()).toBeNull();
    expect(mocks.battleHistory).toHaveBeenCalledWith({
      vehicleId: null,
      mapKey: null,
      dataQuality: "all",
      wn8Availability: "all",
      dateFrom: null,
      dateTo: null,
      cursor: null,
      pageSize: 1,
    });
  });

  it("stays away during a background import when there are battles: the import bar shows", async () => {
    await renderApp();
    await act(async () =>
      emit({
        name: "import-progress",
        payload: { done: 10, total: 400, finished: false, cancelled: false },
      }),
    );
    expect(bar()).toBeNull();
    expect(screen.getByText("Odczyt powtórek: 10 z 400")).toBeInTheDocument();
  });

  it("has no pulse and leaves at once with reduced motion", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("reduce"),
      media: query,
      addEventListener() {},
      removeEventListener() {},
    }));
    vi.useFakeTimers();
    let release: () => void = () => undefined;
    mocks.snapshot.mockImplementation(
      () => new Promise((resolve) => (release = () => resolve(fx.snapshot()))),
    );
    render(
      <LanguageProvider initial="pl">
        <App />
      </LanguageProvider>,
    );
    await act(async () => {
      vi.advanceTimersByTime(400);
    });
    expect(bar()).not.toBeNull();
    expect(bar()).not.toHaveClass("loading-pulse");
    release();
    await flush();
    expect(bar()).toBeNull();
  });
});

describe("new battle motion (spec 29 §6)", () => {
  type Handler = (event: { name: string; payload?: unknown }) => void;
  let emit: Handler = () => undefined;
  const animate = vi.fn();
  beforeEach(() => {
    mocks.onEvents.mockImplementation((handler: Handler) => {
      emit = handler;
      return () => {};
    });
    animate.mockClear();
    Object.defineProperty(Element.prototype, "animate", { value: animate, configurable: true });
  });
  afterEach(() => {
    delete (Element.prototype as { animate?: unknown }).animate;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });
  async function newBattle(summary = fx.summary(10, 3_400)) {
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    mocks.sessionSummary.mockResolvedValue(summary);
    await act(async () => emit({ name: "data-changed" }));
    await flush();
  }
  const tiles = (container: HTMLElement) =>
    Array.from(container.querySelectorAll(".battle-strip .tile"));
  const delta = (container: HTMLElement) => container.querySelector(".figure-delta");

  it("never moves anything on entering Session, also after battles came in meanwhile", async () => {
    const { container } = await renderApp();
    expect(container.querySelectorAll(".tile-new")).toHaveLength(0);
    expect(delta(container)).toBeNull();
    await go("Historia");
    await newBattle();
    await go("Sesja");
    expect(tiles(container)).toHaveLength(10);
    expect(container.querySelectorAll(".tile-new")).toHaveLength(0);
    expect(delta(container)).toBeNull();
  });

  it("slides in the new tile only, counts the damage up with the rising color", async () => {
    const { container } = await renderApp();
    await newBattle();
    const fresh = tiles(container).filter((tile) => tile.classList.contains("tile-new"));
    expect(fresh).toEqual([tiles(container)[9]]);
    const main = container.querySelector(".figure-main .figure-value") as HTMLElement;
    expect(main.querySelector(".figure-delta")).toHaveClass("figure-up");
    expect(main.style.minWidth).toMatch(/^\d+ch$/);
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 700));
    });
    expect(main.firstChild?.textContent?.replace(/\s/g, " ")).toBe("3 400");
  });

  it("colors a falling number with the falling color", async () => {
    const { container } = await renderApp();
    await newBattle(fx.summary(10, 3_000));
    expect(container.querySelector(".figure-main .figure-delta")).toHaveClass("figure-down");
  });

  it("moves nothing for a battle of an ended session", async () => {
    const endedOnly = () =>
      fx.snapshot({
        activeSession: null,
        recentSessions: fx.snapshot().recentSessions.map((item) => ({ ...item, status: "ended" })),
      });
    mocks.snapshot.mockResolvedValue(endedOnly());
    const ended = fx.summary();
    ended.session = { ...ended.session, status: "ended" } as typeof ended.session;
    mocks.sessionSummary.mockResolvedValue(ended);
    const { container } = await renderApp();
    const later = fx.summary(10, 3_400);
    later.session = { ...later.session, status: "ended" } as typeof later.session;
    mocks.snapshot.mockResolvedValue(endedOnly());
    mocks.sessionSummary.mockResolvedValue(later);
    await act(async () => emit({ name: "data-changed" }));
    await flush();
    expect(container.querySelectorAll(".tile-new")).toHaveLength(0);
    expect(delta(container)).toBeNull();
  });

  it("shows the new value at once with reduced motion", async () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("reduce"),
      media: query,
      addEventListener() {},
      removeEventListener() {},
    }));
    const { container } = await renderApp();
    await newBattle();
    expect(delta(container)).toBeNull();
    expect(
      container.querySelector(".figure-main .figure-value")?.textContent?.replace(/\s/g, " "),
    ).toBe("3 400");
    expect(animate).not.toHaveBeenCalled();
  });

  it("lets a new row enter History at the top and glides the rest down", async () => {
    layOutRows(500);
    const { container } = await renderApp();
    await go("Historia");
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 1_200));
    });
    const page = fx.historyPage(12);
    const newest = { ...page.items[0], battleId: "brand-new" };
    mocks.battleHistory.mockResolvedValue({ items: [newest, ...page.items], nextCursor: null });
    animate.mockClear();
    await newBattle();
    await frame();
    const rows = Array.from(container.querySelectorAll(".hrow-list > li"));
    const entering = rows.filter((row) => row.classList.contains("row-enter"));
    expect(entering).toHaveLength(1);
    expect(entering[0].getAttribute("data-flip")).toBe("brand-new");
    // The old rows moved one row (60 px) down: each glides from its old place. Rows every 60 px
    // from 200 in a 500 px window: the five that started above the bottom edge move, the rows
    // below the window before and after are left alone.
    expect(animate).toHaveBeenCalledTimes(5);
    for (const [frames] of animate.mock.calls)
      expect(frames[0]).toEqual({ transform: "translate(0px, -60px)" });
  });

  it("moves no rows in History on a filter change", async () => {
    layOutRows(2_000);
    await renderApp();
    await go("Historia");
    animate.mockClear();
    // The filtered page starts with another battle, so the rows that stay would move down.
    const page = fx.historyPage(5);
    mocks.battleHistory.mockResolvedValue({
      items: [{ ...page.items[0], battleId: "other" }, ...page.items],
      nextCursor: null,
    });
    fireEvent.change(screen.getByLabelText("Mapa"), { target: { value: "91_test_valley" } });
    await flush();
    await frame();
    expect(animate).not.toHaveBeenCalled();
  });
});

describe("History screen", () => {
  it("filters by tank and map and has an empty state for the filter", async () => {
    await renderApp();
    await go("Historia");
    expect(screen.getByRole("heading", { name: "Historia" })).toBeInTheDocument();
    mocks.battleHistory.mockResolvedValue({ items: [], nextCursor: null });
    fireEvent.change(screen.getByLabelText("Mapa"), { target: { value: "91_test_valley" } });
    await flush();
    expect(mocks.battleHistory).toHaveBeenLastCalledWith(
      expect.objectContaining({ mapKey: "91_test_valley" }),
    );
    expect(screen.getByText("Brak bitew dla tego filtra.")).toBeInTheDocument();
    await go("Wyczyść filtr");
    expect(mocks.battleHistory).toHaveBeenLastCalledWith(
      expect.objectContaining({ mapKey: null, vehicleId: null }),
    );
  });

  it("says when the tank list of the filter cannot be loaded instead of an empty list", async () => {
    mocks.vehicleHistoryOverview.mockRejectedValue({ code: "database_error" });
    await renderApp();
    await go("Historia");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Nie udało się wczytać listy czołgów do filtra.",
    );
    mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview());
    fireEvent.click(
      within(screen.getByRole("alert")).getByRole("button", { name: "Spróbuj ponownie" }),
    );
    await flush();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Czołg")).getByRole("option", { name: "Alpha" }),
    ).toBeTruthy();
  });

  it("lists sessions", async () => {
    await renderApp();
    await go("Historia");
    fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
    await flush();
    expect(screen.getAllByRole("button", { name: /9 bitew/ }).length).toBeGreaterThan(0);
  });
});

describe("weekly report (E7)", () => {
  it("shows the week's goals with where they come from, the trend and last week", async () => {
    const { container } = await renderApp();
    await go("Cele");
    expect(
      screen.getByRole("heading", { level: 1, name: "Cele na ten tydzień" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/^9–15 listopada, 39 bitew do tej pory$/)).toBeInTheDocument();
    const goals = screen.getByRole("region", { name: "Cele tygodnia" });
    expect(
      within(goals).getByRole("heading", { name: "Przeżyj co najmniej 35% bitew" }),
    ).toBeInTheDocument();
    expect(
      within(goals).getByRole("heading", {
        name: /Na Alpha średnio co najmniej 3\s300 obrażeń z asystą/,
      }),
    ).toBeInTheDocument();
    expect(
      within(goals).getByText("Cel to Twoja mediana z 4 tygodni (30%) plus 5 punktów."),
    ).toBeInTheDocument();
    expect(
      within(goals).getByText(/Twoja średnia z 4 tygodni \(3\s000\) plus 10%\./),
    ).toBeInTheDocument();
    expect(within(goals).getByText("teraz, 12 z 39")).toBeInTheDocument();
    expect(within(goals).getByText("osiągnięty, 14 bitew")).toBeInTheDocument();
    expect(within(goals).getByText("blisko, 96%")).toBeInTheDocument();
    expect(within(goals).getByRole("img", { name: "cel osiągnięty" })).toBeInTheDocument();
    const trend = screen
      .getByRole("heading", { name: "Ten tydzień wobec poprzednich czterech" })
      .closest("section")!;
    expect(within(trend).getByText("104%")).toBeInTheDocument();
    expect(within(trend).getByText("poprzednio 97–101%")).toBeInTheDocument();
    expect(within(trend).getByText("poprzednio 95–108%")).toBeInTheDocument();
    expect(within(trend).getByText("tylko kontekst, nie cel")).toBeInTheDocument();
    expect(within(trend).getByText(/2\s150/)).toBeInTheDocument();
    expect(screen.getByText(/Wyzwania na 5 bitew w tym tygodniu: ukończone 1/)).toBeInTheDocument();
    // D12: no goal is about wins.
    expect(goals.textContent).not.toMatch(/wygran|win rate/i);
    expectNoGluedNumbers(trend);
    expect(container.querySelector(".weekly-goal")).not.toBeNull();
  });

  it("says too few battles for a trend and for goals", async () => {
    mocks.weeklyReport.mockResolvedValue(
      fx.weeklyReport({
        battles: 6,
        confidence: "too_few",
        goals: [],
        indicators: [
          { metric: "damage_assist", current: null, previous: [null, null, null, null] },
        ],
        wn8: null,
        previousWeek: { weekStart: 0, battles: 3, goals: [] },
      }),
    );
    await renderApp();
    await go("Cele");
    expect(screen.getByText("Za mało bitew na trend (6 z 15).")).toBeInTheDocument();
    expect(screen.getByText(/Za mało bitew w czterech poprzednich tygodniach/)).toBeInTheDocument();
    expect(screen.getByText("poprzednio za mało bitew")).toBeInTheDocument();
    expect(screen.queryByText("WN8")).not.toBeInTheDocument();
    expect(
      screen.getByText(/Bez celów: za mało wcześniejszych bitew \(3 bitew/),
    ).toBeInTheDocument();
  });

  it("judges last week's goals in one line and still shows the challenges when the report fails", async () => {
    mocks.weeklyReport.mockResolvedValue(
      fx.weeklyReport({
        previousWeek: {
          weekStart: 0,
          battles: 46,
          goals: fx.weeklyReport().goals.map((goal, index) => ({
            ...goal,
            state: index < 2 ? ("achieved" as const) : ("below" as const),
          })),
        },
      }),
    );
    await renderApp();
    await go("Cele");
    expect(screen.getByText("2 z 3 celów osiągnięte w 46 bitwach.")).toBeInTheDocument();
    cleanup();
    mocks.weeklyReport.mockRejectedValue({ code: "database_error" });
    await renderApp();
    await go("Cele");
    expect(screen.getByRole("alert")).toHaveTextContent("Zapis lub odczyt danych się nie powiódł");
    expect(screen.getByRole("heading", { name: "Wyzwania na 5 bitew" })).toBeInTheDocument();
  });
});

describe("Goals screen", () => {
  it("shows the week summary under the week header (spec 30 §2, §4.3)", async () => {
    mocks.recap.mockImplementation((kind: string) =>
      Promise.resolve(kind === "week" ? fx.weekRecap() : fx.sessionRecap()),
    );
    await renderApp();
    await go("Cele");
    expect(mocks.recap).toHaveBeenCalledWith("week", "current");
    const card = screen.getByRole("region", { name: "Podsumowanie tygodnia" });
    expect(within(card).getAllByRole("listitem").length).toBeGreaterThanOrEqual(3);
    const survival = within(card).getByRole("button", { name: /^Fakt F3: przetrwanie 92%/u });
    fireEvent.mouseEnter(survival);
    expect(document.getElementById("recap-target-indicator-survival")).toHaveClass(
      "recap-highlight",
    );
    fireEvent.mouseLeave(survival);
  });

  it("says only that the week has too few battles under 15", async () => {
    mocks.recap.mockImplementation((kind: string) =>
      Promise.resolve(kind === "week" ? fx.weekRecap(6) : fx.sessionRecap()),
    );
    await renderApp();
    await go("Cele");
    const card = screen.getByRole("region", { name: "Podsumowanie tygodnia" });
    const sentences = within(card).getAllByRole("listitem");
    expect(sentences).toHaveLength(1);
    expect(sentences[0].textContent).toMatch(/^Za mało bitew w tym tygodniu/u);
  });

  it("hides the week summary in English", async () => {
    mocks.recap.mockImplementation((kind: string) =>
      Promise.resolve(kind === "week" ? fx.weekRecap() : fx.sessionRecap()),
    );
    await renderApp("en");
    await go("Goals");
    expect(
      screen.queryByRole("region", { name: /Podsumowanie|summary/iu }),
    ).not.toBeInTheDocument();
  });

  it("shows the active goal from the existing 5-battle challenges and never a win-rate goal", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeChallenge: fx.activeChallenge() }));
    await renderApp();
    await go("Cele");
    expect(
      screen.getByText(/Na Alpha: co najmniej 1\s900 obrażeń w 3 z 5 bitew/),
    ).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "2 z 5 bitew celu" })).toBeInTheDocument();
    const title = screen.getByRole("heading", { name: /Na Alpha: co najmniej/ });
    expect(within(title).getByRole("img", { name: "Alpha" })).toBeInTheDocument();
    expect(document.querySelector("main .map-backdrop")).toBeNull();
    expect(document.body.textContent).not.toMatch(/procent wygranych|win rate/i);
  });

  it("is empty when there is nothing to suggest", async () => {
    await renderApp();
    await go("Cele");
    expect(screen.getAllByText(/pojawią się tu propozycje celów/).length).toBeGreaterThan(0);
  });
});

describe("Settings screen", () => {
  it("chooses how Polish texts address the player, neutral by default (spec 30 §2)", async () => {
    await renderApp();
    await go("Ustawienia");
    const select = screen.getByRole("combobox", { name: "Jak się do Ciebie zwracać" });
    expect(select).toHaveValue("neutral");
    expect(
      within(select)
        .getAllByRole("option")
        .map((option) => option.textContent),
    ).toEqual([
      "Bez formy (wygrana, czołg przetrwał)",
      "Forma męska (zadałeś)",
      "Forma żeńska (zadałaś)",
    ]);
    fireEvent.change(select, { target: { value: "feminine" } });
    expect(localStorage.getItem("afterbattle-address")).toBe("feminine");
    expect(select).toHaveValue("feminine");
  });

  it("deletes data only after typing the word in the current language", async () => {
    await renderApp("en");
    await go("Settings");
    await go("Delete all data");
    const dialog = screen.getByRole("dialog");
    const confirm = within(dialog).getByRole("button", { name: "Delete all data" });
    expect(confirm).toBeDisabled();
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "USUŃ" } });
    expect(confirm).toBeDisabled();
    fireEvent.change(within(dialog).getByRole("textbox"), { target: { value: "DELETE" } });
    fireEvent.click(confirm);
    await flush();
    expect(mocks.deleteAllData).toHaveBeenCalledTimes(1);
  });

  it("shows privacy in the same screen and the rebuild report in words", async () => {
    mocks.diagnostics.mockResolvedValue(
      JSON.stringify({
        rebuildReport: { recalculated: 2013, notRandom: 188, withoutReplay: 3, conflicts: 0 },
      }),
    );
    await renderApp();
    await go("Ustawienia");
    expect(screen.getByRole("heading", { name: "Prywatność" })).toBeInTheDocument();
    expect(screen.getByText(/trzymane tylko w pamięci/)).toBeInTheDocument();
    expect(
      screen.getByText(
        /2\s013 bitew przeliczono, 188 nielosowych pominięto, 3 pełnych bitew bez pliku/,
      ),
    ).toBeInTheDocument();
    expect(screen.queryByText(/niepełnych zapisów bitew/)).not.toBeInTheDocument();
    expectNoCodes(document.body);
  });

  it("reports full battles without a file, skipped partial rows and empty sessions separately", async () => {
    mocks.diagnostics.mockResolvedValue(
      JSON.stringify({
        rebuildReport: {
          recalculated: 123,
          notRandom: 0,
          withoutReplay: 2,
          conflicts: 0,
          skippedPartial: 503,
          skippedEmptySessions: 14,
        },
      }),
    );
    await renderApp();
    await go("Ustawienia");
    expect(screen.getByText(/123 bitew przeliczono/)).toHaveTextContent(
      "2 pełnych bitew bez pliku",
    );
    expect(
      screen.getByText(
        "Pominięto 503 niepełnych zapisów bitew i 14 pustych sesji. Zostają w kopii zapasowej.",
      ),
    ).toBeInTheDocument();
    expectNoCodes(document.body);
  });
});

/**
 * Spec 25 §5 and §8: every screen in the states loading / empty / error / no XVM / too few
 * battles, where the state exists for that screen. The matrix with these test names is in
 * the author's E5-fix stage notes.
 */
describe("screen states (spec 25 §5)", () => {
  const pending = () => new Promise<never>(() => {});
  const failure = { code: "database_error" };
  const errorText = "Zapis lub odczyt danych się nie powiódł";
  const main = () => screen.getByRole("main");
  const busy = () =>
    screen.queryAllByRole("status").filter((item) => item.getAttribute("aria-busy") === "true");
  const openFirstBattle = async () => {
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
  };
  function withoutXvm() {
    const base = fx.snapshot();
    return fx.snapshot({
      advancedStatus: { xvmEnabled: false, xvmLastUpdatedAt: null },
      wn8Presentation: {
        ...base.wn8Presentation,
        expectedValues: {
          ...base.wn8Presentation.expectedValues,
          availability: "unavailable",
          sourceVersion: null,
        },
      },
    } as never);
  }
  const suggestion = {
    suggestionId: "suggestion-1",
    challengeType: "damage_consistency",
    vehicleName: "Alpha",
    thresholdValue: 1900,
    contributionFloor: null,
    requiredSuccesses: 3,
    targetBattles: 5,
    frozenBaselineValue: 2702,
  };

  describe("shell", () => {
    it("is loading until the snapshot arrives and explains a snapshot error with a retry", async () => {
      mocks.snapshot.mockReturnValue(pending());
      await renderApp();
      expect(busy()).toHaveLength(1);
      cleanup();
      mocks.snapshot.mockReset();
      mocks.snapshot.mockRejectedValueOnce(failure).mockResolvedValue(fx.snapshot());
      await renderApp();
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
      await go("Spróbuj ponownie");
      expect(
        screen.getByRole("heading", { level: 1, name: /Mocna sesja|Dzisiejsza sesja/ }),
      ).toBeInTheDocument();
    });
  });

  describe("Tanks", () => {
    it("is loading before the list and before the tank details arrive", async () => {
      mocks.vehicleHistoryOverview.mockReturnValue(pending());
      await renderApp();
      await go("Czołgi");
      expect(busy()).toHaveLength(1);
      expect(screen.queryByRole("heading", { name: "Alpha" })).not.toBeInTheDocument();
      cleanup();
      mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview());
      mocks.vehicleBaseline.mockReturnValue(pending());
      await renderApp();
      await go("Czołgi");
      expect(screen.getByRole("heading", { name: "Alpha" })).toBeInTheDocument();
      expect(busy()).toHaveLength(1);
      expect(
        screen.queryByRole("heading", { name: "Twoje mapy na tym czołgu" }),
      ).not.toBeInTheDocument();
    });

    it("explains an error of the list and of the tank details and retries", async () => {
      mocks.vehicleHistoryOverview.mockRejectedValueOnce(failure).mockResolvedValue(fx.overview());
      await renderApp();
      await go("Czołgi");
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
      await go("Spróbuj ponownie");
      expect(screen.getByRole("heading", { name: "Alpha" })).toBeInTheDocument();
      cleanup();
      mocks.vehicleBaseline.mockRejectedValueOnce(failure);
      await renderApp();
      await go("Czołgi");
      expect(screen.getByRole("heading", { name: "Alpha" })).toBeInTheDocument();
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
      await go("Spróbuj ponownie");
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      expect(screen.getByRole("heading", { name: "Twoje mapy na tym czołgu" })).toBeInTheDocument();
    });

    it("keeps every number without XVM and shows no WN8", async () => {
      mocks.snapshot.mockResolvedValue(withoutXvm());
      await renderApp();
      await go("Czołgi");
      expect(screen.getByText(/^Czołg średni, X poziom, 214 bitew/)).toBeInTheDocument();
      expect(screen.getByText(/2\s940/)).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /Synthetic Hills: średnio 3\s480 obrażeń/ }),
      ).toBeInTheDocument();
      expect(main().textContent).not.toMatch(/WN8/);
    });
  });

  describe("Battle", () => {
    it("is loading before the battle arrives and can still go back", async () => {
      mocks.battleView.mockReturnValue(pending());
      await renderApp();
      await openFirstBattle();
      expect(busy()).toHaveLength(1);
      expect(screen.queryByRole("heading", { level: 1, name: /na mapie/ })).not.toBeInTheDocument();
      await go("← Wróć");
      expect(
        screen.getByRole("heading", { level: 1, name: /Mocna sesja|Dzisiejsza sesja/ }),
      ).toBeInTheDocument();
    });

    it("explains an error and retries", async () => {
      mocks.battleView.mockRejectedValueOnce(failure);
      await renderApp();
      await openFirstBattle();
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
      await go("Spróbuj ponownie");
      expect(screen.getByRole("heading", { level: 1, name: /na mapie/ })).toBeInTheDocument();
    });

    it("shows the result and the teams without XVM and no WN8", async () => {
      mocks.snapshot.mockResolvedValue(withoutXvm());
      await renderApp();
      await openFirstBattle();
      expect(screen.getByText(/5\s210/)).toBeInTheDocument();
      expect(screen.getByRole("heading", { name: "Drużyny" })).toBeInTheDocument();
      expect(main().textContent).not.toMatch(/WN8/);
    });
  });

  describe("Goals", () => {
    it("is loading before suggestions and goal history arrive", async () => {
      mocks.challengeSuggestions.mockReturnValue(pending());
      mocks.challengeHistory.mockReturnValue(pending());
      await renderApp();
      await go("Cele");
      expect(
        screen.getByRole("heading", { level: 1, name: "Cele na ten tydzień" }),
      ).toBeInTheDocument();
      expect(busy()).toHaveLength(2);
    });

    it("explains an error of suggestions and of goal history and retries", async () => {
      mocks.challengeSuggestions.mockRejectedValueOnce(failure);
      mocks.challengeHistory.mockRejectedValueOnce(failure);
      await renderApp();
      await go("Cele");
      const alerts = screen.getAllByRole("alert");
      expect(alerts).toHaveLength(2);
      alerts.forEach((alert) => expect(alert).toHaveTextContent(errorText));
      for (const retry of screen.getAllByRole("button", { name: "Spróbuj ponownie" }))
        fireEvent.click(retry);
      await flush();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      expect(screen.getByText("Nie masz jeszcze zakończonych celów.")).toBeInTheDocument();
    });

    it("explains a failed start of a goal", async () => {
      mocks.challengeSuggestions.mockResolvedValue({
        suggestions: [suggestion],
        unavailableReason: null,
      });
      mocks.startVehicleChallenge.mockRejectedValue(failure);
      await renderApp();
      await go("Cele");
      await go("Zacznij");
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
    });

    it("suggests goals without XVM", async () => {
      mocks.snapshot.mockResolvedValue(withoutXvm());
      mocks.challengeSuggestions.mockResolvedValue({
        suggestions: [suggestion],
        unavailableReason: null,
      });
      await renderApp();
      await go("Cele");
      expect(
        screen.getByText(/Na Alpha: co najmniej 1\s900 obrażeń w 3 z 5 bitew/),
      ).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Zacznij" })).toBeInTheDocument();
      expect(main().textContent).not.toMatch(/WN8/);
    });

    it("names the 3-battle threshold when there is no finished session yet", async () => {
      mocks.snapshot.mockResolvedValue(fx.snapshot({ recentSessions: [fx.session()] }));
      await renderApp();
      await go("Cele");
      expect(
        screen.getByText(
          "Po sesji z co najmniej 3 bitwami na jednym czołgu pojawią się tu propozycje celów.",
        ),
      ).toBeInTheDocument();
      expect(mocks.challengeSuggestions).not.toHaveBeenCalled();
    });
  });

  describe("History", () => {
    it("is loading before the first page arrives", async () => {
      mocks.battleHistory.mockReturnValue(pending());
      await renderApp();
      await go("Historia");
      expect(screen.getByRole("heading", { name: "Historia" })).toBeInTheDocument();
      expect(busy()).toHaveLength(1);
    });

    it("is empty without battles, says so during an import and has an empty session list", async () => {
      mocks.battleHistory.mockResolvedValue({ items: [], nextCursor: null });
      await renderApp();
      await go("Historia");
      expect(
        screen.getByText("Historia jest pusta. Wypełni się z plików powtórek."),
      ).toBeInTheDocument();
      cleanup();
      const base = fx.snapshot();
      mocks.snapshot.mockResolvedValue(
        fx.snapshot({
          sourceStatus: { ...base.sourceStatus, importProgress: { done: 300, total: 2000 } },
          recentSessions: [],
          activeSession: null,
        }),
      );
      await renderApp();
      await go("Historia");
      expect(screen.getByText("Historia wypełnia się z plików powtórek.")).toBeInTheDocument();
      expect(screen.getByText(/Odczyt powtórek: 300 z 2\s000/)).toBeInTheDocument();
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      expect(screen.getByText("Nie ma jeszcze żadnej sesji.")).toBeInTheDocument();
    });

    it("explains an error of the first page and of a further page", async () => {
      mocks.battleHistory.mockRejectedValueOnce(failure);
      await renderApp();
      await go("Historia");
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
      mocks.battleHistory.mockResolvedValue({
        ...fx.historyPage(),
        nextCursor: { battleTime: 1, battleId: "battle-0" },
      });
      await go("Spróbuj ponownie");
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      const rows = screen.getAllByRole("button", { name: BATTLE_BUTTON }).length;
      expect(rows).toBeGreaterThan(0);
      mocks.battleHistory.mockRejectedValueOnce(failure);
      await go("Pokaż kolejne");
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
      expect(screen.getAllByRole("button", { name: BATTLE_BUTTON })).toHaveLength(rows);
    });

    it("shows session WN8 only with XVM", async () => {
      mocks.recentSessionsWn8.mockResolvedValue([{ sessionId: "session-1", wn8: { value: 2150 } }]);
      await renderApp();
      await go("Historia");
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      expect(screen.getByText(/WN8 2\s150/)).toBeInTheDocument();
      cleanup();
      mocks.snapshot.mockResolvedValue(withoutXvm());
      await renderApp();
      await go("Historia");
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      expect(screen.getAllByRole("button", { name: /9 bitew/ }).length).toBeGreaterThan(0);
      expect(main().textContent).not.toMatch(/WN8/);
    });
  });

  describe("Settings", () => {
    it("is usable while the catalog status and the diagnostic report are loading", async () => {
      mocks.vehicleClassCatalogStatus.mockReturnValue(pending());
      mocks.diagnostics.mockReturnValue(pending());
      await renderApp();
      await go("Ustawienia");
      expect(screen.getByRole("heading", { name: "Ustawienia" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Kopiuj raport diagnostyczny" })).toBeDisabled();
      expect(screen.getByText("Katalog pojazdów nie jest jeszcze gotowy.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Znajdź grę" })).toBeEnabled();
    });

    it("says what is missing before the game folder, WN8 data and catalog exist", async () => {
      const base = fx.snapshot();
      mocks.snapshot.mockResolvedValue(
        fx.snapshot({
          sourceStatus: { ...base.sourceStatus, configured: false, gameRootDisplay: null },
          wn8Presentation: {
            ...base.wn8Presentation,
            expectedValues: { ...base.wn8Presentation.expectedValues, sourceVersion: null },
          },
        } as never),
      );
      mocks.vehicleClassCatalogStatus.mockResolvedValue({ vehicleCount: 0, sourceVersion: null });
      await renderApp();
      await go("Ustawienia");
      expect(screen.getByText("Nie wybrano folderu gry.")).toBeInTheDocument();
      expect(screen.getByText("Brak pobranych danych.")).toBeInTheDocument();
      expect(screen.getByText("Katalog pojazdów nie jest jeszcze gotowy.")).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Odczytaj wszystkie powtórki" })).toBeDisabled();
    });

    it("explains a failed action and survives a failed diagnostic report", async () => {
      mocks.diagnostics.mockRejectedValue(failure);
      mocks.detectInstallations.mockRejectedValue(failure);
      await renderApp();
      await go("Ustawienia");
      expect(screen.getByRole("button", { name: "Kopiuj raport diagnostyczny" })).toBeDisabled();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      await go("Znajdź grę");
      expect(screen.getByRole("alert")).toHaveTextContent(errorText);
    });

    it("shows XVM as off without a refresh button", async () => {
      mocks.snapshot.mockResolvedValue(withoutXvm());
      await renderApp();
      await go("Ustawienia");
      expect(screen.getByRole("checkbox", { name: "Pobieraj dane WN8 z XVM" })).not.toBeChecked();
      expect(screen.queryByRole("button", { name: "Odśwież dane WN8" })).not.toBeInTheDocument();
      expect(screen.getByText("Brak pobranych danych.")).toBeInTheDocument();
      expect(screen.getByRole("contentinfo")).toHaveTextContent("Dane WN8: wyłączone");
    });
  });
});

describe("texts", () => {
  it.each(["pl", "en"] as const)("no screen shows a code in %s", async (language) => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeChallenge: fx.activeChallenge() }));
    await renderApp(language);
    for (const label of language === "pl"
      ? ["Czołgi", "Cele", "Historia", "Ustawienia", "Sesja"]
      : ["Tanks", "Goals", "History", "Settings", "Session"]) {
      await go(label);
      expectNoCodes(document.body);
    }
  });
});

describe("game folder (hotfix 1.0.0-beta.3)", () => {
  const unconfigured = () => {
    const base = fx.snapshot();
    return fx.snapshot({
      configured: false,
      activeSession: null,
      recentSessions: [],
      sourceStatus: { ...base.sourceStatus, configured: false, gameRootDisplay: null },
    } as never);
  };

  it("lets Settings choose the folder by hand next to detection", async () => {
    mocks.pickGameFolder.mockResolvedValue(String.raw`D:\Games\World_of_Tanks_EU\replays`);
    await renderApp();
    await go("Ustawienia");
    expect(screen.getByRole("button", { name: "Znajdź grę" })).toBeEnabled();
    await go("Wskaż folder gry");
    expect(mocks.pickGameFolder).toHaveBeenCalledTimes(1);
    // The backend normalizes and checks the folder; the window passes it on unchanged.
    expect(mocks.configureLocal).toHaveBeenCalledWith(
      String.raw`D:\Games\World_of_Tanks_EU\replays`,
      "pl",
    );
    expect(screen.getByRole("status")).toHaveTextContent("Zapisano.");
  });

  it("does nothing when the folder dialog is closed without a choice", async () => {
    await renderApp();
    await go("Ustawienia");
    await go("Wskaż folder gry");
    expect(mocks.pickGameFolder).toHaveBeenCalledTimes(1);
    expect(mocks.configureLocal).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText("Zapisano.")).not.toBeInTheDocument();
  });

  it("says exactly what is wrong with a chosen folder", async () => {
    mocks.pickGameFolder.mockResolvedValue(String.raw`D:\Games\World_of_Tanks_EU`);
    mocks.configureLocal.mockRejectedValue({ code: "replays_folder_empty" });
    await renderApp();
    await go("Ustawienia");
    await go("Wskaż folder gry");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Folder z nagraniami bitew (replays) jest pusty. W grze włącz Ustawienia → Ogólne → Nagrywaj bitwy",
    );
    expect(screen.queryByText("Zapisano.")).not.toBeInTheDocument();
  });

  it("says when detection finds nothing and offers the manual choice", async () => {
    await renderApp("en");
    await go("Settings");
    await go("Find the game");
    expect(
      screen.getByText("The game was not found on this computer. Choose the folder manually."),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Choose the game folder" })).toBeEnabled();
  });

  it("shows the detected folders and uses the one picked", async () => {
    mocks.detectInstallations.mockResolvedValue([String.raw`E:\Games\World_of_Tanks_NA`]);
    await renderApp();
    await go("Ustawienia");
    await go("Znajdź grę");
    expect(screen.queryByText(/Nie znaleziono gry/)).not.toBeInTheDocument();
    await go("Użyj tego folderu");
    expect(mocks.configureLocal).toHaveBeenCalledWith(String.raw`E:\Games\World_of_Tanks_NA`, "pl");
  });

  it("asks for the game folder on Session at the first start, with detection and manual choice", async () => {
    mocks.snapshot.mockResolvedValue(unconfigured());
    mocks.detectInstallations.mockResolvedValue([String.raw`D:\Games\World_of_Tanks_EU`]);
    mocks.pickGameFolder.mockResolvedValue(String.raw`F:\WoT\WorldOfTanks.exe`);
    await renderApp();
    expect(screen.getByRole("heading", { name: "Wskaż folder gry" })).toBeInTheDocument();
    expect(screen.getByText(/folder z nagraniami bitew \(replays\) w nim/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Otwórz Ustawienia" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Nie ma jeszcze żadnej sesji/)).not.toBeInTheDocument();
    await go("Znajdź grę");
    await go("Użyj tego folderu");
    expect(mocks.configureLocal).toHaveBeenLastCalledWith(
      String.raw`D:\Games\World_of_Tanks_EU`,
      "pl",
    );
    await go("Wskaż ręcznie");
    expect(mocks.configureLocal).toHaveBeenLastCalledWith(
      String.raw`F:\WoT\WorldOfTanks.exe`,
      "pl",
    );
  });

  it("explains a wrong folder on the first start screen", async () => {
    mocks.snapshot.mockResolvedValue(unconfigured());
    mocks.pickGameFolder.mockResolvedValue(String.raw`D:\Temp\NotAGame`);
    mocks.configureLocal.mockRejectedValue({ code: "game_executable_missing" });
    await renderApp("en");
    await go("Choose manually");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "This folder has no WorldOfTanks.exe file.",
    );
    expectNoCodes(document.body);
  });
});
