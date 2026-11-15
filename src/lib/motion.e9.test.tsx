import { useRef } from "react";
import { act, cleanup, render, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  COUNT_MS,
  DELTA_FADE_MS,
  DELTA_SHOW_MS,
  changeDirection,
  flipOffset,
  reservedChars,
  useArrivals,
  useCountedNumber,
  useFlip,
} from "./motion";

describe("direction of a changed number (spec 29 §6)", () => {
  it("is up, down, or nothing", () => {
    expect(changeDirection(1_000, 1_200)).toBe("up");
    expect(changeDirection(1_200, 1_000)).toBe("down");
    expect(changeDirection(1_000, 1_000)).toBeNull();
    expect(changeDirection(null, 1_000)).toBeNull();
    expect(changeDirection(1_000, null)).toBeNull();
    expect(changeDirection(Number.NaN, 1)).toBeNull();
  });
});

describe("counting without width jumps", () => {
  it("reserves the width of the longer of the old and new text", () => {
    expect(reservedChars(["980", "1 020"])).toBe(5);
    expect(reservedChars(["3 184", "3 190"])).toBe(5);
    expect(reservedChars(["—"])).toBe(1);
    expect(reservedChars([])).toBe(0);
  });
});

describe("FLIP offsets", () => {
  it("moves an element from where it was to where it is, ignoring sub-pixel noise", () => {
    expect(flipOffset({ left: 10, top: 200 }, { left: 10, top: 260 })).toEqual({ x: 0, y: -60 });
    expect(flipOffset({ left: 160, top: 0 }, { left: 0, top: 0 })).toEqual({ x: 160, y: 0 });
    expect(flipOffset({ left: 10, top: 200 }, { left: 10.3, top: 200.4 })).toBeNull();
    expect(flipOffset({ left: 10, top: 200 }, { left: 10, top: 200.5 })).toEqual({ x: 0, y: -0.5 });
    expect(flipOffset({ left: 10.5, top: 200 }, { left: 10, top: 200 })).toEqual({ x: 0.5, y: 0 });
    expect(flipOffset(undefined, { left: 10, top: 200 })).toBeNull();
  });
});

describe("arrivals: only battles that came while the screen is open", () => {
  it("are none on entering the screen, the new ones after that, for the same running session", () => {
    const { result, rerender } = renderHook(({ id, ids, active }) => useArrivals(id, ids, active), {
      initialProps: { id: "s1", ids: ["a", "b"], active: true },
    });
    expect(result.current).toEqual([]);
    rerender({ id: "s1", ids: ["a", "b", "c"], active: true });
    expect(result.current).toEqual(["c"]);
    // A render without a change keeps them (the tile keeps its entrance class).
    rerender({ id: "s1", ids: ["a", "b", "c"], active: true });
    expect(result.current).toEqual(["c"]);
    rerender({ id: "s2", ids: ["x"], active: true });
    expect(result.current).toEqual([]);
    rerender({ id: "s2", ids: ["x", "y"], active: false });
    expect(result.current).toEqual([]);
  });

  it("take the first loaded list as the baseline, not as arrivals", () => {
    const { result, rerender } = renderHook(({ id, ids }) => useArrivals(id, ids, true), {
      initialProps: { id: "all", ids: null as string[] | null },
    });
    expect(result.current).toEqual([]);
    rerender({ id: "all", ids: ["a", "b"] });
    expect(result.current).toEqual([]);
    rerender({ id: "all", ids: null });
    expect(result.current).toEqual([]);
    rerender({ id: "all", ids: ["c", "a", "b"] });
    expect(result.current).toEqual(["c"]);
  });
});

