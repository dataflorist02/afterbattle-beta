import { act, render } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DamastPlate } from "./DamastPlate";
import { FORGE_MS, renderPattern } from "./pattern";
import { damastStats } from "./stats";
import { fromSynthetic, type SyntheticSession } from "./synthetic";
import { handleRequest } from "./worker";

const mocks = vi.hoisted(() => ({ renderStill: vi.fn() }));
vi.mock("./engine", () => ({ renderStill: mocks.renderStill }));

const SESSIONS = JSON.parse(
  readFileSync("docs/design/damast/sesje-syntetyczne.json", "utf8"),
) as Record<"hot", SyntheticSession>;
const plate = damastStats(fromSynthetic("hot", SESSIONS.hot)).plate;

/** Records what the plate asks of a WebGL2 context; `renderer` is what the GPU reports. */
function fakeGl(renderer: string) {
  const progress: number[] = [];
  const calls: string[] = [];
  const uniforms: Record<string, unknown> = {};
  const constants: Record<string, number | string> = {
    COMPILE_STATUS: 1,
    LINK_STATUS: 2,
    RENDERER: 3,
    TEXTURE0: 100,
    UNMASKED_RENDERER_WEBGL: 4,
  };
  const gl = new Proxy(
    {},
    {
      get(_target, name: string) {
        if (name in constants) return constants[name];
        return (...args: unknown[]) => {
          calls.push(name);
          if (name === "getExtension")
            return args[0] === "WEBGL_debug_renderer_info"
              ? { UNMASKED_RENDERER_WEBGL: 4 }
              : { loseContext: () => calls.push("loseContext") };
          if (name === "getParameter") return renderer;
          if (name === "getShaderParameter" || name === "getProgramParameter") return true;
          if (name === "getUniformLocation") return args[1];
          if (name === "uniform1f" && args[0] === "uProgress") progress.push(args[1] as number);
          if (name === "uniform1f") uniforms[args[0] as string] = args[1];
          if (name.startsWith("create")) return { name };
          return undefined;
        };
      },
    },
  );
  return { gl, progress, calls, uniforms };
}

function contexts(webgl2: object | null) {
  const still = {
    createImageData: (width: number, height: number) => ({
      data: new Uint8ClampedArray(width * height * 4),
    }),
    putImageData: vi.fn(),
  };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(((kind: string) =>
    kind === "webgl2" ? webgl2 : kind === "2d" ? still : null) as never);
  return still;
}

function reduceMotion() {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduce"),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }));
}

let frames: FrameRequestCallback[] = [];
beforeEach(() => {
  frames = [];
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    frames.push(callback);
    return frames.length;
  });
  vi.stubGlobal("cancelAnimationFrame", () => {});
  mocks.renderStill.mockImplementation(async (_plate: unknown, width: number, height: number) => ({
    id: 1,
    pixels: new Uint8ClampedArray(width * height * 4),
  }));
  vi.spyOn(document, "hasFocus").mockReturnValue(true);
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  mocks.renderStill.mockReset();
});

async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("Damast plate on screen (spec 34 §4)", () => {
  it("takes the worker's still image when WebGL2 is missing", async () => {
    const still = contexts(null);
    const { container } = render(<DamastPlate plate={plate} forge label="Damast" />);
    await flush();
    const element = container.querySelector(".damast-plate")!;
    expect(element).toHaveAttribute("data-path", "still");
    expect(element).toHaveAttribute("data-state", "ready");
    expect(mocks.renderStill).toHaveBeenCalledTimes(1);
    expect(mocks.renderStill.mock.calls[0][3]).toBe("pixels");
    expect(still.putImageData).toHaveBeenCalledTimes(1);
    expect(frames).toHaveLength(0);
  });

  it.each(["Google SwiftShader", "llvmpipe (LLVM 15.0.7)", "Microsoft Basic Render Driver"])(
    "treats a WebGL2 that runs in software (%s) as missing",
    async (renderer) => {
      const { gl, calls } = fakeGl(renderer);
      contexts(gl);
      const { container } = render(<DamastPlate plate={plate} forge label="Damast" />);
      await flush();
      expect(container.querySelector(".damast-plate")).toHaveAttribute("data-path", "still");
      expect(calls).toContain("loseContext");
      expect(calls).not.toContain("drawArrays");
      expect(frames).toHaveLength(0);
    },
  );

  it("falls back when the GPU refuses the shader", async () => {
    const { gl } = fakeGl("ANGLE (NVIDIA GeForce, Direct3D11)");
    contexts(
      new Proxy(gl, {
        get: (target, name) =>
          name === "getShaderParameter" ? () => false : Reflect.get(target, name),
      }),
    );
    const { container } = render(<DamastPlate plate={plate} forge label="Damast" />);
    await flush();
    expect(container.querySelector(".damast-plate")).toHaveAttribute("data-path", "still");
  });

  it("forges once on hardware WebGL2, in about 4 s, then stays still", async () => {
    const { gl, progress } = fakeGl("ANGLE (NVIDIA GeForce, Direct3D11)");
    contexts(gl);
    const onReady = vi.fn();
    const { container } = render(
      <DamastPlate plate={plate} forge label="Damast" onReady={onReady} />,
    );
    const element = container.querySelector(".damast-plate")!;
    expect(element).toHaveAttribute("data-path", "webgl2");
    expect(element).toHaveAttribute("data-state", "forging");
    const start = performance.now();
    for (const at of [0.25, 0.5, 0.75]) {
      const next = frames.shift()!;
      act(() => next(start + at * FORGE_MS));
    }
    expect(progress[0]).toBe(0);
    expect(progress.slice(1).every((value, index) => value > progress[index])).toBe(true);
    expect(onReady).not.toHaveBeenCalled();
    act(() => frames.shift()!(start + FORGE_MS + 1));
    expect(progress.at(-1)).toBe(1);
    expect(element).toHaveAttribute("data-state", "ready");
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(frames).toHaveLength(0);
    expect(mocks.renderStill).not.toHaveBeenCalled();
  });

  it("shows the finished plate at once with reduced motion", () => {
    reduceMotion();
    const { gl, progress } = fakeGl("ANGLE (Intel UHD Graphics, Direct3D11)");
    contexts(gl);
    const { container } = render(<DamastPlate plate={plate} forge label="Damast" />);
    expect(progress).toEqual([1]);
    expect(frames).toHaveLength(0);
    expect(container.querySelector(".damast-plate")).toHaveAttribute("data-state", "ready");
  });

  it("shows a plate forged before as finished", () => {
    const { gl, progress } = fakeGl("ANGLE (AMD Radeon, Direct3D11)");
    contexts(gl);
    render(<DamastPlate plate={plate} forge={false} label="Damast" />);
    expect(progress).toEqual([1]);
    expect(frames).toHaveLength(0);
  });

  it("waits with forging while the window is in the background", () => {
    vi.spyOn(document, "hasFocus").mockReturnValue(false);
    const { gl, progress } = fakeGl("ANGLE (NVIDIA GeForce, Direct3D11)");
    contexts(gl);
    const { container } = render(<DamastPlate plate={plate} forge label="Damast" />);
    expect(container.querySelector(".damast-plate")).toHaveAttribute("data-state", "waiting");
    expect(progress).toEqual([]);
    expect(frames).toHaveLength(0);
    vi.spyOn(document, "hasFocus").mockReturnValue(true);
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(container.querySelector(".damast-plate")).toHaveAttribute("data-state", "forging");
    expect(frames).toHaveLength(1);
  });
});

