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
    // Spec 32 §7 (author decision 2026-11-17): 1.0.0-rc.1, "Afterbattle 1.0 RC"; rc.2 (author,
    // 2026-11-18): security update of zip, the 1.0.0 clock runs on; rc.3 (author, 2026-11-22):
    // Żar and Damast.
    expect(cargoVersion()).toBe("1.0.0-rc.3");
    expect(APP_NAME).toBe("Afterbattle");
    expect(RELEASE_CHANNEL).toBe("Public beta");
    expect(APP_DISPLAY_NAME).toBe("Afterbattle 1.0 RC 3");
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
    // spec 29 §2: listening to the backend's own events (no emit, no core:default);
    // hotfix 1.0.0-beta.3: opening the folder dialog only (no save, message or file access).
    expect([...capability.permissions].sort()).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "core:window:allow-close",
      "core:window:allow-internal-toggle-maximize",
      "core:window:allow-minimize",
      "core:window:allow-start-dragging",
      "core:window:allow-toggle-maximize",
      "dialog:allow-open",
    ]);
  });
});

describe("automatic updates (spec 32 §5.2)", () => {
  it("has the author's minisign public key, never a private one", () => {
    // The key is base64 of two lines: the minisign comment with the key id, then base64 of
    // "Ed" + 8 bytes of the key id (little-endian) + 32 bytes of the Ed25519 public key.
    const pubkey = tauriConfig.plugins.updater.pubkey;
    expect(pubkey).toMatch(/^[A-Za-z0-9+/]+={0,2}$/);
    const text = atob(pubkey);
    const lines = text.split("\n");
    expect(lines.at(-1)).toBe("");
    expect(lines).toHaveLength(3);
    const id = /^untrusted comment: minisign public key: ([0-9A-F]{16})$/.exec(lines[0])?.[1];
    expect(id).toBeDefined();
    expect(lines[1]).toMatch(/^[A-Za-z0-9+/]+={0,2}$/);
    const key = Uint8Array.from(atob(lines[1]), (char) => char.charCodeAt(0));
    expect(key).toHaveLength(42);
    expect(String.fromCharCode(key[0], key[1])).toBe("Ed");
    const keyId = [...key.slice(2, 10)]
      .reverse()
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join("")
      .toUpperCase();
    expect(keyId).toBe(id);
    expect(text).not.toMatch(/secret|private|encrypted/iu);
  });

  it("asks only the spec endpoint; update artifacts come from release.ps1 with the key", () => {
    expect(tauriConfig.plugins.updater.endpoints).toEqual([
      "https://github.com/dataflorist02/afterbattle-beta/releases/latest/download/latest.json",
    ]);
    // The public CI builds without the key, so the config keeps them off (author decision
    // 2026-11-18); release.ps1 turns them on with --config only when the key is set.
    expect(tauriConfig.bundle.createUpdaterArtifacts).toBe(false);
    const script = readFileSync("scripts/release.ps1", "utf8");
    expect(script).toContain("TAURI_SIGNING_PRIVATE_KEY");
    expect(script).toContain('"createUpdaterArtifacts":true');
    expect(script).toContain("latest.json");
  });

  it("locks no zip below 9.0.5 (rc.2: RUSTSEC-2026-0367 … -0548 in the updater's zip 4.x)", () => {
    const lock = readFileSync("src-tauri/Cargo.lock", "utf8");
    const versions = [...lock.matchAll(/^name = "zip"\r?\nversion = "(\d+)\.(\d+)\.(\d+)"/gm)].map(
      ([, major, minor, patch]) => [Number(major), Number(minor), Number(patch)],
    );
    expect(versions.length).toBeGreaterThan(0);
    for (const [major, minor, patch] of versions)
      expect(
        major * 1e6 + minor * 1e3 + patch,
        `zip ${major}.${minor}.${patch}`,
      ).toBeGreaterThanOrEqual(9_000_005);
  });
});

describe("display name (title bar and window title)", () => {
  it("names a pre-release by its channel and number, a final version by the version", () => {
    // Spec 29 §8: "Afterbattle beta N" with N the pre-release number; final: "Afterbattle 1.0.0".
    expect(displayName("1.0.0-beta.4")).toBe("Afterbattle beta 4");
    expect(displayName("1.0.0-beta.3")).toBe("Afterbattle beta 3");
    expect(displayName("1.0.0-beta.2")).toBe("Afterbattle beta 2");
    expect(displayName("1.0.0-beta.1")).toBe("Afterbattle beta 1");
    expect(displayName("1.2.3-beta.14")).toBe("Afterbattle beta 14");
    // Spec 32 §7: a release candidate by its version line, "Afterbattle 1.0 RC" for rc.1;
    // the fixes after it count on ("Afterbattle 1.0 RC 2").
    expect(displayName("1.0.0-rc.1")).toBe("Afterbattle 1.0 RC");
    expect(displayName("1.0.0-rc.2")).toBe("Afterbattle 1.0 RC 2");
    expect(displayName("1.0.0-rc.3")).toBe("Afterbattle 1.0 RC 3");
    expect(displayName("1.1.0-rc")).toBe("Afterbattle 1.1 RC");
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
