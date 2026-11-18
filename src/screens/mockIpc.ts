/**
 * Mocked Tauri IPC for screenshots outside the app (spec 25 §8): synthetic data only, no game
 * graphics (images answer empty, so placeholders are drawn). The state comes from the URL:
 * `?state=ready|loading|empty|error|noxvm|few|many|firstrun|ask|update` (`many`: 30 session
 * battles and 200 history rows, for the render-time measurement of E5-flare §7; `firstrun`: no
 * game folder yet, detection finds one synthetic folder — hotfix 1.0.0-beta.3; `ask`: the update
 * question; `update`: a found update — spec 32 §5).
 */
import * as fx from "../test/fixtures";

type Handler = (args: Record<string, unknown>) => unknown;

const state = new URLSearchParams(window.location.search).get("state") ?? "ready";
const never = new Promise(() => {});
const fail = (code: string) => Promise.reject(JSON.stringify({ code, message: code }));

function snapshot() {
  if (state === "firstrun") {
    const base = fx.snapshot({ activeSession: null, recentSessions: [], configured: false });
    return {
      ...base,
      sourceStatus: { ...base.sourceStatus, configured: false, gameRootDisplay: null },
    };
  }
  if (state === "empty") return fx.snapshot({ activeSession: null, recentSessions: [] });
  if (state === "noxvm")
    return fx.snapshot({ advancedStatus: { xvmEnabled: false, xvmLastUpdatedAt: null } });
  if (state === "few") return fx.snapshot({ activeChallenge: null });
  return fx.snapshot({ activeChallenge: state === "ready" ? fx.activeChallenge() : null });
}

/** The tank's cell plus a few more, so the Overview grid shows values and a collecting cell. */
function referenceGrid() {
  const tables = fx.referenceTables();
  const [cell] = tables.cells;
  const at = (
    tier: number,
    vehicleClass: (typeof cell)["vehicleClass"],
    ownBattles: number,
    position: number,
  ) => ({
    ...cell,
    tier,
    vehicleClass,
    ownBattles,
    metrics: cell.metrics.map((metric) =>
      metric.metric === "damage" ? { ...metric, ownPosition: position } : metric,
    ),
  });
  return {
    ...tables,
    cells: [
      ...tables.cells,
      at(10, "heavy_tank", 96, 78.2),
      at(11, "medium_tank", 54, 71.4),
      at(10, "light_tank", 40, 83.1),
      at(9, "medium_tank", 22, 49.2),
      {
        ...cell,
        tier: 6,
        vehicleClass: "light_tank" as const,
        level: "count_only" as const,
        population: 120,
        ownBattles: 4,
        metrics: [],
      },
    ],
  };
}

const handlers: Record<string, Handler> = {
  get_snapshot: () => snapshot(),
  get_session_summary: () =>
    state === "few" ? fx.summary(2, 2400) : state === "many" ? fx.summary(30) : fx.summary(),
  get_session_story: () =>
    state === "few"
      ? fx.sessionStory({ headline: { kind: "neutral", percent: null, battles: 2 }, moments: [] })
      : fx.sessionStory(),
  get_recap: (args) =>
    args.kind === "battle"
      ? fx.battleRecap()
      : args.kind === "week"
        ? fx.weekRecap(state === "few" ? 6 : 39)
        : state === "few"
          ? { ...fx.sessionRecap(), facts: fx.sessionRecap().facts.slice(0, 1) }
          : fx.sessionRecap(),
  get_map_record: () => fx.mapRecord(),
  // A-1 (spec 32 §2): "few" shows the state with too little background.
  get_reference_tables: () =>
    state === "few"
      ? fx.referenceTables({ level: "count_only", population: 120 })
      : referenceGrid(),
  get_weekly_report: () =>
    state === "few"
      ? fx.weeklyReport({ battles: 6, confidence: "too_few", goals: [], wn8: null })
      : state === "noxvm"
        ? fx.weeklyReport({ wn8: null })
        : fx.weeklyReport(),
  get_session_maps: (args) =>
    ((args.sessionIds as string[]) ?? []).map((sessionId) => ({
      sessionId,
      mapKeys: ["90_synthetic_hills", "91_test_valley"],
    })),
  get_session_baseline_comparison: () => (state === "few" ? fx.comparison(12) : fx.comparison()),
  get_battle_view: () => (state === "few" ? fx.battleDetail({ teams: null }) : fx.battleDetail()),
  get_vehicle_history_overview: () => (state === "empty" ? fx.overview(0) : fx.overview()),
  get_vehicle_baseline: () => (state === "few" ? fx.vehicleBaseline(1) : fx.vehicleBaseline()),
  get_vehicle_history_report: () => fx.vehicleReport(),
  // Overview asks for the last 100 battles of all tanks (spec 32 §4.2).
  get_performance_trends: (args) =>
    args.vehicleId === null && args.range === "last100"
      ? fx.overviewTrends(state !== "noxvm")
      : state === "few"
        ? fx.trends(1)
        : fx.trends(),
  get_battle_history_page: () =>
    state === "empty"
      ? { items: [], nextCursor: null }
      : fx.historyPage(state === "many" ? 200 : 12),
  get_recent_sessions_wn8: () => [],
  get_challenge_suggestions: () => ({ suggestions: [], unavailableReason: null }),
  get_challenge_history: () => ({ items: [], nextOffset: null }),
  get_vehicle_class_catalog_status: () => ({ vehicleCount: 1270, sourceVersion: "synthetic" }),
  get_expected_values_status: () => fx.snapshot().wn8Presentation.expectedValues,
  get_diagnostics: () =>
    JSON.stringify({
      rebuildReport: { recalculated: 2013, notRandom: 188, withoutReplay: 3, conflicts: 0 },
    }),
  get_attempts_conversion_session_report: () =>
    state === "few"
      ? fx.attemptsReport(2, { shots: 14, hits: 9, penetrations: 7, explosions: 0, damage: 4_800 })
      : fx.attemptsReport(),
  get_attempts_conversion_recent_report: () => fx.attemptsReport(),
  // Screenshots never show names from the game: the readable code stands in for them.
  get_vehicle_names: () => ({ language: null, names: {} }),
  get_game_image: () => new ArrayBuffer(0),
  detect_wot_installations: () => [String.raw`D:\Games\World_of_Tanks_EU`],
  // Spec 32 §5: the first run (and "ask") shows the consent question, "update" a found version.
  get_update_consent: () =>
    state === "firstrun" || state === "ask" ? "unasked" : state === "update" ? "on" : "off",
  check_for_update: () =>
    state === "update"
      ? {
          version: "1.0.1",
          notes: "Poprawki odświeżania Sesji i porównania na ekranie Czołg.",
        }
      : null,
};

async function invoke(command: string, args: Record<string, unknown> = {}) {
  if (command.startsWith("plugin:event|")) return 0;
  if (state === "loading" && command !== "get_snapshot") return never;
  if (state === "error" && command !== "get_snapshot" && command !== "get_game_image")
    return fail("database_error");
  const handler = handlers[command];
  return handler ? handler(args) : undefined;
}

let callbackId = 0;
Object.assign(window as unknown as Record<string, unknown>, {
  __TAURI_INTERNALS__: {
    invoke,
    transformCallback: () => ++callbackId,
    unregisterCallback: () => undefined,
  },
  __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => undefined },
});