describe("Damast plate of a given height (author, 22.11: Session's plate is 360 px)", () => {
  // jsdom lays nothing out: the plate keeps its design width, 940 px, at a pixel ratio of 1.
  it("draws a lower cut of the same field on WebGL2, at the scale of its width", () => {
    const { gl, uniforms } = fakeGl("ANGLE (NVIDIA GeForce, Direct3D11)");
    contexts(gl);
    const { container } = render(
      <DamastPlate plate={plate} forge={false} label="Damast" height={360} />,
    );
    const canvas = container.querySelector("canvas")!;
    expect([canvas.width, canvas.height]).toEqual([940, 360]);
    // 360 device pixels at 940 / 940 units per pixel: Y from 0 to 360, X and zones unchanged.
    expect(uniforms.uFieldHeight).toBe(360);
    expect((container.querySelector(".damast-plate") as HTMLElement).style.height).toBe("360px");
  });

  it("asks the worker for the same cut when WebGL2 is missing", async () => {
    contexts(null);
    render(<DamastPlate plate={plate} forge={false} label="Damast" height={360} />);
    await flush();
    expect(mocks.renderStill).toHaveBeenCalledTimes(1);
    expect(mocks.renderStill.mock.calls[0].slice(1)).toEqual([940, 360, "pixels", 1, 360]);
  });

  it("keeps the plate's own proportions and field without a height (History, the forge)", async () => {
    const { gl, uniforms } = fakeGl("ANGLE (NVIDIA GeForce, Direct3D11)");
    contexts(gl);
    const { container } = render(<DamastPlate plate={plate} forge={false} label="Damast" />);
    const canvas = container.querySelector("canvas")!;
    expect([canvas.width, canvas.height]).toEqual([940, 440]);
    expect(uniforms.uFieldHeight).toBe(440);
    expect((container.querySelector(".damast-plate") as HTMLElement).style.height).toBe("");
  });
});

describe("Damast worker (spec 34 §4)", () => {
  it("draws the same pixels as pattern.ts and encodes files on request", async () => {
    const pixels = await handleRequest({
      id: 7,
      plate,
      width: 47,
      height: 22,
      progress: 1,
      output: "pixels",
    });
    expect(pixels.id).toBe(7);
    expect(pixels.pixels).toEqual(renderPattern(plate, 47, 22, 1));
    // The card's taller field (E14 polish) goes through the worker as well.
    const tall = await handleRequest({
      id: 10,
      plate,
      width: 47,
      height: 33,
      progress: 1,
      output: "pixels",
      fieldHeight: 660,
    });
    expect(tall.pixels).toEqual(renderPattern(plate, 47, 33, 1, { fieldHeight: 660 }));
    const encode = vi.fn(async () => new Uint8Array([0xff, 0xd8, 0xff, 0xd9]).buffer);
    const jpeg = await handleRequest(
      { id: 8, plate, width: 47, height: 22, progress: 1, output: "jpeg" },
      encode,
    );
    expect(encode).toHaveBeenCalledWith(expect.any(Uint8ClampedArray), 47, 22, "image/jpeg");
    expect(jpeg.bytes?.byteLength).toBe(4);
    const failed = await handleRequest(
      { id: 9, plate, width: 47, height: 22, progress: 1, output: "png" },
      async () => {
        throw new Error("no canvas");
      },
    );
    expect(failed).toEqual({ id: 9, error: "no canvas" });
  });
});
