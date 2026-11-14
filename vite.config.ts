import { readFileSync } from "node:fs";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// The version has one source: src-tauri/Cargo.toml (spec 20 E8, R1). Tauri reads it there too.
// Vite, Vitest and Tauri start from the repository root.
const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
const version = /^\[package\][^[]*?^version\s*=\s*"([^"]+)"/m.exec(cargo)?.[1];
if (!version) throw new Error("no [package] version in src-tauri/Cargo.toml");

export default defineConfig({
  plugins: [react()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
});
