import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync("src/styles/tokens.css", "utf8");

function token(name: string): string {
  const match = new RegExp(`--${name}:\\s*([^;]+);`).exec(css);
  if (!match) throw new Error(`missing token --${name}`);
  return match[1].trim();
}

function luminance(hex: string): number {
  const value = hex.replace("#", "");
  const channels = [0, 2, 4].map((offset) => parseInt(value.slice(offset, offset + 2), 16) / 255);
  const [r, g, b] = channels.map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(foreground: string, background: string): number {
  const [light, dark] = [luminance(foreground), luminance(background)].sort((a, b) => b - a);
  return (light + 0.05) / (dark + 0.05);
}

describe("design tokens (spec 25 §3)", () => {
  it("keeps every text color at contrast 4.5:1 or more on the background", () => {
    for (const name of [
      "text",
      "text-strong",
      "text-body",
      "text-soft",
      "text-2",
      "text-3",
      "accent-link",
      "accent",
      "team-enemy",
      "danger",
    ]) {
      expect(contrast(token(name), token("bg")), name).toBeGreaterThanOrEqual(4.5);
    }
    expect(contrast(token("text-on-accent"), token("accent"))).toBeGreaterThanOrEqual(4.5);
  });

  it("has one accent and no violet", () => {
    expect(token("accent")).toBe("#f08a3c");
    expect(css.toLowerCase()).not.toMatch(/violet|purple|#8b5cf6|#a78bfa|#7c3aed/);
  });

  it("loads only local woff2 files that exist, with no network font source", () => {
    const urls = [...css.matchAll(/url\("([^"]+)"\)/g)].map((match) => match[1]);
    expect(urls.length).toBe(12);
    for (const url of urls) {
      expect(url).toMatch(/^\.\.\/assets\/fonts\/[a-z0-9-]+\.woff2$/);
      expect(existsSync(`src/styles/${url}`), url).toBe(true);
    }
    expect(css).not.toMatch(/https?:|googleapis/);
  });

  it("ships the OFL license for both new families", () => {
    expect(readFileSync("src/assets/fonts/OFL-Inter.txt", "utf8")).toContain(
      "SIL Open Font License, Version 1.1",
    );
    expect(readFileSync("src/assets/fonts/OFL-BigShoulders.txt", "utf8")).toContain(
      "SIL Open Font License, Version 1.1",
    );
  });
});

describe("clickable rows (E5-ui §6)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  it("do not select text, show a pointer and a visible keyboard focus", () => {
    const rule = /\.table-row,\s*\.side-item\s*\{([^}]*)\}/.exec(base)?.[1] ?? "";
    expect(rule).toMatch(/user-select:\s*none/);
    expect(rule).toMatch(/cursor:\s*pointer/);
    // E5-flare: the ring is an inset shadow on ::before that fades in only while focused.
    expect(base).toMatch(
      /\.table-row:focus-visible::before,[^{]*\{\s*opacity:\s*1;\s*transition:\s*opacity/,
    );
    expect(base).toMatch(
      /\.side-item::before,[^{]*\{[^}]*box-shadow:\s*inset 0 0 0 2px var\(--accent-link\)/,
    );
  });
});

describe("names (E5-ui §2)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  it("never wrap: one line with an ellipsis", () => {
    const rule = /\.truncate\s*\{([^}]*)\}/.exec(base)?.[1] ?? "";
    expect(rule).toMatch(/white-space:\s*nowrap/);
    expect(rule).toMatch(/text-overflow:\s*ellipsis/);
    expect(rule).toMatch(/overflow:\s*hidden/);
    expect(rule).toMatch(/min-width:\s*0/);
  });
  it("sit in table columns with a minimum and a maximum width", () => {
    const rule = /\.table-head,\s*\.table-row\s*\{([^}]*)\}/.exec(base)?.[1] ?? "";
    expect(rule).toMatch(/grid-template-columns:[^;]*minmax\(140px, 1\.5fr\)/);
    expect(rule).not.toMatch(/minmax\(0,/);
  });
});

