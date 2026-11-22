/**
 * The window's side of Damast rendering (spec 34 §4): one Web Worker for still images (the
 * fallback plate, the tiles of History and the card's plate).
 */
import type { PlateBattle } from "./stats";
import {
  handleRequest,
  type WorkerOutput,
  type WorkerRequest,
  type WorkerResponse,
} from "./worker";

let worker: Worker | null = null;
let nextId = 1;
const pending = new Map<number, (response: WorkerResponse) => void>();

function startWorker(): Worker | null {
  if (worker) return worker;
  if (typeof Worker === "undefined") return null;
  try {
    worker = new Worker(new URL("./worker.ts", import.meta.url), { type: "module" });
  } catch {
    return null;
  }
  worker.onmessage = (event: MessageEvent<WorkerResponse>) => {
    pending.get(event.data.id)?.(event.data);
    pending.delete(event.data.id);
  };
  return worker;
}

/**
 * The plate as a still image, drawn off the window's thread (on it only where workers do not
 * exist). `progress` 1 is the finished plate.
 */
export function renderStill(
  plate: readonly PlateBattle[],
  width: number,
  height: number,
  output: WorkerOutput,
  progress = 1,
  fieldHeight?: number,
): Promise<WorkerResponse> {
  const request: WorkerRequest = {
    id: nextId++,
    plate: [...plate],
    width,
    height,
    progress,
    fieldHeight,
    output,
  };
  const running = startWorker();
  if (!running) return handleRequest(request);
  return new Promise((resolve) => {
    pending.set(request.id, resolve);
    running.postMessage(request);
  });
}
