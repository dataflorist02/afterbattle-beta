import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HowItWorksContext } from "../../components/HelpLink";
import { LanguageProvider } from "../../i18n/language";
import { damastSession } from "../../test/fixtures";
import type { Language } from "../../types";
import { DamastInfo, fitPopover } from "./DamastInfo";
import { damastStats } from "./stats";

const damast = damastSession();
const stats = damastStats(damast);

function renderInfo(language: Language = "pl") {
  const openHow = vi.fn();
  const view = render(
    <LanguageProvider initial={language}>
      <HowItWorksContext.Provider value={openHow}>
        <p>
          <button type="button">elsewhere</button>
        </p>
        <DamastInfo damast={damast} stats={stats} mapOf={() => "Synthetic Hills"} />
      </HowItWorksContext.Provider>
    </LanguageProvider>,
  );
  const button = screen.getByRole("button", {
    name: language === "pl" ? "Jak czytać tę płytkę" : "How to read this plate",
  });
  return { ...view, button, openHow };
}
const dialog = () => screen.queryByRole("dialog");

function reduceMotion() {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduce"),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }));
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("the info button by Damast on Session (author, 22.11)", () => {
  it("is closed by default, a small button that says what it opens", () => {
    const { button } = renderInfo();
    expect(button).toHaveAttribute("aria-expanded", "false");
    expect(button).toHaveClass("damast-info-button");
    expect(dialog()).toBeNull();
  });

  it("opens by a click next to the button, every row with its legend mark", () => {
    const { button } = renderInfo();
    fireEvent.click(button);
    expect(button).toHaveAttribute("aria-expanded", "true");
    const open = dialog()!;
    expect(open).toHaveAttribute("aria-modal", "false");
    expect(button.getAttribute("aria-controls")).toBe(open.id);
    const rows = Array.from(open.querySelectorAll("li"));
    expect(rows.map((row) => row.getAttribute("data-kind"))).toEqual([
      "order",
      "tint",
      "eyes",
      "tiers",
      "waves",
      "spark",
    ]);
    for (const row of rows) expect(row.querySelector(".damast-legend-mark")).not.toBeNull();
    expect(rows[0]).toHaveTextContent("Bitwy idą od lewej do prawej: 9 bitew.");
    expect(rows[5]).toHaveTextContent(
      `Iskra pod płytką: najlepsza bitwa (nr ${stats.best + 1}, Synthetic Hills).`,
    );
    expect(within(open).getByRole("button", { name: "Więcej w „Jak to działa”" })).toBeVisible();
    // Opening does not take the focus away from the button (not modal).
    expect(document.activeElement).not.toBe(open);
  });

  it("opens with Enter and closes with Space, from the keyboard", () => {
    const { button } = renderInfo();
    act(() => button.focus());
    fireEvent.keyDown(button, { key: "Enter" });
    expect(dialog()).not.toBeNull();
    // The click a browser sends after the key does not toggle it back.
    fireEvent.click(button);
    expect(dialog()).not.toBeNull();
    fireEvent.keyUp(button, { key: "Enter" });
    fireEvent.keyDown(button, { key: " " });
    fireEvent.keyUp(button, { key: " " });
    fireEvent.click(button);
    expect(dialog()).toBeNull();
    expect(button).toHaveAttribute("aria-expanded", "false");
  });

  it("closes with Esc and gives the focus back to the button", () => {
    const { button } = renderInfo();
    fireEvent.click(button);
    const more = within(dialog()!).getByRole("button", { name: "Więcej w „Jak to działa”" });
    act(() => more.focus());
    fireEvent.keyDown(more, { key: "Escape" });
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(button);
  });

  it("closes by a click outside it and by a second click on the button", async () => {
    const { button } = renderInfo();
    fireEvent.click(button);
    fireEvent.mouseDown(dialog()!);
    expect(dialog()).not.toBeNull();
    fireEvent.mouseDown(document.body);
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(button);
    fireEvent.click(button);
    expect(dialog()).not.toBeNull();
    fireEvent.click(button);
    expect(dialog()).toBeNull();
    // A click on another control keeps the focus where the player put it.
    fireEvent.click(button);
    const elsewhere = screen.getByRole("button", { name: "elsewhere" });
    fireEvent.mouseDown(elsewhere);
    act(() => elsewhere.focus());
    expect(dialog()).toBeNull();
    expect(document.activeElement).toBe(elsewhere);
  });

  it("leads to the Damast entry of “Jak to działa”", () => {
    const { button, openHow } = renderInfo();
    fireEvent.click(button);
    fireEvent.click(within(dialog()!).getByRole("button", { name: "Więcej w „Jak to działa”" }));
    expect(openHow).toHaveBeenCalledWith("damast");
  });

  it("appears without motion with reduced motion", () => {
    const first = renderInfo();
    fireEvent.click(first.button);
    expect(dialog()).toHaveClass("damast-info-in");
    first.unmount();
    reduceMotion();
    const second = renderInfo();
    fireEvent.click(second.button);
    expect(dialog()).not.toBeNull();
    expect(dialog()).not.toHaveClass("damast-info-in");
  });

  it("speaks English too", () => {
    const { button } = renderInfo("en");
    fireEvent.click(button);
    expect(dialog()).toHaveTextContent("Battles run from left to right: 9 battles.");
    expect(within(dialog()!).getByRole("button", { name: "More in “How it works”" })).toBeVisible();
  });
});

describe("the popover stays inside the window", () => {
  const view = { width: 1000, height: 700 };
  const size = { width: 360, height: 300 };
  it("opens under the button, from its left edge", () => {
    expect(fitPopover({ left: 100, top: 80, bottom: 112 }, size, view)).toEqual({
      shift: 0,
      side: "below",
    });
  });
  it("moves left near the window's right edge and goes above near its bottom", () => {
    const place = fitPopover({ left: 800, top: 500, bottom: 532 }, size, view);
    expect(800 + place.shift + size.width).toBeLessThanOrEqual(view.width - 8);
    expect(place.side).toBe("above");
  });
  it("never starts left of the window", () => {
    const place = fitPopover({ left: 4, top: 80, bottom: 112 }, { width: 990, height: 100 }, view);
    expect(4 + place.shift).toBeGreaterThanOrEqual(8 - 0.5);
  });
});
