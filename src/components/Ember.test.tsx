import { act, cleanup, render, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { heatOf } from "../lib/ember";
import { EmberBreath, Sparks, useWindowBackground } from "./Ember";

function reduceMotion(reduce: boolean) {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: reduce && query.includes("reduce"),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }));
}

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  delete document.documentElement.dataset.window;
});

describe("sparks (spec 33 §2)", () => {
  it("throws 3 to 18 sparks over a warmer card, none over a usual or cooler one", () => {
    for (const [change, count] of [
      [5, 3],
      [30, 18],
      [4, 0],
      [-30, 0],
    ] as const) {
      const { container, unmount } = render(<Sparks heat={heatOf(change)} />);
      expect(container.querySelectorAll(".ember-spark"), String(change)).toHaveLength(count);
      unmount();
    }
  });

  it("puts the sparks in the same places every time, as layers hidden from readers", () => {
    const first = render(<Sparks heat={heatOf(30)} />).container.innerHTML;
    cleanup();
    expect(render(<Sparks heat={heatOf(30)} />).container.innerHTML).toBe(first);
    expect(document.querySelector(".ember-sparks")).toHaveAttribute("aria-hidden", "true");
  });

  it("has no sparks with reduced motion", () => {
    reduceMotion(true);
    const { container } = render(<Sparks heat={heatOf(30)} />);
    expect(container.querySelector(".ember-spark")).toBeNull();
  });
});

describe("the breath of the hot spot", () => {
  it("is a layer hidden from readers", () => {
    const { container } = render(<EmberBreath />);
    expect(container.firstChild).toHaveClass("ember-breath");
    expect(container.firstChild).toHaveAttribute("aria-hidden", "true");
  });

  it("carries the number's text for a glow of its shape (author, 22.11)", () => {
    const { container } = render(<EmberBreath text="3 550" />);
    expect(container.firstChild).toHaveAttribute("data-text", "3 550");
    expect(container.firstChild).toHaveTextContent("");
  });
});

describe("the window in the background (spec 33 §1: nothing during a battle)", () => {
  it("marks the page while the window has no focus or is hidden, and clears it on return", () => {
    renderHook(() => useWindowBackground());
    expect(document.documentElement.dataset.window).toBeUndefined();
    act(() => {
      window.dispatchEvent(new Event("blur"));
    });
    expect(document.documentElement.dataset.window).toBe("background");
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(document.documentElement.dataset.window).toBeUndefined();
    const hidden = vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(document.documentElement.dataset.window).toBe("background");
    hidden.mockReturnValue("visible");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(document.documentElement.dataset.window).toBeUndefined();
  });

  it("stops listening when the app goes away", () => {
    const { unmount } = renderHook(() => useWindowBackground());
    unmount();
    window.dispatchEvent(new Event("blur"));
    expect(document.documentElement.dataset.window).toBeUndefined();
  });
});
