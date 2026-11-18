import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";
import { sessionSentences } from "../lib/recap/session";
import * as fx from "../test/fixtures";
import { expectNoGluedNumbers } from "../test/glued";
import { APP_VERSION } from "../release";
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
  pause: vi.fn(),
  resume: vi.fn(),
  startImport: vi.fn(),
  setXvmEnabled: vi.fn(),
  refreshExpectedValues: vi.fn(),
  updatePreferences: vi.fn(),
  referenceTables: vi.fn(),
  updateConsent: vi.fn(),
  setUpdateConsent: vi.fn(),
  checkForUpdate: vi.fn(),
  installUpdate: vi.fn(),
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
  for (const action of [
    mocks.pause,
    mocks.resume,
    mocks.startImport,
    mocks.setXvmEnabled,
    mocks.refreshExpectedValues,
    mocks.updatePreferences,
  ])
    action.mockResolvedValue(undefined);
  mocks.referenceTables.mockResolvedValue(fx.referenceTables());
  mocks.updateConsent.mockResolvedValue("off");
  mocks.setUpdateConsent.mockImplementation((enabled: boolean) =>
    Promise.resolve(enabled ? "on" : "off"),
  );
  mocks.checkForUpdate.mockResolvedValue(null);
  mocks.installUpdate.mockResolvedValue(undefined);
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

/** Spec 31 §3: opens a summary card ("Pokaż więcej (N)"); returns its sentences, first one first. */
function expandRecap(card: HTMLElement): HTMLElement[] {
  const more = within(card).queryByRole("button", { name: /^Pokaż więcej/u });
  if (more) fireEvent.click(more);
  return recapSentences(card);
}

/** The sentences of a summary card the player can see now (the hidden rest is left out). */
function recapSentences(card: HTMLElement): HTMLElement[] {
  return [
    card.querySelector<HTMLElement>(".recap-first")!,
    ...within(card).queryAllByRole("listitem"),
  ];
}

