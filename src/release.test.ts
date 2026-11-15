import { existsSync, readFileSync, statSync } from "node:fs";
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
    // Spec 29 §8 (author decision 2026-11-15): the second public beta, "Afterbattle beta 2".
    expect(cargoVersion()).toBe("1.0.0-beta.2");
    expect(APP_NAME).toBe("Afterbattle");
    expect(RELEASE_CHANNEL).toBe("Public beta");
    expect(APP_DISPLAY_NAME).toBe("Afterbattle beta 2");
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
    // E5-maps-hd §6 (author decision 2026-11-14): only the custom title bar's window calls;
    // spec 29 §2: listening to the backend's own events (no emit, no core:default).
    expect([...capability.permissions].sort()).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "core:window:allow-close",
      "core:window:allow-internal-toggle-maximize",
      "core:window:allow-minimize",
      "core:window:allow-start-dragging",
      "core:window:allow-toggle-maximize",
    ]);
  });
});

describe("display name (title bar and window title)", () => {
  it("names a pre-release by its channel and number, a final version by the version", () => {
    // Spec 29 §8: "Afterbattle beta N" with N the pre-release number; final: "Afterbattle 1.0.0".
    expect(displayName("1.0.0-beta.2")).toBe("Afterbattle beta 2");
    expect(displayName("1.0.0-beta.1")).toBe("Afterbattle beta 1");
    expect(displayName("1.2.3-beta.14")).toBe("Afterbattle beta 14");
    expect(displayName("1.0.0-rc.2")).toBe("Afterbattle rc 2");
    expect(displayName("1.0.0-beta")).toBe("Afterbattle beta");
    expect(displayName("1.0.0")).toBe("Afterbattle 1.0.0");
    expect(displayName("2.10.3")).toBe("Afterbattle 2.10.3");
    expect(displayName("not a version")).toBe("Afterbattle");
    expect(displayName("1.0")).toBe("Afterbattle");
    expect(displayName("1.0.0-beta.2 ")).toBe("Afterbattle");
  });
});

describe("app icons (spec 29 §4)", () => {
  it("has every icon the bundle names, from the brand icon, without mobile icons", () => {
    expect(tauriConfig.bundle.icon).toEqual([
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.ico",
    ]);
    for (const icon of tauriConfig.bundle.icon) {
      const path = `src-tauri/${icon}`;
      expect(existsSync(path), path).toBe(true);
      expect(statSync(path).size, path).toBeGreaterThan(0);
    }
    expect(existsSync("src-tauri/icons/android")).toBe(false);
    expect(existsSync("src-tauri/icons/ios")).toBe(false);
    expect(readFileSync("src-tauri/icons/icon.svg", "utf8")).toBe(
      readFileSync("docs/design/brand/afterbattle-icon.svg", "utf8"),
    );
  });
});