describe("a counted number", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("counts from the old to the new value, with the direction for a while", async () => {
    const { result, rerender } = renderHook(({ value, on }) => useCountedNumber(value, on), {
      initialProps: { value: 1_000 as number | null, on: true },
    });
    expect(result.current).toMatchObject({ shown: 1_000, direction: null });
    rerender({ value: 1_300, on: true });
    expect(result.current.direction).toBe("up");
    expect(result.current.shown).toBe(1_000);
    const key = result.current.key;
    await act(async () => {
      vi.advanceTimersByTime(COUNT_MS / 2);
    });
    expect(result.current.shown).toBeGreaterThan(1_000);
    expect(result.current.shown).toBeLessThan(1_300);
    await act(async () => {
      vi.advanceTimersByTime(COUNT_MS);
    });
    expect(result.current.shown).toBe(1_300);
    expect(result.current.direction).toBe("up");
    await act(async () => {
      vi.advanceTimersByTime(DELTA_SHOW_MS + DELTA_FADE_MS);
    });
    expect(result.current.direction).toBeNull();
    rerender({ value: 1_100, on: true });
    expect(result.current.direction).toBe("down");
    expect(result.current.key).toBe(key + 1);
  });

  it("shows a new value at once without motion, and nothing for the same value", () => {
    const { result, rerender } = renderHook(({ value, on }) => useCountedNumber(value, on), {
      initialProps: { value: 1_000 as number | null, on: false },
    });
    rerender({ value: 1_300, on: false });
    expect(result.current).toMatchObject({ shown: 1_300, direction: null });
    rerender({ value: 1_300, on: true });
    expect(result.current).toMatchObject({ shown: 1_300, direction: null });
    rerender({ value: null, on: true });
    expect(result.current).toMatchObject({ shown: null, direction: null });
  });
});

describe("a counted number, more cases", () => {
  beforeEach(() => vi.useFakeTimers({ now: 1_000_000 }));
  afterEach(() => vi.useRealTimers());

  it("counts from the first frame, not from the clock's zero", async () => {
    const { result, rerender } = renderHook(({ value }) => useCountedNumber(value, true), {
      initialProps: { value: 0 as number | null },
    });
    // Frames carry the time since the page started: long after it, the count still takes 600 ms.
    await act(async () => {
      vi.advanceTimersByTime(10_000);
    });
    rerender({ value: 600 });
    await act(async () => {
      vi.advanceTimersByTime(COUNT_MS / 2);
    });
    expect(result.current.shown).toBeGreaterThan(0);
    expect(result.current.shown).toBeLessThan(600);
  });

  it("stays still without animation, and leaves only the color timer after a count", async () => {
    const { result, rerender } = renderHook(({ value, on }) => useCountedNumber(value, on), {
      initialProps: { value: 1_000 as number | null, on: false },
    });
    rerender({ value: 1_300, on: false });
    await act(async () => {
      vi.advanceTimersByTime(COUNT_MS / 2);
    });
    expect(result.current.shown).toBe(1_300);
    expect(vi.getTimerCount()).toBe(0);
    rerender({ value: 1_500, on: true });
    await act(async () => {
      vi.advanceTimersByTime(COUNT_MS + 50);
    });
    expect(result.current.shown).toBe(1_500);
    expect(vi.getTimerCount()).toBe(1);
  });

  it("follows a newer change at once: the old count and color timer stop", async () => {
    const { result, rerender } = renderHook(({ value, on }) => useCountedNumber(value, on), {
      initialProps: { value: 1_000 as number | null, on: true },
    });
    rerender({ value: 1_300, on: true });
    await act(async () => {
      vi.advanceTimersByTime(1_000);
    });
    rerender({ value: 1_100, on: true });
    expect(result.current.direction).toBe("down");
    await act(async () => {
      vi.advanceTimersByTime(650);
    });
    // The first change's color timer (due at 1 400 ms) must not end the second one.
    expect(result.current.direction).toBe("down");
    expect(result.current.shown).toBe(1_100);
    rerender({ value: 900, on: false });
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    expect(result.current).toMatchObject({ shown: 900, direction: null });
    // A plain change in the middle of a count stops it: the old frames never come back.
    rerender({ value: 1_500, on: true });
    await act(async () => {
      vi.advanceTimersByTime(100);
    });
    rerender({ value: 700, on: false });
    await act(async () => {
      vi.advanceTimersByTime(300);
    });
    expect(result.current).toMatchObject({ shown: 700, direction: null });
  });
});

