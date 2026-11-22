import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { deflateSync, inflateSync } from "node:zlib";
import { describe, expect, it } from "vitest";
import {
  DAMAST_VERSION,
  PLATE_HEIGHT,
  PLATE_WIDTH,
  hashBytes,
  patternParams,
  plateKey,
  renderPattern,
  smoothLayer,
} from "./pattern";
import { damastStats } from "./stats";
import { fromSynthetic, type SyntheticSession } from "./synthetic";

const SESSIONS = JSON.parse(
  readFileSync("docs/design/damast/sesje-syntetyczne.json", "utf8"),
) as Record<"hot" | "flat" | "cool", SyntheticSession>;
type Name = keyof typeof SESSIONS;
const NAMES: Name[] = ["hot", "flat", "cool"];

/**
 * The mockup's reference algorithm, loaded as it is from the repository (a fixed file of the
 * design folder, no outside input; the repository is an ES module, so `module.exports` is
 * not there).
 */
function reference() {
  const source = readFileSync("docs/design/damast/wzor-referencyjny.js", "utf8");
  return new Function(`${source}; return { damast };`)() as {
    damast: (session: SyntheticSession, w: number, h: number, p: number) => Uint8ClampedArray;
  };
}

const plateOf = (name: Name) => damastStats(fromSynthetic(name, SESSIONS[name])).plate;

/** Reference images (spec 34 §5): low resolution, synthetic data, in the repository. */
const REFERENCE_DIR = "src/features/damast/reference";
const REF_W = 188;
const REF_H = 88;

// --- a minimal PNG writer and reader for the reference images (RGBA, 8 bit, filter 0) ---
const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const byte of bytes) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function concat(parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let at = 0;
  for (const part of parts) {
    out.set(part, at);
    at += part.length;
  }
  return out;
}
function chunk(name: string, data: Uint8Array): Uint8Array {
  const body = concat([new TextEncoder().encode(name), data]);
  const out = new Uint8Array(body.length + 8);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(body, 4);
  view.setUint32(body.length + 4, crc32(body));
  return out;
}
function encodePng(rgba: Uint8ClampedArray, width: number, height: number): Uint8Array {
  const header = new Uint8Array(13);
  new DataView(header.buffer).setUint32(0, width);
  new DataView(header.buffer).setUint32(4, height);
  header.set([8, 6, 0, 0, 0], 8);
  const raw = new Uint8Array(height * (width * 4 + 1));
  for (let y = 0; y < height; y++)
    raw.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), y * (width * 4 + 1) + 1);
  return concat([
    new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", new Uint8Array()),
  ]);
}
function decodePng(file: Uint8Array) {
  const view = new DataView(file.buffer, file.byteOffset, file.byteLength);
  let at = 8;
  let width = 0;
  let height = 0;
  const data: Uint8Array[] = [];
  while (at < file.length) {
    const length = view.getUint32(at);
    const name = new TextDecoder().decode(file.subarray(at + 4, at + 8));
    if (name === "IHDR") {
      width = view.getUint32(at + 8);
      height = view.getUint32(at + 12);
    } else if (name === "IDAT") data.push(file.subarray(at + 8, at + 8 + length));
    at += 12 + length;
  }
  const raw = inflateSync(concat(data));
  const rgba = new Uint8ClampedArray(width * height * 4);
  for (let y = 0; y < height; y++)
    rgba.set(raw.subarray(y * (width * 4 + 1) + 1, (y + 1) * (width * 4 + 1)), y * width * 4);
  return { width, height, rgba };
}

