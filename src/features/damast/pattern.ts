/**
 * Damast (spec 34 §2, §4): the pattern of a plate, deterministic from the session's battles.
 * The algorithm and its parameters are those of the mockup's reference
 * (`docs/design/damast/wzor-referencyjny.js`, Damast-Kuznia): battles from left to right, each
 * with its zone; the zone's tint from r (warm above 1, cool below), one eye per win, the density
 * of the layers from the tier, the wave's amplitude from damage on the tier, and the stages of
 * forging over p (molten metal pours in, the pattern is forged, the steel cools to its tint).
 *
 * The one difference from the reference: the layer edges are smoothed over one pixel (spec 34
 * §4, smoothstep on the edge, from the derivative of the layer coordinate like `fwidth` in the
 * shader), so there are no stairs and no moiré. `smooth: false` gives the reference's image.
 * The WebGL2 shader (`shader.glsl`) and the Web Worker run this same algorithm.
 */
import type { PlateBattle } from "./stats";

/** Version of the algorithm: a new value invalidates every cached plate (spec 34 §4). */
export const DAMAST_VERSION = "damast-1";
/** The plate's design size (the mockup's canvas); the pattern scales to any pixel size. */
export const PLATE_WIDTH = 940;
export const PLATE_HEIGHT = 440;
/** Cached tiles of History: 0.8 of the design size. */
export const TILE_WIDTH = 752;
export const TILE_HEIGHT = 352;
/** One forging, from the first battle poured in to cooled steel (the mockup's 4.2 s). */
export const FORGE_MS = 4_200;

/** Bare steel before a column is poured, and the colours of the tint and of the heat. */
export const EMPTY_RGB = [22, 27, 35] as const;
export const WARM = [
  [236, 205, 130],
  [206, 150, 88],
] as const;
export const COOL = [
  [157, 182, 220],
  [92, 124, 180],
] as const;

export interface Layer {
  /** Thickness in pattern units. */
  thickness: number;
  /** Start in the cycle (the sum of the thicknesses before it). */
  start: number;
  /** Brightness of the layer, 0–1. */
  value: number;
  /** A light (1) or a dark (0) layer; the heat colours them differently. */
  light: 0 | 1;
}

/** An eye of a win: centre, amplitude (signed) and size, in pattern units. */
export interface Eye {
  x: number;
  y: number;
  amplitude: number;
  size: number;
}

/** Everything the pattern needs, the same for the CPU and the shader. */
export interface PatternParams {
  battles: Pick<PlateBattle, "r" | "damageRatio" | "tier">[];
  seed: number;
  phase1: number;
  phase2: number;
  zoneWidth: number;
  layers: Layer[];
  cycle: number;
  eyes: Eye[];
}

export function patternSeed(plate: readonly Pick<PlateBattle, "damage" | "wn8">[]): number {
  return (
    plate.reduce(
      (sum, battle) => sum + Math.round(battle.damage ?? 0) + Math.round(battle.wn8 ?? 0),
      0,
    ) % 997
  );
}

export function patternParams(plate: readonly PlateBattle[]): PatternParams {
  const n = plate.length;
  const seed = patternSeed(plate);
  const rnd = (i: number) => {
    const x = Math.sin(seed * 12.9898 + i * 78.233) * 43758.5453;
    return x - Math.floor(x);
  };
  const zoneWidth = PLATE_WIDTH / Math.max(1, n);
  // A fixed sequence of layers from the seed; the density comes from each battle's tier.
  const thicknesses: [number, 0 | 1][] = [];
  for (let i = 0; i < 9; i++)
    thicknesses.push([5 + rnd(100 + i) * 4, 1], [3 + rnd(200 + i) * 1.6, 0]);
  const layers: Layer[] = [];
  let start = 0;
  thicknesses.forEach(([thickness, light], j) => {
    const value = light ? 0.7 + (0.1 * (j % 3)) / 2 : 0.13 + (0.05 * (j % 3)) / 2;
    layers.push({ thickness, start, value, light });
    start += thickness;
  });
  const eyes: Eye[] = [];
  plate.forEach((battle, i) => {
    if (!battle.win) return;
    eyes.push({
      x: (i + 0.5) * zoneWidth + (rnd(10 + i) - 0.5) * zoneWidth * 0.5,
      y: 80 + rnd(20 + i) * 280,
      amplitude: (i % 2 ? 1 : -1) * (50 + 30 * Math.min(1.5, battle.r)),
      size: 34 + 16 * rnd(30 + i),
    });
  });
  return {
    battles: plate.map(({ r, damageRatio, tier }) => ({ r, damageRatio, tier })),
    seed,
    phase1: seed * 0.0137,
    phase2: seed * 0.0091,
    zoneWidth,
    layers,
    cycle: start,
    eyes,
  };
}

