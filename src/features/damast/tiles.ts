/**
 * The tiles of History (spec 34 §4): each plate drawn once in the worker and kept as a JPEG in
 * the app's data folder; afterwards it comes from there, never drawn live again. The key holds
 * DAMAST_VERSION and the session's data, so either change draws the plate anew.
 */
import { api } from "../../api";
import { renderStill } from "./engine";
import { inFront } from "./front";
import { DAMAST_VERSION, TILE_HEIGHT, TILE_WIDTH, plateKey } from "./pattern";
import type { PlateBattle } from "./stats";

const tileUrls = new Map<string, Promise<string | null>>();
/** One tile drawn at a time, so History never keeps the worker busy with a queue of plates. */
let drawing: Promise<unknown> = Promise.resolve();

function objectUrl(bytes: ArrayBuffer): string {
  return URL.createObjectURL(new Blob([bytes], { type: "image/jpeg" }));
}

/** The key of a plate's tile in the cache (version, size and battles). */
export const tileKey = (plate: readonly PlateBattle[]) => plateKey(plate, TILE_WIDTH, TILE_HEIGHT);

/** Whether the plate already has a tile: a plate is forged on screen only once (spec 34 §3). */
export async function hasTile(plate: readonly PlateBattle[]): Promise<boolean> {
  const cached = await api.damastTile(DAMAST_VERSION, tileKey(plate)).catch(() => null);
  return cached !== null && cached.byteLength > 0;
}

/**
 * The plate's tile as an image address: from the cache when it is there, otherwise drawn once
 * in the worker and kept in the cache. Null when it cannot be drawn.
 */
export function plateTile(plate: readonly PlateBattle[]): Promise<string | null> {
  const key = tileKey(plate);
  const known = tileUrls.get(key);
  if (known) return known;
  const result = (async () => {
    const cached = await api.damastTile(DAMAST_VERSION, key).catch(() => null);
    if (cached && cached.byteLength > 0) return objectUrl(cached);
    // Drawn one at a time, and only with the window in front (never while the game is).
    const turn = drawing
      .then(() => inFront())
      .then(() => renderStill(plate, TILE_WIDTH, TILE_HEIGHT, "jpeg"));
    drawing = turn.catch(() => undefined);
    const drawn = await turn;
    if (!drawn.bytes) return null;
    await api.putDamastTile(DAMAST_VERSION, key, new Uint8Array(drawn.bytes)).catch(() => {});
    return objectUrl(drawn.bytes);
  })();
  tileUrls.set(key, result);
  result.then(
    (url) => url === null && tileUrls.delete(key),
    () => tileUrls.delete(key),
  );
  return result;
}

/** Forget the addresses kept in memory (tests). */
export function resetTiles() {
  for (const url of tileUrls.values())
    void url.then((value) => value && URL.revokeObjectURL?.(value));
  tileUrls.clear();
}
