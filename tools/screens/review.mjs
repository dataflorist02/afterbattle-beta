// Spec 31 §5: UI review of the synthetic screens in a real browser. For every screen in the
// states ready, empty, few, noxvm and error (1280 px): text contrast against its real background
// (≥ 4.5:1, large text ≥ 3:1), a visible keyboard focus on everything Tab reaches, interactive
// elements Tab never reaches, and Tab steps that jump back up the page. Also lists spacing in
// base.css that is not on the 4 px grid. `node review.mjs [root]` prints the findings as JSON.
import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { chromium } from "playwright-core";

const root = resolve(process.argv[2] ?? resolve(import.meta.dirname, "../.."));
const port = 5195;
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
];
const states = ["ready", "empty", "few", "noxvm", "error"];

/** Runs in the page: contrast of every visible text against the background behind it. */
function contrastFindings() {
  const parse = (value) => {
    const match = /rgba?\(([^)]+)\)/.exec(value);
    if (!match) return null;
    const [r, g, b, a = 1] = match[1].split(/[,\s/]+/).filter(Boolean).map(Number);
    return { r, g, b, a };
  };
  const over = (top, bottom) => ({
    r: top.r * top.a + bottom.r * (1 - top.a),
    g: top.g * top.a + bottom.g * (1 - top.a),
    b: top.b * top.a + bottom.b * (1 - top.a),
    a: 1,
  });
  const luminance = ({ r, g, b }) => {
    const channel = (value) => {
      const c = value / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    };
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
  };
  const ratio = (a, b) => {
    const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
    return (hi + 0.05) / (lo + 0.05);
  };
  /** The colour behind an element: its own and its ancestors' backgrounds, composited. */
  const background = (element) => {
    const layers = [];
    for (let node = element; node; node = node.parentElement) {
      const style = getComputedStyle(node);
      if (style.backgroundImage !== "none" && node !== document.body && node !== document.documentElement)
        return null; // gradients and pictures: not measured here
      const colour = parse(style.backgroundColor);
      if (colour && colour.a > 0) layers.push(colour);
      if (colour && colour.a === 1) break;
    }
    let result = { r: 10, g: 16, b: 28, a: 1 }; // --bg when nothing is opaque
    for (const layer of layers.reverse()) result = over(layer, result);
    return result;
  };
  const found = new Map();
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (!node.textContent.trim()) continue;
    const element = node.parentElement;
    const rect = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    if (!rect.width || style.visibility === "hidden" || Number(style.opacity) === 0) continue;
    if (element.closest("[aria-hidden='true'], [inert]")) continue;
    const back = background(element);
    const fore = parse(style.color);
    if (!back || !fore) continue;
    const size = parseFloat(style.fontSize);
    const large = size >= 24 || (size >= 18.66 && Number(style.fontWeight) >= 700);
    const value = ratio(over(fore, back), back);
    const need = large ? 3 : 4.5;
    if (value >= need) continue;
    const name = `${element.tagName.toLowerCase()}.${[...element.classList].join(".")}`;
    const key = `${name} ${style.color}`;
    if (!found.has(key))
      found.set(key, {
        element: name,
        text: node.textContent.trim().slice(0, 40),
        color: style.color,
        ratio: Math.round(value * 100) / 100,
        need,
        disabled: element.closest(":disabled, [aria-disabled='true']") !== null,
      });
  }
  return [...found.values()];
}

/** Runs in the page: whether the focused element shows a ring (own outline/shadow or ::before). */
function focusVisible() {
  const element = document.activeElement;
  if (!element || element === document.body) return null;
  const style = getComputedStyle(element);
  const before = getComputedStyle(element, "::before");
  const after = getComputedStyle(element, "::after");
  const ring =
    (style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0) ||
    style.boxShadow !== "none" ||
    (before.content !== "none" && Number(before.opacity) > 0 && (before.boxShadow !== "none" || before.outlineStyle !== "none" || before.borderStyle !== "none")) ||
    (after.content !== "none" && Number(after.opacity) > 0 && (after.boxShadow !== "none" || after.outlineStyle !== "none"));
  const rect = element.getBoundingClientRect();
  const label = (element.getAttribute("aria-label") || element.textContent || element.getAttribute("href") || "").trim().slice(0, 40);
  return {
    element: `${element.tagName.toLowerCase()}.${[...element.classList].join(".")}`,
    label,
    ring,
    top: Math.round(rect.top + window.scrollY),
    left: Math.round(rect.left),
  };
}

