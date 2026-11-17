import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { ADDRESSES, verbStems } from "../lib/recap/address";
import { checkNeutral } from "../test/recapInvariants";
import { LanguageProvider, storedAddress, useLanguage } from "./language";
import { t, uiTexts, type UiKey } from "./ui";

describe("address setting in the interface texts (spec 30 §2)", () => {
  beforeEach(() => localStorage.clear());

  it("writes every second-person past verb of the Polish texts as a {~stem} marker", () => {
    for (const [key, text] of Object.entries(uiTexts.pl)) {
      // The options of the setting name the forms on purpose ("Forma męska (zadałeś)").
      if (key.startsWith("settingsAddress")) continue;
      expect(text, key).not.toMatch(/\p{L}+(łeś|łaś)(?!\p{L})/u);
    }
    for (const [key, text] of Object.entries(uiTexts.en)) expect(verbStems(text), key).toEqual([]);
  });

  it("has a neutral text without any impersonal verb for every Polish text (2026-11-16)", () => {
    for (const key of Object.keys(uiTexts.pl) as UiKey[]) {
      const text = t("pl", key, undefined, "neutral");
      expect(verbStems(text), key).toEqual([]);
      checkNeutral(text);
    }
    for (const address of ADDRESSES)
      expect(() => t("pl", "momentRising", undefined, address)).not.toThrow();
  });

  it("puts the chosen form into the existing texts, neutral by default", () => {
    expect(t("pl", "survived")).toBe("Czołg przetrwał");
    expect(t("pl", "survived", undefined, "masculine")).toBe("Przeżyłeś");
    expect(t("pl", "momentRising", undefined, "feminine")).toBe("Rozkręcałaś się");
    expect(t("pl", "momentRising")).toBe("Coraz lepiej z każdą bitwą");
    expect(t("pl", "survivedHp", { hp: 120, max: 1500 }, "feminine")).toBe(
      "Przeżyłaś, 120 z 1500 HP",
    );
    expect(t("pl", "survivedHp", { hp: 120, max: 1500 })).toBe("Czołg przetrwał, 120 z 1500 HP");
    expect(t("en", "survived", undefined, "feminine")).toBe(uiTexts.en.survived);
  });

  it("keeps the choice in local storage and applies it through useLanguage", () => {
    expect(storedAddress()).toBe("neutral");
    function Probe() {
      const { tr, address, addressChosen, setAddress } = useLanguage();
      return (
        <button type="button" onClick={() => setAddress("feminine")}>
          {address} {String(addressChosen)}: {tr("survived")}
        </button>
      );
    }
    render(
      <LanguageProvider initial="pl">
        <Probe />
      </LanguageProvider>,
    );
    expect(screen.getByRole("button").textContent).toBe("neutral false: Czołg przetrwał");
    act(() => screen.getByRole("button").click());
    expect(screen.getByRole("button").textContent).toBe("feminine true: Przeżyłaś");
    expect(storedAddress()).toBe("feminine");
    localStorage.setItem("afterbattle-address", "nonsense");
    expect(storedAddress()).toBe("neutral");
    // The impersonal form of beta 4 drafts is now the neutral one.
    localStorage.setItem("afterbattle-address", "impersonal");
    expect(storedAddress()).toBe("neutral");
  });
});
