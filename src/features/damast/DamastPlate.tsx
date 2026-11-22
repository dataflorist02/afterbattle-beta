/**
 * The Damast plate on screen (spec 34 §3–§4). WebGL2 draws it in full resolution ×
 * devicePixelRatio and forges it once, about 4 s: the battles pour in as molten metal, the
 * pattern is forged, the steel cools to its tint. Without hardware WebGL2 the Web Worker draws
 * the finished plate as a still image. With reduced motion the plate is finished at once.
 * Forging starts only with the window in front (Fair Play: never while the game is).
 */
import { useEffect, useRef, useState } from "react";
import { prefersReducedMotion } from "../../lib/motion";
import { renderStill } from "./engine";
import { whenInFront } from "./front";
import { createGlPlate, openGl, type GlPlate } from "./gl";
import { FORGE_MS, PLATE_HEIGHT, PLATE_WIDTH } from "./pattern";
import type { PlateBattle } from "./stats";

/** Largest drawing width in device pixels (a 4K screen at 150% stays below). */
const MAX_WIDTH = 4096;

export type PlatePath = "webgl2" | "still";
export type PlateState = "waiting" | "forging" | "ready";

/**
 * The drawing size in device pixels and the height of the pattern field it shows. Without a
 * CSS height the plate keeps its proportions (940:440) and the plate's own field. With one
 * (Session, author 22.11) the field is cut at the scale of the width: X still runs over 940
 * units, Y over as many as the height holds at that scale — not stretched, a lower (or, in a
 * narrow window, longer) cut of the same field.
 */
export function deviceSize(cssWidth: number, cssHeight?: number) {
  const ratio =
    typeof window !== "undefined" && window.devicePixelRatio > 0 ? window.devicePixelRatio : 1;
  const width = Math.max(1, Math.min(MAX_WIDTH, Math.round(cssWidth * ratio)));
  if (cssHeight === undefined)
    return {
      width,
      height: Math.max(1, Math.round((width * PLATE_HEIGHT) / PLATE_WIDTH)),
      fieldHeight: PLATE_HEIGHT,
    };
  const height = Math.max(1, Math.round((cssHeight * width) / Math.max(1, cssWidth)));
  return { width, height, fieldHeight: (height * PLATE_WIDTH) / width };
}

/** The width the plate takes on screen, followed when the window is resized. */
function useCssWidth(ref: React.RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(PLATE_WIDTH);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    const measure = () => {
      const next = element.clientWidth;
      if (next > 0) setWidth((current) => (Math.abs(current - next) >= 1 ? next : current));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}

export function DamastPlate({
  plate,
  forge,
  label,
  onReady,
  height: cssHeight,
}: {
  plate: readonly PlateBattle[];
  /** Forge it now (the first time the ended session is shown); otherwise show it finished. */
  forge: boolean;
  label: string;
  onReady?: () => void;
  /** A fixed CSS height (Session: 360 px); the field is cut to it at the width's scale. */
  height?: number;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const glCanvas = useRef<HTMLCanvasElement>(null);
  const stillCanvas = useRef<HTMLCanvasElement>(null);
  const cssWidth = useCssWidth(frame);
  const [path, setPath] = useState<PlatePath>("webgl2");
  const [state, setState] = useState<PlateState>("waiting");
  // Forged once per mount: a resize afterwards redraws the finished plate.
  const forged = useRef(!forge);
  const ready = useRef(onReady);
  ready.current = onReady;
  const { width, height, fieldHeight } = deviceSize(cssWidth, cssHeight);
  const battles = JSON.stringify(plate);

  useEffect(() => {
    if (path !== "webgl2") return;
    const canvas = glCanvas.current;
    if (!canvas) return;
    canvas.width = width;
    canvas.height = height;
    const gl = openGl(canvas);
    let drawn: GlPlate | null = null;
    try {
      drawn = gl ? createGlPlate(gl, JSON.parse(battles), width, height, fieldHeight) : null;
    } catch {
      drawn = null;
    }
    if (!drawn) {
      setPath("still");
      return;
    }
    const finish = () => {
      drawn?.draw(1);
      forged.current = true;
      setState("ready");
      ready.current?.();
    };
    if (forged.current || prefersReducedMotion()) {
      finish();
      return () => drawn?.dispose();
    }
    let request = 0;
    const stopWaiting = whenInFront(() => {
      setState("forging");
      const started = performance.now();
      const step = (now: number) => {
        const progress = (now - started) / FORGE_MS;
        if (progress >= 1) {
          finish();
          return;
        }
        drawn?.draw(Math.max(0, progress));
        request = requestAnimationFrame(step);
      };
      drawn?.draw(0);
      request = requestAnimationFrame(step);
    });
    return () => {
      stopWaiting();
      cancelAnimationFrame(request);
      drawn?.dispose();
    };
  }, [path, width, height, fieldHeight, battles]);

  useEffect(() => {
    if (path !== "still") return;
    let live = true;
    void renderStill(JSON.parse(battles), width, height, "pixels", 1, fieldHeight).then(
      (response) => {
        const canvas = stillCanvas.current;
        if (!live || !canvas || !response.pixels) return;
        canvas.width = width;
        canvas.height = height;
        const context = canvas.getContext("2d");
        if (!context) return;
        const image = context.createImageData(width, height);
        image.data.set(response.pixels);
        context.putImageData(image, 0, 0);
        forged.current = true;
        setState("ready");
        ready.current?.();
      },
    );
    return () => {
      live = false;
    };
  }, [path, width, height, fieldHeight, battles]);

  return (
    <div
      ref={frame}
      className="damast-plate"
      role="img"
      aria-label={label}
      data-path={path}
      data-state={state}
      style={cssHeight === undefined ? undefined : { height: `${cssHeight}px` }}
    >
      {path === "webgl2" ? (
        <canvas ref={glCanvas} key="webgl2" aria-hidden="true" />
      ) : (
        <canvas ref={stillCanvas} key="still" aria-hidden="true" />
      )}
    </div>
  );
}
