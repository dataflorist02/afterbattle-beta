// Spec 31 §4–§5: full-page screenshots of every screen (ready state, synthetic data, mocked IPC)
// at 1280 and 1920 px, for any checkout, so the same script shoots the tree before and after a
// change. `node compare.mjs <root> <out> [screen ...]` writes `<out>/<screen>-<width>.png`;
// SCREENS_STATE=empty (or another mock state) shoots that state as `<screen>-<state>-<width>.png`.
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { chromium } from "playwright-core";

const [rootArg, outArg, ...only] = process.argv.slice(2);
if (!rootArg || !outArg) throw new Error("usage: node compare.mjs <root> <out> [screen ...]");
const root = resolve(rootArg);
const out = resolve(outArg);
const port = 5196;
const state = process.env.SCREENS_STATE ?? "ready";
const suffix = state === "ready" ? "" : `-${state}`;
mkdirSync(out, { recursive: true });

const server = spawn(
  process.execPath,
  [resolve(root, "node_modules/vite/bin/vite.js"), "--port", String(port), "--strictPort"],
  { cwd: root, stdio: ["ignore", "pipe", "inherit"] },
);
await new Promise((ready) => server.stdout.on("data", (chunk) => String(chunk).includes("Local") && ready()));

const screens = [
  ["session", null],
  ["overview", "Przegląd"],
  ["tanks", "Czołgi"],
  ["goals", "Cele"],
  ["history", "Historia"],
  ["settings", "Ustawienia"],
  ["battle", ".table-row"],
].filter(([name]) => only.length === 0 || only.includes(name));

const browser = await chromium
  .launch({ channel: "msedge", headless: true })
  .catch(() => chromium.launch({ channel: "chrome", headless: true }));
try {
  for (const width of [1280, 1920]) {
    for (const [name, via] of screens) {
      const page = await browser.newPage({ viewport: { width, height: width > 1280 ? 1200 : 900 } });
      await page.addInitScript(() => localStorage.setItem("afterbattle-language", "pl"));
      await page.goto(`http://localhost:${port}/screens.html?state=${state}`);
      await page.locator(".screen, .page").first().waitFor();
      await page.waitForTimeout(600);
      if (via?.startsWith(".")) {
        if ((await page.locator(via).count()) === 0) {
          await page.close();
          continue;
        }
        await page.locator(via).first().click();
      }
      else if (via) await page.getByRole("button", { name: via, exact: true }).click();
      await page.waitForTimeout(900);
      await page.screenshot({ path: resolve(out, `${name}${suffix}-${width}.png`), fullPage: true });
      await page.close();
    }
  }
} finally {
  await browser.close();
  server.kill();
}
