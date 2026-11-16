import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { errorMessage, errorMessages } from "./errors";

const backendCodes = [
  ...readFileSync("src-tauri/src/error.rs", "utf8").matchAll(
    /AppError::\w+(?: \{ \.\. \})?(?:\(_\))? => "([a-z_]+)"/g,
  ),
].map((match) => match[1]);

describe("error dictionary (D10)", () => {
  it("has a Polish and an English text for every backend error code", () => {
    expect(backendCodes.length).toBeGreaterThanOrEqual(27);
    for (const code of backendCodes) {
      expect(errorMessages, code).toHaveProperty(code);
    }
  });

  it("never shows codes, technical words or English inside Polish texts", () => {
    for (const [code, texts] of Object.entries(errorMessages)) {
      for (const text of Object.values(texts)) {
        expect(text, code).not.toMatch(/[a-z]+_[a-z_]+/);
        // The folder name the player sees in Explorer is allowed in brackets (author 2026-11-16).
        const words = text.toLowerCase().replaceAll("(replays)", "");
        expect(words, code).not.toMatch(/parser|baseline|replay|tracker/);
      }
      expect(texts.pl, code).not.toMatch(/\b(the|and|error|failed)\b/i);
    }
  });

  it("names the replays folder as the player sees it in Explorer", () => {
    expect(errorMessages.replays_folder_missing.pl).toContain(
      "nie ma folderu z nagraniami bitew (replays)",
    );
    expect(errorMessages.replays_folder_empty.pl).toContain(
      "Folder z nagraniami bitew (replays) jest pusty",
    );
    expect(errorMessages.replays_folder_missing.en).toContain(
      "has no battle recordings folder (replays)",
    );
    expect(errorMessages.replays_folder_empty.en).toContain(
      "The battle recordings folder (replays) is empty",
    );
  });

  it("falls back to a generic text for an unknown code", () => {
    expect(errorMessage("something_new", "pl")).toBe(errorMessages.unknown_error.pl);
    expect(errorMessage(undefined, "en")).toBe(errorMessages.unknown_error.en);
    expect(errorMessage("battle_not_found", "pl")).toContain("bitwy");
  });
});