describe("FLIP of a list", () => {
  let offset = 0;
  const animate = vi.fn();
  function List({ ids, trigger, on }: { ids: string[]; trigger: string; on: boolean }) {
    const ref = useRef<HTMLUListElement>(null);
    useFlip(ref, trigger, on);
    return (
      <ul ref={ref}>
        {ids.map((id) => (
          <li key={id} data-flip={id}>
            {id}
          </li>
        ))}
      </ul>
    );
  }
  beforeEach(() => {
    offset = 0;
    animate.mockClear();
    Object.defineProperty(Element.prototype, "animate", { value: animate, configurable: true });
    vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (
      this: Element,
    ) {
      const items = Array.from(document.querySelectorAll("li[data-flip]"));
      const top = 100 + items.indexOf(this) * 50 - offset;
      return {
        top,
        left: 0,
        bottom: top + 40,
        right: 0,
        width: 0,
        height: 40,
        x: 0,
        y: top,
      } as DOMRect;
    });
    vi.stubGlobal("innerHeight", 1_000);
  });
  afterEach(() => {
    cleanup();
    delete (Element.prototype as { animate?: unknown }).animate;
    document.documentElement.style.removeProperty("--motion-enter");
    document.documentElement.style.removeProperty("--ease-out");
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });
  const show = (ids: string[], on = true) => <List ids={ids} trigger={ids.join(",")} on={on} />;

  it("glides the items that moved from their old place, by the motion tokens", () => {
    document.documentElement.style.setProperty("--motion-enter", "300ms");
    document.documentElement.style.setProperty("--ease-out", "cubic-bezier(0.2, 0.7, 0.2, 1)");
    const view = render(show(["a", "b"]));
    view.rerender(show(["n", "a", "b"]));
    expect(animate).toHaveBeenCalledTimes(2);
    expect(animate).toHaveBeenCalledWith(
      [{ transform: "translate(0px, -50px)" }, { transform: "none" }],
      { duration: 300, easing: "cubic-bezier(0.2, 0.7, 0.2, 1)" },
    );
  });

  it("falls back to 250 ms ease-out without the tokens", () => {
    const view = render(show(["a"]));
    view.rerender(show(["n", "a"]));
    expect(animate).toHaveBeenCalledWith(expect.anything(), { duration: 250, easing: "ease-out" });
  });

  it("moves nothing when off, and nothing below the window before and after", () => {
    const view = render(show(["a", "b"], false));
    view.rerender(show(["n", "a", "b"], false));
    expect(animate).not.toHaveBeenCalled();
    cleanup();
    vi.stubGlobal("innerHeight", 160);
    // "a" 100 -> 150 and "b" 150 -> 200 were seen; "c" 200 -> 250 is below the window throughout.
    const list = render(show(["a", "b", "c"]));
    animate.mockClear();
    list.rerender(show(["n", "a", "b", "c"]));
    expect(animate).toHaveBeenCalledTimes(2);
  });

  it("glides an item coming up from below the window, not one staying below it", () => {
    vi.stubGlobal("innerHeight", 150);
    // "a" 150 -> 100 comes into view; "b" 200 -> 150 stays at or below the bottom edge.
    const view = render(show(["x", "a", "b"]));
    animate.mockClear();
    view.rerender(show(["a", "b"]));
    expect(animate).toHaveBeenCalledTimes(1);
    expect(animate).toHaveBeenCalledWith(
      [{ transform: "translate(0px, 50px)" }, { transform: "none" }],
      expect.anything(),
    );
  });

  it("does nothing where the browser cannot animate elements", () => {
    delete (Element.prototype as { animate?: unknown }).animate;
    const view = render(show(["a"]));
    expect(() => view.rerender(show(["n", "a"]))).not.toThrow();
  });

  it("keeps positions current on scroll, so a scrolled list does not glide", async () => {
    vi.useFakeTimers();
    try {
      const view = render(<List ids={["a", "b"]} trigger="1" on />);
      offset = 300;
      // A scroll inside an element does not bubble: the window hears it while capturing.
      document.querySelector("ul")!.dispatchEvent(new Event("scroll"));
      await act(async () => {
        vi.advanceTimersByTime(20);
      });
      view.rerender(<List ids={["a", "b"]} trigger="2" on />);
      expect(animate).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps positions current on resize as well", async () => {
    vi.useFakeTimers();
    try {
      const view = render(<List ids={["a", "b"]} trigger="1" on />);
      offset = -80;
      window.dispatchEvent(new Event("resize"));
      await act(async () => {
        vi.advanceTimersByTime(20);
      });
      view.rerender(<List ids={["a", "b"]} trigger="2" on />);
      expect(animate).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it("stops listening when the list goes", () => {
    const removed = vi.spyOn(window, "removeEventListener");
    const view = render(show(["a"]));
    view.unmount();
    const scroll = removed.mock.calls.find(([name]) => name === "scroll");
    expect(scroll?.[2]).toEqual({ capture: true });
    expect(removed.mock.calls.map(([name]) => name)).toContain("resize");
  });
});