beforeEach(() => {
  vi.clearAllMocks();
  // A test that chose an address form or opened a summary must not leak it into the next one.
  localStorage.removeItem("afterbattle-address");
  localStorage.removeItem("afterbattle-recap-expanded");
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

  // Spec 32 §4: Overview is the first destination (was: six, Session first).
  it("shows the seven destinations with Maps greyed out in the private build", async () => {
    await renderApp();
    const nav = screen.getByRole("navigation", { name: "Główna nawigacja" });
    const names = within(nav)
      .getAllByRole("button")
      .map((button) => button.textContent?.trim());
    expect(names).toHaveLength(7);
    expect(names[0]).toContain("Przegląd");
    expect(names[1]).toContain("Sesja");
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
    const sentences = expandRecap(card);
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

  it("shows only the first sentence until 'Pokaż więcej (N)' opens the rest (spec 31 §3)", async () => {
    await renderApp();
    const card = screen.getByRole("region", { name: "Podsumowanie sesji" });
    expect(card.querySelector(".recap-first")?.textContent).toMatch(/9 bitew/u);
    expect(within(card).queryAllByRole("listitem")).toHaveLength(0);
    const more = within(card).getByRole("button", { name: /^Pokaż więcej \(\d\)$/u });
    const hidden = Number(/\((\d)\)/u.exec(more.textContent ?? "")?.[1]);
    expect(more).toHaveAttribute("aria-expanded", "false");
    const rest = document.getElementById(more.getAttribute("aria-controls") ?? "")!;
    expect(rest).toBeInTheDocument();
    // Hidden sentences take no focus and are not read out.
    expect(rest).toHaveAttribute("inert");
    expect(rest).toHaveAttribute("aria-hidden", "true");
    expect(rest.querySelectorAll("li")).toHaveLength(hidden);
    // No sentence is numbered, the first one and the rest alike (author, 2026-11-17).
    expect(rest.querySelector("ol")).not.toHaveAttribute("start");

    fireEvent.click(more);
    const less = within(card).getByRole("button", { name: "Pokaż mniej" });
    expect(less).toBe(more);
    expect(less).toHaveAttribute("aria-expanded", "true");
    expect(rest).not.toHaveAttribute("inert");
    expect(within(card).getAllByRole("listitem")).toHaveLength(hidden);
    expect(localStorage.getItem("afterbattle-recap-expanded")).toBe("true");

    fireEvent.click(less);
    expect(less).toHaveAttribute("aria-expanded", "false");
    expect(localStorage.getItem("afterbattle-recap-expanded")).toBe("false");
  });

  it("remembers the open summary for every screen, and drops highlights hidden by closing", async () => {
    mocks.recap.mockImplementation((kind: string) =>
      Promise.resolve(kind === "battle" ? fx.battleRecap() : fx.sessionRecap()),
    );
    await renderApp();
    const card = screen.getByRole("region", { name: "Podsumowanie sesji" });
    fireEvent.click(within(card).getByRole("button", { name: /^Pokaż więcej/u }));
    // A reference of a later sentence (damage, F3; F1 and F2 share the first) highlights its
    // number; closing the card removes it.
    const damage = within(card).getByRole("button", { name: /^Fakt F3:/u });
    const figure = document.getElementById("recap-target-damage")!;
    expect(card.querySelector(".recap-first")?.textContent).not.toMatch(/F3/u);
    fireEvent.focus(damage);
    expect(figure).toHaveClass("recap-highlight");
    fireEvent.click(within(card).getByRole("button", { name: "Pokaż mniej" }));
    expect(figure).not.toHaveClass("recap-highlight");
    fireEvent.click(within(card).getByRole("button", { name: /^Pokaż więcej/u }));

    // One setting for all screens: the battle summary opens already expanded.
    fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
    await flush();
    const battle = screen.getByRole("region", { name: "Podsumowanie bitwy" });
    expect(within(battle).getByRole("button", { name: "Pokaż mniej" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    expect(within(battle).getAllByRole("listitem").length).toBeGreaterThanOrEqual(2);
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
    // Spec 32 §4.1: without a session the app opens Overview; Session keeps its empty state.
    await go("Sesja");
    expect(screen.getByText(/^Zagraj bitwę losową, a wyniki sesji/)).toBeInTheDocument();
    cleanup();
    mocks.snapshot.mockResolvedValue(fx.snapshot({ configured: false }));
    await renderApp();
    // RC-Start (fixup after STOP 2): the first start is the welcome card.
    expect(screen.getByRole("heading", { name: "Witaj w Afterbattle" })).toBeInTheDocument();
  });

  it("asks on the first run how to address the player, neutral until answered (2026-11-16)", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ configured: false }));
    await renderApp();
    // RC-Start: the segmented switch of Settings; "Bez formy" is the neutral default shown
    // as chosen, nothing is stored until the player picks.
    const group = screen.getByRole("radiogroup", { name: "Jak się do Ciebie zwracać?" });
    const choices = within(group).getAllByRole("radio");
    expect(choices.map((choice) => choice.textContent)).toEqual(["On", "Ona", "Bez formy"]);
    expect(within(group).getByRole("radio", { name: "Bez formy" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(localStorage.getItem("afterbattle-address")).toBeNull();
    fireEvent.click(within(group).getByRole("radio", { name: "Ona" }));
    expect(localStorage.getItem("afterbattle-address")).toBe("feminine");
    expect(within(group).getByRole("radio", { name: "Ona" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    fireEvent.click(within(group).getByRole("radio", { name: "Bez formy" }));
    expect(localStorage.getItem("afterbattle-address")).toBe("neutral");
    cleanup();
    // The forms exist only in Polish.
    await renderApp("en");
    expect(screen.queryByRole("radiogroup")).not.toBeInTheDocument();
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
    const sentences = expandRecap(card);
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
    expect(screen.getByText(/^Zagraj bitwę losową, a czołgi/)).toBeInTheDocument();
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
    expect(expandRecap(card).length).toBeGreaterThanOrEqual(3);
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
    const sentences = recapSentences(card);
    expect(sentences).toHaveLength(1);
    expect(sentences[0].textContent).toMatch(/^Za mało bitew w tym tygodniu/u);
    // One sentence: nothing to open.
    expect(within(card).queryByRole("button", { name: /^Pokaż/u })).not.toBeInTheDocument();
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
  // Spec 31 §4: the new layout keeps every option working as before. These tests find each
  // option by its name, not by its place, so they hold across the rearrangement.
  describe("every option (characterization before the spec 31 §4 layout)", () => {
    it("game folder: find, use, choose by hand, pause and resume", async () => {
      mocks.detectInstallations.mockResolvedValue(["C:\\Games\\SyntheticGame"]);
      mocks.pickGameFolder.mockResolvedValue("D:\\SyntheticGame");
      await renderApp();
      await go("Ustawienia");
      expect(screen.getByText("C:\\Games\\SyntheticGame")).toBeInTheDocument();
      await go("Znajdź grę");
      expect(mocks.detectInstallations).toHaveBeenCalledTimes(1);
      await go("Użyj tego folderu");
      expect(mocks.configureLocal).toHaveBeenCalledWith("C:\\Games\\SyntheticGame", "pl");
      await go("Wskaż folder gry");
      expect(mocks.configureLocal).toHaveBeenLastCalledWith("D:\\SyntheticGame", "pl");
      // Author's fixup after STOP 1: saving battles is a switch like WN8 (was: a button).
      const saving = screen.getByRole("checkbox", { name: "Zapisywanie bitew" });
      expect(saving).toBeChecked();
      expect(screen.queryByText("Włączone")).not.toBeInTheDocument();
      fireEvent.click(saving);
      await flush();
      expect(mocks.pause).toHaveBeenCalledTimes(1);
    });

    it("resumes saving when paused", async () => {
      const base = fx.snapshot();
      mocks.snapshot.mockResolvedValue({
        ...base,
        sourceStatus: { ...base.sourceStatus, paused: true },
      });
      await renderApp();
      await go("Ustawienia");
      const saving = screen.getByRole("checkbox", { name: "Zapisywanie bitew" });
      expect(saving).not.toBeChecked();
      fireEvent.click(saving);
      await flush();
      expect(mocks.resume).toHaveBeenCalledTimes(1);
    });

    it("data: read all battle files, vehicle catalog, WN8 from XVM with refresh", async () => {
      await renderApp();
      await go("Ustawienia");
      await go("Odczytaj wszystkie powtórki");
      expect(mocks.startImport).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/^Wersja klienta 2\.0, 1\s270 pojazdów$/u)).toBeInTheDocument();
      const xvm = screen.getByRole("checkbox", { name: "Pobieraj dane WN8 z XVM" });
      expect(xvm).toBeChecked();
      fireEvent.click(xvm);
      await flush();
      expect(mocks.setXvmEnabled).toHaveBeenCalledWith(false);
      await go("Odśwież dane WN8");
      expect(mocks.refreshExpectedValues).toHaveBeenCalledTimes(1);
    });

    // Spec 31 §4 changes how these are saved: at once, without "Zapisz" (was: one Save button).
    it("you: language and the inactivity limit are saved at once, with 'Zapisano.'", async () => {
      await renderApp();
      await go("Ustawienia");
      expect(screen.queryByRole("button", { name: "Zapisz" })).not.toBeInTheDocument();
      const minutes = screen.getByRole("spinbutton", { name: /Limit bezczynności/u });
      fireEvent.change(minutes, { target: { value: "45" } });
      expect(mocks.updatePreferences).not.toHaveBeenCalled();
      fireEvent.blur(minutes);
      await flush();
      expect(mocks.updatePreferences).toHaveBeenCalledWith("pl", 45);
      const you = screen.getByRole("region", { name: "Ty" });
      expect(within(you).getByRole("status")).toHaveTextContent("Zapisano.");
      // Leaving the field unchanged saves nothing again.
      fireEvent.blur(minutes);
      await flush();
      expect(mocks.updatePreferences).toHaveBeenCalledTimes(1);
      fireEvent.click(screen.getByRole("radio", { name: "English" }));
      await flush();
      expect(mocks.updatePreferences).toHaveBeenLastCalledWith("en", 45);
      expect(screen.getByRole("heading", { name: "Settings", level: 1 })).toBeInTheDocument();
      expect(screen.getByRole("radiogroup", { name: "Language" })).toBeInTheDocument();
    });

    it("you: the address form is kept at once, with 'Zapisano.'", async () => {
      await renderApp();
      await go("Ustawienia");
      fireEvent.click(screen.getByRole("radio", { name: "On" }));
      await flush();
      expect(localStorage.getItem("afterbattle-address")).toBe("masculine");
      const you = screen.getByRole("region", { name: "Ty" });
      expect(within(you).getByRole("status")).toHaveTextContent("Zapisano.");
    });

    it("about: privacy texts, diagnostics copied, version", async () => {
      const writeText = vi.fn(() => Promise.resolve());
      Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
      await renderApp();
      await go("Ustawienia");
      expect(screen.getByText(/Aplikacja nie ma telemetrii/u)).toBeInTheDocument();
      await go("Kopiuj raport diagnostyczny");
      expect(writeText).toHaveBeenCalledWith("{}");
      const about = screen.getByRole("region", { name: "O aplikacji" });
      expect(about).toHaveTextContent(APP_VERSION);
      expect(screen.getByRole("button", { name: "Usuń wszystkie dane" })).toBeInTheDocument();
      // No repeats (author, 2026-11-17): WN8 data is in Data, the version only in About.
      expect(about).not.toHaveTextContent(/Dane WN8/u);
      expect(screen.queryByRole("contentinfo")).not.toBeInTheDocument();
    });
  });

  describe("layout (spec 31 §4)", () => {
    const groups = ["Gra", "Ty", "Dane", "Prywatność i dane", "O aplikacji"];
    const group = (name: string) => screen.getByRole("region", { name });

    it("shows the groups in the order of use, with a list of links to them", async () => {
      await renderApp();
      await go("Ustawienia");
      const headings = within(screen.getByRole("main"))
        .getAllByRole("heading", { level: 2 })
        .map((heading) => heading.textContent);
      expect(headings).toEqual(groups);
      const links = within(screen.getByRole("navigation", { name: "Grupy ustawień" })).getAllByRole(
        "link",
      );
      expect(links.map((link) => link.textContent)).toEqual(groups);
      for (const [index, link] of links.entries()) {
        const target = document.getElementById(link.getAttribute("href")!.slice(1));
        expect(target, groups[index]).toBe(group(groups[index]));
      }
    });

    it("puts each option in its group", async () => {
      await renderApp();
      await go("Ustawienia");
      const has = (name: string, role: "button" | "checkbox" | "radiogroup", label: RegExp) =>
        expect(within(group(name)).getByRole(role, { name: label })).toBeInTheDocument();
      has("Gra", "button", /^Znajdź grę$/u);
      has("Gra", "button", /^Wskaż folder gry$/u);
      has("Gra", "checkbox", /^Zapisywanie bitew$/u);
      has("Ty", "radiogroup", /^Język$/u);
      has("Ty", "radiogroup", /^Jak się do Ciebie zwracać$/u);
      expect(
        within(group("Ty")).getByRole("spinbutton", { name: /Limit bezczynności/u }),
      ).toBeInTheDocument();
      has("Dane", "button", /^Odczytaj wszystkie powtórki$/u);
      has("Dane", "checkbox", /^Pobieraj dane WN8 z XVM$/u);
      expect(within(group("Dane")).getByText(/^Wersja klienta 2\.0/u)).toBeInTheDocument();
      expect(
        within(group("Prywatność i dane")).getByText(/nie ma telemetrii/u),
      ).toBeInTheDocument();
      has("O aplikacji", "button", /^Kopiuj raport diagnostyczny$/u);
      // Deleting everything comes last in its group; only its button is in the warning colour.
      const privacy = within(group("Prywatność i dane")).getAllByRole("button");
      expect(privacy.at(-1)).toHaveAccessibleName("Usuń wszystkie dane");
      expect(privacy.at(-1)).toHaveClass("button-danger");
      expect(document.querySelector(".setting-danger")).toBeNull();
    });

    it("marks the group in view in the list of links (aria-current)", async () => {
      const observers: { callback: IntersectionObserverCallback; targets: Element[] }[] = [];
      class FakeObserver {
        targets: Element[] = [];
        constructor(public callback: IntersectionObserverCallback) {
          observers.push(this);
        }
        observe(target: Element) {
          this.targets.push(target);
        }
        unobserve() {}
        disconnect() {}
      }
      vi.stubGlobal("IntersectionObserver", FakeObserver);
      try {
        await renderApp();
        await go("Ustawienia");
        const nav = screen.getByRole("navigation", { name: "Grupy ustawień" });
        const current = () =>
          within(nav)
            .getAllByRole("link")
            .filter((link) => link.getAttribute("aria-current") === "true")
            .map((link) => link.textContent);
        expect(current()).toEqual(["Gra"]);
        const observer = observers.at(-1)!;
        expect(observer.targets.map((target) => target.id)).toEqual([
          "settings-game",
          "settings-you",
          "settings-data",
          "settings-privacy",
          "settings-about",
        ]);
        const entry = (id: string, isIntersecting: boolean) =>
          ({
            target: document.getElementById(id)!,
            isIntersecting,
          }) as unknown as IntersectionObserverEntry;
        act(() =>
          observer.callback(
            [entry("settings-game", false), entry("settings-data", true)],
            observer as unknown as IntersectionObserver,
          ),
        );
        expect(current()).toEqual(["Dane"]);
      } finally {
        vi.unstubAllGlobals();
      }
    });

    it("gives every option a label and one sentence about it", async () => {
      await renderApp();
      await go("Ustawienia");
      const options = [...document.querySelectorAll<HTMLElement>(".setting")];
      expect(options.length).toBeGreaterThanOrEqual(10);
      for (const option of options) {
        const label = option.querySelector(".setting-label")?.textContent ?? "";
        const description = option.querySelector(".setting-desc")?.textContent ?? "";
        expect(label, option.outerHTML).not.toBe("");
        expect(description, label).toMatch(/^[A-ZĄĆĘŁŃÓŚŹŻ„][^.!?]*[^.!?\s][.!?]$/u);
      }
      expectNoCodes(document.body);
    });

    it("has the same groups in English", async () => {
      await renderApp("en");
      await go("Settings");
      const headings = within(screen.getByRole("main"))
        .getAllByRole("heading", { level: 2 })
        .map((heading) => heading.textContent);
      expect(headings).toEqual(["Game", "You", "Data", "Privacy and data", "About"]);
    });
  });

  it("chooses how Polish texts address the player, neutral by default (spec 30 §2)", async () => {
    await renderApp();
    await go("Ustawienia");
    // Author's fixup after STOP 1: a segmented control On / Ona / Bez formy, like the first run.
    const group = screen.getByRole("radiogroup", { name: "Jak się do Ciebie zwracać" });
    const radios = within(group).getAllByRole("radio");
    expect(radios.map((radio) => radio.textContent)).toEqual(["On", "Ona", "Bez formy"]);
    expect(within(group).getByRole("radio", { name: "Bez formy" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    fireEvent.click(within(group).getByRole("radio", { name: "Ona" }));
    expect(localStorage.getItem("afterbattle-address")).toBe("feminine");
    expect(within(group).getByRole("radio", { name: "Ona" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
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
    expect(screen.getByRole("heading", { name: "Prywatność i dane" })).toBeInTheDocument();
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
        screen.getByText("Odczytaj powtórki z folderu gry, a historia wypełni się sama."),
      ).toBeInTheDocument();
      // Spec 31 §5: the empty state has its action.
      await go("Odczytaj wszystkie powtórki");
      expect(mocks.startImport).toHaveBeenCalledTimes(1);
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
      expect(
        screen.getByText("Zagraj bitwę losową, a sesje pojawią się tu same."),
      ).toBeInTheDocument();
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
      // The footer is on every screen but Settings (author's fixup after STOP 1).
      expect(screen.getByRole("contentinfo")).toHaveTextContent("Dane WN8: wyłączone");
      await go("Ustawienia");
      expect(screen.getByRole("checkbox", { name: "Pobieraj dane WN8 z XVM" })).not.toBeChecked();
      expect(screen.queryByRole("button", { name: "Odśwież dane WN8" })).not.toBeInTheDocument();
      expect(screen.getByText("Brak pobranych danych.")).toBeInTheDocument();
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
    expect(screen.getByRole("heading", { name: "Folder gry" })).toBeInTheDocument();
    expect(screen.getByText(/folder z nagraniami bitew \(replays\) w nim/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Otwórz Ustawienia" })).not.toBeInTheDocument();
    expect(screen.queryByText(/a wyniki sesji pojawią się/)).not.toBeInTheDocument();
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

describe("UI review (spec 31 §5)", () => {
  it("says in one sentence what to do on every empty screen", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null, recentSessions: [] }));
    mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview(0));
    mocks.battleHistory.mockResolvedValue({ items: [], nextCursor: null });
    await renderApp();
    const texts: string[] = [];
    const collect = () =>
      texts.push(
        ...[...document.querySelectorAll(".card.state > p")].map((p) => p.textContent ?? ""),
      );
    collect();
    await go("Czołgi");
    collect();
    await go("Historia");
    collect();
    fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
    await flush();
    collect();
    expect(texts).toHaveLength(4);
    for (const text of texts) expect(text).toMatch(/^[A-ZĄĆĘŁŃÓŚŹŻ][^.!?]*[^.!?\s]\.$/u);
  });
});

describe("Tank screen: against the players in your battles (spec 32 §3)", () => {
  const section = () => screen.getByRole("region", { name: "Na tle graczy z Twoich bitew" });
  const openTanks = async () => {
    await renderApp();
    await go("Czołgi");
  };

  it("shows the tier and class cell with percentiles and the player's median, without judging", async () => {
    await openTanks();
    expect(mocks.referenceTables).toHaveBeenCalled();
    const card = section();
    expect(card).toHaveTextContent(/Czołgi średnie X poziomu: 5\s354 wyników innych graczy/u);
    expect(card).toHaveTextContent(/Twoja mediana ze wszystkich 131 Twoich bitew w tej komórce/u);
    const damage = within(card).getByRole("group", { name: "Obrażenia" });
    // RC-Czolg: name and "Ty X · gracze Y" on the left, the bar, the percentile on the right.
    expect(damage).toHaveTextContent(/Ty 1\s870 · gracze 1\s650/u);
    expect(damage.querySelector(".ref-percentile")).toHaveTextContent(/^58\.\s*percentyl$/u);
    // p25–p75 and p90 are in the bar's label instead of a line of text under it.
    expect(within(damage).getByRole("img")).toHaveAccessibleName(
      /Obrażenia: środkowa połowa graczy 1\s100–2\s300, mediana graczy 1\s650, 90% graczy do 3\s050, Ty 1\s870/u,
    );
    expect(damage).not.toHaveTextContent(/25–75%|90%/u);
    expect(within(card).getAllByRole("group")).toHaveLength(7);
    expect(card.textContent).not.toMatch(/lepsz|gorsz|słab|dobr|świetn|ocen/iu);
    expectNoCodes(card);
  });

  it("explains the bar in a legend (RC-Czolg)", async () => {
    await openTanks();
    const legend = within(section()).getByRole("list", { name: "Legenda" });
    expect(
      within(legend)
        .getAllByRole("listitem")
        .map((item) => item.textContent),
    ).toEqual([
      "pasek: środkowa połowa graczy",
      "kreska: mediana graczy",
      "kropka: Twoja mediana",
      "prawa kreska: 90% graczy",
    ]);
    for (const mark of [
      ".ref-legend-range",
      ".ref-legend-median",
      ".ref-legend-own",
      ".ref-legend-p90",
    ])
      expect(legend.querySelector(mark), mark).not.toBeNull();
  });

  it("leaves the dot out of the legend without the player's median", async () => {
    mocks.referenceTables.mockResolvedValue(
      fx.referenceTables({
        ownBattles: 4,
        metrics: [
          fx.referenceMetric("damage", { ownCount: 4, ownMedian: null, ownPosition: null }),
        ],
      }),
    );
    await openTanks();
    const legend = within(section()).getByRole("list", { name: "Legenda" });
    expect(legend).not.toHaveTextContent(/kropka/u);
  });

  it("shows in every row the numbers of its own metric", async () => {
    await openTanks();
    const rows: [string, RegExp, string][] = [
      ["Obrażenia", /Ty 1\s870 · gracze 1\s650/u, "58."],
      ["Asysta (radio i gąsienice)", /Ty 410 · gracze 320/u, "63."],
      ["Wykrycia", /Ty 1,5 · gracze 1(?!,)/u, "61."],
      ["Zniszczenia", /Ty 1 · gracze 1/u, "53."],
      ["Zablokowane pancerzem", /Ty 1\s200 · gracze 860/u, "66."],
      ["Czas życia (s)", /Ty 269 · gracze 277/u, "47."],
    ];
    for (const [name, numbers, percentile] of rows) {
      const row = within(section()).getByRole("group", { name });
      expect(row, name).toHaveTextContent(numbers);
      expect(row.querySelector(".ref-percentile"), name).toHaveTextContent(percentile);
    }
    expect(
      within(within(section()).getByRole("group", { name: "Czas życia (s)" })).getByRole("img"),
    ).toHaveAccessibleName(
      /środkowa połowa graczy 180–380, mediana graczy 277, 90% graczy do 440/u,
    );
  });

  it("keeps accuracy as background only", async () => {
    await openTanks();
    const accuracy = within(section()).getByRole("group", { name: "Celność (%)" });
    expect(accuracy).toHaveTextContent(/tylko tło/u);
    expect(accuracy).toHaveTextContent(/gracze 71/u);
    expect(accuracy).not.toHaveTextContent(/Ty /u);
    expect(accuracy.querySelector(".ref-percentile")).toBeNull();
    expect(accuracy.querySelector(".ref-own")).toBeNull();
  });

  it("highlights the mark of a number on hover and focus", async () => {
    await openTanks();
    const damage = within(section()).getByRole("group", { name: "Obrażenia" });
    const median = within(damage).getByText(/gracze 1\s650/u);
    const mark = damage.querySelector(".ref-median")!;
    fireEvent.mouseEnter(median);
    expect(mark).toHaveClass("is-active");
    fireEvent.mouseLeave(median);
    expect(mark).not.toHaveClass("is-active");
    fireEvent.focus(within(damage).getByText(/Ty 1\s870/u));
    expect(damage.querySelector(".ref-own")).toHaveClass("is-active");
  });

  it("says how far the background is from 200 results", async () => {
    mocks.referenceTables.mockResolvedValue(
      fx.referenceTables({ level: "count_only", population: 120 }),
    );
    await openTanks();
    expect(section()).toHaveTextContent("Za mało danych: tło od 200 wyników, masz 120.");
    expect(within(section()).queryAllByRole("group")).toHaveLength(0);
  });

  it("marks an approximate background and the player's median from 10 battles", async () => {
    mocks.referenceTables.mockResolvedValue(
      fx.referenceTables({
        level: "approximate",
        population: 140,
        ownBattles: 4,
        metrics: [
          fx.referenceMetric("damage", { ownCount: 4, ownMedian: null, ownPosition: null }),
        ],
      }),
    );
    await openTanks();
    expect(section()).toHaveTextContent(/Orientacyjnie: mniej niż 200 wyników \(140\)/u);
    expect(section()).toHaveTextContent("Twoja mediana od 10 bitew w tej komórce (masz 4).");
    expect(section().querySelector(".ref-own")).toBeNull();
  });

  it("explains a tank without a tier and a cell below tier VI", async () => {
    mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview(3, null));
    await openTanks();
    expect(section()).toHaveTextContent("Porównanie wymaga tieru i klasy czołgu.");
    cleanup();
    mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview(3, 5));
    mocks.referenceTables.mockResolvedValue(
      fx.referenceTables({ tier: 5, level: "count_only", population: 300 }),
    );
    await openTanks();
    expect(section()).toHaveTextContent("Porównanie od VI poziomu.");
  });
});

describe("Overview and the start screen (spec 32 §4)", () => {
  const noRecentSession = () =>
    fx.snapshot({
      activeSession: null,
      recentSessions: [fx.session({ id: "session-0", status: "ended", endedAt: fx.NOW })],
    });
  const openOverview = async () => {
    mocks.snapshot.mockResolvedValue(noRecentSession());
    mocks.performanceTrends.mockResolvedValue(fx.overviewTrends());
    await renderApp();
  };
  const block = (name: string) => screen.getByRole("region", { name });

  it("opens Overview without a recent session; it is first in the navigation", async () => {
    await openOverview();
    expect(screen.getByRole("heading", { name: "Przegląd", level: 1 })).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: "Główna nawigacja" });
    const first = within(nav).getAllByRole("button")[0];
    expect(first).toHaveAccessibleName("Przegląd");
    expect(first).toHaveAttribute("aria-current", "page");
  });

  it("opens Session during a session, and a choice in the navigation wins", async () => {
    await renderApp();
    expect(screen.getByRole("region", { name: "Podsumowanie sesji" })).toBeInTheDocument();
    await go("Przegląd");
    expect(screen.getByRole("heading", { name: "Przegląd", level: 1 })).toBeInTheDocument();
  });

  it("shows the last session with its first summary sentence and opens it", async () => {
    await openOverview();
    const last = block("Ostatnia sesja");
    expect(mocks.sessionSummary).toHaveBeenCalledWith("session-0");
    expect(last).toHaveTextContent(/9 bitew/u);
    // RC-Przeglad: the card spans the row, the best battle's map is its background, and the
    // big numbers are the average damage, the wins and the session's WN8.
    expect(last).toHaveClass("map-backdrop");
    expect(last).toHaveTextContent(/3\s184\s*średnie obrażenia/u);
    expect(last).toHaveTextContent(/6 z 9\s*wygranych/u);
    expect(last).toHaveTextContent(/2\s150\s*WN8 sesji/u);
    expect(last).toHaveTextContent(/Wygrane: 6 z 9 bitew/u);
    fireEvent.click(within(last).getByRole("button", { name: /Otwórz sesję/u }));
    await flush();
    expect(screen.getByRole("region", { name: "Podsumowanie sesji" })).toBeInTheDocument();
  });

  it("shows trends over 100 battles, win rate as context, WN8 only with XVM", async () => {
    await openOverview();
    const trends = block("Trendy");
    expect(mocks.performanceTrends).toHaveBeenCalledWith("last100");
    // One caption (author's fixup after STOP 2).
    expect(trends.querySelector(".overview-note")).toHaveTextContent(
      /^ostatnie 100 bitew wobec 100 wcześniejszych$/u,
    );
    expect(trends).not.toHaveTextContent(/100 ostatnich bitew wobec/u);
    // A small line chart per metric, described in words.
    expect(within(trends).getAllByRole("img")).toHaveLength(3);
    expect(within(trends).getAllByRole("img")[0]).toHaveAccessibleName(
      /Obrażenia w 10 punktach ostatnich 100 bitew/u,
    );
    expect(trends).toHaveTextContent(/Obrażenia\s*2\s702/u);
    expect(trends).toHaveTextContent(/poprzednio 2\s600/u);
    expect(trends).toHaveTextContent(/tylko kontekst/u);
    expect(trends).toHaveTextContent(/WN8\s*2\s150/u);
    cleanup();
    mocks.snapshot.mockResolvedValue({ ...noRecentSession(), ...{} });
    mocks.performanceTrends.mockResolvedValue(fx.overviewTrends(false));
    await renderApp();
    expect(block("Trendy")).toHaveTextContent(/WN8: wyłączone/u);
  });

  it("names the best tanks by their percentile in their group and the most played in a line", async () => {
    await openOverview();
    const tanks = block("Czołgi");
    // Decision 2026-11-18: the percentile of the tank's median damage in its tier × class.
    expect(tanks).toHaveTextContent(
      "Najwyższy percentyl mediany obrażeń czołgu na tle graczy z Twoich bitew w tej samej grupie tier × klasa, od 10 bitew.",
    );
    const rows = within(tanks).getAllByRole("listitem");
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringMatching(/^Bravo.*średni X.*64 bitwy.*88\..*w średnich X$/u),
      expect.stringMatching(/^Alpha.*131 bitew.*81\..*w średnich X$/u),
      expect.stringMatching(/^Charlie.*41 bitew.*79\..*w średnich X$/u),
    ]);
    expect(tanks).toHaveTextContent(
      /Najczęściej grane: Alpha \(214\), Bravo \(174\), Charlie \(134\)/u,
    );
    expect(tanks.textContent).not.toMatch(/lepsz|gorsz|słab|świetn/iu);
    fireEvent.click(within(tanks).getAllByRole("button", { name: /Alpha/u })[0]);
    await flush();
    expect(screen.getByRole("heading", { name: "Alpha", level: 1 })).toBeInTheDocument();
  });

  it("places the player against the players of their battles, position only", async () => {
    mocks.referenceTables.mockResolvedValue({
      ...fx.referenceTables(),
      cells: [
        ...fx.referenceTables().cells,
        {
          ...fx.referenceTables().cells[0],
          tier: 6,
          vehicleClass: "light_tank",
          level: "count_only",
          population: 120,
          ownBattles: 4,
          metrics: [],
        },
        {
          ...fx.referenceTables().cells[0],
          tier: 4,
          vehicleClass: "light_tank",
          level: "count_only",
          population: 90,
          metrics: [],
        },
      ],
    });
    await openOverview();
    const background = block("Na tle graczy z Twoich bitew");
    expect(background).toHaveTextContent(/Percentyl Twojej mediany obrażeń w grupie tier × klasa/u);
    const tiles = within(background).getAllByRole("listitem");
    expect(tiles.map((tile) => tile.textContent)).toEqual([
      expect.stringMatching(/^średnie X\s*58\.$/u),
      expect.stringMatching(/^lekkie VI\s*Tło od 200 wyników, masz 120\.$/u),
    ]);
    // Four columns, no holes: one value tile (1) and the collecting tile widened to 3.
    expect(tiles[0].style.gridColumn).toBe("");
    expect(tiles[1].style.gridColumn).toBe("span 3");
    fireEvent.click(within(background).getByRole("button", { name: "Otwórz czołgi" }));
    await flush();
    expect(screen.getByRole("heading", { name: "Alpha", level: 1 })).toBeInTheDocument();
  });

  it("opens History from the trends and shows the map of the session's best battle", async () => {
    await openOverview();
    // RC-Przeglad: the card itself carries the map (no separate picture inside it).
    expect(block("Ostatnia sesja")).toHaveClass("map-backdrop");
    fireEvent.click(within(block("Trendy")).getByRole("button", { name: "Otwórz historię" }));
    await flush();
    expect(screen.getByRole("heading", { name: "Historia", level: 1 })).toBeInTheDocument();
  });

  it("shows the week's goals", async () => {
    await openOverview();
    const goals = block("Cele tygodnia");
    // RC-Przeglad: the count of goals met and a bar for each goal.
    expect(goals).toHaveTextContent(/1 z 3\s*celów spełnionych/u);
    expect(within(goals).getAllByRole("img")).toHaveLength(3);
    fireEvent.click(within(goals).getByRole("button", { name: /Otwórz cele/u }));
    await flush();
    expect(
      screen.getByRole("heading", { name: "Cele na ten tydzień", level: 1 }),
    ).toBeInTheDocument();
  });

  it("says when the best tanks appear if no cell has a background yet", async () => {
    mocks.referenceTables.mockResolvedValue(
      fx.referenceTables({ level: "count_only", population: 120 }),
    );
    await openOverview();
    expect(block("Czołgi")).toHaveTextContent(
      "Pojawią się, gdy czołg ma 10 bitew w grupie z co najmniej 200 wynikami innych graczy.",
    );
    expect(block("Czołgi")).toHaveTextContent(/Najczęściej grane: Alpha \(214\)/u);
  });

  it("keeps every number in one piece (no breakable space inside a number)", async () => {
    await openOverview();
    const main = screen.getByRole("main");
    expect(main.textContent).toMatch(/3\u00a0184/u);
    expect(main.textContent).not.toMatch(/\d[ \u2009]\d{3}/u);
  });

  it("says what to do with no battles yet", async () => {
    mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null, recentSessions: [] }));
    mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview(0));
    await renderApp();
    expect(
      screen.getByText("Zagraj bitwę losową, a przegląd wypełni się sam."),
    ).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Trendy" })).not.toBeInTheDocument();
  });
});

describe("automatic updates (spec 32 §5)", () => {
  it("asks once whether to check for updates and checks only after a yes", async () => {
    mocks.updateConsent.mockResolvedValue("unasked");
    await renderApp();
    const ask = screen.getByRole("group", { name: "Sprawdzać aktualizacje przy starcie?" });
    // Makieta RC-Start: what the check sends, said in the question itself.
    expect(ask).toHaveTextContent(/Nic o Tobie nie wysyła/u);
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
    fireEvent.click(within(ask).getByRole("button", { name: "Tak" }));
    await flush();
    expect(mocks.setUpdateConsent).toHaveBeenCalledWith(true);
    expect(mocks.checkForUpdate).toHaveBeenCalledTimes(1);
    await go("Dalej");
    expect(
      screen.queryByRole("group", { name: "Sprawdzać aktualizacje przy starcie?" }),
    ).not.toBeInTheDocument();
  });

  it("asks on the first-run screen once, below the folder and the address", async () => {
    mocks.updateConsent.mockResolvedValue("unasked");
    mocks.snapshot.mockResolvedValue(fx.snapshot({ configured: false }));
    await renderApp();
    const asks = screen.getAllByRole("group", { name: "Sprawdzać aktualizacje przy starcie?" });
    expect(asks).toHaveLength(1);
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
  });

  it("says when the install failed and keeps the notice", async () => {
    mocks.updateConsent.mockResolvedValue("on");
    mocks.checkForUpdate.mockResolvedValue({ version: "1.0.1", notes: null });
    mocks.installUpdate.mockRejectedValue({ code: "update_error" });
    await renderApp();
    fireEvent.click(screen.getByRole("button", { name: "Zainstaluj i uruchom ponownie" }));
    await flush();
    expect(screen.getByRole("alert")).toHaveTextContent(/Nie udało się zainstalować/u);
    expect(screen.getByRole("region", { name: "Dostępna wersja 1.0.1" })).toBeInTheDocument();
  });

  it("checks once after the switch is turned on in Settings", async () => {
    await renderApp();
    await go("Ustawienia");
    fireEvent.click(screen.getByRole("checkbox", { name: "Sprawdzanie aktualizacji" }));
    await flush();
    expect(mocks.checkForUpdate).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("checkbox", { name: "Sprawdzanie aktualizacji" }));
    await flush();
    fireEvent.click(screen.getByRole("checkbox", { name: "Sprawdzanie aktualizacji" }));
    await flush();
    expect(mocks.checkForUpdate).toHaveBeenCalledTimes(1);
  });

  it("makes no request after a no", async () => {
    mocks.updateConsent.mockResolvedValue("unasked");
    await renderApp();
    fireEvent.click(screen.getByRole("button", { name: "Nie" }));
    await flush();
    expect(mocks.setUpdateConsent).toHaveBeenCalledWith(false);
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
  });

  it("offers a found update and installs it only on a click", async () => {
    mocks.updateConsent.mockResolvedValue("on");
    mocks.checkForUpdate.mockResolvedValue({ version: "1.0.1", notes: "Poprawki." });
    await renderApp();
    const notice = screen.getByRole("region", { name: "Dostępna wersja 1.0.1" });
    expect(notice).toHaveTextContent("Poprawki.");
    expect(mocks.checkForUpdate).toHaveBeenCalledTimes(1);
    expect(mocks.installUpdate).not.toHaveBeenCalled();
    fireEvent.click(within(notice).getByRole("button", { name: "Później" }));
    expect(screen.queryByRole("region", { name: /Dostępna wersja/u })).not.toBeInTheDocument();
    cleanup();
    await renderApp();
    fireEvent.click(screen.getByRole("button", { name: "Zainstaluj i uruchom ponownie" }));
    await flush();
    expect(mocks.installUpdate).toHaveBeenCalledTimes(1);
  });

  it("has a switch in Privacy and data; without consent About points to the releases page", async () => {
    await renderApp();
    await go("Ustawienia");
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
    const privacy = screen.getByRole("region", { name: "Prywatność i dane" });
    const toggle = within(privacy).getByRole("checkbox", { name: "Sprawdzanie aktualizacji" });
    expect(toggle).not.toBeChecked();
    expect(screen.getByRole("region", { name: "O aplikacji" })).toHaveTextContent(
      "github.com/dataflorist02/afterbattle-beta/releases",
    );
    fireEvent.click(toggle);
    await flush();
    expect(mocks.setUpdateConsent).toHaveBeenCalledWith(true);
    expect(toggle).toBeChecked();
  });
});

describe("first start (RC-Start, author's fixup after STOP 2)", () => {
  const unconfigured = () => {
    const base = fx.snapshot();
    return fx.snapshot({
      configured: false,
      activeSession: null,
      recentSessions: [],
      sourceStatus: { ...base.sourceStatus, configured: false, gameRootDisplay: null },
    } as never);
  };
  /** Focusable controls of the welcome screen in Tab order (DOM order, no positive tabindex). */
  const tabOrder = () =>
    [...screen.getByRole("main").querySelectorAll<HTMLElement>("button, input, [tabindex]")]
      .filter((element) => !(element as HTMLButtonElement).disabled && element.tabIndex === 0)
      .map((element) => element.textContent);

  it("is one card without the navigation, three numbered steps, and starts only with a folder", async () => {
    mocks.snapshot.mockResolvedValue(unconfigured());
    mocks.updateConsent.mockResolvedValue("unasked");
    await renderApp();
    expect(screen.queryByRole("navigation", { name: "Główna nawigacja" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Witaj w Afterbattle", level: 1 })).toBeVisible();
    const card = screen.getByRole("region", { name: "Pierwsze ustawienia" });
    const steps = within(card).getAllByRole("group");
    expect(steps.map((step) => step.getAttribute("aria-label") ?? step.textContent)).toHaveLength(
      3,
    );
    expect(within(steps[0]).getByRole("heading", { name: "Folder gry" })).toBeVisible();
    expect(
      within(steps[1]).getByRole("heading", { name: "Jak się do Ciebie zwracać?" }),
    ).toBeVisible();
    expect(
      within(steps[2]).getByRole("heading", { name: "Sprawdzać aktualizacje przy starcie?" }),
    ).toBeVisible();
    expect(steps[2]).toHaveTextContent(/Nic o Tobie nie wysyła/u);
    expect(screen.getByRole("button", { name: "Zaczynamy" })).toBeDisabled();
    expect(tabOrder()).toEqual(["Znajdź grę", "Wskaż ręcznie", "Bez formy", "Tak", "Nie"]);
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
  });

  it("saves all three answers and opens the app after Zaczynamy", async () => {
    mocks.snapshot.mockResolvedValueOnce(unconfigured()).mockResolvedValue(fx.snapshot());
    mocks.updateConsent.mockResolvedValue("unasked");
    mocks.detectInstallations.mockResolvedValue([String.raw`D:\Games\World_of_Tanks_EU`]);
    mocks.configureLocal.mockResolvedValue(fx.snapshot());
    await renderApp();
    await go("Znajdź grę");
    await go("Użyj tego folderu");
    expect(mocks.configureLocal).toHaveBeenCalledWith(String.raw`D:\Games\World_of_Tanks_EU`, "pl");
    const folder = within(screen.getByRole("region", { name: "Pierwsze ustawienia" })).getAllByRole(
      "group",
    )[0];
    expect(folder).toHaveTextContent(/Znaleziony:/u);
    expect(folder.querySelector(".welcome-dot-done")).not.toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Ona" }));
    expect(localStorage.getItem("afterbattle-address")).toBe("feminine");
    fireEvent.click(screen.getByRole("button", { name: "Tak" }));
    await flush();
    expect(mocks.setUpdateConsent).toHaveBeenCalledWith(true);
    expect(screen.getByRole("button", { name: "Tak" })).toHaveAttribute("aria-pressed", "true");
    expect(tabOrder()).toEqual([
      "Znajdź grę",
      "Wskaż inny folder",
      "Użyj tego folderu",
      "Ona",
      "Tak",
      "Nie",
      "Zaczynamy",
    ]);
    await go("Zaczynamy");
    expect(screen.getByRole("navigation", { name: "Główna nawigacja" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Witaj w Afterbattle" })).not.toBeInTheDocument();
  });

  it("asks a player coming from a beta only about updates, in the same card", async () => {
    mocks.updateConsent.mockResolvedValue("unasked");
    await renderApp();
    expect(screen.queryByRole("navigation", { name: "Główna nawigacja" })).not.toBeInTheDocument();
    const card = screen.getByRole("region", { name: "Pierwsze ustawienia" });
    expect(within(card).getAllByRole("group")).toHaveLength(1);
    expect(card).toHaveTextContent(/Sprawdzać aktualizacje przy starcie\?/u);
    expect(screen.getByRole("button", { name: "Dalej" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Nie" }));
    await flush();
    expect(mocks.setUpdateConsent).toHaveBeenCalledWith(false);
    expect(mocks.checkForUpdate).not.toHaveBeenCalled();
    await go("Dalej");
    expect(screen.getByRole("navigation", { name: "Główna nawigacja" })).toBeInTheDocument();
  });

  it("does not ask a player who already answered", async () => {
    mocks.updateConsent.mockResolvedValue("off");
    await renderApp();
    expect(screen.queryByRole("region", { name: "Pierwsze ustawienia" })).not.toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Główna nawigacja" })).toBeInTheDocument();
  });
});
