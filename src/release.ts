export const APP_NAME = "Afterbattle";
/** From `src-tauri/Cargo.toml`, the single source of the version (injected by Vite). */
export const APP_VERSION: string = __APP_VERSION__;
/** The first public release is a beta (author decision 2026-11-14; internally v2). */
export const RELEASE_CHANNEL = "Public beta";

/**
 * The name in the title bar and the window title (spec 29 §8): a pre-release by its channel and
 * number ("Afterbattle beta 2" for 1.0.0-beta.2), a final version by the version
 * ("Afterbattle 1.0.0").
 */
export function displayName(version: string): string {
  const match = /^(\d+\.\d+\.\d+)(?:-([A-Za-z]+)(?:\.(\d+))?)?$/.exec(version);
  if (!match) return APP_NAME;
  const [, release, channel, number] = match;
  if (!channel) return `${APP_NAME} ${release}`;
  return number ? `${APP_NAME} ${channel} ${number}` : `${APP_NAME} ${channel}`;
}

export const APP_DISPLAY_NAME = displayName(APP_VERSION);
