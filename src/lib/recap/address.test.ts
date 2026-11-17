import { describe, expect, it } from "vitest";
import { ADDRESSES, DEFAULT_ADDRESS, applyAddress, fitsAddress, pastVerb } from "./address";

describe("address forms (spec 30 §2, §5)", () => {
  it("defaults to the neutral form", () => {
    expect(DEFAULT_ADDRESS).toBe("neutral");
    expect(ADDRESSES).toEqual(["neutral", "masculine", "feminine"]);
  });

  it("makes both gendered forms from one verb stem", () => {
    expect(pastVerb("zada", "masculine")).toBe("zadałeś");
    expect(pastVerb("wygra", "feminine")).toBe("wygrałaś");
    expect(pastVerb("przeży", "masculine")).toBe("przeżyłeś");
  });

  it("has no verb in the neutral form (decision 2026-11-16: no impersonal -no/-to)", () => {
    expect(() => pastVerb("zada", "neutral")).toThrow(/zada/);
    expect(() => applyAddress("{~Przeży}", "neutral")).toThrow();
    expect(fitsAddress("{~zada} 3 obrażenia", "neutral")).toBe(false);
    expect(fitsAddress("{~zada} 3 obrażenia", "masculine")).toBe(true);
    expect(fitsAddress("wygrana na mapie", "neutral")).toBe(true);
  });

  it("keeps a capital letter of the stem", () => {
    expect(pastVerb("Przeży", "masculine")).toBe("Przeżyłeś");
    expect(pastVerb("Rozkręca", "feminine")).toBe("Rozkręcałaś");
  });

  it("replaces every {~stem} marker and leaves other text alone", () => {
    expect(applyAddress("{~Przeży}, {hp} z {max} HP", "feminine")).toBe(
      "Przeżyłaś, {hp} z {max} HP",
    );
    expect(applyAddress("{~Rozkręca} się, a potem {~zwolni}", "masculine")).toBe(
      "Rozkręcałeś się, a potem zwolniłeś",
    );
    expect(applyAddress("Bez czasownika", "neutral")).toBe("Bez czasownika");
  });
});
