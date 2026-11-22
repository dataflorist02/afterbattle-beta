import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { t as translate } from "../../i18n/ui";
import type { DamastSession } from "../../types";
import { PLATE_HEIGHT, PLATE_WIDTH } from "./pattern";
import {
  CARD_COLUMN,
  CARD_FIELD_HEIGHT,
  CARD_HEIGHT,
  CARD_LOGO_SIZE,
  CARD_MARGIN,
  CARD_PLATE,
  CARD_STRIP,
  CARD_WIDTH,
  cardDate,
  cardText,
  drawCard,
  saveCard,
  type CardContext,
} from "./card";
import { MARK_BAR, MARK_OUTLINE } from "../../components/BrandMark";
import { LoadedImage } from "../../test/loadedImage";
import { damastStats } from "./stats";
import { fromSynthetic, type SyntheticSession } from "./synthetic";

const mocks = vi.hoisted(() => ({ renderStill: vi.fn(), exportDamastCard: vi.fn() }));
vi.mock("./engine", () => ({ renderStill: mocks.renderStill }));
vi.mock("../../api", () => ({ api: { exportDamastCard: mocks.exportDamastCard } }));

const SESSIONS = JSON.parse(
  readFileSync("docs/design/damast/sesje-syntetyczne.json", "utf8"),
) as Record<"hot", SyntheticSession>;

/** Synthetic stand-ins for what must never reach a card: a nickname and an account id. */
const NICK = "SyntheticNick_42";
const ACCOUNT = "500123456";

/** The hot session, with the nickname and account id planted in every id it carries. */
function plantedSession(): DamastSession {
  const session = fromSynthetic(`${NICK}-${ACCOUNT}`, SESSIONS.hot);
  return {
    ...session,
    session: { ...session.session, id: `${NICK}-${ACCOUNT}` },
    battles: session.battles.map((battle, index) => ({
      ...battle,
      battleId: `${NICK}-${ACCOUNT}-${index}`,
    })),
  };
}

const tr = (key: Parameters<typeof translate>[1], params?: Record<string, string | number>) =>
  translate("pl", key, params);

/** What a canvas call drew: texts with their position and baseline, images with their box. */
interface Drawn {
  text?: string;
  image?: unknown;
  x: number;
  y: number;
  width?: number;
  height?: number;
  baseline?: string;
  font?: string;
}

/** A 2D context that records every text, image and call. */
function recorder() {
  const texts: string[] = [];
  const calls: string[] = [];
  const drawn: Drawn[] = [];
  const context = new Proxy({} as Record<string | symbol, unknown>, {
    get(target, name) {
      if (name in target) return target[name];
      if (name === "measureText") return (text: string) => ({ width: text.length * 10 });
      if (name === "createRadialGradient") return () => ({ addColorStop() {} });
      if (name === "createImageData")
        return (width: number, height: number) => ({
          data: new Uint8ClampedArray(width * height * 4),
        });
      return (...args: unknown[]) => {
        calls.push(String(name));
        if (name === "fillText") {
          texts.push(String(args[0]));
          drawn.push({
            text: String(args[0]),
            x: args[1] as number,
            y: args[2] as number,
            baseline: target.textBaseline as string,
            font: target.font as string,
          });
        }
        if (name === "drawImage")
          drawn.push({
            image: args[0],
            x: args[1] as number,
            y: args[2] as number,
            width: args[3] as number,
            height: args[4] as number,
          });
      };
    },
    set(target, name, value) {
      target[name] = value;
      return true;
    },
  });
  return { context: context as unknown as CardContext, texts, calls, drawn };
}

const normal = (text: string) => text.replace(/\s/gu, " ");

beforeEach(() => {
  mocks.renderStill.mockImplementation(async (_plate, width: number, height: number) => ({
    id: 1,
    pixels: new Uint8ClampedArray(width * height * 4),
  }));
  mocks.exportDamastCard.mockResolvedValue(true);
});
afterEach(() => {
  vi.restoreAllMocks();
  mocks.renderStill.mockReset();
  mocks.exportDamastCard.mockReset();
});

