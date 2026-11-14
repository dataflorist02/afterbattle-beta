import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";
import * as fx from "../test/fixtures";

const mocks = vi.hoisted(() => ({ gameImage: vi.fn(), vehicleNames: vi.fn() }));
vi.mock("../api", () => ({ api: mocks }));

const { MapBackdrop, placeholderIndex, PLACEHOLDERS } = await import("./MapBackdrop");
const { HistoryRows, groupByDay, dayLabel } = await import("./HistoryRows");
const { BattleStrip } = await import("./session/SessionStory");
const { clearGameImages, scaledImage, pictureSize, boxSize, needsSoftFit } =
  await import("../hooks/useGameImage");

const NOW = new Date(2026, 10, 14, 12, 0).getTime();
const at = (day: number, hour = 21) => new Date(2026, 10, day, hour, 0).getTime();

async function flush() {
  for (let index = 0; index < 4; index++) await act(async () => Promise.resolve());
}

beforeEach(() => {
  clearGameImages();
  mocks.gameImage.mockReset().mockResolvedValue(new ArrayBuffer(0));
});
afterEach(cleanup);

describe("day headers (E5-flare §5)", () => {
  it("groups consecutive battles by calendar day and names today and yesterday", () => {
    const groups = groupByDay(
      [
        { battleTime: at(14, 9) },
        { battleTime: at(14, 1) },
        { battleTime: at(13) },
        { battleTime: at(10) },
      ],
      NOW,
    );
    expect(groups.map((group) => [group.days, group.items.length])).toEqual([
      [0, 2],
      [1, 1],
      [4, 1],
    ]);
    const tr = (key: string) => key;
    expect(dayLabel(at(14), NOW, "pl", tr as never)).toBe("dayToday");
    expect(dayLabel(at(13), NOW, "pl", tr as never)).toBe("dayYesterday");
    expect(dayLabel(at(10), NOW, "pl", tr as never)).toBe("10 listopada 2026");
    expect(groupByDay([], NOW)).toEqual([]);
  });
});

