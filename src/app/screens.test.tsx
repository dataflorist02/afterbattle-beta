import { readFileSync } from "node:fs";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";
import { sessionSentences } from "../lib/recap/session";
import * as fx from "../test/fixtures";
import { LoadedImage } from "../test/loadedImage";
import { expectNoGluedNumbers } from "../test/glued";
import { APP_VERSION } from "../release";
import type { Language } from "../types";
import { FULL_HEAT_PERCENT, SPARKS_MAX, SPARKS_MIN, USUAL_PERCENT } from "../lib/ember";
import { CARD_HEIGHT, CARD_WIDTH } from "../features/damast/card";
import {
  DAMAGE_WEIGHT,
  DAMAST_REFERENCE_BATTLES,
  MIN_TIER_BATTLES,
  WN8_WEIGHT,
} from "../features/damast/stats";
import {
  DAMAGE_THRESHOLD_PERCENT,
  MIN_BASELINE_BATTLES,
  MIN_SESSION_BATTLES,
} from "../lib/sessionReference";

const mocks = vi.hoisted(() => ({
  snapshot: vi.fn(),
  sessionSummary: vi.fn(),
  sessionBaselineComparison: vi.fn(),
  battleView: vi.fn(),
  vehicleHistoryOverview: vi.fn(),
  vehicleBaseline: vi.fn(),
  vehicleTemperatures: vi.fn(),
  vehicleHistoryReport: vi.fn(),
  performanceTrends: vi.fn(),
  battleHistory: vi.fn(),
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
  damastSession: vi.fn(),
  damastSessions: vi.fn(),
  damastTile: vi.fn(),
  putDamastTile: vi.fn(),
  exportDamastCard: vi.fn(),
  attemptsConversionSession: vi.fn(() => new Promise(() => {})),
  attemptsConversionRecent: vi.fn(() => new Promise(() => {})),
  onEvents: vi.fn<(handler: (event: { name: string; payload?: unknown }) => void) => () => void>(
    () => () => {},
  ),
}));
vi.mock("../api", () => ({ api: mocks }));
// Damast plates are drawn by a worker; on screen tests a still plate comes at once.
const damastEngine = vi.hoisted(() => ({
  renderStill: vi.fn(async (_plate: unknown, width: number, height: number, output: string) =>
    output === "pixels"
      ? { id: 1, pixels: new Uint8ClampedArray(width * height * 4) }
      : { id: 1, bytes: new Uint8Array([0xff, 0xd8, 0xff, 0xd9]).buffer },
  ),
}));
vi.mock("../features/damast/engine", () => damastEngine);

const { App } = await import("./App");
const { lastBattle, mapTag } = await import("../pages/TanksPage");
const { clearGameImages } = await import("../hooks/useGameImage");
const { resetTiles, tileKey: tileKeyOf } = await import("../features/damast/tiles");
const { damastStats } = await import("../features/damast/stats");