describe("Damast card (spec 34 §3)", () => {
  it("writes the session's labels and numbers, as on the mockup", () => {
    const damast = plantedSession();
    const text = cardText(damast, damastStats(damast), "pl", tr);
    expect(normal(text.title)).toBe("Sesja 21.11 · 13 bitew");
    expect(normal(text.headline.value)).toBe("2 592");
    expect(text.headline.label).toBe("WN8 sesji");
    expect(text.figures.map((figure) => [normal(figure.value), figure.label])).toEqual([
      ["3 527", "obrażenia"],
      ["8 z 13", "wygranych"],
      ["10,0", "średni tier"],
    ]);
    expect(text.strip).toHaveLength(13);
    expect(text.strip.filter((battle) => battle.best)).toEqual([
      { win: true, tier: "X", best: true },
    ]);
    expect(text.strip.findIndex((battle) => battle.best)).toBe(3);
    expect(text.strip.map((battle) => battle.win)).toEqual(SESSIONS.hot.b.map((b) => b.w === 1));
  });

  it("has no nickname and no account id, neither in its texts nor in what it draws", () => {
    const damast = plantedSession();
    const stats = damastStats(damast);
    const text = cardText(damast, stats, "pl", tr);
    expect(JSON.stringify(text)).not.toContain(NICK);
    expect(JSON.stringify(text)).not.toContain(ACCOUNT);
    const { context, texts } = recorder();
    drawCard(context, text, { plate: {} as CanvasImageSource, logo: {} as CanvasImageSource });
    expect(texts.length).toBeGreaterThan(0);
    for (const written of texts) {
      expect(written).not.toContain(NICK);
      expect(written).not.toContain(ACCOUNT);
    }
    // Labels and numbers only: every text on the card is one of these.
    expect(new Set(texts.map(normal))).toEqual(
      new Set([
        "Sesja 21.11 · 13 bitew",
        "2 592",
        "WN8 sesji",
        "3 527",
        "obrażenia",
        "8 z 13",
        "wygranych",
        "10,0",
        "średni tier",
        "IX",
        "X",
        "XI",
        "AFTERBATTLE",
      ]),
    );
  });

  it("shows damage as the big number without WN8", () => {
    const damast = { ...plantedSession(), wn8: null };
    const text = cardText(damast, damastStats(damast), "en", (key, params) =>
      translate("en", key, params),
    );
    expect(normal(text.headline.value)).toBe("3,527");
    expect(text.headline.label).toBe("damage");
    expect(text.figures.map((figure) => figure.label)).toEqual(["wins", "average tier"]);
    expect(normal(text.title)).toBe("Session 21/11 · 13 battles");
  });

  it("saves a 1200×630 PNG through the backend's save dialog, named by the date only", async () => {
    const damast = plantedSession();
    const stats = damastStats(damast);
    const sizes: [number, number][] = [];
    const drawn = recorder();
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function (
      this: HTMLCanvasElement,
    ) {
      sizes.push([this.width, this.height]);
      return drawn.context;
    } as never);
    // jsdom loads no images: the logo "loads" as soon as it has its address.
    vi.stubGlobal("Image", LoadedImage);
    const png = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 1, 2, 3]);
    vi.spyOn(HTMLCanvasElement.prototype, "toBlob").mockImplementation(function (
      this: HTMLCanvasElement,
      callback: BlobCallback,
      type?: string,
    ) {
      expect(type).toBe("image/png");
      expect([this.width, this.height]).toEqual([CARD_WIDTH, CARD_HEIGHT]);
      callback(new Blob([png]));
    });
    await expect(saveCard(damast, stats, cardText(damast, stats, "pl", tr))).resolves.toBe(true);
    expect(sizes).toEqual([
      [CARD_PLATE.width, CARD_PLATE.height],
      [CARD_WIDTH, CARD_HEIGHT],
    ]);
    expect(mocks.renderStill).toHaveBeenCalledWith(
      stats.plate,
      CARD_PLATE.width,
      CARD_PLATE.height,
      "pixels",
      1,
      CARD_FIELD_HEIGHT,
    );
    expect(mocks.exportDamastCard).toHaveBeenCalledWith(cardDate(damast.session.startedAt), png);
    expect(cardDate(damast.session.startedAt)).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    for (const written of drawn.texts)
      expect(written).not.toMatch(new RegExp(`${NICK}|${ACCOUNT}`));
    // E14 polish, point 1: the logo is the app's BrandMark, drawn as an image from its SVG.
    const images = drawn.drawn.filter((item) => item.image !== undefined);
    const logo = images.find((item) =>
      String((item.image as { src?: string }).src ?? "").startsWith("data:image/svg+xml"),
    );
    expect(logo).toBeDefined();
    const svg = decodeURIComponent(String((logo!.image as { src: string }).src).split(",")[1]);
    expect(svg).toContain(`points="${MARK_OUTLINE}"`);
    expect(svg).toContain(`points="${MARK_BAR}"`);
    expect(drawn.texts).toContain("AFTERBATTLE");
    // The temporary triangle of the mockup is gone.
    expect(drawn.calls).not.toContain("moveTo");
    vi.unstubAllGlobals();
  });
});

