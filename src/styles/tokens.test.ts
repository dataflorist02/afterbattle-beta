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

  it("has the brand palette of spec 29 §3 under new names only", () => {
    const palette: [string, string][] = [
      ["bg-deep", "#070b14"],
      ["surface", "#0d1422"],
      ["navy", "#16223a"],
      ["navy-hi", "#1e3156"],
      ["steel-line", "#8b97ab"],
      ["ember-lo", "#e0661f"],
      ["ember-deep", "#9a3a1a"],
      ["ember-white", "#ffebd0"],
      ["up", "var(--accent-soft)"],
      ["down", "#6e8fc6"],
      ["scroll-thumb", "rgba(139, 151, 171, 0.35)"],
      ["scroll-thumb-hover", "rgba(139, 151, 171, 0.6)"],
      ["scroll-thumb-active", "var(--accent)"],
    ];
    for (const [name, value] of palette) expect(token(name), name).toBe(value);
    // Existing names stay (spec 29 §3): nothing is renamed.
    for (const name of ["accent", "accent-soft", "accent-link", "text-2", "bg", "danger"])
      expect(token(name), name).toBeTruthy();
    // No new token repeats an existing literal: each new color is written once.
    const literals = [...css.matchAll(/--[\w-]+:\s*(#[0-9a-f]{6})\s*;/gi)].map((m) =>
      m[1].toLowerCase(),
    );
    for (const [name, value] of palette.filter(([, value]) => value.startsWith("#")))
      expect(
        literals.filter((literal) => literal === value),
        name,
      ).toHaveLength(1);
    // A falling number reads on the background, and differs from a rising one.
    expect(contrast(token("down"), token("bg"))).toBeGreaterThanOrEqual(4.5);
    expect(token("down")).not.toBe(token("accent-soft"));
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

  it("never loops an animation, except the pulse of a loading screen without progress", () => {
    // Spec 29 §5: with no progress to show, the ember at the base pulses calmly (opacity only).
    const pulse = "animation: ember-pulse 1600ms ease-in-out infinite;";
    expect(base.split("infinite")).toHaveLength(2);
    expect(base).toMatch(
      /\.loading-pulse \.loading-hot \{\s*animation: ember-pulse 1600ms ease-in-out infinite;/,
    );
    for (const [, value] of base.matchAll(/animation:\s*([^;]+);/g))
      if (`animation: ${value};` !== pulse) expect(value).not.toMatch(/infinite|alternate/);
  });

  it("draws the loading screen as spec 29 §5 says", () => {
    expect(base).toMatch(/\.loading-screen\s*\{[^}]*background:\s*var\(--bg-deep\);/);
    expect(base).toMatch(/\.loading-mark\s*\{[^}]*width:\s*clamp\(160px, 22vmin, 280px\);/);
    expect(base).toMatch(
      /\.loading-rise\s*\{[^}]*transform-origin:\s*50% 100%;[^}]*transition:\s*transform 300ms var\(--ease-out\);/,
    );
    expect(base).toMatch(/\.loading-out\s*\{[^}]*opacity:\s*0;[^}]*transition:\s*opacity 200ms/);
    expect(base).toMatch(
      /@keyframes ember-pulse\s*\{\s*0%,\s*100%\s*\{\s*opacity:\s*1;\s*\}\s*50%\s*\{\s*opacity:\s*0\.7;\s*\}\s*\}/,
    );
  });

  it("fades a loaded map picture in without resizing its tile (spec 29 §1)", () => {
    expect(base).toMatch(
      /\.map-backdrop-picture\s*\{\s*position:\s*absolute;\s*inset:\s*0;\s*z-index:\s*-2;/,
    );
    expect(base).toMatch(/\.map-tile\s*\{[^}]*min-height:\s*190px;/);
    const rule =
      /\.map-backdrop\[data-loaded="true"\] \.map-backdrop-picture,\s*\.map-backdrop\[data-loaded="true"\] \.map-backdrop-sharp\s*\{\s*animation:\s*picture-in var\(--motion-base\) var\(--ease-out\) both;\s*\}/;
    expect(base).toMatch(rule);
    expect(base).toMatch(
      /@keyframes picture-in\s*\{\s*from\s*\{\s*opacity:\s*0;\s*\}\s*to\s*\{\s*opacity:\s*1;\s*\}\s*\}/,
    );
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

describe("scrollbar and title bar (spec 29 §4, §7)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");

  it("gives every scrolling area a thin bar in the brand colors, by standard CSS only", () => {
    expect(base).toMatch(
      /\*\s*\{\s*scrollbar-width:\s*thin;\s*scrollbar-color:\s*var\(--scroll-thumb\) transparent;\s*\}/,
    );
    expect(base).toMatch(
      /\*:hover\s*\{\s*scrollbar-color:\s*var\(--scroll-thumb-hover\) transparent;\s*\}/,
    );
    expect(base).toMatch(
      /\*:active\s*\{\s*scrollbar-color:\s*var\(--scroll-thumb-active\) transparent;\s*\}/,
    );
    // Dragging wins over hovering: the :active rule comes after :hover.
    expect(base.indexOf("*:active {")).toBeGreaterThan(base.indexOf("*:hover {"));
    expect(base).not.toMatch(/::-webkit-scrollbar/);
  });

  it("puts the title bar on the brand surface", () => {
    expect(base).toMatch(/\.titlebar\s*\{[^}]*background:\s*var\(--surface\);/);
  });
});

describe("changed numbers (spec 29 §6)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");

  it("count in tabular digits and show the direction by a fading overlay, never by color motion", () => {
    expect(base).toMatch(
      /\.figure-value\s*\{[^}]*position:\s*relative;[^}]*font-variant-numeric:\s*tabular-nums;/,
    );
    expect(base).toMatch(
      /\.figure-delta\s*\{[^}]*position:\s*absolute;[^}]*animation:\s*delta-fade 1400ms linear both;/,
    );
    expect(base).toMatch(/\.figure-up\s*\{\s*color:\s*var\(--up\);\s*\}/);
    expect(base).toMatch(/\.figure-down\s*\{\s*color:\s*var\(--down\);\s*\}/);
    // Visible for 1 s of 1.4 s, then 400 ms of fading.
    expect(base).toMatch(
      /@keyframes delta-fade\s*\{\s*0%,\s*71\.4%\s*\{\s*opacity:\s*1;\s*\}\s*100%\s*\{\s*opacity:\s*0;\s*\}\s*\}/,
    );
  });
});