function setDefaults() {
  mocks.snapshot.mockResolvedValue(fx.snapshot());
  mocks.sessionSummary.mockResolvedValue(fx.summary());
  mocks.sessionBaselineComparison.mockResolvedValue(fx.comparison());
  mocks.battleView.mockResolvedValue(fx.battleDetail());
  mocks.vehicleHistoryOverview.mockResolvedValue(fx.overview());
  mocks.vehicleBaseline.mockResolvedValue(fx.vehicleBaseline());
  mocks.vehicleTemperatures.mockResolvedValue([]);
  mocks.vehicleHistoryReport.mockResolvedValue(fx.vehicleReport());
  mocks.performanceTrends.mockResolvedValue(fx.trends());
  mocks.battleHistory.mockResolvedValue(fx.historyPage());
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
  mocks.damastSession.mockImplementation((id: string) => Promise.resolve(fx.damastSession(id)));
  mocks.damastSessions.mockResolvedValue(fx.damastPage(fx.damastSessions(2)));
  // Plates already forged: their tiles come from the cache.
  mocks.damastTile.mockResolvedValue(new Uint8Array([0xff, 0xd8, 0xff, 0xd9]).buffer);
  mocks.putDamastTile.mockResolvedValue(undefined);
  mocks.exportDamastCard.mockResolvedValue(true);
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
    // Spec 33 §4: the comparison as a number and "zwykle", how it is counted behind the "?".
    expect(screen.getByText(/^o 18% więcej niż zwykle \(2\s702\)$/u)).toBeInTheDocument();
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

  // Spec 33 §4 moved the sentence from the Session screen to "Jak to działa" (was: on Session).
  it("states the inactivity limit from the settings (U5), in 'Jak to działa'", async () => {
    await renderApp();
    expect(screen.queryByText(/Sesja kończy się sama po 60 min/u)).not.toBeInTheDocument();
    await go("Ustawienia");
    fireEvent.click(screen.getByRole("button", { name: "Które bitwy się liczą" }));
    expect(screen.getByText(/kończy sama po 60 min bez bitwy/u)).toBeVisible();
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
    expect(screen.getByText(/^o 53% mniej niż zwykle \(3\s244\)$/u)).toBeVisible();
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
    // Author, 22.11: the average is named in the legend above, never on the chart itself.
    const figure = chart.closest(".chart-figure")!;
    expect(figure.querySelector(".chart-legend")).toHaveTextContent(/^Twoja średnia 2\s\d{3}$/u);
    expect(chart.textContent).not.toMatch(/średnia/u);
  });

  it("reads the damage chart from the trend's own battles, by keyboard", async () => {
    await renderApp();
    await go("Czołgi");
    const plot = document.querySelector(".chart-plot") as HTMLElement;
    fireEvent.keyDown(plot, { key: "End" });
    // The newest point of fx.trends(): ordinal 119, its battle 3 503 damage on Synthetic Hills.
    const said = (
      document.querySelector(".chart-figure [aria-live='polite']")?.textContent ?? ""
    ).replace(/\s/gu, " ");
    expect(said).toMatch(/^Bitwa 119 · /u);
    expect(said).toContain("obrażenia 3 503");
    expect(said).toContain("Synthetic Hills");
    expect(said).toContain("Alpha");
    expect(document.querySelector(".chart-tip")).not.toBeNull();
    fireEvent.keyDown(plot, { key: "Escape" });
    expect(document.querySelector(".chart-tip")).toBeNull();
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
    // Spec 33 §4: where the goals come from is in "Jak to działa" (Twoja średnia), behind a "?".
    expect(within(goals).queryByText(/Cel to Twoja mediana z 4 tygodni/u)).not.toBeInTheDocument();
    expect(within(goals).queryByText(/Twoja średnia z 4 tygodni/u)).not.toBeInTheDocument();
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
    // Wins stay context only (D12); the words saying so moved to "Jak to działa".
    expect(within(trend).queryByText("tylko kontekst, nie cel")).not.toBeInTheDocument();
    expect(
      within(trend).getByRole("button", { name: "Jak to działa: Twoja średnia" }),
    ).toBeInTheDocument();
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
      mocks.damastSessions.mockResolvedValue(fx.damastPage([]));
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
      // Spec 34 §3: the session cards are Damast plates with "WN8" after the number.
      await renderApp();
      await go("Historia");
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      expect(document.querySelector(".damast-tile")).toHaveTextContent(/1\s800\s*WN8/u);
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
    mocks.damastSessions.mockResolvedValue(fx.damastPage([]));
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
    // Spec 33 §4: what the marker means is in "Jak to działa" (Na tle graczy), behind the "?".
    expect(card).not.toHaveTextContent(/Znacznik: Twoja mediana/u);
    expect(
      within(card).getByRole("button", { name: "Jak to działa: Na tle graczy" }),
    ).toBeInTheDocument();
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

  // Spec 33 §4: the legend stays as signs; its words are for readers (and in "Jak to działa"),
  // on the screen only "Ty" by the dot (was: four visible phrases).
  it("explains the bar in a legend of signs (RC-Czolg, spec 33 §4)", async () => {
    await openTanks();
    const legend = within(section()).getByRole("list", { name: "Legenda" });
    const items = within(legend).getAllByRole("listitem");
    expect(items.map((item) => item.getAttribute("title"))).toEqual([
      "pasek: środkowa połowa graczy",
      "kreska: mediana graczy",
      "kropka: Twoja mediana",
      "prawa kreska: 90% graczy",
    ]);
    for (const item of items)
      expect(item.querySelector(".visually-hidden")).toHaveTextContent(item.getAttribute("title")!);
    const visible = (item: HTMLElement) =>
      Array.from(item.childNodes)
        .filter((node) => !(node as HTMLElement).classList?.contains("visually-hidden"))
        .map((node) => node.textContent)
        .join("");
    expect(items.map(visible)).toEqual(["", "", "Ty", ""]);
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
    // One caption (author's fixup after STOP 2), the window only (spec 33 §4, RC: "Żar").
    expect(trends.querySelector(".overview-note")).toHaveTextContent(/^ostatnie 100 bitew$/u);
    expect(trends).not.toHaveTextContent(/100 ostatnich bitew wobec/u);
    // A small line chart per metric, described in words.
    expect(within(trends).getAllByRole("img")).toHaveLength(3);
    expect(within(trends).getAllByRole("img")[0]).toHaveAccessibleName(
      /Obrażenia w 10 punktach ostatnich 100 bitew/u,
    );
    expect(trends).toHaveTextContent(/Obrażenia\s*2\s702/u);
    expect(trends).toHaveTextContent(/poprzednio 2\s600/u);
    // Wins stay context only (D12); the words saying so are in "Jak to działa" (spec 33 §4).
    expect(trends).not.toHaveTextContent(/tylko kontekst/u);
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
    // Decision 2026-11-18: the percentile of the tank's median damage in its tier × class; the
    // rule is in "Jak to działa" behind the "?" (spec 33 §4).
    expect(tanks).not.toHaveTextContent(/Najwyższy percentyl mediany/u);
    expect(
      within(tanks).getByRole("button", { name: "Jak to działa: Na tle graczy" }),
    ).toBeInTheDocument();
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
    // What a percentile is: in "Jak to działa" behind the "?" (spec 33 §4).
    expect(background).not.toHaveTextContent(/Percentyl Twojej mediany/u);
    expect(
      within(background).getByRole("button", { name: "Jak to działa: Na tle graczy" }),
    ).toBeInTheDocument();
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

/** Spec 33 ("Żar"): temperature against the player's own average, one hot spot per screen. */
describe("Żar (spec 33)", () => {
  type Handler = (event: { name: string; payload?: unknown }) => void;
  let emit: Handler = () => undefined;
  beforeEach(() => {
    mocks.onEvents.mockImplementation((handler: Handler) => {
      emit = handler;
      return () => {};
    });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });
  function reduceMotion() {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("reduce"),
      media: query,
      addEventListener() {},
      removeEventListener() {},
    }));
  }
  const hotSpots = (container: HTMLElement) => Array.from(container.querySelectorAll(".ember-hot"));
  /**
   * Author, 22.11: the hot spot glows by its main number (ember core), whose glow layer breathes
   * and carries the same text; the frame is gone. Returns the number's text.
   */
  const glowingNumber = (hot: Element, selector: string) => {
    const figures = Array.from(hot.querySelectorAll(".ember-figure"));
    expect(figures).toHaveLength(1);
    expect(figures[0].matches(selector)).toBe(true);
    const breath = figures[0].querySelector(":scope > .ember-breath");
    expect(breath).not.toBeNull();
    expect(breath).toHaveAttribute("aria-hidden", "true");
    expect(breath?.getAttribute("data-text")).toBe(figures[0].textContent);
    expect(hot.querySelectorAll(".ember-breath")).toHaveLength(1);
    return (figures[0].textContent ?? "").replace(/\s/gu, " ");
  };
  /** A battle arrives in the running session (a backend event after the battle). */
  async function newBattle(summary = fx.summary(10, 3_400)) {
    mocks.snapshot.mockResolvedValue(fx.snapshot());
    mocks.sessionSummary.mockResolvedValue(summary);
    await act(async () => emit({ name: "data-changed" }));
    await flush();
  }

  describe("Session", () => {
    it("warms the card of numbers by the session against the own average, with sparks", async () => {
      const { container } = await renderApp();
      const hero = container.querySelector(".session-flare-hero") as HTMLElement;
      // 3 184 against 2 702 on the same tanks: +18%, intensity (18 − 5) / 25.
      expect(hero).toHaveAttribute("data-temp", "warmer");
      expect(hero).toHaveClass("ember-tinted");
      expect(hero.style.getPropertyValue("--heat")).toBe("0.52");
      expect(hero.querySelectorAll(".ember-spark")).toHaveLength(11);
      expect(within(hero).getByText("cieplej niż zwykle")).toBeInTheDocument();
    });

    it("cools the card without sparks below the own average", async () => {
      // One session temperature (author's decision after E14): the Damast one.
      mocks.damastSession.mockImplementation((id: string) => Promise.resolve(fx.coolerDamast(id)));
      const { container } = await renderApp();
      const hero = container.querySelector(".session-flare-hero") as HTMLElement;
      expect(hero).toHaveAttribute("data-temp", "cooler");
      expect(hero.querySelector(".ember-spark")).toBeNull();
      expect(within(hero).getByText("chłodniej niż zwykle")).toBeInTheDocument();
    });

    it("has no temperature while the comparison is not there yet", async () => {
      // Without any own average before the session there is no temperature (spec 34 §2).
      mocks.damastSession.mockImplementation((id: string) => {
        const plate = fx.damastSession(id);
        return Promise.resolve({
          ...plate,
          reference: { ...plate.reference, wn8: null, averageDamage: null, tiers: [] },
        });
      });
      const { container } = await renderApp();
      const hero = container.querySelector(".session-flare-hero") as HTMLElement;
      expect(hero).not.toHaveAttribute("data-temp");
      expect(hero.querySelector(".ember-spark")).toBeNull();
    });

    it("makes the best battle's tile the one hot spot, breathing", async () => {
      const { container } = await renderApp();
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toHaveClass("tile-best");
      // battle-5, the highest r: its damage is the glowing number.
      expect(glowingNumber(hot[0], ".tile-damage")).toBe("3 550");
    });

    it("sweeps a new battle's tile once and heats the changed numbers after the count", async () => {
      const { container } = await renderApp();
      expect(container.querySelector(".ember-sweep")).toBeNull();
      expect(container.querySelector(".ember-heat")).toBeNull();
      await newBattle();
      const swept = Array.from(container.querySelectorAll(".battle-strip .ember-swept"));
      expect(swept).toHaveLength(1);
      expect(swept[0]).toHaveClass("tile-new");
      expect(swept[0].querySelector(".ember-sweep")).not.toBeNull();
      expect(container.querySelector(".figure-main .ember-heat")).not.toBeNull();
    });

    /** The last session has ended (its Damast plate and spark are shown). */
    function endedLastSession() {
      mocks.snapshot.mockResolvedValue(
        fx.snapshot({
          activeSession: null,
          recentSessions: [
            fx.session({ id: "session-0", status: "ended", startedAt: fx.NOW - 24 * 3_600_000 }),
          ],
        }),
      );
      const summary = fx.summary();
      mocks.sessionSummary.mockResolvedValue({
        ...summary,
        session: { ...summary.session, id: "session-0", status: "ended" },
      });
    }

    it("lets the spark of an ended session breathe like every hot spot (author, E14 polish)", async () => {
      endedLastSession();
      const { container } = await renderApp();
      await go("Sesja");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toHaveClass("damast-dot", "damast-best");
      // The breath lies in the round dot itself: no rectangular frame around the dot.
      const pip = hot[0].querySelector(".damast-pip");
      expect(pip?.querySelector(".ember-breath")).not.toBeNull();
      expect(hot[0].querySelectorAll(".ember-breath")).toHaveLength(1);
    });

    it("keeps the spark a still hot spot with reduced motion", async () => {
      reduceMotion();
      endedLastSession();
      const { container } = await renderApp();
      await go("Sesja");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toHaveClass("damast-best");
    });

    it("keeps the temperature and a still hot spot with reduced motion, without sparks or sweeps", async () => {
      reduceMotion();
      const { container } = await renderApp();
      const hero = container.querySelector(".session-flare-hero") as HTMLElement;
      expect(hero).toHaveAttribute("data-temp", "warmer");
      expect(hero.querySelector(".ember-spark")).toBeNull();
      expect(hotSpots(container)).toHaveLength(1);
      await newBattle();
      expect(container.querySelectorAll(".battle-strip .tile")).toHaveLength(10);
      expect(container.querySelector(".ember-sweep, .ember-heat")).toBeNull();
    });
  });

  describe("Overview", () => {
    const lastSession = () => screen.getByRole("region", { name: "Ostatnia sesja" });

    it("gives the last session card the session's temperature, without sparks", async () => {
      await renderApp();
      await go("Przegląd");
      const card = lastSession();
      expect(card).toHaveAttribute("data-temp", "warmer");
      expect(card).toHaveClass("ember-tinted");
      expect(card.style.getPropertyValue("--heat")).toBe("0.52");
      expect(card.querySelector(".ember-spark")).toBeNull();
      expect(card).toHaveTextContent(/cieplej niż zwykle/u);
    });

    it("ends the card with the session's best battle as the one hot spot", async () => {
      const { container } = await renderApp();
      await go("Przegląd");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(lastSession()).toContainElement(hot[0] as HTMLElement);
      // The highest r (author's decision after E14): battle-5, 3 550 damage.
      // E14 polish: a small block, the label above the number, the map below it.
      expect(hot[0]).toHaveClass("overview-best");
      const parts = Array.from(hot[0].children).map((child) => [
        child.className,
        (child.textContent ?? "").replace(/\s/gu, " "),
      ]);
      expect(parts).toEqual([
        ["overview-best-label", "najlepsza bitwa"],
        ["num overview-best-value ember-figure", "3 550"],
        ["overview-best-map", "Synthetic Hills"],
      ]);
      expect(glowingNumber(hot[0], ".overview-best-value")).toBe("3 550");
    });

    it("cools the card for a session below the own average", async () => {
      mocks.damastSession.mockImplementation((id: string) => Promise.resolve(fx.coolerDamast(id)));
      await renderApp();
      await go("Przegląd");
      expect(lastSession()).toHaveAttribute("data-temp", "cooler");
      expect(lastSession()).toHaveTextContent(/chłodniej niż zwykle/u);
    });
  });

  describe("History", () => {
    /** Alpha (101) and Bravo (102) both average 3 000 damage. */
    function tanksAveraging3000() {
      const tanks = fx.overview(2);
      for (const tank of tanks.vehicles) tank.averageDamage = 3_000;
      tanks.vehicles[0].vehicleId = 101;
      tanks.vehicles[1].vehicleId = 102;
      return tanks;
    }
    const rowButtons = (container: HTMLElement) =>
      Array.from(container.querySelectorAll<HTMLElement>(".hrow-list .hrow"));

    it("draws a thermogram of the recent sessions, oldest first, with the sessions' temperature", async () => {
      mocks.damastSession.mockImplementation((id: string) =>
        Promise.resolve(id === "session-0" ? fx.coolerDamast(id) : fx.damastSession(id)),
      );
      await renderApp();
      await go("Historia");
      const thermogram = screen.getByRole("list", { name: /Termogram/u });
      const cells = within(thermogram).getAllByRole("listitem");
      expect(cells).toHaveLength(2);
      // session-0 started a day earlier and was cooler; the running session is warmer.
      expect(cells.map((cell) => cell.getAttribute("data-temp"))).toEqual(["cooler", "warmer"]);
      expect(cells[1]).toHaveAccessibleName(/cieplej niż zwykle/u);
      // The axis counts sessions, not days (author's fixup after the E13 checkpoint).
      const ends = Array.from(document.querySelectorAll(".thermogram-ends > span"));
      expect(ends.map((end) => end.textContent)).toEqual(["2 sesje temu", "ostatnia sesja"]);
    });

    it("labels the axis of a full thermogram '30 sesji temu', in English too", async () => {
      const sessions = Array.from({ length: 30 }, (_, index) =>
        fx.session({
          id: `session-${index}`,
          status: index === 0 ? "active" : "ended",
          startedAt: fx.NOW - index * 24 * 3_600_000,
        }),
      );
      mocks.snapshot.mockResolvedValue(fx.snapshot({ recentSessions: sessions }));
      await renderApp();
      await go("Historia");
      const ends = () =>
        Array.from(document.querySelectorAll(".thermogram-ends > span")).map(
          (end) => end.textContent,
        );
      expect(
        within(screen.getByRole("list", { name: /Termogram/u })).getAllByRole("listitem"),
      ).toHaveLength(30);
      expect(ends()).toEqual(["30 sesji temu", "ostatnia sesja"]);
      cleanup();
      await renderApp("en");
      await go("History");
      expect(ends()).toEqual(["30 sessions ago", "latest session"]);
    });

    it("marks every battle row by its damage against the own average on that tank", async () => {
      mocks.vehicleHistoryOverview.mockResolvedValue(tanksAveraging3000());
      const { container } = await renderApp();
      await go("Historia");
      const temperature = (index: number) =>
        rowButtons(container)[index].querySelector(".hrow-temp")?.getAttribute("data-temp");
      // 2 000 (cooler), 2 930 (as usual), 5 410 (warmer) against 3 000.
      expect(temperature(0)).toBe("cooler");
      expect(temperature(3)).toBe("usual");
      expect(temperature(11)).toBe("warmer");
    });

    it("makes the record of the rows shown the one hot spot", async () => {
      const { container } = await renderApp();
      await go("Historia");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toHaveClass("hrow");
      expect(glowingNumber(hot[0], ".hrow-damage")).toBe("5 410");
    });

    it("sweeps a new row once", async () => {
      const { container } = await renderApp();
      await go("Historia");
      expect(container.querySelector(".ember-sweep")).toBeNull();
      const page = fx.historyPage(12);
      const newest = { ...page.items[0], battleId: "brand-new" };
      mocks.battleHistory.mockResolvedValue({ items: [newest, ...page.items], nextCursor: null });
      await newBattle();
      const swept = Array.from(container.querySelectorAll(".hrow-list .ember-swept"));
      expect(swept).toHaveLength(1);
      expect(swept[0].closest("li")).toHaveAttribute("data-flip", "brand-new");
    });

    it("puts the session's temperature by every session card", async () => {
      // Spec 34 §3 (E14): the session cards are Damast plates; their temperature is the
      // plate's (mean r − 1, spec 34 §2) with the same ±5% rule; the thermogram stays above.
      const sessions = fx.damastSessions(2);
      sessions[1] = {
        ...sessions[1],
        reference: { ...sessions[1].reference, wn8: 3_200 },
      };
      mocks.damastSessions.mockResolvedValue(fx.damastPage(sessions));
      await renderApp();
      await go("Historia");
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      const cards = Array.from(document.querySelectorAll(".damast-tile"));
      expect(cards).toHaveLength(2);
      expect(cards.map((card) => card.getAttribute("data-temp"))).toEqual(["warmer", "cooler"]);
      expect(cards[0]).toHaveTextContent(/9 bitew · cieplej/u);
      expect(cards[1]).toHaveTextContent(/9 bitew · chłodniej/u);
      expect(document.querySelectorAll(".thermogram-cell")).toHaveLength(2);
    });
  });

  describe("Tanks", () => {
    const dot = (name: string) =>
      within(screen.getByRole("navigation", { name: "Czołgi" }))
        .getByRole("button", { name: new RegExp(name, "u") })
        .querySelector(".tank-dot");

    it("puts a temperature dot by each tank from one call: the last 10 against its own average", async () => {
      // Last 10 against last 100 average damage: 2 940 / 2 702, 2 702 / 2 702, 2 000 / 2 702.
      mocks.vehicleTemperatures.mockResolvedValue([
        fx.vehicleTemperature(101, 2_940),
        fx.vehicleTemperature(102, 2_702),
        fx.vehicleTemperature(103, 2_000),
      ]);
      await renderApp();
      await go("Czołgi");
      await flush();
      expect(dot("Alpha")).toHaveAttribute("data-temp", "warmer");
      expect(dot("Bravo")).toHaveAttribute("data-temp", "usual");
      expect(dot("Charlie")).toHaveAttribute("data-temp", "cooler");
      // One request for the whole list (author's fixup after E13); the baseline only for the
      // header of the chosen tank.
      expect(mocks.vehicleTemperatures).toHaveBeenCalledTimes(1);
      expect(new Set(mocks.vehicleBaseline.mock.calls.map(([id]) => id))).toEqual(new Set([101]));
    });

    it("leaves the dot out for a tank the backend leaves out (under 10 battles)", async () => {
      mocks.vehicleTemperatures.mockResolvedValue([fx.vehicleTemperature(101, 2_940)]);
      await renderApp();
      await go("Czołgi");
      await flush();
      expect(dot("Alpha")).not.toBeNull();
      expect(dot("Bravo")).toBeNull();
      expect(dot("Charlie")).toBeNull();
    });

    it("leaves the dot out while the last 100 hold fewer than 10 counted battles", async () => {
      mocks.vehicleTemperatures.mockResolvedValue([fx.vehicleTemperature(101, 2_940, 9)]);
      await renderApp();
      await go("Czołgi");
      await flush();
      expect(dot("Alpha")).toBeNull();
    });

    it("heats the chart line above the own average and keeps it steel below", async () => {
      const { container } = await renderApp();
      await go("Czołgi");
      const chart = container.querySelector("svg.chart") as SVGElement;
      const gradient = chart.querySelector("linearGradient.chart-heat") as SVGElement;
      expect(gradient).not.toBeNull();
      const stops = Array.from(gradient.querySelectorAll("stop"));
      expect(stops.map((stop) => stop.getAttribute("class"))).toEqual([
        "chart-heat-warm",
        "chart-heat-warm",
        "chart-heat-steel",
        "chart-heat-steel",
      ]);
      // The switch from ember to steel is at the height of the average line.
      expect(stops[1].getAttribute("offset")).toBe(stops[2].getAttribute("offset"));
      const line = chart.querySelector(".chart-line") as SVGPathElement;
      expect(line.style.stroke.replace(/"/g, "")).toBe(`url(#${gradient.id})`);
    });

    it("makes the tank's best map the one hot spot; the second best keeps only its tag", async () => {
      const report = fx.vehicleReport();
      report.maps.push({ ...report.maps[1], mapKey: "92_grey_harbor", mapName: "Grey Harbor" });
      mocks.vehicleHistoryReport.mockResolvedValue(report);
      const { container } = await renderApp();
      await go("Czołgi");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toHaveTextContent("Synthetic Hills");
      expect(glowingNumber(hot[0], ".map-tile-damage .num")).toMatch(/^\d[\d ]*$/u);
      expect(container.querySelectorAll(".map-tile.tile-best")).toHaveLength(1);
      expect(container.querySelectorAll(".map-tag-best")).toHaveLength(2);
    });
  });

  describe("Battle", () => {
    /** The battle of `battleDetail()` was played on Bravo (102), 5 210 damage. */
    function bravoAveraging(average: number) {
      const tanks = fx.overview(2);
      tanks.vehicles[1].vehicleId = 102;
      tanks.vehicles[1].averageDamage = average;
      return tanks;
    }
    async function openBattle() {
      const view = await renderApp();
      fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
      await flush();
      return view;
    }
    const hero = (container: HTMLElement) =>
      container.querySelector(".battle-flare-hero") as HTMLElement;

    it("heats the result once against the own average on that tank and says how it usually is", async () => {
      mocks.vehicleHistoryOverview.mockResolvedValue(bravoAveraging(3_000));
      const { container } = await openBattle();
      expect(hero(container)).toHaveAttribute("data-temp", "warmer");
      expect(hero(container)).toHaveClass("ember-tinted");
      expect(hero(container)).toHaveTextContent(/cieplej niż zwykle/u);
      const damage = container.querySelector(".battle-hero-damage") as HTMLElement;
      expect(damage).toHaveTextContent(/zwykle 3\s000/u);
      expect(damage.querySelectorAll(".ember-heat")).toHaveLength(1);
    });

    it("cools it below the own average", async () => {
      mocks.vehicleHistoryOverview.mockResolvedValue(bravoAveraging(6_000));
      const { container } = await openBattle();
      expect(hero(container)).toHaveAttribute("data-temp", "cooler");
      expect(hero(container)).toHaveTextContent(/chłodniej niż zwykle/u);
    });

    it("makes the result the hot spot only when it is the best battle of its session", async () => {
      const { container } = await openBattle();
      expect(hotSpots(container)).toHaveLength(0);
      cleanup();
      // The opened battle (battle-4) has the session's highest r (author's decision after E14).
      mocks.damastSession.mockImplementation((id: string) =>
        Promise.resolve(fx.damastWithBest(4, id)),
      );
      const second = await openBattle();
      const hot = hotSpots(second.container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toHaveClass("battle-hero-damage");
      expect(glowingNumber(hot[0], ".battle-damage-value")).toMatch(/^\d[\d ]*$/u);
    });

    it("keeps the temperature without the heat motion with reduced motion", async () => {
      reduceMotion();
      mocks.vehicleHistoryOverview.mockResolvedValue(bravoAveraging(3_000));
      const { container } = await openBattle();
      expect(hero(container)).toHaveAttribute("data-temp", "warmer");
      expect(container.querySelector(".ember-heat")).toBeNull();
    });
  });

  describe("Goals", () => {
    const card = (kind: string) => document.getElementById(`recap-target-goal-${kind}`)!;
    const bar = (kind: string) => card(kind).querySelector(".track") as HTMLElement;

    it("draws the goal bars from steel to ember by their progress", async () => {
      await renderApp();
      await go("Cele");
      // Survival 30.8 of 35, XP 1 390 of 1 450, the tank goal achieved.
      expect(bar("survival")).toHaveClass("ember-track");
      expect(bar("survival").style.getPropertyValue("--progress")).toBe("0.88");
      expect(bar("xp").style.getPropertyValue("--progress")).toBe("0.959");
      expect(bar("tank_damage_assist").style.getPropertyValue("--progress")).toBe("1");
    });

    it("makes the goal closest to completion the one hot spot", async () => {
      const { container } = await renderApp();
      await go("Cele");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(hot[0]).toBe(card("xp"));
      expect(glowingNumber(hot[0], ".weekly-goal-value .num")).toMatch(/1\s390/u);
    });

    it("lets the 5-battle challenge be the hot spot when it is the closest", async () => {
      const report = fx.weeklyReport();
      for (const goal of report.goals) goal.state = "achieved";
      mocks.weeklyReport.mockResolvedValue(report);
      mocks.snapshot.mockResolvedValue(fx.snapshot({ activeChallenge: fx.activeChallenge() }));
      const { container } = await renderApp();
      await go("Cele");
      const hot = hotSpots(container);
      expect(hot).toHaveLength(1);
      expect(within(hot[0] as HTMLElement).getByText("Aktywny cel")).toBeInTheDocument();
      expect(glowingNumber(hot[0], ".stat-value")).toMatch(/^\d+$/u);
    });

    it("flashes and sweeps a goal completed while the screen is open, once", async () => {
      const { container } = await renderApp();
      await go("Cele");
      expect(container.querySelector(".ember-flash")).toBeNull();
      const report = fx.weeklyReport();
      report.goals[2] = { ...report.goals[2], current: 1_460, state: "achieved" };
      mocks.weeklyReport.mockResolvedValue(report);
      await newBattle();
      expect(container.querySelectorAll(".ember-flash")).toHaveLength(1);
      expect(card("xp").querySelector(".ember-flash")).not.toBeNull();
      expect(card("xp")).toHaveClass("ember-swept");
      // The hot spot moves on to the goal that is now the closest.
      expect(hotSpots(container)).toEqual([card("survival")]);
    });

    it("does not flash a goal that was already achieved when the screen opened", async () => {
      const { container } = await renderApp();
      await go("Cele");
      expect(card("tank_damage_assist")).not.toHaveClass("ember-swept");
      expect(container.querySelector(".ember-flash")).toBeNull();
    });
  });

  describe("Navigation", () => {
    const sessionLink = () =>
      within(screen.getByRole("navigation", { name: "Główna nawigacja" })).getByRole("button", {
        name: /Sesja/u,
      });

    it("puts a still ember dot by Session while a session runs, not a hot spot", async () => {
      await renderApp();
      const dot = sessionLink().querySelector(".rail-ember");
      expect(dot).not.toBeNull();
      expect(dot).toHaveAttribute("aria-hidden", "true");
      expect(dot).not.toHaveClass("ember-hot");
      expect(dot?.querySelector(".ember-breath")).toBeNull();
      // The running session is said in words as well.
      expect(sessionLink()).toHaveAccessibleName("Sesja");
      expect(sessionLink()).toHaveAccessibleDescription("sesja trwa");
    });

    it("has no dot without a running session", async () => {
      mocks.snapshot.mockResolvedValue(fx.snapshot({ activeSession: null }));
      await renderApp();
      expect(sessionLink().querySelector(".rail-ember")).toBeNull();
      expect(sessionLink()).not.toHaveAccessibleDescription();
    });
  });

  describe("Settings", () => {
    it("stays cool: no temperature and no hot spot", async () => {
      const { container } = await renderApp();
      await go("Ustawienia");
      expect(container.querySelector(".page [data-temp]")).toBeNull();
      expect(hotSpots(container)).toHaveLength(0);
    });

    it("flashes once with 'Zapisano.' after every save", async () => {
      await renderApp();
      await go("Ustawienia");
      fireEvent.click(screen.getByRole("radio", { name: "On" }));
      await flush();
      const saved = within(screen.getByRole("region", { name: "Ty" })).getByRole("status");
      const first = saved.querySelector(".ember-flash");
      expect(first).not.toBeNull();
      fireEvent.click(screen.getByRole("radio", { name: "Ona" }));
      await flush();
      // A new save starts a new flash (a new layer), not the old one again.
      expect(saved.querySelectorAll(".ember-flash")).toHaveLength(1);
      expect(saved.querySelector(".ember-flash")).not.toBe(first);
    });

    it("saves without a flash with reduced motion", async () => {
      reduceMotion();
      await renderApp();
      await go("Ustawienia");
      fireEvent.click(screen.getByRole("radio", { name: "On" }));
      await flush();
      const saved = within(screen.getByRole("region", { name: "Ty" })).getByRole("status");
      expect(saved).toHaveTextContent("Zapisano.");
      expect(saved.querySelector(".ember-flash")).toBeNull();
    });
  });

  describe("'Jak to działa' (spec 33 §4)", () => {
    const TITLES = [
      "Które bitwy się liczą",
      "Twoja średnia",
      "WN8 i XVM",
      "Na tle graczy",
      "Podsumowania",
      "Temperatura (żar i chłód)",
      "Damast",
      "Prywatność i aktualizacje",
    ];
    /** A constant of the Rust backend, read from its source. */
    const rust = (file: string, name: string) =>
      Number(new RegExp(`const ${name}: \\w+ = (\\d+)`).exec(readFileSync(file, "utf8"))?.[1]);
    async function openEntry(title: string): Promise<HTMLElement> {
      await renderApp();
      await go("Ustawienia");
      const button = screen.getByRole("button", { name: title });
      fireEvent.click(button);
      await flush();
      expect(button).toHaveAttribute("aria-expanded", "true");
      return document.getElementById(button.getAttribute("aria-controls")!)!;
    }

    it("lists eight short entries under 'O aplikacji', closed until clicked", async () => {
      await renderApp();
      await go("Ustawienia");
      const about = screen.getByRole("region", { name: "O aplikacji" });
      const how = within(about).getByRole("heading", { name: "Jak to działa" });
      expect(how).toBeInTheDocument();
      const buttons = TITLES.map((title) => within(about).getByRole("button", { name: title }));
      for (const button of buttons) expect(button).toHaveAttribute("aria-expanded", "false");
      fireEvent.click(buttons[0]);
      expect(buttons[0]).toHaveAttribute("aria-expanded", "true");
      fireEvent.click(buttons[0]);
      expect(buttons[0]).toHaveAttribute("aria-expanded", "false");
    });

    it("which battles count, and when a session ends (the setting)", async () => {
      const entry = await openEntry("Które bitwy się liczą");
      expect(entry).toHaveTextContent(/Bitwy losowe z pełnymi wynikami/u);
      expect(entry).toHaveTextContent(/Pluton się liczy/u);
      expect(entry).toHaveTextContent(/po 60 min bez bitwy/u);
    });

    it("the own average with the thresholds of the code", async () => {
      const entry = await openEntry("Twoja średnia");
      expect(MIN_BASELINE_BATTLES).toBe(
        rust("src-tauri/src/session_insights.rs", "MIN_BASELINE_BATTLES"),
      );
      expect(MIN_SESSION_BATTLES).toBe(
        rust("src-tauri/src/session_insights.rs", "MIN_SESSION_BATTLES"),
      );
      expect(entry).toHaveTextContent(new RegExp(`od ${MIN_SESSION_BATTLES} bitew w sesji`, "u"));
      expect(entry).toHaveTextContent(
        new RegExp(`co najmniej ${MIN_BASELINE_BATTLES} wcześniejszymi bitwami`, "u"),
      );
      expect(entry).toHaveTextContent(new RegExp(`od ${DAMAGE_THRESHOLD_PERCENT}%`, "u"));
      expect(entry).toHaveTextContent(
        /ostatnie 10 bitew wobec średniej ze 100 ostatnich, od 10 bitew/u,
      );
      expect(entry).toHaveTextContent(/ostatnie 100 bitew wobec 100 wcześniejszych/u);
    });

    it("WN8 and XVM", async () => {
      const entry = await openEntry("WN8 i XVM");
      expect(entry).toHaveTextContent(/wartości oczekiwanych XVM/u);
      expect(entry).toHaveTextContent(/raz na dobę/u);
    });

    it("the players of your battles with the A-1 limits of the backend", async () => {
      const entry = await openEntry("Na tle graczy");
      const rows = rust("src-tauri/src/reference.rs", "MIN_ROWS");
      const approximate = rust("src-tauri/src/reference.rs", "MIN_ROWS_APPROXIMATE");
      const own = rust("src-tauri/src/reference.rs", "MIN_OWN_BATTLES");
      expect([rows, approximate, own]).toEqual([200, 50, 10]);
      expect(entry).toHaveTextContent(new RegExp(`od ${rows} wyników`, "u"));
      expect(entry).toHaveTextContent(new RegExp(`od ${approximate} orientacyjnie`, "u"));
      expect(entry).toHaveTextContent(new RegExp(`mediana od ${own} Twoich bitew`, "u"));
      expect(entry).toHaveTextContent(/od VI poziomu/u);
      expect(entry).toHaveTextContent(/50 = środek graczy/u);
      // The A-1 legend as a sentence lives here; on the screen only its signs stay.
      expect(entry).toHaveTextContent(/Pasek: środkowa połowa graczy/u);
    });

    it("summaries: templates and references", async () => {
      const entry = await openEntry("Podsumowania");
      expect(entry).toHaveTextContent(/według stałych reguł/u);
      expect(entry).toHaveTextContent(/odnośnik/u);
    });

    it("the temperature with the thresholds of the code", async () => {
      const entry = await openEntry("Temperatura (żar i chłód)");
      expect(entry).toHaveTextContent(new RegExp(`mniej niż ±${USUAL_PERCENT}%`, "u"));
      expect(entry).toHaveTextContent(new RegExp(`pełny przy ±${FULL_HEAT_PERCENT}%`, "u"));
      expect(entry).toHaveTextContent(
        new RegExp(
          `od ${SPARKS_MIN} przy \\+${USUAL_PERCENT}% do ${SPARKS_MAX} przy \\+${FULL_HEAT_PERCENT}%`,
          "u",
        ),
      );
      expect(entry).toHaveTextContent(/nigdy do innych graczy/u);
    });

    it("Damast (spec 34) with the numbers of the code", async () => {
      const entry = await openEntry("Damast");
      expect(DAMAST_REFERENCE_BATTLES).toBe(rust("src-tauri/src/baseline.rs", "MAX_WINDOW"));
      expect([WN8_WEIGHT, DAMAGE_WEIGHT, MIN_TIER_BATTLES]).toEqual([0.6, 0.4, 10]);
      expect(entry).toHaveTextContent(
        new RegExp(
          `${WN8_WEIGHT * 100}% WN8 wobec Twojej średniej WN8 i ${DAMAGE_WEIGHT * 100}% obrażeń`,
          "u",
        ),
      );
      expect(entry).toHaveTextContent(
        new RegExp(`z ostatnich ${DAMAST_REFERENCE_BATTLES} bitew przed sesją`, "u"),
      );
      expect(entry).toHaveTextContent(new RegExp(`mniej niż ${MIN_TIER_BATTLES} bitwach`, "u"));
      expect(entry).toHaveTextContent(new RegExp(`granicą ±${USUAL_PERCENT}%`, "u"));
      expect(entry).toHaveTextContent(/Sam tier nie podnosi wyniku/u);
      expect(entry).toHaveTextContent(
        /Najlepsza bitwa sesji — wszędzie w aplikacji — to ta z najwyższym wynikiem/u,
      );
      expect(entry).toHaveTextContent(
        /Temperatura sesji — na karcie Sesji, w Przeglądzie, w termogramie i na płytce/u,
      );
      expect(entry).toHaveTextContent(/oczko we wzorze to wygrana/u);
      expect(entry).toHaveTextContent(/raz, po zamknięciu sesji, nigdy w jej trakcie/u);
      expect(entry).toHaveTextContent(
        new RegExp(`Karta ${CARD_WIDTH}×${CARD_HEIGHT}.*bez nicku i ID konta`, "u"),
      );
      expect(entry).toHaveTextContent(/nic nie jest wysyłane/u);
    });

    it("privacy and updates", async () => {
      const entry = await openEntry("Prywatność i aktualizacje");
      expect(entry).toHaveTextContent(/zostają na tym komputerze, bez telemetrii/u);
      expect(entry).toHaveTextContent(/dopiero po kliknięciu/u);
    });

    it("has every entry in English as well", async () => {
      await renderApp("en");
      await go("Settings");
      const about = screen.getByRole("region", { name: "About" });
      expect(within(about).getByRole("heading", { name: "How it works" })).toBeInTheDocument();
      expect(
        within(about)
          .getAllByRole("button")
          .filter((button) => button.hasAttribute("aria-expanded")),
      ).toHaveLength(8);
      expect(within(about).getByRole("button", { name: "Damast" })).toBeInTheDocument();
    });
  });

  describe("one hot spot per screen (spec 33 §1)", () => {
    it("lets at most one place glow on every screen, Settings none", async () => {
      mocks.snapshot.mockResolvedValue(fx.snapshot({ activeChallenge: fx.activeChallenge() }));
      // battle-4, opened below, is the session's best battle (its highest r).
      mocks.damastSession.mockImplementation((id: string) =>
        Promise.resolve(fx.damastWithBest(4, id)),
      );
      const { container } = await renderApp();
      const count: Record<string, number> = { Sesja: hotSpots(container).length };
      fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
      await flush();
      count.Bitwa = hotSpots(container).length;
      for (const page of ["Przegląd", "Historia", "Czołgi", "Cele", "Ustawienia"]) {
        await go(page);
        count[page] = hotSpots(container).length;
      }
      await go("Historia");
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      count["Historia, sesje"] = hotSpots(container).length;
      expect(count).toEqual({
        Sesja: 1,
        Bitwa: 1,
        Przegląd: 1,
        Historia: 1,
        Czołgi: 1,
        Cele: 1,
        Ustawienia: 0,
        // Spec 34 §3: only the record session's plate glows.
        "Historia, sesje": 1,
      });
    });
  });

  describe("nothing during a battle (spec 33 §1, Fair Play)", () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    it("moves and counts nothing while a session runs without a new battle, the game in front", async () => {
      // The app never knows a battle is on (Fair Play); while one is played, the session runs,
      // no replay arrives and the game window has the focus.
      const { container } = await renderApp();
      expect(screen.getByRole("button", { name: "Sesja" })).toHaveAccessibleDescription(
        "sesja trwa",
      );
      const calls = () =>
        Object.fromEntries(
          Object.entries(mocks).map(([name, mock]) => [name, mock.mock.calls.length]),
        );
      const before = calls();
      const html = container.innerHTML;
      vi.useFakeTimers();
      act(() => {
        window.dispatchEvent(new Event("blur"));
      });
      expect(document.documentElement.dataset.window).toBe("background");
      act(() => {
        vi.advanceTimersByTime(15 * 60_000);
      });
      vi.useRealTimers();
      await flush();
      // Nothing asked the backend again, nothing was counted again, nothing new moves.
      expect(calls()).toEqual(before);
      expect(container.innerHTML).toBe(html);
      expect(
        container.querySelector(".ember-sweep, .ember-heat, .ember-flash, .tile-new"),
      ).toBeNull();
      act(() => {
        window.dispatchEvent(new Event("focus"));
      });
      expect(document.documentElement.dataset.window).toBeUndefined();
    });
  });

  describe("less text on the screens (spec 33 §4)", () => {
    /**
     * Explaining sentences that may stay on the screens: empty states and errors only (one
     * sentence of what to do). Empty: with full data no screen explains how it counts.
     */
    const ALLOWED_EXPLANATIONS: RegExp[] = [
      // Last week without goals: a state (too few battles back then), not an explanation.
      /^Bez celów: za mało wcześniejszych bitew \(\d+ bitew w tamtym tygodniu\)\.$/u,
      // WN8 off: a state with where to turn it on.
      /^WN8: wyłączone \(Ustawienia, grupa Dane\)\.$/u,
      // No finished goals yet: an empty state.
      /^Nie masz jeszcze zakończonych celów\.$/u,
    ];
    /**
     * Not explanations, so not on the list above: lines of labels and numbers, and content the
     * player reads for itself (insights and the first summary sentence).
     */
    const DATA_AND_CONTENT: RegExp[] = [
      /^(Dzisiejsza|Ostatnia) sesja: Od \d+:\d+, \d+ bitew losowych, (aktywna|zakończona)$/u,
      /^\d+ listopada 2026,? \d+:\d+.*(bitwa trwała|bitew)/u,
      /^Poziomy \d+ i \d+, plutony: \d+ i \d+$/u,
      /^· \p{L}+ [IVX]+ · \d+ bit(wy|ew)$/u,
      /^Najczęściej grane: /u,
      /^Czołg \p{L}+, [IVX]+ poziom, \d+ bitew, ostatnia /u,
      /^Czołgi \p{L}+ [IVX]+ poziomu: [\d\s]+ wyników innych graczy\.$/u,
      /^\d+–\d+ listopada, \d+ bitew do tej pory$/u,
      /^Wyzwania na 5 bitew w tym tygodniu: ukończone \d+, nieudane \d+, porzucone \d+\.$/u,
      /^\d+ \d+ z \d+ bitew, \d+ udanych$/u,
      /^Potrzeba \d+ udanych bitew z \d+\. Twoja średnia [\d\s]+$/u,
      /^\d+ z \d+ bitew miało jednostronny przebieg$/u,
      /^o \d+% (więcej|mniej) niż zwykle \([\d\s]+\)\?porównano \d+ z \d+ bitew$/u,
      // Insights of the session and the first summary sentence: findings, not how they are found.
      /^W tej sesji zadajesz o \d+% (więcej|mniej) obrażeń niż zwykle/u,
      /^Na \p{L}+ (zadajesz|przeżywasz|masz) /u,
      /^Wygrane: \d+ z \d+ bitew, czyli/u,
    ];
    /** A sentence or a note of five words or more, outside summaries and "Jak to działa". */
    function explanations(root: HTMLElement): string[] {
      const texts = Array.from(root.querySelectorAll<HTMLElement>("p, .muted, [class*='note']"))
        .filter((element) => !element.closest(".recap, .how, [role='dialog'], nav, .app-footer"))
        .map((element) => (element.textContent ?? "").replace(/\s+/g, " ").trim())
        .filter((text) => (text.match(/\p{L}+/gu) ?? []).length >= 5)
        .filter((text) => !DATA_AND_CONTENT.some((pattern) => pattern.test(text)));
      return [...new Set(texts)];
    }
    async function everyScreen(): Promise<string[]> {
      const { container } = await renderApp();
      const found = explanations(container);
      fireEvent.click(screen.getAllByRole("button", { name: BATTLE_BUTTON })[0]);
      await flush();
      found.push(...explanations(container));
      for (const page of ["Przegląd", "Historia", "Czołgi", "Cele"]) {
        await go(page);
        found.push(...explanations(container));
      }
      await go("Historia");
      fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
      await flush();
      found.push(...explanations(container));
      return [...new Set(found)];
    }

    it("shows labels, numbers, units and short captions, no explaining sentences", async () => {
      mocks.recap.mockImplementation((kind: string) =>
        Promise.resolve(kind === "battle" ? fx.battleRecap() : fx.sessionRecap()),
      );
      mocks.snapshot.mockResolvedValue(fx.snapshot({ activeChallenge: fx.activeChallenge() }));
      expect(
        (await everyScreen()).filter(
          (text) => !ALLOWED_EXPLANATIONS.some((pattern) => pattern.test(text)),
        ),
      ).toEqual([]);
    });

    it("leads from a '?' straight to its entry in 'Jak to działa'", async () => {
      await renderApp();
      await go("Przegląd");
      const tanks = screen.getByRole("region", { name: "Czołgi" });
      fireEvent.click(within(tanks).getByRole("button", { name: "Jak to działa: Na tle graczy" }));
      await flush();
      expect(screen.getByRole("heading", { name: "Ustawienia", level: 1 })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Na tle graczy" })).toHaveAttribute(
        "aria-expanded",
        "true",
      );
      expect(screen.getByRole("button", { name: "Twoja średnia" })).toHaveAttribute(
        "aria-expanded",
        "false",
      );
    });

    it("puts a '?' by the numbers that may raise a question, on every screen that has them", async () => {
      const help = (name: string) =>
        screen.getAllByRole("button", { name: `Jak to działa: ${name}` }).length;
      await renderApp();
      expect(help("Twoja średnia")).toBe(1);
      await go("Przegląd");
      expect(help("Na tle graczy")).toBe(2);
      await go("Czołgi");
      expect(help("Na tle graczy")).toBe(1);
      await go("Cele");
      expect(help("Twoja średnia")).toBe(1);
    });

    it("keeps in 'Jak to działa' what left the screens", async () => {
      await renderApp();
      await go("Ustawienia");
      const open = (title: string) => {
        const button = screen.getByRole("button", { name: title });
        fireEvent.click(button);
        return document.getElementById(button.getAttribute("aria-controls")!)!;
      };
      const battles = open("Które bitwy się liczą");
      expect(battles).toHaveTextContent(/Jednostronny przebieg to tylko kontekst/u);
      const average = open("Twoja średnia");
      expect(average).toHaveTextContent(/średnia z 10 kolejnych bitew/u);
      expect(average).toHaveTextContent(/Twoja mediana z 4 tygodni plus 5 punktów/u);
      expect(average).toHaveTextContent(/Twoja średnia z 4 tygodni plus 10%/u);
      expect(average).toHaveTextContent(/Wygrane: tylko kontekst, nigdy cel/u);
      expect(average).toHaveTextContent(/bezpośrednim trafieniem i penetracją/u);
    });
  });
});

/** Spec 34 ("Damast"): the plate of an ended session, forged once, never during a session. */
describe("Damast (spec 34)", () => {
  beforeEach(() => {
    // jsdom has no canvas: a still plate draws into this stand-in (no WebGL2 here).
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(((kind: string) =>
      kind === "2d"
        ? {
            createImageData: (width: number, height: number) => ({
              data: new Uint8ClampedArray(width * height * 4),
            }),
            putImageData: () => {},
          }
        : null) as never);
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });
  const HOUR = 3_600_000;
  /** The last session has ended; no session runs. */
  function endedSession() {
    mocks.snapshot.mockResolvedValue(
      fx.snapshot({
        activeSession: null,
        recentSessions: [
          fx.session({ id: "session-0", status: "ended", startedAt: fx.NOW - 24 * HOUR }),
        ],
      }),
    );
    const summary = fx.summary();
    mocks.sessionSummary.mockResolvedValue({
      ...summary,
      session: { ...summary.session, id: "session-0", status: "ended" },
    });
  }
  async function openEnded() {
    endedSession();
    const rendered = await renderApp();
    await go("Sesja");
    return rendered;
  }
  const hotSpots = (container: HTMLElement) => Array.from(container.querySelectorAll(".ember-hot"));

  it("shows no plate while the session runs", async () => {
    const { container } = await renderApp();
    // The running session's data is read for its temperature only; no plate, no tile.
    expect(container.querySelector(".damast, .damast-plate")).toBeNull();
    expect(mocks.damastTile).not.toHaveBeenCalled();
    expect(damastEngine.renderStill).not.toHaveBeenCalled();
  });

  it("gives a session one temperature: on Session, Overview, the thermogram and its plate", async () => {
    // Author's decision after E14: the Damast temperature everywhere. The damage comparison says
    // +18% warmer on purpose; the plate's mean r says cooler, and so must every place.
    const cooler = fx.coolerDamast("session-0");
    const expected = damastStats(cooler).heat!;
    expect(expected.temperature).toBe("cooler");
    expect(fx.comparison().vehicles.length).toBeGreaterThan(0);
    mocks.damastSession.mockImplementation((id: string) =>
      Promise.resolve({ ...cooler, session: { ...cooler.session, id } }),
    );
    mocks.damastSessions.mockResolvedValue(fx.damastPage([cooler]));
    endedSession();
    const { container } = await renderApp();
    const seen: Record<string, [string | null, string]> = {};
    const read = (name: string, element: Element | null) => {
      const html = element as HTMLElement | null;
      seen[name] = [
        html?.getAttribute("data-temp") ?? null,
        html?.style.getPropertyValue("--heat") ?? "",
      ];
    };
    await go("Sesja");
    read("Sesja", container.querySelector(".session-flare-hero"));
    await go("Przegląd");
    read("Przegląd", screen.getByRole("region", { name: "Ostatnia sesja" }));
    await go("Historia");
    read("termogram", container.querySelector(".thermogram-cell"));
    fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
    await flush();
    read("płytka", container.querySelector(".damast-tile"));
    const heat = String(Math.round(expected.intensity * 1000) / 1000);
    expect(seen).toEqual({
      Sesja: ["cooler", heat],
      Przegląd: ["cooler", heat],
      termogram: ["cooler", heat],
      płytka: ["cooler", heat],
    });
    // "zwykle N" stays on Session as the damage reference (author's decision).
    await go("Sesja");
    expect(container.querySelector(".session-flare-hero")).toHaveTextContent(/zwykle/u);
  });

  it("puts the plate, its battle bar and the numbers under an ended session", async () => {
    await openEnded();
    expect(mocks.damastSession).toHaveBeenCalledWith("session-0");
    const damast = screen.getByRole("region", { name: "Damast" });
    expect(within(damast).getByRole("img", { name: "Płytka damastu sesji" })).toBeInTheDocument();
    const strip = within(damast).getByRole("list", { name: "Bitwy na płytce" });
    const dots = within(strip).getAllByRole("listitem");
    expect(dots).toHaveLength(9);
    expect(dots.map((dot) => dot.classList.contains("damast-win"))).toEqual(
      fx.damastSession().battles.map((battle) => battle.win),
    );
    expect(dots[0]).toHaveAccessibleName("wygrana, tier VIII");
    expect(dots[1]).toHaveAccessibleName("bez wygranej, tier IX");
    expect(dots[5]).toHaveAccessibleName("wygrana, tier X, najlepsza");
    expect(damast).toHaveTextContent(/2\s150\s*WN8 sesji/u);
    expect(damast).toHaveTextContent(/3\s184\s*obrażenia/u);
    expect(damast).toHaveTextContent(/6 z 9\s*wygranych/u);
    expect(damast).toHaveTextContent(/9,0\s*średni tier/u);
    expect(within(damast).getByRole("button", { name: "Zapisz kartę" })).toBeEnabled();
    expect(within(damast).getByRole("button", { name: "Do historii" })).toBeEnabled();
  });

  it("puts the plate on the full width, 360 px tall, the bar under it, then one row", async () => {
    // Author, 22.11: no side column. Plate, battle bar, and one row: the title over WN8 and
    // three figures on the left, the actions with their message on the right.
    await openEnded();
    const damast = screen.getByRole("region", { name: "Damast" });
    const card = damast.querySelector(".damast-card")!;
    expect(Array.from(card.children).map((child) => child.className)).toEqual([
      "damast-plate",
      "damast-strip",
      "damast-row",
    ]);
    expect((card.children[0] as HTMLElement).style.height).toBe("360px");
    expect(damast.querySelector(".damast-side")).toBeNull();
    const row = card.children[2];
    expect(Array.from(row.children).map((child) => child.className)).toEqual([
      "damast-summary",
      "damast-actions",
    ]);
    const summary = row.children[0];
    expect(Array.from(summary.children).map((child) => child.className)).toEqual([
      "damast-title",
      "damast-numbers",
    ]);
    const spaced = (text: string | null) => (text ?? "").replace(/\s/gu, " ");
    expect(spaced(summary.children[0].textContent)).toMatch(/^Sesja \d{2}\.\d{2} · 9 bitew$/u);
    const numbers = summary.children[1];
    expect(Array.from(numbers.children).map((child) => child.className)).toEqual([
      "damast-headline",
      "damast-figures",
    ]);
    expect(spaced(numbers.children[0].textContent)).toBe("2 150WN8 sesji");
    expect(
      Array.from(numbers.querySelectorAll(".damast-figure")).map((figure) =>
        spaced(figure.textContent),
      ),
    ).toEqual(["3 184obrażenia", "6 z 9wygranych", "9,0średni tier"]);
    const actions = row.children[1];
    expect(
      within(actions as HTMLElement)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Zapisz kartę", "Do historii"]);
    const live = actions.querySelector("[aria-live]");
    expect(live).toHaveAttribute("aria-live", "polite");
    expect(live).toHaveTextContent("");
  });

  it("has a closed ⓘ by the Damast heading; open, it reads this plate and keeps one hot spot", async () => {
    // Author, 22.11: "Jak czytać tę płytkę", next to the heading, not modal.
    const { container } = await openEnded();
    const damast = screen.getByRole("region", { name: "Damast" });
    const info = within(damast).getByRole("button", { name: "Jak czytać tę płytkę" });
    expect(info.closest(".damast-head")?.querySelector("h2")).toHaveTextContent("Damast");
    expect(info).toHaveAttribute("aria-expanded", "false");
    expect(within(damast).queryByRole("dialog")).toBeNull();
    fireEvent.click(info);
    const dialog = within(damast).getByRole("dialog", { name: "Jak czytać tę płytkę" });
    // The spark's row names the plate's best battle (highest r: battle-5, the sixth) and its map.
    const stats = damastStats(fx.damastSession());
    const map = fx
      .summary()
      .battles.find(({ battle }) => battle.arenaId === stats.plate[stats.best].battleId)
      ?.battle.mapName;
    expect(map).toBeTruthy();
    expect(dialog).toHaveTextContent(`Iskra pod płytką: najlepsza bitwa (nr 6, ${map}).`);
    expect(dialog).toHaveTextContent(`Oczka: ${stats.wins}, jedno za każdą wygraną.`);
    expect(hotSpots(container)).toHaveLength(1);
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(within(damast).queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(info);
  });

  it("opens the Damast entry of “Jak to działa” from the ⓘ", async () => {
    await openEnded();
    const damast = screen.getByRole("region", { name: "Damast" });
    fireEvent.click(within(damast).getByRole("button", { name: "Jak czytać tę płytkę" }));
    fireEvent.click(screen.getByRole("button", { name: "Więcej w „Jak to działa”" }));
    await flush();
    expect(screen.getByRole("button", { name: "Damast", expanded: true })).toBeInTheDocument();
  });

  it("makes the best battle's spark the one hot spot of the screen", async () => {
    const { container } = await openEnded();
    const hot = hotSpots(container);
    expect(hot).toHaveLength(1);
    expect(hot[0]).toHaveClass("damast-dot", "damast-best");
    // Author's decision after E14 polish: it breathes like every hot spot, in the round dot.
    expect(hot[0].querySelector(".damast-pip .ember-breath")).not.toBeNull();
    // The tile of the session's best battle keeps its mark, without the glow.
    const best = container.querySelector(".battle-strip .tile-best");
    expect(best).not.toBeNull();
    expect(best).not.toHaveClass("ember-hot");
  });

  it("marks the highest r as the best battle of an ended session, for the spark and the tile", async () => {
    // The story's best battle (most damage) is battle-8; the plate's highest r is battle-5.
    expect(fx.sessionStory().bestBattle?.battleId).toBe("battle-8");
    const stats = damastStats(fx.damastSession());
    expect(stats.plate[stats.best].battleId).toBe("battle-5");
    const { container } = await openEnded();
    const spark = container.querySelector(".damast-strip .damast-best");
    expect(spark).toHaveAccessibleName(/najlepsza$/u);
    expect(
      Array.from(container.querySelectorAll(".damast-strip .damast-dot")).indexOf(spark!),
    ).toBe(5);
    const marked = Array.from(container.querySelectorAll(".battle-strip .tile-best"));
    expect(marked).toHaveLength(1);
    expect(marked[0].closest("li")).toHaveAttribute("data-flip", "battle-5");
    expect(marked[0].getAttribute("aria-label")).toMatch(/^Najlepsza/u);
    // The summary's "najlepsza bitwa" keeps pointing at the battle its sentence names (spec 30).
    expect(container.querySelector("#recap-target-best")?.closest("li")).toHaveAttribute(
      "data-flip",
      "battle-8",
    );
  });

  it("marks the highest r on a running session too", async () => {
    // Author's decision after E14: the best battle is the highest r everywhere.
    const { container } = await renderApp();
    const marked = Array.from(container.querySelectorAll(".battle-strip .tile-best"));
    expect(marked).toHaveLength(1);
    expect(marked[0].closest("li")).toHaveAttribute("data-flip", "battle-5");
    expect(marked[0]).toHaveClass("ember-hot");
  });

  it("names one best battle on Session, under the plate and in Overview: the highest r", async () => {
    // battle-3 (Test Valley, 2 930) gets the highest r; the most damage stays with battle-8.
    mocks.damastSession.mockImplementation((id: string) =>
      Promise.resolve(fx.damastWithBest(3, id)),
    );
    // The backend's moment names the battle with the most damage (battle-8).
    const story = fx.sessionStory();
    mocks.sessionStory.mockResolvedValue({
      ...story,
      moments: [...story.moments, { kind: "best_battle", battle: story.bestBattle! }],
    });
    const { container } = await openEnded();
    const spaced = (text: string | null | undefined) => (text ?? "").replace(/\s/gu, " ");
    // The card of numbers shows the best battle's map.
    expect(
      within(container.querySelector(".session-flare-hero") as HTMLElement).getByText(
        /Tło: grafika mapy najlepszej bitwy \(Test Valley\)/u,
      ),
    ).toBeInTheDocument();
    // The spark under the plate and the strip's mark: the same battle.
    const dots = Array.from(container.querySelectorAll(".damast-strip .damast-dot"));
    expect(dots.findIndex((dot) => dot.classList.contains("damast-best"))).toBe(3);
    const marked = Array.from(container.querySelectorAll(".battle-strip .tile-best"));
    expect(marked.map((tile) => tile.closest("li")?.getAttribute("data-flip"))).toEqual([
      "battle-3",
    ]);
    // The moment "Najlepsza bitwa" shows its damage.
    expect(spaced(screen.getByText("Najlepsza bitwa").parentElement?.textContent)).toMatch(
      /2 930/u,
    );
    // Overview's last session card ends with the same battle and its damage.
    await go("Przegląd");
    expect(spaced(container.querySelector(".overview-best")?.textContent)).toMatch(
      /^najlepsza bitwa2 930Test Valley$/u,
    );
  });

  it("shows a plate forged before as finished, without drawing its tile again", async () => {
    const { container } = await openEnded();
    expect(container.querySelector(".damast-plate")).toHaveAttribute("data-state", "ready");
    expect(mocks.putDamastTile).not.toHaveBeenCalled();
  });

  it("keeps a new plate in the cache once it stands, so it is forged only once", async () => {
    mocks.damastTile.mockResolvedValue(new ArrayBuffer(0));
    // Another session's data than in the other tests: a tile key of its own.
    const other = fx.damastSessions(1, 7)[0];
    mocks.damastSession.mockImplementation((id: string) =>
      Promise.resolve({ ...other, session: { ...other.session, id } }),
    );
    const { container } = await openEnded();
    await flush();
    expect(container.querySelector(".damast-plate")).toHaveAttribute("data-state", "ready");
    expect(mocks.putDamastTile).toHaveBeenCalledTimes(1);
    expect(mocks.putDamastTile.mock.calls[0][0]).toBe("damast-1");
    expect(mocks.putDamastTile.mock.calls[0][1]).toMatch(/^[0-9a-f]{32}$/);
  });

  it("opens History on its Sessions tab", async () => {
    await openEnded();
    fireEvent.click(screen.getByRole("button", { name: "Do historii" }));
    await flush();
    expect(screen.getByRole("heading", { level: 1, name: "Historia" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Sesje" })).toBeChecked();
  });

  it("saves the card through the backend's save dialog and says so", async () => {
    await openEnded();
    const drawn: string[] = [];
    const context = new Proxy({} as Record<string | symbol, unknown>, {
      get(target, name) {
        if (name in target) return target[name];
        if (name === "measureText") return (text: string) => ({ width: text.length * 10 });
        if (name === "createRadialGradient") return () => ({ addColorStop() {} });
        if (name === "createImageData")
          return (width: number, height: number) => ({
            data: new Uint8ClampedArray(width * height * 4),
          });
        return (...args: unknown[]) => {
          if (name === "fillText") drawn.push(String(args[0]));
        };
      },
      set(target, name, value) {
        target[name] = value;
        return true;
      },
    });
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(context as never);
    // jsdom loads no images: the card's logo "loads" as soon as it has its address.
    vi.stubGlobal("Image", LoadedImage);
    vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation((callback) =>
      callback(new Blob([new Uint8Array([0x89, 0x50, 0x4e, 0x47])])),
    );
    // Fake clocks that still run, so the 4 s of the message can be skipped.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent.click(screen.getByRole("button", { name: "Zapisz kartę" }));
    await flush();
    expect(mocks.exportDamastCard).toHaveBeenCalledTimes(1);
    expect(mocks.exportDamastCard.mock.calls[0][0]).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(mocks.exportDamastCard.mock.calls[0][1]).toEqual(
      new Uint8Array([0x89, 0x50, 0x4e, 0x47]),
    );
    // E14 polish: next to the buttons, read out politely, gone after 4 s.
    const live = screen
      .getByRole("region", { name: "Damast" })
      .querySelector(".damast-actions [aria-live='polite']") as HTMLElement;
    expect(live).toHaveTextContent("Zapisano kartę.");
    act(() => {
      vi.advanceTimersByTime(3_900);
    });
    expect(live).toHaveTextContent("Zapisano kartę.");
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(live).toHaveTextContent("");
    vi.useRealTimers();
    expect(drawn).toContain("AFTERBATTLE");
    // Nothing from the data that names a player or a battle reaches the card.
    for (const text of drawn) expect(text).not.toMatch(/session-0|battle-|Alpha|Bravo|Synthetic/u);
  });
});

/** Spec 34 §3–§4: History → Sessions as Damast plates, from the cache. */
describe("Damast in History (spec 34)", () => {
  let urls = 0;
  beforeEach(() => {
    resetTiles();
    urls = 0;
    Object.assign(URL, { createObjectURL: () => `blob:damast-${++urls}` });
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });
  async function openSessions() {
    const rendered = await renderApp();
    await go("Historia");
    fireEvent.click(screen.getByRole("radio", { name: "Sesje" }));
    await flush();
    return rendered;
  }
  /** The backend's pages of `count` sessions, 30 at a time, as `get_damast_sessions` pages. */
  function pagesOf(count: number) {
    const all = fx.damastSessions(count);
    mocks.damastSessions.mockImplementation(
      (cursor: { sessionId: string } | null, limit: number) => {
        const from = cursor ? all.findIndex((item) => item.session.id === cursor.sessionId) + 1 : 0;
        const items = all.slice(from, from + limit);
        return Promise.resolve(fx.damastPage(items, from + limit < all.length));
      },
    );
    return all;
  }

  it("shows 200 sessions as plates from the cache, none drawn live", async () => {
    const all = pagesOf(200);
    const { container } = await openSessions();
    for (let page = 1; page < 7; page++) {
      fireEvent.click(screen.getByRole("button", { name: "Pokaż kolejne" }));
      await flush();
    }
    expect(screen.queryByRole("button", { name: "Pokaż kolejne" })).toBeNull();
    const tiles = Array.from(container.querySelectorAll(".damast-tile"));
    expect(tiles).toHaveLength(200);
    const images = Array.from(
      container.querySelectorAll<HTMLImageElement>("img.damast-tile-image"),
    );
    expect(images).toHaveLength(200);
    expect(images.every((image) => image.src.startsWith("blob:damast-"))).toBe(true);
    expect(images.every((image) => image.getAttribute("loading") === "lazy")).toBe(true);
    // Every plate came from the cache: one read each, nothing drawn, nothing written.
    expect(mocks.damastTile).toHaveBeenCalledTimes(200);
    expect(damastEngine.renderStill).not.toHaveBeenCalled();
    expect(mocks.putDamastTile).not.toHaveBeenCalled();
    expect(mocks.damastSessions.mock.calls.map((call) => call[1])).toEqual(Array(7).fill(30));
    // Newest first, with the battle bar (no spark) under each plate.
    const { formatDate, formatNumber } = await import("../lib/format");
    const spaced = (text: string | null) => (text ?? "").replace(/\s/gu, " ");
    expect(spaced(tiles[0].textContent)).toContain(
      spaced(formatDate(all[0].session.startedAt, "pl")),
    );
    expect(spaced(tiles[0].textContent)).toContain(spaced(`${formatNumber(all[0].wn8, "pl")} WN8`));
    expect(tiles[0].querySelectorAll(".damast-dot")).toHaveLength(9);
    expect(container.querySelector(".damast-tiles .damast-best")).toBeNull();
  });

  it("draws a plate missing from the cache once, in the worker, and keeps it", async () => {
    pagesOf(3);
    mocks.damastTile.mockImplementation(async (_version: string, key: string) =>
      key === damastTileKey(1)
        ? new ArrayBuffer(0)
        : new Uint8Array([0xff, 0xd8, 0xff, 0xd9]).buffer,
    );
    await openSessions();
    await flush();
    expect(damastEngine.renderStill).toHaveBeenCalledTimes(1);
    expect(damastEngine.renderStill.mock.calls[0][3]).toBe("jpeg");
    expect(mocks.putDamastTile).toHaveBeenCalledTimes(1);
    expect(mocks.putDamastTile.mock.calls[0][1]).toBe(damastTileKey(1));
    expect(document.querySelectorAll("img.damast-tile-image")).toHaveLength(3);
  });

  it("has no ⓘ by the plates of History (author, 22.11: only on Session)", async () => {
    const { container } = await openSessions();
    expect(container.querySelectorAll(".damast-tile").length).toBeGreaterThan(0);
    expect(container.querySelector(".damast-info, .damast-info-button")).toBeNull();
    expect(screen.queryByRole("button", { name: "Jak czytać tę płytkę" })).toBeNull();
  });

  it("lets only the record session glow", async () => {
    const all = pagesOf(5);
    const record = [...all].sort((a, b) => (b.wn8 ?? 0) - (a.wn8 ?? 0))[0];
    const { container } = await openSessions();
    const hot = Array.from(container.querySelectorAll(".ember-hot"));
    expect(hot).toHaveLength(1);
    expect(hot[0]).toHaveClass("damast-tile");
    const { formatNumber } = await import("../lib/format");
    const spaced = (text: string | null) => (text ?? "").replace(/\s/gu, " ");
    expect(spaced(hot[0].textContent)).toContain(spaced(`${formatNumber(record.wn8, "pl")} WN8`));
    // Author, 22.11: no frame; its WN8 glows in ember core and only that glow breathes.
    const figure = hot[0].querySelector(".damast-tile-caption .num.ember-figure");
    expect(spaced(figure?.textContent ?? null)).toBe(spaced(formatNumber(record.wn8, "pl")));
    expect(figure?.querySelector(".ember-breath")).toHaveAttribute(
      "data-text",
      figure?.textContent ?? "",
    );
    expect(hot[0].querySelectorAll(".ember-breath")).toHaveLength(1);
  });

  it("opens a session from its plate, older ones too", async () => {
    const all = pagesOf(40);
    const old = all[35];
    mocks.sessionSummary.mockImplementation((id: string) => {
      const summary = fx.summary();
      return Promise.resolve({ ...summary, session: { ...old.session, id } });
    });
    mocks.damastSession.mockImplementation((id: string) =>
      Promise.resolve({ ...old, session: { ...old.session, id } }),
    );
    await openSessions();
    fireEvent.click(screen.getByRole("button", { name: "Pokaż kolejne" }));
    await flush();
    fireEvent.click(document.querySelectorAll(".damast-tile")[35]);
    await flush();
    expect(mocks.sessionSummary).toHaveBeenLastCalledWith(old.session.id);
    expect(mocks.damastSession).toHaveBeenLastCalledWith(old.session.id);
    expect(screen.getByRole("region", { name: "Damast" })).toBeInTheDocument();
  });
});

/** The cache key of the `index`-th synthetic History session's plate. */
function damastTileKey(index: number): string {
  return tileKeyOf(damastStats(fx.damastSessions(index + 1)[index]).plate);
}
