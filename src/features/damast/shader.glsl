#version 300 es
// Damast (spec 34 §4): the plate's pattern per pixel, the algorithm of pattern.ts (the mockup's
// reference) on the GPU, in full resolution × devicePixelRatio. The parameters that do not
// change while forging come from the CPU: per column (uColumns: amplitude, density, tint), the
// eyes of the wins (uEyes) and the fixed sequence of layers. Layer edges are smoothed over one
// pixel with fwidth, as in pattern.ts. Only the steel's grain differs from the CPU (float
// precision of the hash); it carries no data. uFieldHeight is the height of the pattern field
// (pattern.ts `fieldHeight`): 440 for the plate, a lower cut for Session's 360 px plate.
precision highp float;
precision highp int;
precision highp sampler2D;

const int LAYERS = 18;
const float PLATE_W = 940.0;
const float PLATE_H = 440.0;
const vec3 EMPTY = vec3(22.0, 27.0, 35.0);
const vec3 WARM0 = vec3(236.0, 205.0, 130.0);
const vec3 WARM1 = vec3(206.0, 150.0, 88.0);
const vec3 COOL0 = vec3(157.0, 182.0, 220.0);
const vec3 COOL1 = vec3(92.0, 124.0, 180.0);

uniform vec2 uSize;
uniform float uFieldHeight;
uniform float uProgress;
uniform float uBattles;
uniform float uPhase1;
uniform float uPhase2;
uniform float uCycle;
uniform float uLayerStart[LAYERS];
uniform float uLayerValue[LAYERS];
uniform float uLayerLight[LAYERS];
uniform int uEyeCount;
uniform sampler2D uColumns;
uniform sampler2D uEyes;

out vec4 outColor;

float ease(float x) {
  x = clamp(x, 0.0, 1.0);
  return x * x * (3.0 - 2.0 * x);
}

void main() {
  float px = floor(gl_FragCoord.x);
  float py = uSize.y - 1.0 - floor(gl_FragCoord.y);
  float X = px * PLATE_W / uSize.x;
  float Y = py * uFieldHeight / uSize.y;
  // Below the plate's bottom line the tint's gradient and the heat's glow stop (pattern.ts).
  float depth = min(1.0, Y / PLATE_H);
  float n = max(1.0, uBattles);

  // Battles pour in one after another, glow, and cool down.
  float zone = X / (PLATE_W / n);
  float done = uProgress / 0.55 * n;
  float visible = clamp((done - zone) * 1.5 + 0.5, 0.0, 1.0);
  float heat = max(0.0, 1.0 - max(0.0, done - zone) / (n * 0.55)) *
    (1.0 - ease((uProgress - 0.6) / 0.4));
  float forged = ease((uProgress - 0.45) / 0.35);
  float tinted = ease((uProgress - 0.8) / 0.2);

  vec4 column = texelFetch(uColumns, ivec2(int(px), 0), 0);
  float w = column.x * (30.0 * sin(X * 0.009 + uPhase1 + 1.5 * sin(Y * 0.012 + uPhase2)) +
    14.0 * sin(X * 0.026 - Y * 0.01 + uPhase2) + 5.0 * sin(X * 0.05 + Y * 0.031 + uPhase1));
  for (int i = 0; i < uEyeCount; i++) {
    vec4 eye = texelFetch(uEyes, ivec2(i, 0), 0);
    float dx = X - eye.x;
    if (abs(dx) > 4.0 * eye.w) continue;
    float dy = (Y - eye.y) * 1.5;
    w += eye.z * exp(-(dx * dx + dy * dy) / (2.0 * eye.w * eye.w));
  }
  float v = Y * column.y + forged * w + 600.0;
  // Smoothed layer edges: half the change of v to the neighbouring pixels on each side.
  float edge = max(1e-4, 0.5 * fwidth(v));
  float t = mod(v, uCycle);

  float lastValue = uLayerValue[LAYERS - 1];
  float lastLight = uLayerLight[LAYERS - 1];
  float g = lastValue;
  float light = lastLight;
  float beforeValue = lastValue;
  float beforeLight = lastLight;
  for (int k = 0; k < LAYERS; k++) {
    float s = smoothstep(-edge, edge, t - uLayerStart[k]);
    g += (uLayerValue[k] - beforeValue) * s;
    light += (uLayerLight[k] - beforeLight) * s;
    beforeValue = uLayerValue[k];
    beforeLight = uLayerLight[k];
  }
  float wrap = smoothstep(-edge, edge, t - uCycle);
  g += (uLayerValue[0] - lastValue) * wrap;
  light += (uLayerLight[0] - lastLight) * wrap;

  g *= 0.86 + 0.2 * (1.0 - Y / PLATE_H);
  g += 0.12 * exp(-pow(X - 280.0 - 0.55 * Y, 2.0) / (2.0 * 140.0 * 140.0));
  float grain = sin(X * 12.9898 + Y * 78.233) * 43758.5453;
  g += (grain - trunc(grain)) * 0.022;
  vec3 color = g * vec3(236.0, 240.0, 247.0);

  float tint = column.z * tinted;
  if (abs(tint) > 0.08) {
    vec3 tone = tint > 0.0 ? mix(WARM0, WARM1, depth) : mix(COOL0, COOL1, depth);
    float amount = min(0.6, abs(tint) * 0.65);
    vec3 base = color / 255.0;
    vec3 top = tone / 255.0;
    vec3 overlay = 255.0 * mix(2.0 * base * top, 1.0 - 2.0 * (1.0 - base) * (1.0 - top),
      step(0.5, base));
    color += (overlay - color) * amount;
  }
  if (heat > 0.0) {
    float glow = heat * (0.55 + 0.45 * depth);
    vec3 molten = vec3(190.0 + 65.0 * light, 74.0 + 126.0 * light, 26.0 + 114.0 * light);
    color += (molten - color) * glow;
  }
  color = visible <= 0.0 ? EMPTY : EMPTY + (color - EMPTY) * visible;
  outColor = vec4(clamp(color, 0.0, 255.0) / 255.0, 1.0);
}
