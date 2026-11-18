import { existsSync, readFileSync, readdirSync } from "node:fs";
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

  it("animates only transform and opacity (and the height of an opening summary)", () => {
    const transitions = [...base.matchAll(/transition(?:-property)?:\s*([^;]+);/g)].map(
      (m) => m[1],
    );
    expect(transitions.length).toBeGreaterThan(3);
    for (const value of transitions)
      for (const part of value.split(","))
        expect(part.trim().split(/\s+/)[0].replace("!important", ""), value).toMatch(
          /^(transform|opacity|grid-template-rows)$/,
        );
    // Spec 31 §3: the one height animation is the summary card's rest, in the enter time.
    const heights = [...base.matchAll(/([^{}]+)\{[^}]*transition:\s*grid-template-rows[^;]*;/g)];
    expect(heights.map((match) => match[1].trim())).toEqual([".recap-rest"]);
    // The opened sentences carry no numbers, like the first one (author, 2026-11-17).
    expect(base).toMatch(/\.recap-list\s*\{[^}]*list-style:\s*none;[^}]*padding-left:\s*0;/);
    expect(base).toMatch(
      /\.recap-rest\s*\{[^}]*transition:\s*grid-template-rows var\(--motion-enter\) var\(--ease-out\);/,
    );
    expect(base).toMatch(
      /\.recap-more\[aria-expanded="true"\] svg\s*\{\s*transform:\s*rotate\(180deg\)/,
    );
    const frames = [...base.matchAll(/@keyframes [\w-]+\s*\{((?:[^{}]*\{[^}]*\})*)\s*\}/g)];
    expect(frames.length).toBeGreaterThanOrEqual(5);
    for (const [, body] of frames)
      for (const [, property] of body.matchAll(/([a-z-]+)\s*:/g))
        expect(property).toMatch(/^(transform|opacity)$/);
  });

  it("never loops an animation, except the pulse of a loading screen without progress", () => {
    // Spec 29 §5: with no progress to show, the ember at the base pulses calmly (opacity only).
    const pulse = "animation: ember-pulse var(--motion-pulse) var(--ease-in-out) infinite;";
    expect(tokens).toMatch(/--motion-pulse:\s*1600ms;/);
    expect(base.split("infinite")).toHaveLength(2);
    expect(base).toMatch(
      /\.loading-pulse \.loading-hot \{\s*animation: ember-pulse var\(--motion-pulse\) var\(--ease-in-out\) infinite;/,
    );
    for (const [, value] of base.matchAll(/animation:\s*([^;]+);/g))
      if (`animation: ${value};` !== pulse) expect(value).not.toMatch(/infinite|alternate/);
  });

  it("draws the loading screen as spec 29 §5 says", () => {
    expect(base).toMatch(/\.loading-screen\s*\{[^}]*background:\s*var\(--bg-deep\);/);
    expect(base).toMatch(/\.loading-mark\s*\{[^}]*width:\s*clamp\(160px, 22vmin, 280px\);/);
    expect(base).toMatch(
      /\.loading-rise\s*\{[^}]*transform-origin:\s*50% 100%;[^}]*transition:\s*transform var\(--motion-progress\) var\(--ease-out\);/,
    );
    expect(base).toMatch(
      /\.loading-out\s*\{[^}]*opacity:\s*0;[^}]*transition:\s*opacity var\(--motion-base\)/,
    );
    expect(tokens).toMatch(/--motion-progress:\s*300ms;/);
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
      /\.figure-delta\s*\{[^}]*position:\s*absolute;[^}]*animation:\s*delta-fade var\(--motion-delta\) var\(--ease-linear\) both;/,
    );
    expect(base).toMatch(/\.figure-up\s*\{\s*color:\s*var\(--up\);\s*\}/);
    expect(base).toMatch(/\.figure-down\s*\{\s*color:\s*var\(--down\);\s*\}/);
    // Visible for 1 s of 1.4 s, then 400 ms of fading.
    expect(base).toMatch(
      /@keyframes delta-fade\s*\{\s*0%,\s*71\.4%\s*\{\s*opacity:\s*1;\s*\}\s*100%\s*\{\s*opacity:\s*0;\s*\}\s*\}/,
    );
  });
});

