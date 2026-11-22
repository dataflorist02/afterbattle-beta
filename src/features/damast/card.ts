/**
 * The Damast card (spec 34 §3): 1200×630 PNG with the plate, the battle bar and the session's
 * numbers, saved by the player through the save dialog. Nothing is sent anywhere. The card
 * holds labels and numbers only, built from the session's numbers alone: no nickname, no
 * account id, no tank or map names (the backend also strips any text chunk from the PNG).
 */
import { api } from "../../api";
import type { UiKey } from "../../i18n/ui";
import { formatNumber, formatTier } from "../../lib/format";
import type { DamastSession, Language } from "../../types";
import { renderStill } from "./engine";
import { CARD_WORDMARK, loadBrandLogo } from "./logo";
import { PLATE_WIDTH } from "./pattern";
import type { DamastStats } from "./stats";

export const CARD_WIDTH = 1200;
export const CARD_HEIGHT = 630;
/**
 * The card's layout (E14 polish, point 2), the same pattern as the Damast section on Session:
 * the plate with its battle bar on the left, a column of numbers on the right, exactly as tall
 * as the plate and the bar together. 48 px margins all round: the plate with its bar fills the
 * card's inner height (author, after E14 polish), a longer cut of the same pattern field.
 */
export const CARD_MARGIN = 48;
const CARD_GAP = 48;
const COLUMN_WIDTH = 380;
const PLATE_W = CARD_WIDTH - 2 * CARD_MARGIN - COLUMN_WIDTH - CARD_GAP;
/** The battle bar under the plate: dots, then the tiers ending on its bottom line. */
const STRIP_GAP = 10;
const STRIP_H = 36;
const TOP = CARD_MARGIN;
const BLOCK_H = CARD_HEIGHT - 2 * CARD_MARGIN;
const PLATE_H = BLOCK_H - STRIP_GAP - STRIP_H;

/** The plate on the card, in the plate's own proportions. */
export const CARD_PLATE = { x: CARD_MARGIN, y: TOP, width: PLATE_W, height: PLATE_H };
/**
 * The pattern field the card's plate shows: as wide as every plate (940 units), as tall as
 * the plate needs at the scale of its width. Not stretched: the same field, a longer cut.
 */
export const CARD_FIELD_HEIGHT = (PLATE_WIDTH * PLATE_H) / PLATE_W;
/** The battle bar, under the plate. */
export const CARD_STRIP = {
  x: CARD_MARGIN,
  y: TOP + PLATE_H + STRIP_GAP,
  width: PLATE_W,
  height: STRIP_H,
};
/** The column of numbers: from the plate's top to the bar's bottom. */
export const CARD_COLUMN = {
  x: CARD_MARGIN + PLATE_W + CARD_GAP,
  y: TOP,
  width: COLUMN_WIDTH,
  height: BLOCK_H,
};

/** The card's colours: the app's tokens (tokens.css), read when the card is drawn. */
export interface CardPalette {
  bg: string;
  /** The tier under the spark (--ember-core). */
  core: string;
  text: string;
  soft: string;
  muted: string;
  accent: string;
  glow: string;
}

export function cardPalette(): CardPalette {
  const style = typeof document !== "undefined" ? getComputedStyle(document.documentElement) : null;
  const token = (name: string) => style?.getPropertyValue(name).trim() ?? "";
  return {
    bg: token("--bg"),
    core: token("--ember-core"),
    text: token("--text"),
    soft: token("--text-soft"),
    muted: token("--text-2"),
    accent: token("--ember-hot") || token("--accent"),
    glow: token("--ember-glow"),
  };
}
const NUMBER_FONT = '"Big Shoulders", Inter, system-ui, sans-serif';
const TEXT_FONT = "Inter, system-ui, sans-serif";

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

export interface CardText {
  /** "Sesja 21.11 · 13 bitew". */
  title: string;
  /** The big number: the session's WN8, or its damage without WN8. */
  headline: { value: string; label: string };
  figures: { value: string; label: string }[];
  strip: { win: boolean; tier: string; best: boolean }[];
  warmer: boolean;
}

/** "13 bitew", "2 bitwy", "1 bitwa"; "13 battles" (Polish plural forms). */
export function battlesLabel(count: number, language: Language, tr: Translate): string {
  const form = new Intl.PluralRules(language === "pl" ? "pl-PL" : "en-GB").select(count);
  const key: UiKey =
    form === "one" ? "damastBattlesOne" : form === "few" ? "damastBattlesFew" : "damastBattlesMany";
  return tr(key, { count });
}

/** The average tier with one decimal: "10,0". */
export function formatAverageTier(tier: number | null, language: Language): string {
  if (tier === null) return "—";
  return new Intl.NumberFormat(language === "pl" ? "pl-PL" : "en-GB", {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  }).format(tier);
}

