import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { describe, expect, it } from "vitest";

function files(root: string): string[] {
  if (!existsSync(root)) return [];
  return readdirSync(root).flatMap((name) => {
    const path = `${root}/${name}`;
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

describe("E5 checkpoint checks (spec 20 E5 step 4, spec 25 §8)", () => {
  it("has no color literal outside tokens.css and no alpha.css", () => {
    const sources = files("src").filter(
      (path) =>
        /\.(css|tsx?)$/.test(path) && !path.endsWith("tokens.css") && !/\.test\.tsx?$/.test(path),
    );
    expect(existsSync("src/alpha.css")).toBe(false);
    for (const path of sources) {
      const text = readFileSync(path, "utf8");
      expect(text, path).not.toMatch(/#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/);
      expect(text, path).not.toContain("alpha.css");
    }
  });

  it("ships no game graphics: only allowed images, fonts and synthetic screenshots", () => {
    const images = [
      ...files("src"),
      ...files("public"),
      ...files("src-tauri/icons"),
      ...files("docs"),
    ].filter((path) => /\.(png|jpe?g|dds|webp|gif|bmp|svg)$/i.test(path));
    for (const path of images) {
      const allowed =
        path.startsWith("src-tauri/icons/") ||
        path.startsWith("docs/status/screens/") ||
        path === "public/favicon.svg";
      expect(allowed, path).toBe(true);
      expect(statSync(path).size, path).toBeLessThan(2_000_000);
    }
  });
});

describe("CI (spec 20 E8)", () => {
  const ci = readFileSync(".github/workflows/ci.yml", "utf8");
  const dependabot = readFileSync(".github/dependabot.yml", "utf8");

  it("pins every action to a full commit SHA", () => {
    const uses = [...ci.matchAll(/^\s*-?\s*uses:\s*(\S+)/gm)].map((match) => match[1]);
    expect(uses.length).toBeGreaterThan(0);
    for (const action of uses) expect(action, action).toMatch(/^[\w.-]+\/[\w.-]+@[0-9a-f]{40}$/);
  });

  it("runs test, audit and build jobs with read-only permissions", () => {
    expect(ci).toMatch(/^permissions:\s*\n\s+contents:\s*read/m);
    for (const job of ["test", "audit", "build"]) expect(ci).toMatch(new RegExp(`^  ${job}:`, "m"));
    expect(ci).toContain("cargo audit");
    expect(ci).toContain("pnpm audit --prod --audit-level high");
    expect(ci).toContain("pnpm tauri build");
    // The build job keeps the installer and its checksum as an artifact, without secrets.
    expect(ci).toContain("actions/upload-artifact@");
    expect(ci).toContain("SHA256SUMS.txt");
    expect(ci).not.toMatch(/secrets\./);
  });

  it("lets Dependabot update the actions too", () => {
    for (const ecosystem of ["npm", "cargo", "github-actions"])
      expect(dependabot).toContain(`package-ecosystem: ${ecosystem}`);
  });
});

describe("release script (spec 20 E8)", () => {
  it("builds the NSIS installer from a clean tree and writes SHA256SUMS.txt", () => {
    const script = readFileSync("scripts/release.ps1", "utf8");
    expect(script).toContain("git status --porcelain");
    expect(script).toContain("pnpm tauri build --bundles nsis");
    expect(script).toContain("Get-FileHash -Algorithm SHA256");
    expect(script).toContain("SHA256SUMS.txt");
    expect(script).toContain("src-tauri\\Cargo.toml");
    // It never publishes anything.
    expect(script).not.toMatch(/git push|git tag|gh release|Invoke-WebRequest|curl/);
  });
});

describe("no destructive tool in the test suite (spec 20 E8, N9)", () => {
  it("keeps destructive tools out of src-tauri/tests: no test reaches %APPDATA%", () => {
    for (const path of files("src-tauri/tests"))
      expect(readFileSync(path, "utf8"), path).not.toMatch(/APPDATA|LOCALAPPDATA/);
  });
});