describe("snappy motion (spec 31 §2)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const tokens = readFileSync("src/styles/tokens.css", "utf8");
  const ms = (name: string) => {
    const match = new RegExp(`--${name}:\\s*(\\d+)ms;`).exec(tokens);
    if (!match) throw new Error(`missing time token --${name}`);
    return Number(match[1]);
  };

  it("keeps the thresholds of the table in the tokens", () => {
    expect(ms("motion-base")).toBeGreaterThanOrEqual(150);
    expect(ms("motion-base")).toBeLessThanOrEqual(200);
    expect(ms("motion-hover")).toBeGreaterThanOrEqual(120);
    expect(ms("motion-hover")).toBeLessThanOrEqual(150);
    expect(ms("motion-enter")).toBeLessThanOrEqual(250);
    expect(ms("motion-count")).toBe(600);
    expect(ms("motion-instant")).toBe(0);
    expect(ms("motion-skeleton-delay")).toBe(300);
  });

  it("writes no time or curve outside the tokens, in CSS and in components", () => {
    const declarations = [
      ...base.matchAll(/(?:transition|animation)(?:-[a-z-]+)?:\s*([^;]+);/g),
    ].map((match) => match[1]);
    expect(declarations.length).toBeGreaterThan(10);
    for (const value of declarations) {
      const literal = value.replace(/var\(--[\w-]+(?:,\s*[^)]*)?\)/g, "");
      expect(literal, value).not.toMatch(
        /\d(?:ms|s)\b|cubic-bezier|steps\(|\bease\b|ease-|\blinear\b/,
      );
    }
    const walk = (dir: string): string[] =>
      readdirSync(`src/${dir}`).flatMap((name) => {
        const path = dir ? `${dir}/${name}` : name;
        return /\.\w+$/.test(name) ? [path] : walk(path);
      });
    const files = walk("").filter(
      (file) => /\.(tsx|css)$/.test(file) && !/test|tokens\.css$/.test(file),
    );
    expect(files.length).toBeGreaterThan(20);
    for (const file of files) {
      const text = readFileSync(`src/${file}`, "utf8");
      expect(text, file).not.toMatch(/cubic-bezier/);
      if (file.endsWith(".tsx")) expect(text, file).not.toMatch(/\b\d+\s?ms\b|\d(?:ms|s)["'`]/);
    }
  });

  it("moves a screen in by a fade with a rise of at most 8 px in the base time", () => {
    const frames = /@keyframes screen-in\s*\{\s*from\s*\{([^}]*)\}/.exec(base)?.[1] ?? "";
    const rise = Number(/translateY\((\d+)px\)/.exec(frames)?.[1]);
    expect(frames).toMatch(/opacity:\s*0/);
    expect(rise).toBeGreaterThan(0);
    expect(rise).toBeLessThanOrEqual(8);
    expect(base).toMatch(/\.screen\s*\{[^}]*animation:\s*screen-in var\(--motion-base\)/);
  });

  it("presses buttons, rows and tiles at once: no transition on :active, scale of at most 2%", () => {
    const pressed =
      /([^{}]*:active[^{}]*)\{([^}]*transition-duration:\s*var\(--motion-instant\)[^}]*)\}/.exec(
        base.replace(/\/\*[\s\S]*?\*\//g, ""),
      );
    expect(pressed, "an :active rule without delay").not.toBeNull();
    const selectors = (pressed?.[1] ?? "").split(",").map((part) => part.trim());
    for (const selector of [
      ".button:active",
      ".segment:active",
      ".tile:active",
      ".hrow:active",
      ".recap-more:active",
    ])
      expect(selectors, selector).toContain(selector);
    const scale = Number(/transform:\s*scale\(([\d.]+)\)/.exec(pressed?.[2] ?? "")?.[1]);
    expect(scale).toBeGreaterThanOrEqual(0.98);
    expect(scale).toBeLessThan(1);
    // Rows of tables and side lists change their background on press (no transition on it).
    expect(base).toMatch(/\.table-row:active,\s*\.side-item:active\s*\{\s*background:/);
  });

  it("keeps the footer below the window while a screen loads (no layout shift, CLS 0)", () => {
    // The footer would otherwise sit at the bottom of the window and jump down with the data.
    expect(base).toMatch(/\n\.page \{[^}]*min-height:\s*calc\(100vh - var\(--titlebar-height\)\);/);
  });

  it("fades the focus ring in the hover time", () => {
    expect(base).toMatch(
      /:focus-visible::before\s*\{\s*opacity:\s*1;\s*transition:\s*opacity var\(--motion-hover\)/,
    );
  });

  it("shows a loading skeleton only after 300 ms, in a box of a fixed height", () => {
    expect(base).toMatch(/\.skeleton\s*\{[^}]*height:\s*120px;/);
    expect(base).toMatch(
      /\.skeleton\s*\{[^}]*animation:\s*skeleton-in var\(--motion-fast\) var\(--ease-out\) var\(--motion-skeleton-delay\) both;/,
    );
    expect(base).toMatch(/@keyframes skeleton-in\s*\{\s*from\s*\{\s*opacity:\s*0;/);
  });
});

describe("settings layout (spec 31 §4)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const rule = (selector: string) => {
    const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`(?:^|\\})\\s*${escaped}\\s*\\{([^}]*)\\}`, "m").exec(base)?.[1] ?? "";
  };

  it("switches move in the hover time and 'Zapisano.' fades out", () => {
    // The knob slides (transform), the accent fill fades in (opacity): only these two move.
    expect(rule(".switch::before")).toMatch(/transition:\s*opacity var\(--motion-hover\)/);
    expect(rule(".switch::after")).toMatch(/transition:\s*transform var\(--motion-hover\)/);
    expect(rule(".setting-saved")).toMatch(/transition:\s*opacity var\(--motion-hover\)/);
    expect(rule('.setting-saved[data-shown="false"]')).toMatch(/opacity:\s*0/);
  });

  it("lists the groups on the left in a wide window and above them in a narrow one", () => {
    expect(base).toMatch(
      /@media \(min-width: 1100px\)\s*\{[^@]*\.settings-layout\s*\{[^}]*grid-template-columns:/,
    );
    expect(rule(".settings-nav")).toMatch(/position:\s*sticky/);
    // A link to a group stops below the title bar, not under it.
    expect(rule(".settings-group")).toMatch(/scroll-margin-top:/);
    // Author's fixup after STOP 1 (RC-Ustawienia): no red rule or title, only the button warns.
    expect(base).not.toMatch(/\.setting-danger/);
    expect(rule(".button-danger")).toMatch(/color:\s*var\(--danger\)/);
    expect(rule(".button-danger")).toMatch(/border-color:\s*var\(--danger\)/);
    // The group in view is marked in the list of links.
    expect(rule('.settings-nav a[aria-current="true"]')).toMatch(/color:\s*var\(--text\)/);
  });
});

describe("spacing and radii (spec 31 §5)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const declarations = (property: RegExp) =>
    base
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => property.test(line));

  it("puts margins, paddings and gaps on the 4 px grid (hairlines up to 3 px aside)", () => {
    for (const line of declarations(/^(margin|padding|gap|row-gap|column-gap)[\w-]*:/))
      for (const [, value] of line.matchAll(/(-?\d+(?:\.\d+)?)px/g)) {
        const px = Math.abs(Number(value));
        if (px <= 3) continue;
        expect(px % 4, line).toBe(0);
      }
  });

  it("takes radii from the tokens, except thin bars and pills", () => {
    for (const line of declarations(/^border-radius:/)) {
      const value = line.replace(/^border-radius:\s*|;$/g, "");
      if (/var\(--radius-[\w-]+\)|^inherit$|^50%$|^999px$/.test(value)) continue;
      // A bar or a swatch a few pixels high is rounded by half its height.
      for (const [, px] of value.matchAll(/(\d+)px/g))
        expect(Number(px), line).toBeLessThanOrEqual(6);
    }
  });
});