export const ease = (x: number) => {
  const t = Math.max(0, Math.min(1, x));
  return t * t * (3 - 2 * t);
};

/**
 * The parameters of each column that do not change while forging: the wave's amplitude, the
 * density of the layers and the tint (−1 cool … 1 warm), each a weighted mix of the battles
 * along the plate (chronology from the left).
 */
export function columnParams(params: PatternParams, width: number) {
  const { battles, zoneWidth } = params;
  const sig = zoneWidth * 0.7;
  const sig2 = zoneWidth * 1.0;
  const amp = new Float32Array(width);
  const density = new Float32Array(width);
  const tint = new Float32Array(width);
  for (let x = 0; x < width; x++) {
    const X = (x * PLATE_WIDTH) / width;
    let sw = 0;
    let sw2 = 0;
    let a = 0;
    let dn = 0;
    let tt = 0;
    battles.forEach((battle, i) => {
      const cx = (i + 0.5) * zoneWidth;
      const w = Math.exp(-Math.pow(X - cx, 2) / (2 * sig * sig));
      const w2 = Math.exp(-Math.pow(X - cx, 2) / (2 * sig2 * sig2));
      sw += w;
      sw2 += w2;
      a += w * (0.45 + 0.9 * Math.min(1.6, battle.damageRatio));
      dn += w * (0.8 + 0.12 * (battle.tier - 8));
      tt += w2 * Math.max(-1, Math.min(1, (battle.r - 1) / 0.3));
    });
    amp[x] = sw > 0 ? a / sw : 0;
    density[x] = sw > 0 ? dn / sw : 0.8;
    tint[x] = sw2 > 0 ? tt / sw2 : 0;
  }
  return { amp, density, tint };
}

/** How far a column is poured (0–1) and how hot it still glows (0–1) at progress p. */
export function columnProgress(X: number, p: number, n: number) {
  const zone = X / (PLATE_WIDTH / Math.max(1, n));
  const done = (p / 0.55) * n;
  const visible = Math.max(0, Math.min(1, (done - zone) * 1.5 + 0.5));
  const heat = Math.max(0, 1 - Math.max(0, done - zone) / (n * 0.55)) * (1 - ease((p - 0.6) / 0.4));
  return { visible, heat };
}

/** Displacement of the layers at a point (pattern units): the waves and the eyes. */
function wave(params: PatternParams, amplitude: number, X: number, Y: number): number {
  const { phase1: ph1, phase2: ph2 } = params;
  let w =
    amplitude *
    (30 * Math.sin(X * 0.009 + ph1 + 1.5 * Math.sin(Y * 0.012 + ph2)) +
      14 * Math.sin(X * 0.026 - Y * 0.01 + ph2) +
      5 * Math.sin(X * 0.05 + Y * 0.031 + ph1));
  for (const eye of params.eyes) {
    const dx = X - eye.x;
    if (dx > 4 * eye.size || dx < -4 * eye.size) continue;
    const dy = (Y - eye.y) * 1.5;
    w += eye.amplitude * Math.exp(-(dx * dx + dy * dy) / (2 * eye.size * eye.size));
  }
  return w;
}

/** The reference's table of layers: every 1/8 of a unit, the layer it falls in. */
function layerTable(params: PatternParams) {
  const size = Math.ceil(params.cycle * 8);
  const value = new Float32Array(size + 1);
  const light = new Uint8Array(size + 1);
  for (const layer of params.layers) {
    const end = layer.start + layer.thickness;
    for (let q = Math.floor(layer.start * 8); q < Math.min(size, Math.ceil(end * 8)); q++) {
      value[q] = layer.value;
      light[q] = layer.light;
    }
  }
  return { size, value, light };
}

