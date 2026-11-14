import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import packageJson from "../package.json";
import tauriConfig from "../src-tauri/tauri.conf.json";
import capability from "../src-tauri/capabilities/default.json";
import { APP_DISPLAY_NAME, APP_NAME, APP_VERSION, RELEASE_CHANNEL, displayName } from "./release";

/** The version from `src-tauri/Cargo.toml`, the single source of truth (spec 20 E8, R1). */
function cargoVersion(): string {
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
  return /^\[package\][^[]*?^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1] ?? "";
}

describe("release identity", () => {
  it("takes the version from Cargo.toml everywhere and keeps product metadata aligned", () => {
    // Author decision 2026-11-14: the first public release is beta 1.0 (internally v2).
    expect(cargoVersion()).toBe("1.0.0-beta.1");
    expect(APP_NAME).toBe("Afterbattle");
    expect(RELEASE_CHANNEL).toBe("Public beta");
    expect(APP_DISPLAY_NAME).toBe("Afterbattle beta 1.0");
    expect(tauriConfig.app.windows[0].title).toBe(APP_DISPLAY_NAME);
    expect(APP_VERSION).toBe(cargoVersion());
    expect(packageJson.version).toBe(cargoVersion());
    // Tauri reads the version from Cargo.toml when tauri.conf.json has none.
    expect("version" in tauriConfig).toBe(false);
    expect(tauriConfig.productName).toBe(APP_NAME);
    expect(tauriConfig.identifier).toBe("app.afterbattle.desktop");
  });
  it("keeps the release frontend local with no broad capability grants", () => {
    expect(tauriConfig.build.frontendDist).toBe("../dist");
    expect(tauriConfig.app.security.csp).toContain("default-src 'self'");
    expect(tauriConfig.app.security.csp).toContain("object-src 'none'");
    // E5-maps-hd §6 (author decision 2026-11-14): only the custom title bar's window calls.
    expect([...capability.permissions].sort()).toEqual([
      "core:window:allow-close",
      "core:window:allow-internal-toggle-maximize",
      "core:window:allow-minimize",
      "core:window:allow-start-dragging",
      "core:window:allow-toggle-maximize",
    ]);
  });
});

describe("display name (title bar and window title)", () => {
  it("names the channel and major.minor of a pre-release, the plain version otherwise", () => {
    expect(displayName("1.0.0-beta.1")).toBe("Afterbattle beta 1.0");
    expect(displayName("1.2.3-beta.4")).toBe("Afterbattle beta 1.2");
    expect(displayName("1.0.0-rc.2")).toBe("Afterbattle rc 1.0");
    expect(displayName("1.0.0")).toBe("Afterbattle 1.0");
    expect(displayName("not a version")).toBe("Afterbattle");
  });
});
