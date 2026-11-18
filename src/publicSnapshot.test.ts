import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, describe, expect, it } from "vitest";

/**
 * scripts/make-public-snapshot.ps1 — the only public file, with this test, that names the
 * excluded files (the script's scan exempts both).
 */
const script = readFileSync("scripts/make-public-snapshot.ps1", "utf8");
const exclusions = [
  ...(/\$Exclusions = @\(([\s\S]*?)\n\)/.exec(script)?.[1] ?? "").matchAll(/'([^']+)'/g),
].map((match) => match[1]);

// Cold start of pwsh on the CI runner can take more than the default 5 s per test.
const POWERSHELL_TEST_TIMEOUT = 30_000;
const itPowerShell = (name: string, test: () => void) => it(name, test, POWERSHELL_TEST_TIMEOUT);

describe("public snapshot script", () => {
  it("excludes the private folders and files as whole paths", () => {
    for (const path of [
      "docs/research/",
      "docs/audit/",
      "docs/status/*.md",
      "STATUS.md",
      "CLAUDE.md",
      ".github/dependabot.yml",
      // Research trial code (moonshot sprint 2026-11-15): lives on research/* branches only.
      "research/",
    ])
      expect(exclusions).toContain(path);
  });

  it("scans before committing, stops on excluded names, and never publishes", () => {
    expect(script).toContain("gitleaks");
    for (const name of ["account id", "Windows user path", "private key", "excluded name"])
      expect(script).toContain(`'${name}'`);
    for (const extension of [".dds", ".pkg", ".mo"]) expect(script).toContain(`'${extension}'`);
    expect(script.indexOf("new scan finding")).toBeLessThan(script.indexOf("'commit'"));
    expect(script).not.toMatch(/git push|gh repo|gh release|Invoke-WebRequest|--global/);
    // `-like '??*'` would treat "?" as a wildcard and see every file as untracked.
    expect(script).toContain(".StartsWith('??')");
    // Only the script and this test may name the excluded files.
    expect(/\$NameScanExempt = @\(([\s\S]*?)\)/.exec(script)?.[1]).toMatch(
      /^\s*'scripts\/make-public-snapshot\.ps1',\s*'src\/publicSnapshot\.test\.ts'\s*$/,
    );
  });

  it("knows the update public key as one exact gitleaks false positive, not a pattern", () => {
    // Author's fixup after STOP 2: the minisign public key in tauri.conf.json is public by design.
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")) as {
      plugins: { updater: { pubkey: string } };
    };
    const known = [
      ...(/\$KnownGitleaks = @\(([\s\S]*?)\n\)/.exec(script)?.[1] ?? "").matchAll(
        /Rule = '([^']+)'; File = '([^']+)'; Secret = '([^']+)'/g,
      ),
    ].map(([, rule, file, secret]) => ({ rule, file, secret }));
    expect(known).toHaveLength(8);
    expect(known.filter((entry) => entry.file === "src-tauri/tauri.conf.json")).toEqual([
      {
        rule: "generic-api-key",
        file: "src-tauri/tauri.conf.json",
        secret: config.plugins.updater.pubkey,
      },
    ]);
    // Key rotated before 1.0.0-rc.1 (author, 2026-11-18): the entry names the new key only.
    const pubkeyEntry = known.find((entry) => entry.file === "src-tauri/tauri.conf.json");
    expect(atob(pubkeyEntry?.secret ?? "").split("\n")[0]).toBe(
      "untrusted comment: minisign public key: 112017D205DB8B09",
    );
    for (const entry of known) expect(entry.secret, entry.file).not.toMatch(/[*?[\]]/);
  });

  it("sets the author only in the public repository", () => {
    expect(script).toMatch(/'config', 'user\.email'/);
    expect(script).toMatch(/'config', 'user\.name'/);
  });
});

/**
 * Dependency updates run only in the private repository (Dependabot there also updates the
 * pinned actions); the public copy has no Dependabot configuration, only security alerts.
 */
describe.skipIf(!existsSync(".github/dependabot.yml"))("Dependabot (private repository)", () => {
  it("updates npm, cargo and the actions", () => {
    const dependabot = readFileSync(".github/dependabot.yml", "utf8");
    for (const ecosystem of ["npm", "cargo", "github-actions"])
      expect(dependabot).toContain(`package-ecosystem: ${ecosystem}`);
  });
});

/**
 * The excluded-name scan (author decision 2026-11-15): a one-segment excluded folder is searched
 * as a path prefix, so the ordinary word passes and a path inside the folder stops the script.
 */