const smoothstep = (edge0: number, edge1: number, x: number) => ease((x - edge0) / (edge1 - edge0));

/**
 * Brightness and lightness of the layer at `t` (0 … cycle) with the edges smoothed over ±`w`:
 * the last layer's value plus each edge's step, the wrap from the last layer to the first
 * included. Far from an edge this is exactly the layer's value.
 */
export function smoothLayer(layers: readonly Layer[], cycle: number, t: number, w: number) {
  const last = layers[layers.length - 1];
  let value = last.value;
  let light: number = last.light;
  layers.forEach((layer, k) => {
    const before = k === 0 ? last : layers[k - 1];
    const step = smoothstep(-w, w, t - layer.start);
    value += (layer.value - before.value) * step;
    light += (layer.light - before.light) * step;
  });
  const wrap = smoothstep(-w, w, t - cycle);
  value += (layers[0].value - last.value) * wrap;
  light += (layers[0].light - last.light) * wrap;
  return { value, light };
}

export interface RenderOptions {
  /** Smooth the layer edges (spec 34 §4); false gives the mockup's reference image. */
  smooth?: boolean;
  /**
   * Height of the pattern field in pattern units (default PLATE_HEIGHT, the plate everywhere in
   * the app). A taller field is a longer cut of the same pattern: the X axis and the battles'
   * zones stay, Y runs from 0 to `fieldHeight` (author, after E14 polish: the card's plate
   * fills the card's height without stretching). Below 440 nothing new is invented: the tint's
   * gradient and the glow of the heat stop at their bottom value. A lower field is the plate's
   * top band at the same scale (author, 22.11: Session's plate is 360 px tall).
   */
  fieldHeight?: number;
}

/**
 * The plate as RGBA pixels, `width` × `height`, at forging progress `p` (0 … 1; 1 is the
 * finished, cooled plate).
 */