describe("Damast pattern (spec 34 §2, §4)", () => {
  it("draws the mockup's reference image pixel for pixel without the smoothing", () => {
    const { damast } = reference();
    for (const name of NAMES)
      for (const p of [0.2, 0.45, 0.7, 1]) {
        const ours = renderPattern(plateOf(name), 94, 44, p, { smooth: false });
        const theirs = damast(SESSIONS[name], 94, 44, p);
        expect(hashBytes(ours), `${name} at p ${p}`).toBe(hashBytes(theirs));
      }
  });

  it("gives the same session the same hash and different sessions different ones", () => {
    const hashes = NAMES.map((name) => hashBytes(renderPattern(plateOf(name), REF_W, REF_H, 1)));
    for (const [index, name] of NAMES.entries())
      expect(hashBytes(renderPattern(plateOf(name), REF_W, REF_H, 1))).toBe(hashes[index]);
    expect(new Set(hashes).size).toBe(3);
    // One battle more changes the plate.
    const longer = { ...SESSIONS.flat, b: [...SESSIONS.flat.b, SESSIONS.flat.b[0]] };
    const plate = damastStats(fromSynthetic("flat", longer)).plate;
    expect(hashBytes(renderPattern(plate, REF_W, REF_H, 1))).not.toBe(hashes[1]);
  });

  it.each(NAMES)("matches the reference image of the %s session in the repository", (name) => {
    const rgba = renderPattern(plateOf(name), REF_W, REF_H, 1);
    const file = `${REFERENCE_DIR}/${name}.png`;
    // Regenerate after a deliberate change of the algorithm (and of DAMAST_VERSION):
    // UPDATE_DAMAST_REFERENCE=1 pnpm vitest run src/features/damast/pattern.test.ts
    if (process.env.UPDATE_DAMAST_REFERENCE) {
      mkdirSync(REFERENCE_DIR, { recursive: true });
      writeFileSync(file, encodePng(rgba, REF_W, REF_H));
    }
    const stored = decodePng(readFileSync(file));
    expect([stored.width, stored.height]).toEqual([REF_W, REF_H]);
    // Float functions may differ in the last bit between engines: one level of 255 at most.
    let worst = 0;
    for (let i = 0; i < rgba.length; i++)
      worst = Math.max(worst, Math.abs(rgba[i] - stored.rgba[i]));
    expect(worst).toBeLessThanOrEqual(1);
  });

  it("keeps the plate of the default field height exactly as before (author, E14 polish)", () => {
    for (const name of NAMES) {
      const plate = plateOf(name);
      expect(hashBytes(renderPattern(plate, REF_W, REF_H, 1, { fieldHeight: PLATE_HEIGHT }))).toBe(
        hashBytes(renderPattern(plate, REF_W, REF_H, 1)),
      );
    }
  });

  it.each(NAMES)(
    "draws a taller field of the %s session as the same pattern, longer: the top band is the plate",
    (name) => {
      const plate = plateOf(name);
      const base = renderPattern(plate, REF_W, REF_H, 1);
      // 1.5 times as tall at the same scale: Y from 0 to 660 pattern units instead of 440.
      const tallHeight = REF_H * 1.5;
      const tall = renderPattern(plate, REF_W, tallHeight, 1, {
        fieldHeight: (PLATE_HEIGHT * tallHeight) / REF_H,
      });
      expect(tall.length).toBe(REF_W * tallHeight * 4);
      expect(hashBytes(tall.subarray(0, base.length))).toBe(hashBytes(base));
      // Below the band the pattern goes on (not a copy of the band, not empty steel).
      const below = tall.subarray(base.length, base.length + REF_W * 4);
      expect(hashBytes(below)).not.toBe(hashBytes(base.subarray(0, REF_W * 4)));
      expect(below.some((value, index) => index % 4 !== 3 && value > 60)).toBe(true);
    },
  );

  it.each(NAMES)(
    "draws a lower field of the %s session as the plate's top band at the same scale (Session)",
    (name) => {
      const plate = plateOf(name);
      const base = renderPattern(plate, REF_W, REF_H, 1);
      // Author, 22.11: Session's plate is 360 px tall at any width: a lower cut of the field,
      // here Y from 0 to 300 units instead of 440, the scale of the X axis unchanged.
      const lowHeight = 60;
      const low = renderPattern(plate, REF_W, lowHeight, 1, {
        fieldHeight: (PLATE_HEIGHT * lowHeight) / REF_H,
      });
      expect(low.length).toBe(REF_W * lowHeight * 4);
      // Every row, the last one too, is the plate's row pixel for pixel.
      expect(hashBytes(low)).toBe(hashBytes(base.subarray(0, low.length)));
    },
  );

  it("smooths a layer edge over the given width and keeps the layer's value elsewhere", () => {
    const { layers, cycle } = patternParams(plateOf("hot"));
    const second = layers[1];
    // In the middle of a layer: its value exactly.
    const middle = smoothLayer(layers, cycle, second.start + second.thickness / 2, 0.1);
    expect(middle.value).toBeCloseTo(second.value, 12);
    expect(middle.light).toBeCloseTo(second.light, 12);
    // On the edge: half way between the two layers.
    const edge = smoothLayer(layers, cycle, second.start, 0.5);
    expect(edge.value).toBeCloseTo((layers[0].value + second.value) / 2, 12);
    // Across the wrap of the cycle as well, from the last layer to the first.
    const wrap = smoothLayer(layers, cycle, 0, 0.5);
    expect(wrap.value).toBeCloseTo((layers[layers.length - 1].value + layers[0].value) / 2, 12);
    expect(smoothLayer(layers, cycle, cycle - 1e-9, 0.5).value).toBeCloseTo(wrap.value, 6);
  });

  it("takes one eye per win, and the zones follow the battles from the left", () => {
    const plate = plateOf("hot");
    const params = patternParams(plate);
    expect(params.eyes).toHaveLength(plate.filter((battle) => battle.win).length);
    expect(params.zoneWidth).toBeCloseTo(PLATE_WIDTH / plate.length, 12);
    const xs = params.eyes.map((eye) => eye.x);
    expect([...xs].sort((a, b) => a - b)).toEqual(xs);
    for (const eye of params.eyes) {
      expect(eye.y).toBeGreaterThanOrEqual(80);
      expect(eye.y).toBeLessThanOrEqual(PLATE_HEIGHT - 80);
    }
  });

  it("keys a cached plate by the algorithm version, the size and the battles", () => {
    const plate = plateOf("hot");
    const key = plateKey(plate, 752, 352);
    expect(key).toMatch(/^[0-9a-f]{32}$/);
    expect(plateKey(plateOf("hot"), 752, 352)).toBe(key);
    expect(plateKey(plate, 376, 176)).not.toBe(key);
    expect(plateKey(plateOf("flat"), 752, 352)).not.toBe(key);
    const changed = plate.map((battle, index) =>
      index === 0 ? { ...battle, damage: (battle.damage ?? 0) + 1 } : battle,
    );
    expect(plateKey(changed, 752, 352)).not.toBe(key);
    expect(DAMAST_VERSION).toMatch(/^[a-z0-9][a-z0-9.-]{0,15}$/);
  });
});
