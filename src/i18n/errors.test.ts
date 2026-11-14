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
        expect(text.toLowerCase(), code).not.toMatch(/parser|baseline|replay|tracker/);
      }
      expect(texts.pl, code).not.toMatch(/\b(the|and|error|failed)\b/i);
    }
  });

  it("falls back to a generic text for an unknown code", () => {
    expect(errorMessage("something_new", "pl")).toBe(errorMessages.unknown_error.pl);
    expect(errorMessage(undefined, "en")).toBe(errorMessages.unknown_error.en);
    expect(errorMessage("battle_not_found", "pl")).toContain("bitwy");
  });
});
