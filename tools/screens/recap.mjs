// Spec 31 §3: the summary cards of Session, Battle and Goals, closed and open, as screenshots of
// the synthetic screens (screens.html, mocked IPC). `node recap.mjs [root] [out]`: the checkout to
// capture (default: this repository) and the folder for `recap-<screen>-<closed|open>.png`
// (default: docs/status/screens/e11). Also prints, per card, whether opening moved anything above
// the card and how long the opening took.
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { chromium } from "playwright-core";

const here = resolve(import.meta.dirname, "../..");
const root = resolve(process.argv[2] ?? here);
const out = resolve(process.argv[3] ?? resolve(here, "docs/status/screens/e11"));
const port = 5197;
mkdirSync(out, { recursive: true });

const server = spawn(
  process.execPath,
  [resolve(root, "node_modules/vite/bin/vite.js"), "--port", String(port), "--strictPort"],
  { cwd: root, stdio: ["ignore", "pipe", "inherit"] },
);
await new Promise((ready) => server.stdout.on("data", (chunk) => String(chunk).includes("Local") && ready()));

const browser = await chromium
  .launch({ channel: "msedge", headless: true })
  .catch(() => chromium.launch({ channel: "chrome", headless: true }));

const screens = [
  ["session", "Podsumowanie sesji", null],
  ["battle", "Podsumowanie bitwy", ".table-row"],
  ["goals", "Podsumowanie tygodnia", "Cele"],
];
const rows = [];
try {
  for (const [name, title, via] of screens) {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
    await page.addInitScript(() => localStorage.setItem("afterbattle-language", "pl"));
    await page.goto(`http://localhost:${port}/screens.html?state=ready`);
    await page.locator(".screen").first().waitFor();
    await page.waitForTimeout(600);
    if (via?.startsWith(".")) await page.locator(via).first().click();
    else if (via) await page.getByRole("button", { name: via, exact: true }).click();
    const card = page.getByRole("region", { name: title });
    await card.waitFor();
    await page.waitForTimeout(600);
    await card.screenshot({ path: resolve(out, `recap-${name}-closed.png`) });
    const more = card.getByRole("button", { name: /^Pokaż więcej/u });
    if ((await more.count()) === 0) {
      rows.push([name, "no toggle", "", ""]);
      await page.close();
      continue;
    }
    // Scroll so the card starts near the top: anything above it must stay where it is.
    await card.evaluate((element) => window.scrollTo(0, element.getBoundingClientRect().top + window.scrollY - 120));
    const before = await card.evaluate((element) => ({ top: element.getBoundingClientRect().top, y: window.scrollY }));
    const started = await page.evaluate(() => performance.now());
    await more.click();
    const took = await card.evaluate(async (element, start) => {
      const rest = element.querySelector(".recap-rest");
      let last = -1;
      let stableSince = performance.now();
      while (performance.now() - stableSince < 120) {
        await new Promise(requestAnimationFrame);
        const height = rest.getBoundingClientRect().height;
        if (height !== last) {
          last = height;
          stableSince = performance.now();
        }
      }
      return Math.round(stableSince - start);
    }, started);
    const after = await card.evaluate((element) => ({ top: element.getBoundingClientRect().top, y: window.scrollY }));
    await card.screenshot({ path: resolve(out, `recap-${name}-open.png`) });
    rows.push([name, `${after.top - before.top} px`, `${after.y - before.y} px`, `${took} ms`]);
    await page.close();
  }
} finally {
  await browser.close();
  server.kill();
}
console.log("| Screen | Card top moved | Scroll moved | Opening done after |");
console.log("|---|---|---|---|");
for (const row of rows) console.log(`| ${row.join(" | ")} |`);