/** The date of the file name, local: "2026-11-21". */
export function cardDate(timestamp: number): string {
  const date = new Date(timestamp);
  const two = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
}

/** Everything written on the card: the session's labels and numbers, nothing else. */
export function cardText(
  damast: DamastSession,
  stats: DamastStats,
  language: Language,
  tr: Translate,
): CardText {
  const date = new Intl.DateTimeFormat(language === "pl" ? "pl-PL" : "en-GB", {
    day: "2-digit",
    month: "2-digit",
  }).format(new Date(damast.session.startedAt));
  const damage = {
    value: formatNumber(damast.averageDamage, language),
    label: tr("damastDamage"),
  };
  const wins = {
    value: tr("figureOf", {
      value: formatNumber(stats.wins, language),
      count: formatNumber(stats.count, language),
    }),
    label: tr("figureWins"),
  };
  const tier = { value: formatAverageTier(stats.averageTier, language), label: tr("damastTier") };
  const withWn8 = damast.wn8 !== null;
  return {
    title: tr("damastCardTitle", { date, battles: battlesLabel(stats.count, language, tr) }),
    headline: withWn8
      ? { value: formatNumber(damast.wn8, language), label: tr("sessionWn8") }
      : damage,
    figures: withWn8 ? [damage, wins, tier] : [wins, tier],
    strip: stats.plate.map((battle, index) => ({
      win: battle.win,
      tier: formatTier(Math.round(battle.tier)) ?? "",
      best: index === stats.best,
    })),
    warmer: stats.heat?.temperature === "warmer",
  };
}

/** The canvas calls the card uses (a recording stand-in in tests). */
export type CardContext = Pick<
  CanvasRenderingContext2D,
  | "fillStyle"
  | "globalAlpha"
  | "strokeStyle"
  | "lineWidth"
  | "font"
  | "textAlign"
  | "textBaseline"
  | "fillRect"
  | "fillText"
  | "measureText"
  | "beginPath"
  | "arc"
  | "fill"
  | "stroke"
  | "moveTo"
  | "lineTo"
  | "closePath"
  | "save"
  | "restore"
  | "clip"
  | "roundRect"
  | "drawImage"
  | "createRadialGradient"
  | "shadowColor"
  | "shadowBlur"
  | "letterSpacing"
>;

/** The pictures on the card: the finished plate (CARD_PLATE's size) and the app's logo. */
export interface CardImages {
  plate: CanvasImageSource;
  logo: CanvasImageSource;
}

/** The monogram's size on the card, in pixels. */
export const CARD_LOGO_SIZE = 40;
/** The title line "Sesja DD.MM · N bitew" (font size, drawn from its top). */
const TITLE_SIZE = 20;
/**
 * The block of numbers, top to bottom: WN8 (96 px), its label (18 px) at 102, the three figures
 * (40 px) at 152 and their labels (15 px) at 198, ending at 213.
 */
const NUMBERS = { label: 102, figures: 152, figureLabels: 198, height: 213 };