export function renderPattern(
  plate: readonly PlateBattle[],
  width: number,
  height: number,
  p: number,
  { smooth = true, fieldHeight = PLATE_HEIGHT }: RenderOptions = {},
): Uint8ClampedArray {
  const params = patternParams(plate);
  const n = Math.max(1, plate.length);
  const { amp: colAmp, density: colDen, tint: colT } = columnParams(params, width);
  const forged = ease((p - 0.45) / 0.35);
  const tinted = ease((p - 0.8) / 0.2);
  const visible = new Float32Array(width);
  const heat = new Float32Array(width);
  for (let x = 0; x < width; x++) {
    const progress = columnProgress((x * PLATE_WIDTH) / width, p, n);
    visible[x] = progress.visible;
    heat[x] = progress.heat;
  }
  // The layer coordinate v of every pixel first: its differences are the edge width. One row
  // more under the image when it still lies above the plate's bottom line (a lower field): the
  // plate's own row there, so the last row is smoothed exactly as in the full plate.
  // (Y of the row under the image, written as `nextY` below so the two always agree.)
  const underY = (height * fieldHeight) / height;
  const rows = underY < PLATE_HEIGHT - 1e-9 ? height + 1 : height;
  const v = new Float64Array(width * rows);
  for (let y = 0; y < rows; y++) {
    const Y = (y * fieldHeight) / height;
    for (let x = 0; x < width; x++) {
      const X = (x * PLATE_WIDTH) / width;
      v[y * width + x] = Y * colDen[x] + forged * wave(params, colAmp[x], X, Y) + 600;
    }
  }
  const table = smooth ? null : layerTable(params);
  const out = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y++) {
    const Y = (y * fieldHeight) / height;
    for (let x = 0; x < width; x++) {
      const X = (x * PLATE_WIDTH) / width;
      const o = (y * width + x) * 4;
      if (visible[x] <= 0) {
        out[o] = EMPTY_RGB[0];
        out[o + 1] = EMPTY_RGB[1];
        out[o + 2] = EMPTY_RGB[2];
        out[o + 3] = 255;
        continue;
      }
      const here = v[y * width + x];
      let g: number;
      let light: number;
      if (table) {
        let q = Math.floor((here % params.cycle) * 8);
        if (q >= table.size) q = table.size - 1;
        g = table.value[q];
        light = table.light[q];
      } else {
        // fwidth: the change of v to the next pixel across and down, backwards at the right edge
        // and from the plate's bottom line (Y 440) on. That line is a place in the field, not the
        // image's last row, so a taller field's top band is exactly the plate (E14 polish) and a
        // lower field is exactly the plate's top band (22.11).
        const across = x + 1 < width ? v[y * width + x + 1] - here : here - v[y * width + x - 1];
        const nextY = ((y + 1) * fieldHeight) / height;
        const down =
          y + 1 < rows && nextY < PLATE_HEIGHT - 1e-9
            ? v[(y + 1) * width + x] - here
            : y > 0
              ? here - v[(y - 1) * width + x]
              : 0;
        const w = Math.max(1e-4, 0.5 * (Math.abs(across) + Math.abs(down)));
        ({ value: g, light } = smoothLayer(params.layers, params.cycle, here % params.cycle, w));
      }
      g *= 0.86 + 0.2 * (1 - Y / PLATE_HEIGHT);
      g += 0.12 * Math.exp(-Math.pow(X - 280 - 0.55 * Y, 2) / (2 * 140 * 140));
      g += ((Math.sin(X * 12.9898 + Y * 78.233) * 43758.5453) % 1) * 0.022;
      let r = g * 236;
      let gg = g * 240;
      let bb = g * 247;
      const t = colT[x] * tinted;
      if (Math.abs(t) > 0.08) {
        const palette = t > 0 ? WARM : COOL;
        const f = Math.min(1, Y / PLATE_HEIGHT);
        const a = Math.min(0.6, Math.abs(t) * 0.65);
        const tone = [0, 1, 2].map((c) => palette[0][c] + (palette[1][c] - palette[0][c]) * f);
        const overlay = (base: number, top: number) => {
          const A = base / 255;
          const T = top / 255;
          return 255 * (A < 0.5 ? 2 * A * T : 1 - 2 * (1 - A) * (1 - T));
        };
        r += (overlay(r, tone[0]) - r) * a;
        gg += (overlay(gg, tone[1]) - gg) * a;
        bb += (overlay(bb, tone[2]) - bb) * a;
      }
      const h = heat[x];
      if (h > 0) {
        const hh = h * (0.55 + 0.45 * Math.min(1, Y / PLATE_HEIGHT));
        r += (190 + 65 * light - r) * hh;
        gg += (74 + 126 * light - gg) * hh;
        bb += (26 + 114 * light - bb) * hh;
      }
      const m = visible[x];
      out[o] = EMPTY_RGB[0] + (r - EMPTY_RGB[0]) * m;
      out[o + 1] = EMPTY_RGB[1] + (gg - EMPTY_RGB[1]) * m;
      out[o + 2] = EMPTY_RGB[2] + (bb - EMPTY_RGB[2]) * m;
      out[o + 3] = 255;
    }
  }
  return out;
}

/** 128-bit FNV-1a over bytes (four lanes), as 32 hex digits: image hashes and cache keys. */
export function hashBytes(bytes: ArrayLike<number>): string {
  const lanes = [0x811c9dc5, 0x01000193 ^ 0x9e3779b9, 0x85ebca6b, 0xc2b2ae35];
  for (let i = 0; i < bytes.length; i++)
    for (let lane = 0; lane < 4; lane++) {
      lanes[lane] ^= (bytes[i] + lane * 31) & 0xff;
      lanes[lane] = Math.imul(lanes[lane], 0x01000193 + lane * 2);
    }
  return lanes.map((lane) => (lane >>> 0).toString(16).padStart(8, "0")).join("");
}

export function hashText(text: string): string {
  return hashBytes(new TextEncoder().encode(text));
}

/**
 * Key of a cached plate (spec 34 §4): the algorithm version, the pixel size and everything of
 * the battles the pattern reads. Another version or other session data gives another key.
 */
export function plateKey(plate: readonly PlateBattle[], width: number, height: number): string {
  return hashText(
    JSON.stringify({
      version: DAMAST_VERSION,
      width,
      height,
      battles: plate.map((battle) => [
        battle.r,
        battle.damageRatio,
        battle.tier,
        battle.win,
        battle.damage,
        battle.wn8,
      ]),
    }),
  );
}
