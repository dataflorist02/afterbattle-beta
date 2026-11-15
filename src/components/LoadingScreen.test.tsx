import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";
import { LoadingScreen } from "./LoadingScreen";

function show(needed: boolean, fraction: number | null) {
  return (
    <LanguageProvider initial="en">
      <LoadingScreen needed={needed} fraction={fraction} />
    </LanguageProvider>
  );
}
const bar = () => screen.queryByRole("progressbar", { name: "Loading" });

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("loading screen component (spec 29 §5)", () => {
  it("names exactly the state it is in", () => {
    const view = render(show(true, null));
    expect(bar()!.className).toBe("loading-screen loading-pulse");
    view.rerender(show(true, 0.5));
    expect(bar()!.className).toBe("loading-screen");
    expect(bar()).toHaveAttribute("aria-valuenow", "50");
    view.rerender(show(false, 0.5));
    expect(bar()!.className).toBe("loading-screen loading-finishing");
    expect(bar()).toHaveAttribute("aria-valuenow", "100");
    act(() => {
      vi.advanceTimersByTime(450);
    });
    expect(bar()!.className).toBe("loading-screen loading-finishing loading-out");
  });

  it("stays when it is needed again while finishing, and the old timer does not end it", () => {
    const view = render(show(true, 0.3));
    view.rerender(show(false, 0.3));
    act(() => {
      vi.advanceTimersByTime(200);
    });
    view.rerender(show(true, 0.4));
    act(() => {
      vi.advanceTimersByTime(1_000);
    });
    expect(bar()!.className).toBe("loading-screen");
    expect(bar()).toHaveAttribute("aria-valuenow", "40");
  });

  it("is never shown when not needed, and not again once gone", () => {
    const view = render(show(false, null));
    expect(bar()).toBeNull();
    view.rerender(show(true, null));
    view.rerender(show(false, null));
    act(() => {
      vi.advanceTimersByTime(450);
    });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(bar()).toBeNull();
    view.rerender(show(true, null));
    expect(bar()).toBeNull();
  });
});
