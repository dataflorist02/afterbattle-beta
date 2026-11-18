// Spec 31 §2: times of the motion table and layout shifts, measured in a real browser on the
// synthetic screens (screens.html, mocked IPC). `node timing.mjs [root]`: the root of a checkout
// to measure (default: this repository), so the same script measures a tree before a change.
import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { chromium } from "playwright-core";

const root = resolve(process.argv[2] ?? resolve(import.meta.dirname, "../.."));
const port = 5198;
const server = spawn(
  process.execPath,
  [resolve(root, "node_modules/vite/bin/vite.js"), "--port", String(port), "--strictPort"],
  { cwd: root, stdio: ["ignore", "pipe", "inherit"] },
);
await new Promise((ready) => server.stdout.on("data", (chunk) => String(chunk).includes("Local") && ready()));

const browser = await chromium
  .launch({ channel: "msedge", headless: true })
  .catch(() => chromium.launch({ channel: "chrome", headless: true }));

/** Opens the synthetic window with a layout-shift observer from the first paint. */
async function open(state = "ready") {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.addInitScript(() => {
    localStorage.setItem("afterbattle-language", "pl");
    window.__shifts = [];
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries())
        if (!entry.hadRecentInput) window.__shifts.push({ value: entry.value, at: entry.startTime });
    }).observe({ type: "layout-shift", buffered: true });
  });
  await page.goto(`http://localhost:${port}/screens.html?state=${state}`);
  await page.locator(".screen").first().waitFor();
  await page.waitForTimeout(1200);
  return page;
}

/** Sum of layout shifts since `from` (ms on the page clock). */
const shiftsSince = (page, from) =>
  page.evaluate((since) => window.__shifts.filter((s) => s.at >= since).reduce((sum, s) => sum + s.value, 0), from);

/** Time from a press on `selector` to its full pressed state (scale at most 0.981). */
async function pressTime(page, selector) {
  const element = page.locator(selector).first();
  await element.scrollIntoViewIfNeeded();
  const box = await element.boundingBox();
  await page.evaluate((sel) => {
    const target = document.querySelector(sel);
    window.__press = null;
    target.addEventListener(
      "pointerdown",
      () => {
        const start = performance.now();
        const watch = () => {
          const matrix = new DOMMatrixReadOnly(getComputedStyle(target).transform);
          const background = getComputedStyle(target).backgroundColor;
          if (matrix.a <= 0.981 || background !== window.__pressBackground)
            window.__press = performance.now() - start;
          else if (performance.now() - start < 1000) requestAnimationFrame(watch);
        };
        requestAnimationFrame(watch);
      },
      { once: true, capture: true },
    );
    window.__pressBackground = getComputedStyle(target).backgroundColor;
  }, selector);
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.waitForTimeout(250);
  await page.evaluate((sel) => (window.__pressBackground = getComputedStyle(document.querySelector(sel)).backgroundColor), selector);
  await page.mouse.down();
  await page.waitForTimeout(400);
  // Off the element before the release: a press, not a click (no navigation).
  await page.mouse.move(1, 1);
  await page.mouse.up();
  return page.evaluate(() => window.__press);
}

/** Screen change: click on a rail button, time to the new screen in the DOM and its animation. */
async function screenChange(page, label) {
  const before = await page.evaluate(() => performance.now());
  const t0 = Date.now();
  await page.getByRole("button", { name: label, exact: true }).click();
  await page.waitForFunction(() => document.querySelector(".screen")?.textContent?.length > 0);
  const inDom = Date.now() - t0;
  const motion = await page.evaluate(() => {
    const screen = document.querySelector(".screen");
    const style = getComputedStyle(screen);
    return { name: style.animationName, duration: style.animationDuration };
  });
  await page.waitForTimeout(1200);
  return { inDom, motion, shift: await shiftsSince(page, before) };
}

const cssValue = (page, name) =>
  page.evaluate((token) => getComputedStyle(document.documentElement).getPropertyValue(token).trim() || "(brak)", name);

const rows = [];
try {
  const page = await open();
  rows.push(["CLS Sesja (wejście)", (await shiftsSince(page, 0)).toFixed(4)]);
  rows.push(["Sesja: wciśnięcie kafelka bitwy", `${(await pressTime(page, ".tile"))?.toFixed(0)} ms`]);
  rows.push(["Sesja: przycisk, wciśnięcie", `${(await pressTime(page, ".button"))?.toFixed(0)} ms`]);
  for (const [label, name] of [
    ["Historia", "Historia"],
    ["Czołgi", "Czołgi"],
    ["Cele", "Cele"],
    ["Sesja", "Sesja"],
  ]) {
    const result = await screenChange(page, label);
    rows.push([
      `Przejście → ${name}`,
      `w DOM ${result.inDom} ms; animacja ${result.motion.name} ${result.motion.duration}; CLS ${result.shift.toFixed(4)}`,
    ]);
  }
  await page.getByRole("button", { name: "Historia", exact: true }).click();
  await page.waitForTimeout(1200);
  rows.push(["Historia: wciśnięcie wiersza", `${(await pressTime(page, ".hrow"))?.toFixed(0)} ms`]);
  const before = await page.evaluate(() => performance.now());
  await page.locator(".hrow").first().click();
  await page.waitForTimeout(1500);
  rows.push(["CLS Bitwa (z Historii)", (await shiftsSince(page, before)).toFixed(4)]);
  rows.push(["--motion-hover / --motion-base / --motion-enter", `${await cssValue(page, "--motion-hover")} / ${await cssValue(page, "--motion-base")} / ${await cssValue(page, "--motion-enter")}`]);
  rows.push(["--motion-count", await cssValue(page, "--motion-count")]);
  await page.close();
} finally {
  await browser.close();
  server.kill();
}
console.log(`| Miara | Wynik (${root}) |\n|---|---|`);
for (const [name, value] of rows) console.log(`| ${name} | ${value} |`);
