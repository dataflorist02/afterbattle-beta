// Screenshots of every screen in every state (spec 25 §8), outside Tauri: Vite serves
// screens.html with the mocked IPC and synthetic data; the installed Edge or Chrome
// renders it headless. Game graphics are never loaded; placeholders are drawn instead.
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { chromium } from "playwright-core";

const root = resolve(import.meta.dirname, "../..");
const out = resolve(root, "docs/status/screens");
const port = 5199;
mkdirSync(out, { recursive: true });

const server = spawn(process.execPath, [resolve(root, "node_modules/vite/bin/vite.js"), "--port", String(port), "--strictPort"], {
  cwd: root,
  stdio: ["ignore", "pipe", "inherit"],
});
await new Promise((ready) => server.stdout.on("data", (chunk) => String(chunk).includes("Local") && ready()));

const pages = [
  ["session", null],
  ["tanks", "Czołgi"],
  ["goals", "Cele"],
  ["history", "Historia"],
  ["settings", "Ustawienia"],
  ["battle", null],
];
const states = ["ready", "loading", "empty", "error", "noxvm", "few"];
// E5-ui §1: every state at 1280 px (file `<screen>-<state>.png`), and every screen ready at the
// wide widths (file `<screen>-ready-<width>.png`).
const shots = [
  ...states.map((state) => [state, 1280, ""]),
  ["ready", 1920, "-1920"],
  ["ready", 2560, "-2560"],
];

// The system browser: Edge when present, otherwise Chrome.
const browser = await chromium
  .launch({ channel: "msedge", headless: true })
  .catch(() => chromium.launch({ channel: "chrome", headless: true }));
try {
  for (const [state, width, suffix] of shots) {
    for (const [name, label] of pages) {
      const page = await browser.newPage({ viewport: { width, height: width > 1280 ? 1200 : 900 } });
      await page.addInitScript(() => localStorage.setItem("afterbattle-language", "pl"));
      await page.goto(`http://localhost:${port}/screens.html?state=${state}`);
      await page.waitForTimeout(400);
      if (label) await page.getByRole("button", { name: label, exact: true }).click();
      if (name === "battle") {
        const row = page.locator(".table-row").first();
        if ((await row.count()) === 0) {
          await page.close();
          continue;
        }
        await row.click();
      }
      await page.waitForTimeout(400);
      await page.screenshot({ path: resolve(out, `${name}-${state}${suffix}.png`), fullPage: true });
      await page.close();
    }
  }
  // E5-flare §7: render time with 30 session tiles and 200 history rows (synthetic, real browser).
  const page = await browser.newPage({ viewport: { width: 1920, height: 1200 } });
  await page.addInitScript(() => localStorage.setItem("afterbattle-language", "pl"));
  let started = Date.now();
  await page.goto(`http://localhost:${port}/screens.html?state=many`);
  await page.locator(".battle-strip li").nth(29).waitFor();
  const session = Date.now() - started;
  started = Date.now();
  await page.getByRole("button", { name: "Historia", exact: true }).click();
  await page.locator(".hrow").nth(199).waitFor();
  const history = Date.now() - started;
  console.log(`render ms (load to content): session with 30 tiles ${session}, history with 200 rows ${history}`);
  // E5-flare-fix2: on long screens the Settings button stays visible, also after scrolling.
  const settingsVisible = async (where) => {
    for (const scroll of ["top", "bottom"]) {
      await page.evaluate((to) => window.scrollTo(0, to === "top" ? 0 : document.body.scrollHeight), scroll);
      await page.waitForTimeout(100);
      const box = await page.getByRole("button", { name: "Ustawienia", exact: true }).boundingBox();
      const height = page.viewportSize().height;
      const inside = box && box.y >= 0 && box.y + box.height <= height;
      console.log(`settings visible on ${where} (${scroll}): ${inside ? "yes" : "NO"}`);
      if (!inside) throw new Error(`Settings not visible on ${where} at the ${scroll}`);
    }
  };
  await settingsVisible("history with 200 rows");
  await page.getByRole("button", { name: "Sesja", exact: true }).click();
  await page.locator(".battle-strip li").nth(29).waitFor();
  await settingsVisible("session with 30 battles");
  await page.close();
} finally {
  await browser.close();
  server.kill();
}
console.log(`screenshots in ${out}`);