describe("A-1 comparison (spec 32 §3)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");
  const rules = [...base.matchAll(/([^{}]*\.ref-[^{}]*)\{([^}]*)\}/g)];

  it("draws the bars, the median and the player's marker, and keeps them still", () => {
    const selectors = rules.map((match) => match[1].trim()).join("\n");
    for (const part of [".ref-track", ".ref-range", ".ref-median", ".ref-p90", ".ref-own"])
      expect(selectors).toContain(part);
    for (const [, selector, body] of rules) {
      expect(body, selector).not.toMatch(/transition|animation/);
    }
    // A highlighted number and its mark are shown without motion.
    expect(selectors).toMatch(/\.is-active/);
  });
});

describe("Overview (spec 32 §4)", () => {
  const base = readFileSync("src/styles/base.css", "utf8");

  it("lays the blocks out in rows of two in a wide window, rows that press at once", () => {
    // RC-Przeglad (fixup 2026-11-18): the session card on the whole width, then rows of two
    // blocks stretched to the same height, so the page has no holes.
    expect(base).toMatch(/\.overview-row\s*\{[^}]*display:\s*grid;[^}]*align-items:\s*stretch;/);
    expect(base).toMatch(
      /@media \(min-width: 1100px\)\s*\{[^@]*\.overview-row\s*\{[^}]*grid-template-columns:\s*repeat\(2,/,
    );
    expect(base).toMatch(/\.overview-cells\s*\{[^}]*grid-auto-flow:\s*dense;/);
    expect(base).toMatch(
      /\.overview-link:active[^{]*\{[^}]*transition-duration:\s*var\(--motion-instant\)/,
    );
  });
});
