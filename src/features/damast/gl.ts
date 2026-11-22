/**
 * Damast on WebGL2 (spec 34 §4): the main path. A context that is missing, fails, or runs in
 * software (SwiftShader, llvmpipe, the Microsoft Basic Render Driver) is refused, and the plate
 * is drawn by the Web Worker instead, as one still image.
 */
import fragmentSource from "./shader.glsl?raw";
import { PLATE_HEIGHT, columnParams, patternParams } from "./pattern";
import type { PlateBattle } from "./stats";

const VERTEX_SOURCE = `#version 300 es
void main() {
  vec2 corner = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  gl_Position = vec4(corner * 2.0 - 1.0, 0.0, 1.0);
}`;

const SOFTWARE_RENDERERS = /swiftshader|llvmpipe|softpipe|software|basic render/i;

/** The renderer's name, unmasked when the browser allows it. */
function rendererName(gl: WebGL2RenderingContext): string {
  const info = gl.getExtension("WEBGL_debug_renderer_info");
  const name = info ? gl.getParameter(info.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER);
  return typeof name === "string" ? name : "";
}

/**
 * A hardware WebGL2 context for the plate, or null: then the plate takes the fallback path.
 * `failIfMajorPerformanceCaveat` already refuses most software contexts; the renderer's name
 * catches the rest.
 */
export function openGl(canvas: HTMLCanvasElement): WebGL2RenderingContext | null {
  let gl: WebGL2RenderingContext | null;
  try {
    gl = canvas.getContext("webgl2", {
      failIfMajorPerformanceCaveat: true,
      antialias: false,
      alpha: false,
      depth: false,
      stencil: false,
      premultipliedAlpha: false,
      preserveDrawingBuffer: true,
      powerPreference: "default",
    }) as WebGL2RenderingContext | null;
  } catch {
    return null;
  }
  if (!gl) return null;
  if (SOFTWARE_RENDERERS.test(rendererName(gl))) {
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return null;
  }
  return gl;
}

export interface GlPlate {
  /** Draws the plate at forging progress p (0 … 1). */
  draw(progress: number): void;
  dispose(): void;
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error("damast: no shader");
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`damast: shader does not compile: ${log ?? ""}`);
  }
  return shader;
}

function floatTexture(
  gl: WebGL2RenderingContext,
  unit: number,
  width: number,
  data: Float32Array,
): WebGLTexture {
  const texture = gl.createTexture();
  if (!texture) throw new Error("damast: no texture");
  gl.activeTexture(gl.TEXTURE0 + unit);
  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, width, 1, 0, gl.RGBA, gl.FLOAT, data);
  return texture;
}

/**
 * Prepares one plate of `width` × `height` device pixels: the program, the per-column texture
 * and the eyes. `fieldHeight` is the height of the pattern field it shows (pattern.ts), the
 * plate's 440 by default. Throws when the GPU refuses (the caller falls back to the worker).
 */
export function createGlPlate(
  gl: WebGL2RenderingContext,
  plate: readonly PlateBattle[],
  width: number,
  height: number,
  fieldHeight: number = PLATE_HEIGHT,
): GlPlate {
  const params = patternParams(plate);
  const program = gl.createProgram();
  if (!program) throw new Error("damast: no program");
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX_SOURCE);
  const fragment = compile(gl, gl.FRAGMENT_SHADER, fragmentSource);
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS))
    throw new Error(`damast: program does not link: ${gl.getProgramInfoLog(program) ?? ""}`);
  gl.useProgram(program);

  const columns = columnParams(params, width);
  const columnData = new Float32Array(width * 4);
  for (let x = 0; x < width; x++) {
    columnData[x * 4] = columns.amp[x];
    columnData[x * 4 + 1] = columns.density[x];
    columnData[x * 4 + 2] = columns.tint[x];
  }
  const eyeCount = Math.max(1, params.eyes.length);
  const eyeData = new Float32Array(eyeCount * 4);
  params.eyes.forEach((eye, index) =>
    eyeData.set([eye.x, eye.y, eye.amplitude, eye.size], index * 4),
  );
  const textures = [floatTexture(gl, 0, width, columnData), floatTexture(gl, 1, eyeCount, eyeData)];

  const at = (name: string) => gl.getUniformLocation(program, name);
  gl.uniform1i(at("uColumns"), 0);
  gl.uniform1i(at("uEyes"), 1);
  gl.uniform2f(at("uSize"), width, height);
  gl.uniform1f(at("uFieldHeight"), fieldHeight);
  gl.uniform1f(at("uBattles"), plate.length);
  gl.uniform1f(at("uPhase1"), params.phase1);
  gl.uniform1f(at("uPhase2"), params.phase2);
  gl.uniform1f(at("uCycle"), params.cycle);
  gl.uniform1fv(
    at("uLayerStart"),
    params.layers.map((layer) => layer.start),
  );
  gl.uniform1fv(
    at("uLayerValue"),
    params.layers.map((layer) => layer.value),
  );
  gl.uniform1fv(
    at("uLayerLight"),
    params.layers.map((layer) => layer.light),
  );
  gl.uniform1i(at("uEyeCount"), params.eyes.length);
  const progress = at("uProgress");
  const vertexArray = gl.createVertexArray();
  gl.bindVertexArray(vertexArray);
  gl.viewport(0, 0, width, height);

  return {
    draw(value: number) {
      gl.useProgram(program);
      gl.uniform1f(progress, Math.max(0, Math.min(1, value)));
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    },
    dispose() {
      for (const texture of textures) gl.deleteTexture(texture);
      gl.deleteVertexArray(vertexArray);
      gl.deleteProgram(program);
      gl.deleteShader(vertex);
      gl.deleteShader(fragment);
    },
  };
}
