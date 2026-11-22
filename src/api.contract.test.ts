import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/** Spec 20 T3: every registered Tauri command is called from `api.ts` and nothing else is. */
function registeredCommands(): string[] {
  const source = readFileSync("src-tauri/src/lib.rs", "utf8");
  const block = /generate_handler!\[([\s\S]*?)\]/.exec(source)?.[1] ?? "";
  return block
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean)
    .sort();
}

function calledCommands(): string[] {
  const source = readFileSync("src/api.ts", "utf8");
  return [
    ...new Set(
      [...source.matchAll(/command<[\s\S]*?>\(\s*"([a-z0-9_]+)"/g)].map((match) => match[1]),
    ),
  ].sort();
}

describe("IPC contract (generate_handler ↔ api.ts)", () => {
  it("calls exactly the registered commands", () => {
    const registered = registeredCommands();
    expect(registered.length).toBeGreaterThan(30);
    expect(calledCommands()).toEqual(registered);
  });

  it("contains the E5 commands and none of the removed ones (spec 20 §7)", () => {
    const registered = registeredCommands();
    for (const name of ["get_game_image", "get_battle_view", "start_import", "cancel_import"])
      expect(registered).toContain(name);
    for (const name of [
      "import_local_history",
      "sync_battles",
      "refresh_xvm",
      "configure_profile",
      "store_api_key",
      // E14 (author's decision): the old History session list's commands, no screen calls them.
      "get_recent_sessions_wn8",
      "get_session_maps",
    ])
      expect(registered).not.toContain(name);
  });
});
