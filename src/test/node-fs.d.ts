declare module "node:fs" {
  export function readFileSync(path: string, encoding: "utf8"): string;
  export function readFileSync(path: string): Uint8Array;
  export function appendFileSync(path: string, data: string): void;
  export function existsSync(path: string): boolean;
  export function readdirSync(path: string): string[];
  export function statSync(path: string): { isDirectory(): boolean; size: number };
  export function writeFileSync(path: string, data: string, encoding: "utf8"): void;
  export function writeFileSync(path: string, data: Uint8Array): void;
  export function mkdirSync(path: string, options: { recursive: boolean }): void;
  export function mkdtempSync(prefix: string): string;
  export function rmSync(path: string, options: { recursive: boolean; force: boolean }): void;
}

declare module "node:zlib" {
  export function deflateSync(data: Uint8Array, options?: { level: number }): Uint8Array;
  export function inflateSync(data: Uint8Array): Uint8Array;
}

declare module "node:os" {
  export function tmpdir(): string;
}

declare module "node:path" {
  export function join(...parts: string[]): string;
}

declare module "node:child_process" {
  export function spawnSync(
    command: string,
    args: string[],
    options: { encoding: "utf8"; timeout?: number },
  ): { status: number | null; stdout: string; stderr: string };
}

declare const process: { platform: string; env: Record<string, string | undefined> };