const browser = await chromium
  .launch({ channel: "msedge", headless: true })
  .catch(() => chromium.launch({ channel: "chrome", headless: true }));
const report = { contrast: [], focus: [], unreachable: [], order: [], spacing: [] };
try {
  for (const state of states) {
    for (const [name, via] of screens) {
      const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
      await page.addInitScript(() => localStorage.setItem("afterbattle-language", "pl"));
      await page.goto(`http://localhost:${port}/screens.html?state=${state}`);
      await page.locator(".screen, .page").first().waitFor();
      await page.waitForTimeout(700);
      if (via?.startsWith(".")) {
        const row = page.locator(via).first();
        if ((await row.count()) === 0) {
          await page.close();
          continue;
        }
        await row.click();
      } else if (via) await page.getByRole("button", { name: via, exact: true }).click();
      await page.waitForTimeout(900);
      // The review looks at end states: no transitions in the way of the measurements.
      await page.addStyleTag({ content: "*, *::before, *::after { transition: none !important; animation: none !important; }" });
      const where = `${name}/${state}`;
      for (const item of await page.evaluate(contrastFindings)) report.contrast.push({ where, ...item });

      await page.evaluate(() => {
        document.activeElement?.blur();
        window.scrollTo(0, 0);
      });
      const seen = [];
      for (let step = 0; step < 160; step++) {
        await page.keyboard.press("Tab");
        const focus = await page.evaluate(focusVisible);
        if (!focus) continue;
        const key = `${focus.element}|${focus.label}|${focus.top}|${focus.left}`;
        if (seen.some((item) => item.key === key)) break;
        seen.push({ key, ...focus });
      }
      for (const item of seen)
        if (!item.ring) report.focus.push({ where, element: item.element, label: item.label });
      // Inside the main area, a step that goes back up by more than a row is a finding.
      for (let index = 1; index < seen.length; index++) {
        const [previous, next] = [seen[index - 1], seen[index]];
        if (next.left > 110 && previous.left > 110 && next.top < previous.top - 60)
          report.order.push({ where, from: previous.label, to: next.label, up: previous.top - next.top });
      }
      const reachable = new Set(seen.map((item) => `${item.element}|${item.label}`));
      const all = await page.evaluate(() =>
        [...document.querySelectorAll("a[href], button, input, select, textarea, [tabindex]")]
          .filter((element) => {
            if (element.disabled || element.tabIndex < 0) return false;
            if (element.closest("[inert], [aria-hidden='true']")) return false;
            const rect = element.getBoundingClientRect();
            return rect.width > 0 && rect.height > 0;
          })
          .map((element) => ({
            element: `${element.tagName.toLowerCase()}.${[...element.classList].join(".")}`,
            label: (element.getAttribute("aria-label") || element.textContent || element.getAttribute("href") || "").trim().slice(0, 40),
          })),
      );
      for (const item of all)
        if (!reachable.has(`${item.element}|${item.label}`)) report.unreachable.push({ where, ...item });
      await page.close();
    }
  }
} finally {
  await browser.close();
  server.kill();
}

// Spacing on the 4 px grid: margin, padding and gap written in px in base.css.
const css = readFileSync(resolve(root, "src/styles/base.css"), "utf8");
for (const [line, text] of css.split(/\r?\n/).entries()) {
  const match = /^\s*(margin|padding|gap|row-gap|column-gap)[\w-]*:\s*([^;]+);/.exec(text);
  if (!match) continue;
  const off = [...match[2].matchAll(/(-?\d+(?:\.\d+)?)px/g)].map((m) => Number(m[1])).filter((value) => Math.abs(value) > 3 && value % 4 !== 0); // hairlines up to 3 px aside
  if (off.length) report.spacing.push({ line: line + 1, rule: text.trim() });
}
console.log(JSON.stringify(report, null, 1));