describe("map pictures (E5-flare §0, §7)", () => {
  it("chooses a stable placeholder per map", () => {
    expect(placeholderIndex("91_test_valley")).toBe(placeholderIndex("91_test_valley"));
    expect(placeholderIndex(null)).toBe(0);
    const spread = new Set(
      ["01_a", "02_b", "03_c", "04_d", "05_e", "06_f", "07_g"].map(placeholderIndex),
    );
    expect(spread.size).toBeGreaterThan(2);
    for (const value of spread) expect(value).toBeLessThan(PLACEHOLDERS);
  });

  it("asks for a picture only once the element is visible", async () => {
    const observers: { callback: IntersectionObserverCallback; observed: Element[] }[] = [];
    class FakeObserver {
      observed: Element[] = [];
      constructor(public callback: IntersectionObserverCallback) {
        observers.push(this);
      }
      observe(element: Element) {
        this.observed.push(element);
      }
      disconnect() {}
    }
    vi.stubGlobal("IntersectionObserver", FakeObserver);
    try {
      render(<MapBackdrop mapKey="91_test_valley" width={160} data-testid="tile" />);
      await flush();
      expect(mocks.gameImage).not.toHaveBeenCalled();
      expect(screen.getByTestId("tile")).toHaveAttribute("data-loaded", "false");
      await act(async () =>
        observers[0].callback(
          [{ isIntersecting: true } as IntersectionObserverEntry],
          observers[0] as unknown as IntersectionObserver,
        ),
      );
      await flush();
      // Before layout (jsdom): the estimate from `width`, 160 × 75 CSS px in device-pixel steps.
      expect(mocks.gameImage).toHaveBeenCalledWith("map", "91_test_valley", 192, 80);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it("keeps the original bytes when the engine cannot scale and scales otherwise", async () => {
    const bytes = new Uint8Array([1, 2, 3]).buffer;
    expect((await scaledImage(bytes, { width: 320, height: 100 })).size).toBe(3);
    expect((await scaledImage(bytes, undefined)).size).toBe(3);
    const drawn: number[][] = [];
    vi.stubGlobal("createImageBitmap", async () => ({ width: 1000, height: 470, close() {} }));
    vi.stubGlobal(
      "OffscreenCanvas",
      class {
        constructor(
          public width: number,
          public height: number,
        ) {}
        getContext() {
          return { drawImage: (_: unknown, ...args: number[]) => drawn.push(args) };
        }
        async convertToBlob() {
          return new Blob([new Uint8Array(7)]);
        }
      },
    );
    try {
      expect((await scaledImage(bytes, { width: 320, height: 100 })).size).toBe(7);
      expect(drawn[0]).toEqual([0, 0, 320, 150]);
      // A tall box: scaled by height, so `cover` never stretches it again (E5-maps-hd §5).
      expect((await scaledImage(bytes, { width: 128, height: 128 })).size).toBe(7);
      expect(drawn[1]).toEqual([0, 0, 272, 128]);
      expect((await scaledImage(bytes, { width: 2000, height: 100 })).size).toBe(3);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

/** Header bytes of a synthetic PNG of the given size (no real picture). */
function pngHeader(width: number, height: number): ArrayBuffer {
  const bytes = new Uint8Array(32);
  bytes.set([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  new DataView(bytes.buffer).setUint32(16, width);
  new DataView(bytes.buffer).setUint32(20, height);
  return bytes.buffer;
}

describe("sharp map pictures (E5-maps-hd)", () => {
  it("reads the picture size from the PNG header", () => {
    expect(pictureSize(pngHeader(500, 235))).toEqual({ width: 500, height: 235 });
    expect(pictureSize(new Uint8Array([1, 2, 3]).buffer)).toEqual({ width: 0, height: 0 });
  });

  it("asks for the box in device pixels, rounded up in steps", () => {
    expect(boxSize(160, 75, 1)).toEqual({ width: 192, height: 80 });
    expect(boxSize(1000, 60, 2)).toEqual({ width: 2048, height: 128 });
    expect(boxSize(2200, 520, 1.25)).toEqual({ width: 2752, height: 656 });
    expect(boxSize(0.2, 0.2, 1)).toEqual({ width: 64, height: 16 });
    expect(boxSize(5000, 3000, 2)).toEqual({ width: 4096, height: 4096 });
  });

  it("never shows a picture more than 1.25 times its pixels", () => {
    expect(needsSoftFit({ width: 625, height: 100 }, { width: 500, height: 235 })).toBe(false);
    expect(needsSoftFit({ width: 626, height: 100 }, { width: 500, height: 235 })).toBe(true);
    expect(needsSoftFit({ width: 100, height: 294 }, { width: 500, height: 235 })).toBe(true);
    expect(needsSoftFit({ width: 2048, height: 96 }, { width: 2048, height: 96 })).toBe(false);
    expect(needsSoftFit({ width: 100, height: 100 }, { width: 0, height: 0 })).toBe(false);
  });

  it("shows a small picture only on the right over a blurred backdrop, a sharp one as it is", async () => {
    const created: Blob[] = [];
    const original = URL.createObjectURL;
    URL.createObjectURL = (blob: Blob) => {
      created.push(blob);
      return `blob:synthetic-${created.length}`;
    };
    try {
      mocks.gameImage.mockResolvedValue(pngHeader(500, 235));
      render(<MapBackdrop mapKey="01_small" width={1400} data-testid="small" />);
      await flush();
      const small = screen.getByTestId("small");
      expect(mocks.gameImage).toHaveBeenCalledWith("map", "01_small", 1408, 672);
      expect(small).toHaveAttribute("data-fit", "soft");
      expect(small.querySelector(".map-backdrop-sharp")).not.toBeNull();
      // At most 1.25 × 500 px wide (device pixel ratio 1).
      expect(small.style.getPropertyValue("--map-picture-width")).toBe("625px");
      cleanup();
      mocks.gameImage.mockResolvedValue(pngHeader(1408, 672));
      render(<MapBackdrop mapKey="02_sharp" width={1400} data-testid="sharp" />);
      await flush();
      const sharp = screen.getByTestId("sharp");
      expect(sharp).toHaveAttribute("data-fit", "cover");
      expect(sharp.querySelector(".map-backdrop-sharp")).toBeNull();
      expect(created.at(-1)?.type).toBe("image/png");
    } finally {
      URL.createObjectURL = original;
    }
  });
});

describe("render time (E5-flare §7)", () => {
  it("draws a session strip of 30 tiles and 200 history rows quickly", () => {
    const battles = Array.from({ length: 30 }, (_, index) => fx.battle(index));
    let started = performance.now();
    render(
      <LanguageProvider initial="pl">
        <BattleStrip
          battles={battles}
          bestBattleId={battles[3].arenaId}
          newBattleIds={[]}
          onOpenBattle={() => undefined}
        />
      </LanguageProvider>,
    );
    const strip = performance.now() - started;
    expect(screen.getAllByRole("button")).toHaveLength(30);
    cleanup();
    const rows = fx.historyPage(200).items;
    started = performance.now();
    render(
      <LanguageProvider initial="pl">
        <HistoryRows items={rows} onOpenBattle={() => undefined} now={NOW} />
      </LanguageProvider>,
    );
    const history = performance.now() - started;
    expect(screen.getAllByRole("button")).toHaveLength(200);
    console.info(
      `render ms: strip of 30 = ${strip.toFixed(0)}, history of 200 = ${history.toFixed(0)}`,
    );
    expect(strip).toBeLessThan(1_000);
    expect(history).toBeLessThan(3_000);
  });
});
