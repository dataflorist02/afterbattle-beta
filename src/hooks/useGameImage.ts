import { useEffect, useState } from "react";
import { api } from "../api";

type Kind = "vehicle" | "map";
const MAX_URLS = 256;
/** E5-maps-hd: a picture is never shown more than this much larger than its pixels. */
export const MAX_UPSCALE = 1.25;
/** Largest box side the backend accepts, in device pixels. */
const MAX_BOX_SIDE = 4096;

export interface Size {
  width: number;
  height: number;
}
/** A picture ready to show: its `blob:` URL and its own size in pixels (0 when unknown). */
export interface GameImage extends Size {
  url: string;
}

/** Pictures already read, oldest first; `null` remembers a missing image. */
const urls = new Map<string, GameImage | null>();

function remember(key: string, image: GameImage | null) {
  urls.delete(key);
  urls.set(key, image);
  while (urls.size > MAX_URLS) {
    const [oldest, old] = urls.entries().next().value as [string, GameImage | null];
    urls.delete(oldest);
    if (old) URL.revokeObjectURL(old.url);
  }
}

/** Forgets every image URL ("Delete all local data", a new game folder). */
export function clearGameImages() {
  for (const image of urls.values()) if (image) URL.revokeObjectURL(image.url);
  urls.clear();
}

/** Size of a PNG from its IHDR chunk (0 × 0 when the bytes are not a PNG). */
export function pictureSize(bytes: ArrayBuffer): Size {
  const view = new DataView(bytes);
  if (bytes.byteLength >= 24 && view.getUint8(0) === 0x89)
    return { width: view.getUint32(16), height: view.getUint32(20) };
  return { width: 0, height: 0 };
}

/**
 * The box a picture fills, in device pixels, rounded up to steps of 64 × 16 so that small
 * layout changes reuse the same picture; at most 4096 per side.
 */
export function boxSize(cssWidth: number, cssHeight: number, ratio: number): Size {
  const step = (value: number, by: number) =>
    Math.min(MAX_BOX_SIDE, Math.max(by, Math.ceil((value * ratio) / by) * by));
  return { width: step(cssWidth, 64), height: step(cssHeight, 16) };
}

/** True when covering `box` would scale `picture` up more than {@link MAX_UPSCALE}. */
export function needsSoftFit(box: Size, picture: Size): boolean {
  if (!picture.width || !picture.height) return false;
  return Math.max(box.width / picture.width, box.height / picture.height) > MAX_UPSCALE;
}

/**
 * E5-flare §0: a PNG for a box is scaled down before the window keeps it, so memory holds small
 * images only. E5-maps-hd: scaled like `cover` (the larger of both ratios), so the box never has
 * to stretch it again. Without `createImageBitmap`/`OffscreenCanvas` (tests, old engines) or when
 * the picture is not larger than the box, the original bytes are used.
 */
export async function scaledImage(bytes: ArrayBuffer, box: Size | undefined): Promise<Blob> {
  const original = new Blob([bytes], { type: "image/png" });
  if (!box || typeof createImageBitmap !== "function" || typeof OffscreenCanvas === "undefined")
    return original;
  try {
    const bitmap = await createImageBitmap(original);
    const ratio = Math.max(box.width / bitmap.width, box.height / bitmap.height);
    if (ratio >= 1) {
      bitmap.close();
      return original;
    }
    const canvas = new OffscreenCanvas(
      Math.max(1, Math.round(bitmap.width * ratio)),
      Math.max(1, Math.round(bitmap.height * ratio)),
    );
    canvas.getContext("2d")?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    bitmap.close();
    return await canvas.convertToBlob({ type: "image/png" });
  } catch {
    return original;
  }
}

async function toImage(bytes: ArrayBuffer, box: Size | undefined): Promise<GameImage> {
  const size = pictureSize(bytes);
  const blob = await scaledImage(bytes, box);
  const ratio =
    box && size.width && size.height
      ? Math.min(1, Math.max(box.width / size.width, box.height / size.height))
      : 1;
  return {
    url: URL.createObjectURL(blob),
    width: Math.round(size.width * ratio),
    height: Math.round(size.height * ratio),
  };
}

/**
 * A vehicle icon or map image from the game folder (D19), or `null` for the placeholder.
 * The bytes come from the backend's RAM cache; nothing is written to disk. `box` (device pixels)
 * asks for a map picture sharp at that size (E5-maps-hd); `enabled: false` defers the request
 * until the element is visible. While a new size loads, the previous picture stays.
 */
export function useGameImage(
  kind: Kind,
  key: string | null | undefined,
  options: { box?: Size; enabled?: boolean } = {},
): GameImage | null {
  const { box, enabled = true } = options;
  const boxWidth = box?.width;
  const boxHeight = box?.height;
  const sized = boxWidth && boxHeight ? `@${boxWidth}x${boxHeight}` : "";
  const cacheKey = key ? `${kind}:${key}${sized}` : "";
  const [image, setImage] = useState<GameImage | null>(() =>
    cacheKey ? (urls.get(cacheKey) ?? null) : null,
  );
  useEffect(() => {
    if (!key) {
      setImage(null);
      return;
    }
    if (urls.has(cacheKey)) {
      setImage(urls.get(cacheKey) ?? null);
      return;
    }
    if (!enabled) return;
    let live = true;
    const request =
      boxWidth && boxHeight
        ? api.gameImage(kind, key, boxWidth, boxHeight)
        : api.gameImage(kind, key);
    const target = boxWidth && boxHeight ? { width: boxWidth, height: boxHeight } : undefined;
    request
      .then(async (bytes) => {
        const next = bytes && bytes.byteLength > 0 ? await toImage(bytes, target) : null;
        remember(cacheKey, next);
        if (live) setImage(next);
      })
      .catch(() => {
        if (live) setImage(null);
      });
    return () => {
      live = false;
    };
  }, [kind, key, cacheKey, enabled, boxWidth, boxHeight]);
  return image;
}

/**
 * True once the element has been on screen (E5-flare §7: only visible pictures load). Without
 * `IntersectionObserver` everything counts as visible.
 */
export function useVisible<T extends Element>(element: T | null): boolean {
  const [visible, setVisible] = useState(() => typeof IntersectionObserver === "undefined");
  useEffect(() => {
    if (visible || !element) return;
    if (typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) {
          setVisible(true);
          observer.disconnect();
        }
      },
      { rootMargin: "200px" },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [element, visible]);
  return visible;
}