describe("the card's layout (E14 polish, point 2)", () => {
  function drawHot() {
    const damast = plantedSession();
    const stats = damastStats(damast);
    const text = cardText(damast, stats, "pl", tr);
    const drawn = recorder();
    const plate = { plate: true } as unknown as CanvasImageSource;
    const logo = { logo: true } as unknown as CanvasImageSource;
    drawCard(drawn.context, text, { plate, logo });
    return { text, drawn: drawn.drawn, plate, logo };
  }
  const near = (actual: number, expected: number) =>
    expect(Math.abs(actual - expected), `${actual} vs ${expected}`).toBeLessThanOrEqual(2);

  it("fills the card's inner height with the plate and its bar, 48 px all round, no empty bands", () => {
    expect(CARD_MARGIN).toBe(48);
    expect(CARD_PLATE.x).toBe(CARD_MARGIN);
    // Author, after E14 polish: the plate with its bar from 48 px to 582 px (630 − 48).
    expect(CARD_PLATE.y).toBe(48);
    expect(CARD_STRIP.y + CARD_STRIP.height).toBe(CARD_HEIGHT - CARD_MARGIN);
    expect(CARD_STRIP.y + CARD_STRIP.height).toBe(582);
    expect(CARD_COLUMN.width).toBe(380);
    expect(CARD_COLUMN.x + CARD_COLUMN.width).toBe(CARD_WIDTH - CARD_MARGIN);
    expect(CARD_COLUMN.y).toBe(CARD_PLATE.y);
    expect(CARD_COLUMN.y + CARD_COLUMN.height).toBe(CARD_STRIP.y + CARD_STRIP.height);
    // The width stays; the plate is a longer cut of the same field, not a stretched image:
    // the field's height grows with the plate's at the scale of its width.
    expect(CARD_PLATE.width).toBe(676);
    expect(CARD_FIELD_HEIGHT).toBeCloseTo((PLATE_WIDTH * CARD_PLATE.height) / CARD_PLATE.width, 9);
    expect(CARD_FIELD_HEIGHT).toBeGreaterThan(PLATE_HEIGHT);
  });

  it("starts the column's first line on the plate's top and ends the logo on the bar's bottom", () => {
    const { text, drawn, plate, logo } = drawHot();
    const plateBox = drawn.find((item) => item.image === plate)!;
    const logoBox = drawn.find((item) => item.image === logo)!;
    const title = drawn.find((item) => item.text === text.title)!;
    expect(title.baseline).toBe("top");
    near(title.y, plateBox.y);
    near(title.x, CARD_COLUMN.x);
    const stripBottom = CARD_STRIP.y + CARD_STRIP.height;
    near(logoBox.y + logoBox.height!, stripBottom);
    expect([logoBox.width, logoBox.height]).toEqual([CARD_LOGO_SIZE, CARD_LOGO_SIZE]);
    near(logoBox.x, CARD_COLUMN.x);
    // The tiers end on the bar's bottom line.
    const tiers = drawn.filter((item) => item.text !== undefined && /^(IX|X|XI)$/u.test(item.text));
    expect(tiers.length).toBe(text.strip.length);
    for (const tier of tiers) {
      expect(tier.baseline).toBe("bottom");
      near(tier.y, stripBottom);
    }
  });

  it("centres the block of numbers between the title line and the logo (author, 22.11)", () => {
    const { text, drawn, logo } = drawHot();
    const at = (value: string) => drawn.find((item) => item.text === value)!;
    /** Every text of the column is drawn from its top: its bottom is y + the font's size. */
    const bottomOf = (item: Drawn) => {
      expect(item.baseline).toBe("top");
      return item.y + Number(/(\d+)px/u.exec(item.font ?? "")?.[1]);
    };
    const titleBottom = bottomOf(at(text.title));
    const blockTop = at(text.headline.value).y;
    const blockBottom = Math.max(...text.figures.map((figure) => bottomOf(at(figure.label))));
    const logoTop = drawn.find((item) => item.image === logo)!.y;
    const above = blockTop - titleBottom;
    const below = logoTop - blockBottom;
    near(above, below);
    // Truly in the middle of the free space, not glued to the title.
    expect(above).toBeGreaterThan(80);
  });

  it("stacks title, WN8 (96 px), its label, then three figures (40 px) in a grid of 3", () => {
    const { text, drawn } = drawHot();
    const at = (value: string) => drawn.find((item) => item.text === value)!;
    const title = at(text.title);
    const headline = at(text.headline.value);
    const headlineLabel = at(text.headline.label);
    expect(headline.font).toMatch(/96px/);
    expect(headline.y).toBeGreaterThan(title.y);
    expect(headlineLabel.y).toBeGreaterThan(headline.y);
    const figures = text.figures.map((figure) => [at(figure.value), at(figure.label)]);
    const third = CARD_COLUMN.width / 3;
    figures.forEach(([value, label], index) => {
      expect(value.font).toMatch(/40px/);
      near(value.x, CARD_COLUMN.x + index * third);
      expect(label.x).toBe(value.x);
      expect(value.y).toBeGreaterThan(headlineLabel.y);
      expect(label.y).toBeGreaterThan(value.y);
      expect(value.baseline).toBe("top");
    });
  });
});
