/**
 * Damast in a Web Worker (spec 34 §4): the fallback path when WebGL2 is missing or runs in
 * software, and the maker of the cached tiles of History and of the card's plate. The same
 * algorithm as the shader (pattern.ts), off the window's thread; the result is one still image
 * (pixels, or a JPEG / PNG file made with OffscreenCanvas).
 */
import { renderPattern } from "./pattern";
import type { PlateBattle } from "./stats";

export type WorkerOutput = "pixels" | "jpeg" | "png";

export interface WorkerRequest {
  id: number;
  plate: PlateBattle[];
  width: number;
  height: number;
  progress: number;
  output: WorkerOutput;
  /** Height of the pattern field (pattern.ts `fieldHeight`); the plate's 440 by default. */
  fieldHeight?: number;
}

export interface WorkerResponse {
  id: number;
  pixels?: Uint8ClampedArray;
  bytes?: ArrayBuffer;
  error?: string;
}

/** Tile quality: small files, no visible blocks on fine layers. */
export const JPEG_QUALITY = 0.9;

export type Encoder = (
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
  type: "image/jpeg" | "image/png",
) => Promise<ArrayBuffer>;

/** Encodes pixels with OffscreenCanvas (available in workers of the app's WebView). */
export const encodeWithOffscreenCanvas: Encoder = async (pixels, width, height, type) => {
  const canvas = new OffscreenCanvas(width, height);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("damast: no 2d context");
  context.putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0);
  const blob = await canvas.convertToBlob({ type, quality: JPEG_QUALITY });
  return blob.arrayBuffer();
};

export async function handleRequest(
  request: WorkerRequest,
  encode: Encoder = encodeWithOffscreenCanvas,
): Promise<WorkerResponse> {
  try {
    const pixels = renderPattern(request.plate, request.width, request.height, request.progress, {
      fieldHeight: request.fieldHeight,
    });
    if (request.output === "pixels") return { id: request.id, pixels };
    const type = request.output === "jpeg" ? "image/jpeg" : "image/png";
    return { id: request.id, bytes: await encode(pixels, request.width, request.height, type) };
  } catch (error) {
    return { id: request.id, error: error instanceof Error ? error.message : String(error) };
  }
}

interface WorkerScope {
  onmessage: ((event: MessageEvent<WorkerRequest>) => void) | null;
  postMessage(message: WorkerResponse, transfer: Transferable[]): void;
}

const isWorker =
  typeof (globalThis as { WorkerGlobalScope?: unknown }).WorkerGlobalScope !== "undefined";
if (isWorker) {
  const scope = globalThis as unknown as WorkerScope;
  scope.onmessage = (event) => {
    void handleRequest(event.data).then((response) => {
      const transfer: Transferable[] = [];
      if (response.pixels) transfer.push(response.pixels.buffer as ArrayBuffer);
      if (response.bytes) transfer.push(response.bytes);
      scope.postMessage(response, transfer);
    });
  };
}