describe("responsive grid (E5-ui §1)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const block = (query: string) => {
    const start = base.indexOf(`@media (${query})`);
    expect(start, query).toBeGreaterThan(-1);
    let depth = 0;
    for (let index = base.indexOf("{", start); index < base.length; index++) {
      if (base[index] === "{") depth++;
      if (base[index] === "}" && --depth === 0) return base.slice(start, index);
    }
    return "";
  };
  it("keeps one column up to 1440 px and widens to 1840 px, centred, above it", () => {
    expect(base).toMatch(/\.page\s*\{[^}]*max-width:\s*1180px/);
    const wide = block("min-width: 1441px");
    expect(wide).toMatch(/\.page\s*\{[^}]*max-width:\s*1840px;[^}]*margin-inline:\s*auto/);
    for (const layout of ["session-layout", "battle-layout", "session-extra"])
      expect(wide, layout).toMatch(new RegExp(`\\.${layout}\\s*\\{[^}]*grid-template-columns`));
    expect(wide).toMatch(/\.col-extra\s*\{\s*display:\s*block/);
    expect(block("min-width: 1920px")).toMatch(/\.tank-layout\s*\{[^}]*grid-template-columns/);
  });
  it("changes only the grid, never the spacing or size tokens", () => {
    const tokens = readFileSync("src/styles/tokens.css", "utf8");
    expect(tokens).toMatch(/--space-8:\s*44px/);
    expect(tokens).toMatch(/--size-hero:\s*104px/);
    expect(block("min-width: 1441px")).not.toMatch(/--space|--size/);
  });
});

describe("motion (E5-flare §7a)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const tokens = readFileSync("src/styles/tokens.css", "utf8");
  const reduced = base.slice(base.indexOf("@media (prefers-reduced-motion: reduce)"));

  it("has motion tokens next to the colors", () => {
    for (const [name, value] of [
      ["motion-fast", "100ms"],
      ["motion-base", "200ms"],
      ["motion-enter", "250ms"],
      ["motion-count", "600ms"],
    ])
      expect(tokens).toMatch(new RegExp(`--${name}:\\s*${value};`));
    expect(tokens).toMatch(/--ease-out:\s*cubic-bezier/);
    expect(tokens).toMatch(/--ease-in:\s*cubic-bezier/);
  });

  it("animates only transform and opacity", () => {
    const transitions = [...base.matchAll(/transition(?:-property)?:\s*([^;]+);/g)].map(
      (m) => m[1],
    );
    expect(transitions.length).toBeGreaterThan(3);
    for (const value of transitions)
      for (const part of value.split(","))
        expect(part.trim().split(/\s+/)[0].replace("!important", ""), value).toMatch(
          /^(transform|opacity)$/,
        );
    const frames = [...base.matchAll(/@keyframes [\w-]+\s*\{((?:[^{}]*\{[^}]*\})*)\s*\}/g)];
    expect(frames.length).toBeGreaterThanOrEqual(5);
    for (const [, body] of frames)
      for (const [, property] of body.matchAll(/([a-z-]+)\s*:/g))
        expect(property).toMatch(/^(transform|opacity)$/);
  });

  it("never loops an animation", () => {
    expect(base).not.toMatch(/infinite/);
    for (const [, value] of base.matchAll(/animation:\s*([^;]+);/g))
      expect(value).not.toMatch(/infinite|alternate/);
  });

  it("turns off movement for prefers-reduced-motion", () => {
    expect(reduced).toMatch(/animation:\s*none\s*!important/);
    expect(reduced).toMatch(/transition-property:\s*opacity\s*!important/);
    expect(reduced).toMatch(/\.tile:hover \.map-backdrop-picture[^{]*\{\s*transform:\s*none/);
  });
});

describe("navigation rail and history rows (E5-flare-fix2)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const tokens = readFileSync("src/styles/tokens.css", "utf8");
  it("keeps the rail in the window on desktop and lets it scroll only on its own", () => {
    const rail = /\n\.rail \{([^}]*)\}/.exec(base)?.[1] ?? "";
    expect(rail).toMatch(/position:\s*sticky/);
    // E5-maps-hd §6: below the custom title bar.
    expect(rail).toMatch(/top:\s*var\(--titlebar-height\)/);
    expect(rail).toMatch(/height:\s*calc\(100vh - var\(--titlebar-height\)\)/);
    expect(rail).toMatch(/overflow-y:\s*auto/);
    expect(base).toMatch(/\.rail-spacer \{\s*flex-grow:\s*1/);
    const narrow = base.slice(base.lastIndexOf("@media (max-width: 760px)"));
    expect(narrow).toMatch(/\.rail \{[^}]*position:\s*static/);
    expect(narrow).toMatch(/\.shell \{[^}]*flex-direction:\s*column/);
  });
  it("enters history rows in 320 ms over 6 px with the ease-out token", () => {
    expect(tokens).toMatch(/--motion-row:\s*320ms;/);
    expect(base).toMatch(
      /\.row-enter \{[^}]*animation:\s*row-in var\(--motion-row\) var\(--ease-out\) both/,
    );
    expect(base).toMatch(
      /@keyframes row-in \{\s*from \{\s*opacity:\s*0;\s*transform:\s*translateY\(6px\)/,
    );
    expect(base).toMatch(/\.screen-still \{\s*animation:\s*none/);
  });
});
