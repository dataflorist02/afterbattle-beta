function damastStats(s) {
  const r = s.b.map((x) => 0.6 * x.wn8 / s.avgWN8 + 0.4 * x.d / s.avgDmg[x.t]);
  const n = r.length, mean = r.reduce((a, b) => a + b, 0) / n;
  return { r: r, T: Math.round((mean - 1) * 100), n: n,
    dmg: Math.round(s.b.reduce((a, x) => a + x.d, 0) / n), wn8: Math.round(s.b.reduce((a, x) => a + x.wn8, 0) / n),
    wins: s.b.filter((x) => x.w).length, tier: s.b.reduce((a, x) => a + x.t, 0) / n,
    best: r.indexOf(Math.max.apply(null, r)) };
}
function damast(s, W, H, p) {
  const st = damastStats(s), b = s.b, n = b.length;
  const seed = b.reduce((a, x) => a + x.d + x.wn8, 0) % 997;
  const rnd = (i) => { const x = Math.sin(seed * 12.9898 + i * 78.233) * 43758.5453; return x - Math.floor(x); };
  const ph1 = seed * 0.0137, ph2 = seed * 0.0091;
  const zw = 940 / n, sig = zw * 0.7, sig2 = zw * 1.0;
  // stała sekwencja warstw (z ziarna), gęstość zależy od tieru bitwy
  const th = []; for (let i = 0; i < 9; i++) th.push([5 + rnd(100 + i) * 4, 1], [3 + rnd(200 + i) * 1.6, 0]);
  const cyc = th.reduce((a, x) => a + x[0], 0), LN = Math.ceil(cyc * 8);
  const LUT = new Float32Array(LN + 1), LT = new Uint8Array(LN + 1); let pos = 0;
  th.forEach((t, j) => { const e = pos + t[0]; for (let q = Math.floor(pos * 8); q < Math.min(LN, Math.ceil(e * 8)); q++) { LUT[q] = t[1] ? 0.70 + 0.1 * (j % 3) / 2 : 0.13 + 0.05 * (j % 3) / 2; LT[q] = t[1]; } pos = e; });
  const ease = (x) => { x = Math.max(0, Math.min(1, x)); return x * x * (3 - 2 * x); };
  const amp = ease((p - 0.45) / 0.35), tintP = ease((p - 0.8) / 0.2);
  // parametry kolumn: ważona mieszanka bitew wzdłuż długości płytki (chronologia od lewej)
  const colAmp = new Float32Array(W), colDen = new Float32Array(W), colT = new Float32Array(W), colHeat = new Float32Array(W), colVis = new Float32Array(W);
  for (let x = 0; x < W; x++) {
    const X = x * 940 / W; let sw = 0, sw2 = 0, a = 0, dn = 0, tt = 0;
    for (let i = 0; i < n; i++) {
      const cx = (i + 0.5) * zw, w = Math.exp(-Math.pow(X - cx, 2) / (2 * sig * sig)), w2 = Math.exp(-Math.pow(X - cx, 2) / (2 * sig2 * sig2));
      sw += w; sw2 += w2; a += w * (0.45 + 0.9 * Math.min(1.6, b[i].d / s.avgDmg[b[i].t]));
      dn += w * (0.8 + 0.12 * (b[i].t - 8)); tt += w2 * Math.max(-1, Math.min(1, (st.r[i] - 1) / 0.3));
    }
    colAmp[x] = a / sw; colDen[x] = dn / sw; colT[x] = tt / sw2;
    const zi = X / zw, done = p / 0.55 * n; // bitwy pojawiają się po kolei
    colVis[x] = Math.max(0, Math.min(1, (done - zi) * 1.5 + 0.5));
    colHeat[x] = Math.max(0, 1 - Math.max(0, done - zi) / (n * 0.55)) * (1 - ease((p - 0.6) / 0.4));
  }
  // oczka: jedno na wygraną
  const eyes = [];
  b.forEach((x, i) => { if (x.w) eyes.push([(i + 0.5) * zw + (rnd(10 + i) - 0.5) * zw * 0.5, 80 + rnd(20 + i) * 280, (i % 2 ? 1 : -1) * (50 + 30 * Math.min(1.5, st.r[i])), 34 + 16 * rnd(30 + i)]); });
  const out = new Uint8ClampedArray(W * H * 4);
  const WARM = [[236, 205, 130], [206, 150, 88]], COOL = [[157, 182, 220], [92, 124, 180]];
  for (let y = 0; y < H; y++) {
    const Y = y * 440 / H;
    for (let x = 0; x < W; x++) {
      const X = x * 940 / W, o = (y * W + x) * 4;
      if (colVis[x] <= 0) { out[o] = 22; out[o + 1] = 27; out[o + 2] = 35; out[o + 3] = 255; continue; }
      let w = colAmp[x] * (30 * Math.sin(X * 0.009 + ph1 + 1.5 * Math.sin(Y * 0.012 + ph2)) + 14 * Math.sin(X * 0.026 - Y * 0.01 + ph2) + 5 * Math.sin(X * 0.05 + Y * 0.031 + ph1));
      for (const e of eyes) { const dx = X - e[0]; if (dx > 4 * e[3] || dx < -4 * e[3]) continue; const dy = (Y - e[1]) * 1.5; w += e[2] * Math.exp(-(dx * dx + dy * dy) / (2 * e[3] * e[3])); }
      const v = Y * colDen[x] + amp * w + 600;
      let q = Math.floor((v % cyc) * 8); if (q >= LN) q = LN - 1;
      let g = LUT[q]; const light = LT[q];
      g *= 0.86 + 0.2 * (1 - Y / 440);
      g += 0.12 * Math.exp(-Math.pow(X - 280 - 0.55 * Y, 2) / (2 * 140 * 140));
      g += (Math.sin(X * 12.9898 + Y * 78.233) * 43758.5453 % 1) * 0.022;
      let r = g * 236, gg = g * 240, bb = g * 247;
      const t = colT[x] * tintP;
      if (Math.abs(t) > 0.08) {
        const P = t > 0 ? WARM : COOL, f = Y / 440, a = Math.min(0.6, Math.abs(t) * 0.65);
        const tc = [0, 1, 2].map((c) => P[0][c] + (P[1][c] - P[0][c]) * f);
        const ov = (A, T) => { A /= 255; T /= 255; return 255 * (A < 0.5 ? 2 * A * T : 1 - 2 * (1 - A) * (1 - T)); };
        r += (ov(r, tc[0]) - r) * a; gg += (ov(gg, tc[1]) - gg) * a; bb += (ov(bb, tc[2]) - bb) * a;
      }
      const h = colHeat[x];
      if (h > 0) { const hh = h * (0.55 + 0.45 * Y / 440); r += ((light ? 255 : 190) - r) * hh; gg += ((light ? 200 : 74) - gg) * hh; bb += ((light ? 140 : 26) - bb) * hh; }
      const m = colVis[x]; r = 22 + (r - 22) * m; gg = 27 + (gg - 27) * m; bb = 35 + (bb - 35) * m;
      out[o] = r; out[o + 1] = gg; out[o + 2] = bb; out[o + 3] = 255;
    }
  }
  return out;
}
if (typeof module !== 'undefined') module.exports = { damast, damastStats };
