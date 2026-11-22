import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DAMAST_VERSION, TILE_HEIGHT, TILE_WIDTH } from "./pattern";
import { damastStats } from "./stats";
import { fromSynthetic, type SyntheticSession } from "./synthetic";
import { hasTile, plateTile, resetTiles, tileKey } from "./tiles";

const mocks = vi.hoisted(() => ({
  renderStill: vi.fn(),
  damastTile: vi.fn(),
  putDamastTile: vi.fn(),
}));
vi.mock("./engine", () => ({ renderStill: mocks.renderStill }));
vi.mock("../../api", () => ({
  api: { damastTile: mocks.damastTile, putDamastTile: mocks.putDamastTile },
}));

const SESSIONS = JSON.parse(
  readFileSync("docs/design/damast/sesje-syntetyczne.json", "utf8"),
) as Record<"hot" | "cool", SyntheticSession>;
const hot = damastStats(fromSynthetic("hot", SESSIONS.hot)).plate;
const cool = damastStats(fromSynthetic("cool", SESSIONS.cool)).plate;
const JPEG = new Uint8Array([0xff, 0xd8, 0xff, 0xe0, 0, 0, 0xff, 0xd9]);

beforeEach(() => {
  resetTiles();
  let url = 0;
  vi.stubGlobal("URL", Object.assign(URL, { createObjectURL: () => `blob:tile-${++url}` }));
  mocks.putDamastTile.mockResolvedValue(undefined);
  mocks.renderStill.mockResolvedValue({ id: 1, bytes: JPEG.buffer });
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
});
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  vi.restoreAllMocks();
});

describe("Damast tiles (spec 34 §4)", () => {
  it("takes a cached tile without drawing it", async () => {
    mocks.damastTile.mockResolvedValue(JPEG.buffer);
    await expect(plateTile(hot)).resolves.toMatch(/^blob:/);
    expect(mocks.damastTile).toHaveBeenCalledWith(DAMAST_VERSION, tileKey(hot));
    expect(mocks.renderStill).not.toHaveBeenCalled();
    expect(mocks.putDamastTile).not.toHaveBeenCalled();
    await expect(hasTile(hot)).resolves.toBe(true);
  });

  it("draws a missing tile once in the worker and keeps it in the cache", async () => {
    mocks.damastTile.mockResolvedValue(new ArrayBuffer(0));
    await expect(hasTile(hot)).resolves.toBe(false);
    const [first, second] = await Promise.all([plateTile(hot), plateTile(hot)]);
    expect(first).toBe(second);
    expect(mocks.renderStill).toHaveBeenCalledTimes(1);
    expect(mocks.renderStill).toHaveBeenCalledWith(hot, TILE_WIDTH, TILE_HEIGHT, "jpeg");
    expect(mocks.putDamastTile).toHaveBeenCalledWith(
      DAMAST_VERSION,
      tileKey(hot),
      new Uint8Array(JPEG),
    );
  });

  it("draws one tile at a time", async () => {
    mocks.damastTile.mockResolvedValue(new ArrayBuffer(0));
    let release: () => void = () => {};
    mocks.renderStill.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = () => resolve({ id: 1, bytes: JPEG.buffer });
        }),
    );
    const first = plateTile(hot);
    const second = plateTile(cool);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mocks.renderStill).toHaveBeenCalledTimes(1);
    release();
    await Promise.all([first, second]);
    expect(mocks.renderStill).toHaveBeenCalledTimes(2);
  });

  it("gives no tile when the worker cannot draw it, and tries again later", async () => {
    mocks.damastTile.mockResolvedValue(new ArrayBuffer(0));
    mocks.renderStill.mockResolvedValueOnce({ id: 1, error: "no canvas" });
    await expect(plateTile(hot)).resolves.toBeNull();
    expect(mocks.putDamastTile).not.toHaveBeenCalled();
    await expect(plateTile(hot)).resolves.toMatch(/^blob:/);
  });

  it("draws a missing tile only with the window in front (Fair Play)", async () => {
    mocks.damastTile.mockResolvedValue(new ArrayBuffer(0));
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const tile = plateTile(hot);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(mocks.renderStill).not.toHaveBeenCalled();
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    window.dispatchEvent(new Event("focus"));
    await expect(tile).resolves.toMatch(/^blob:/);
    expect(mocks.renderStill).toHaveBeenCalledTimes(1);
  });
});