describe.skipIf(process.platform !== "win32")("excluded-name scan of folders", () => {
  function scan(text: string): string {
    const result = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts/make-public-snapshot.ps1",
        "-NameScanOnly",
        text,
      ],
      { encoding: "utf8", timeout: 60_000 },
    );
    expect(result.status, result.stderr).toBe(0);
    return result.stdout.trim();
  }

  itPowerShell("lets the word research pass and stops on a path inside research/", () => {
    expect(scan("Architecture, privacy, threat model, checklists, and research")).toBe(
      "NAME SCAN: clean",
    );
    expect(scan("Research notes folder")).toBe("NAME SCAN: clean");
    expect(scan("see research/a1-reference/x")).toBe("NAME SCAN: hit (research/)");
  });

  itPowerShell("lets the word .claude pass and stops on a path inside .claude/", () => {
    expect(scan("settings of .claude are local")).toBe("NAME SCAN: clean");
    expect(scan("see .claude/settings.json")).toBe("NAME SCAN: hit (.claude/)");
  });

  itPowerShell("keeps searching a folder of more segments as a whole name", () => {
    expect(scan("see docs/research for notes")).toBe("NAME SCAN: hit (docs/research)");
    expect(scan("see src-tauri/tools")).toBe("NAME SCAN: hit (src-tauri/tools)");
  });

  // rc.1 snapshot fixup (author, 2026-11-18): the snapshot stopped on excluded names in a spec and
  // a source comment; the same scan over the tracked files the snapshot keeps runs here first.
  itPowerShell("finds no excluded name in the tracked files the snapshot keeps", () => {
    const result = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts/make-public-snapshot.ps1",
        "-NameScanTracked",
      ],
      { encoding: "utf8", timeout: 60_000 },
    );
    expect(result.status, result.stderr).toBe(0);
    expect(result.stdout.trim()).toBe("NAME SCAN: clean");
  });
});

/** Runs the script's author check with a substituted `gh` (a PowerShell script, no network). */
describe.skipIf(process.platform !== "win32")("commit author from GitHub (no network)", () => {
  const dir = mkdtempSync(join(tmpdir(), "afterbattle-gh-"));
  afterAll(() => rmSync(dir, { recursive: true, force: true }));
  let count = 0;

  function fakeGh(authCode: number, apiCode: number, apiOutput: string): string {
    const path = join(dir, `gh-${count++}.ps1`);
    writeFileSync(
      path,
      [
        "if ($args[0] -eq 'auth') { exit " + authCode + " }",
        "if ($args[0] -eq 'api') { @'",
        apiOutput,
        "'@",
        "exit " + apiCode + " }",
        "exit 9",
      ].join("\r\n"),
      "utf8",
    );
    return path;
  }

  function resolve(gh: string, extra: string[] = []) {
    const result = spawnSync(
      "powershell",
      [
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        "scripts/make-public-snapshot.ps1",
        "-ResolveAuthorOnly",
        "-GhCommand",
        gh,
        ...extra,
      ],
      { encoding: "utf8", timeout: 60_000 },
    );
    return { ok: result.status === 0, out: `${result.stdout}\n${result.stderr}` };
  }

  const user = JSON.stringify({ id: 1234567, login: "synthetic-user", name: "Synthetic Name" });

  itPowerShell("builds the noreply address and the name from the logged-in account", () => {
    const result = resolve(fakeGh(0, 0, user));
    expect(result.ok).toBe(true);
    expect(result.out).toContain(
      "AUTHOR: Synthetic Name <1234567+synthetic-user@users.noreply.github.com>",
    );
  });

  itPowerShell("uses the login when the account has no name", () => {
    const result = resolve(fakeGh(0, 0, JSON.stringify({ id: 7, login: "nameless", name: null })));
    expect(result.out).toContain("AUTHOR: nameless <7+nameless@users.noreply.github.com>");
  });

  itPowerShell("refuses when gh is not logged in", () => {
    const result = resolve(fakeGh(1, 0, user));
    expect(result.ok).toBe(false);
    expect(result.out).toContain("not logged in");
  });

  itPowerShell("refuses when reading the account fails or returns nothing usable", () => {
    for (const [code, output] of [
      [1, user],
      [0, ""],
      [0, "not json"],
      [0, JSON.stringify({ login: "no-id" })],
    ] as const) {
      const result = resolve(fakeGh(0, code, output));
      expect(result.ok, output).toBe(false);
      expect(result.out, output).toContain("reading the GitHub account failed");
    }
  });

  itPowerShell("checks -AuthorEmail like the read address: only GitHub noreply addresses", () => {
    const gh = fakeGh(0, 0, user);
    const good = resolve(gh, ["-AuthorEmail", "99+other@users.noreply.github.com"]);
    expect(good.out).toContain("AUTHOR: Synthetic Name <99+other@users.noreply.github.com>");
    for (const address of ["someone@example.com", "x@users.noreply.github.com.evil.example"]) {
      const bad = resolve(gh, ["-AuthorEmail", address]);
      expect(bad.ok, address).toBe(false);
      expect(bad.out, address).toContain("users.noreply.github.com");
    }
  });
});
