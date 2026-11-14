import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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

describe("public snapshot script", () => {
  it("excludes the private folders and files as whole paths", () => {
    for (const path of [
      "docs/research/",
      "docs/audit/",
      "docs/status/*.md",
      "STATUS.md",
      "CLAUDE.md",
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

  it("sets the author only in the public repository", () => {
    expect(script).toMatch(/'config', 'user\.email'/);
    expect(script).toMatch(/'config', 'user\.name'/);
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

  it("builds the noreply address and the name from the logged-in account", () => {
    const result = resolve(fakeGh(0, 0, user));
    expect(result.ok).toBe(true);
    expect(result.out).toContain(
      "AUTHOR: Synthetic Name <1234567+synthetic-user@users.noreply.github.com>",
    );
  });

  it("uses the login when the account has no name", () => {
    const result = resolve(fakeGh(0, 0, JSON.stringify({ id: 7, login: "nameless", name: null })));
    expect(result.out).toContain("AUTHOR: nameless <7+nameless@users.noreply.github.com>");
  });

  it("refuses when gh is not logged in", () => {
    const result = resolve(fakeGh(1, 0, user));
    expect(result.ok).toBe(false);
    expect(result.out).toContain("not logged in");
  });

  it("refuses when reading the account fails or returns nothing usable", () => {
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

  it("checks -AuthorEmail like the read address: only GitHub noreply addresses", () => {
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
