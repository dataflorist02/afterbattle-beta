export const APP_NAME = "Afterbattle";
/** From `src-tauri/Cargo.toml`, the single source of the version (injected by Vite). */
export const APP_VERSION: string = __APP_VERSION__;
/** The first public release is a beta (author decision 2026-11-14; internally v2). */
export const RELEASE_CHANNEL = "Public beta";

/**
 * The name in the title bar and the window title: the pre-release channel and major.minor
 * ("Afterbattle beta 1.0" for 1.0.0-beta.1), or major.minor of a final version.
 */
export function displayName(version: string): string {
  const match = /^(\d+)\.(\d+)\.\d+(?:-([A-Za-z]+)[.\d]*)?$/.exec(version);
  if (!match) return APP_NAME;
  const [, major, minor, channel] = match;
  return channel ? `${APP_NAME} ${channel} ${major}.${minor}` : `${APP_NAME} ${major}.${minor}`;
}

export const APP_DISPLAY_NAME = displayName(APP_VERSION);