/** Draws the whole card. */
export function drawCard(
  ctx: CardContext,
  text: CardText,
  { plate, logo }: CardImages,
  PALETTE: CardPalette = cardPalette(),
) {
  ctx.fillStyle = PALETTE.bg;
  ctx.fillRect(0, 0, CARD_WIDTH, CARD_HEIGHT);
  if (text.warmer) {
    const glow = ctx.createRadialGradient(300, CARD_HEIGHT, 0, 300, CARD_HEIGHT, 460);
    glow.addColorStop(0, PALETTE.glow);
    glow.addColorStop(1, "transparent");
    ctx.fillStyle = glow;
    ctx.globalAlpha = 0.3;
    ctx.fillRect(0, 0, CARD_WIDTH, CARD_HEIGHT);
    ctx.globalAlpha = 1;
  }

  const { x, y, width, height } = CARD_PLATE;
  ctx.save();
  ctx.beginPath();
  ctx.roundRect(x, y, width, height, 20);
  ctx.clip();
  ctx.drawImage(plate, x, y, width, height);
  ctx.restore();

  // The battle bar: a full dot for a win, an empty one otherwise, the tier below on the bar's
  // bottom line; the best battle's spark in ember with a soft glow, as on Session.
  const stripBottom = CARD_STRIP.y + CARD_STRIP.height;
  const count = Math.max(1, text.strip.length);
  const step = CARD_STRIP.width / count;
  ctx.textAlign = "center";
  text.strip.forEach((battle, index) => {
    const cx = CARD_STRIP.x + step * (index + 0.5);
    const cy = CARD_STRIP.y + 8;
    const color = battle.best ? PALETTE.accent : PALETTE.soft;
    ctx.shadowColor = battle.best ? PALETTE.glow : "transparent";
    ctx.shadowBlur = battle.best ? 8 : 0;
    ctx.beginPath();
    ctx.arc(cx, cy, battle.best ? 5.6 : 5, 0, Math.PI * 2);
    if (battle.win) {
      ctx.fillStyle = color;
      ctx.fill();
    } else {
      ctx.strokeStyle = color;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    ctx.shadowColor = "transparent";
    ctx.shadowBlur = 0;
    ctx.fillStyle = battle.best ? PALETTE.core : PALETTE.muted;
    ctx.font = `500 13px ${TEXT_FONT}`;
    ctx.textBaseline = "bottom";
    ctx.fillText(battle.tier, cx, stripBottom);
  });

  // The column: the title on the plate's top line, the logo ending on the bar's bottom line,
  // and between them the block of numbers (WN8 and its label, three figures in a grid of
  // three) in the middle of the free space (author, 22.11).
  const column = CARD_COLUMN;
  const left = column.x;
  const bottom = column.y + column.height;
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  ctx.fillStyle = PALETTE.muted;
  ctx.font = `400 ${TITLE_SIZE}px ${TEXT_FONT}`;
  ctx.fillText(text.title, left, column.y);
  const free = bottom - CARD_LOGO_SIZE - (column.y + TITLE_SIZE);
  const top = Math.round(column.y + TITLE_SIZE + (free - NUMBERS.height) / 2);
  ctx.fillStyle = PALETTE.text;
  ctx.font = `300 96px ${NUMBER_FONT}`;
  ctx.fillText(text.headline.value, left, top);
  ctx.fillStyle = PALETTE.soft;
  ctx.font = `400 18px ${TEXT_FONT}`;
  ctx.fillText(text.headline.label, left, top + NUMBERS.label);
  const third = column.width / 3;
  text.figures.forEach((figure, index) => {
    const at = left + index * third;
    ctx.fillStyle = PALETTE.text;
    ctx.font = `300 40px ${NUMBER_FONT}`;
    ctx.fillText(figure.value, at, top + NUMBERS.figures);
    ctx.fillStyle = PALETTE.muted;
    ctx.font = `400 15px ${TEXT_FONT}`;
    ctx.fillText(figure.label, at, top + NUMBERS.figureLabels);
  });

  // The brand (E14 polish): the app's monogram (BrandMark, as in the navigation) and its name.
  ctx.drawImage(logo, left, bottom - CARD_LOGO_SIZE, CARD_LOGO_SIZE, CARD_LOGO_SIZE);
  ctx.fillStyle = PALETTE.text;
  ctx.font = `300 30px ${NUMBER_FONT}`;
  ctx.textBaseline = "middle";
  ctx.letterSpacing = "0.12em";
  ctx.fillText(CARD_WORDMARK, left + CARD_LOGO_SIZE + 14, bottom - CARD_LOGO_SIZE / 2);
  ctx.letterSpacing = "0px";
}

/** Waits for the app's own fonts, so the card is drawn with them and not a fallback. */
async function fontsReady() {
  const fonts = typeof document !== "undefined" ? document.fonts : undefined;
  if (!fonts) return;
  await Promise.all(
    [`300 96px ${NUMBER_FONT}`, `300 30px ${NUMBER_FONT}`, `400 20px ${TEXT_FONT}`].map((font) =>
      fonts.load(font).catch(() => []),
    ),
  );
}

/** The card as PNG bytes: the plate from the worker, the rest drawn on a 1200×630 canvas. */
export async function cardPng(stats: DamastStats, text: CardText): Promise<Uint8Array> {
  const { width, height } = CARD_PLATE;
  const [still, logo] = await Promise.all([
    renderStill(stats.plate, width, height, "pixels", 1, CARD_FIELD_HEIGHT),
    loadBrandLogo(CARD_LOGO_SIZE),
    fontsReady(),
  ]);
  if (!still.pixels) throw new Error("damast: the plate was not drawn");
  const plate = document.createElement("canvas");
  plate.width = width;
  plate.height = height;
  const plateContext = plate.getContext("2d");
  if (!plateContext) throw new Error("damast: no 2d context");
  const image = plateContext.createImageData(width, height);
  image.data.set(still.pixels);
  plateContext.putImageData(image, 0, 0);

  const card = document.createElement("canvas");
  card.width = CARD_WIDTH;
  card.height = CARD_HEIGHT;
  const context = card.getContext("2d");
  if (!context) throw new Error("damast: no 2d context");
  drawCard(context, text, { plate, logo });
  const blob = await new Promise<Blob | null>((resolve) => card.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("damast: the card was not encoded");
  return new Uint8Array(await blob.arrayBuffer());
}

/** "Zapisz kartę": draws the card and hands it to the save dialog; false when cancelled. */
export async function saveCard(
  damast: DamastSession,
  stats: DamastStats,
  text: CardText,
): Promise<boolean> {
  const png = await cardPng(stats, text);
  return api.exportDamastCard(cardDate(damast.session.startedAt), png);
}
