import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { LanguageProvider } from "../i18n/language";
import { DamageChart, chartX, placeTip, readingDate, type ChartBattle } from "./DamageChart";

const NOW = Date.UTC(2026, 10, 20, 18, 0);
const values = [2_400, 2_650, 2_500, 2_900, 3_100];
/** Synthetic battles under the five points (oldest first). */
const battles: ChartBattle[] = values.map((_, index) => ({
  number: 96 + index,
  time: NOW + index * 3_600_000,
  damage: 2_000 + index * 450,
  map: index % 2 ? "Synthetic Hills" : "Test Valley",
  tank: "Alpha",
}));

function renderChart(average: number | null = 2_702) {
  const view = render(
    <LanguageProvider initial="pl">
      <DamageChart values={values} average={average} battles={battles} />
    </LanguageProvider>,
  );
  const plot = view.container.querySelector(".chart-plot") as HTMLElement;
  // jsdom lays nothing out: the plot is as large as the chart's own units.
  vi.spyOn(plot, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    width: 640,
    height: 200,
    right: 640,
    bottom: 200,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  return { ...view, plot };
}

const spaced = (text: string | null | undefined) => (text ?? "").replace(/\s/gu, " ");
const tip = (container: HTMLElement) => container.querySelector(".chart-tip");
const live = (container: HTMLElement) =>
  container.querySelector(".chart-figure [aria-live='polite']");

function reduceMotion() {
  vi.stubGlobal("matchMedia", (query: string) => ({
    matches: query.includes("reduce"),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  }));
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("the line chart's average (author, 22.11)", () => {
  it("names the average in a legend above the chart, with a sample of the dashed line", () => {
    const { container } = renderChart();
    const legend = container.querySelector(".chart-legend")!;
    expect(spaced(legend.textContent)).toBe("Twoja średnia 2 702");
    expect(legend.querySelector("svg line.chart-legend-dash")).not.toBeNull();
    // The legend stands above the plot.
    const figure = container.querySelector(".chart-figure")!;
    expect(Array.from(figure.children).indexOf(legend)).toBeLessThan(
      Array.from(figure.children).indexOf(container.querySelector(".chart-plot")!),
    );
  });

  it("keeps the dashed line on the chart without any text by it", () => {
    const { container } = renderChart();
    const svg = container.querySelector("svg.chart")!;
    expect(svg.querySelector(".chart-average line")).not.toBeNull();
    expect(svg.querySelector(".chart-average text")).toBeNull();
    expect(svg.textContent).not.toMatch(/średnia/u);
  });

  it("has no legend without an average", () => {
    const { container } = renderChart(null);
    expect(container.querySelector(".chart-legend")).toBeNull();
    expect(container.querySelector(".chart-average")).toBeNull();
  });
});

describe("reading the line chart (author, 22.11)", () => {
  it("shows the battle under the pointer: a vertical line, a point, and its values", () => {
    const { container, plot } = renderChart();
    expect(tip(container)).toBeNull();
    fireEvent.mouseMove(plot, { clientX: chartX(3, values.length) + 4, clientY: 100 });
    const cursor = container.querySelector("line.chart-cursor")!;
    expect(Number(cursor.getAttribute("x1"))).toBeCloseTo(chartX(3, values.length), 1);
    expect(cursor.getAttribute("x1")).toBe(cursor.getAttribute("x2"));
    const point = container.querySelector("circle.chart-point")!;
    expect(point.getAttribute("cx")).toBe(cursor.getAttribute("x1"));
    const text = spaced(tip(container)?.textContent);
    expect(text).toContain("Bitwa 99");
    expect(text).toContain(readingDate(battles[3].time!, "pl"));
    expect(text).toContain("obrażenia 3 350");
    expect(text).toContain("średnia krocząca 2 900");
    expect(text).toContain("Synthetic Hills");
    expect(text).toContain("Alpha");
    expect(spaced(live(container)?.textContent)).toContain("Bitwa 99");
    fireEvent.mouseLeave(plot);
    expect(tip(container)).toBeNull();
    expect(container.querySelector(".chart-cursor")).toBeNull();
  });

  it("moves the reading with the keyboard: arrows, Home, End, Esc", () => {
    const { container, plot } = renderChart();
    expect(plot).toHaveAttribute("tabindex", "0");
    act(() => plot.focus());
    const reading = () => spaced(live(container)?.textContent);
    fireEvent.keyDown(plot, { key: "ArrowLeft" });
    // The first arrow starts at the newest battle, the highlighted end of the line.
    expect(reading()).toContain("Bitwa 100");
    expect(reading()).toContain("obrażenia 3 800");
    fireEvent.keyDown(plot, { key: "ArrowLeft" });
    expect(reading()).toContain("Bitwa 99");
    fireEvent.keyDown(plot, { key: "Home" });
    expect(reading()).toContain("Bitwa 96");
    expect(reading()).toContain("Test Valley");
    fireEvent.keyDown(plot, { key: "ArrowLeft" });
    expect(reading()).toContain("Bitwa 96");
    fireEvent.keyDown(plot, { key: "ArrowRight" });
    expect(reading()).toContain("Bitwa 97");
    expect(spaced(tip(container)?.textContent)).toContain("obrażenia 2 450");
    fireEvent.keyDown(plot, { key: "End" });
    expect(reading()).toContain("Bitwa 100");
    fireEvent.keyDown(plot, { key: "Escape" });
    expect(tip(container)).toBeNull();
    expect(reading()).toBe("");
  });

  it("announces the reading politely", () => {
    const { container } = renderChart();
    expect(live(container)).toHaveAttribute("aria-live", "polite");
  });

  it("lets the tip appear without motion with reduced motion", () => {
    const first = renderChart();
    fireEvent.keyDown(first.plot, { key: "End" });
    expect(tip(first.container)).toHaveClass("chart-tip-in");
    first.unmount();
    reduceMotion();
    const second = renderChart();
    fireEvent.keyDown(second.plot, { key: "End" });
    expect(tip(second.container)).not.toBeNull();
    expect(tip(second.container)).not.toHaveClass("chart-tip-in");
  });

  it("reads a chart without battles by the point's number and value alone", () => {
    const { container } = render(
      <LanguageProvider initial="en">
        <DamageChart values={values} average={null} />
      </LanguageProvider>,
    );
    const plot = container.querySelector(".chart-plot") as HTMLElement;
    fireEvent.keyDown(plot, { key: "Home" });
    const head = container.querySelector(".chart-tip-head");
    expect(spaced(head?.textContent)).toBe("Battle 1");
    expect(spaced(tip(container)?.textContent)).toBe("Battle 1rolling average 2,400");
    expect(screen.getAllByText(/^Battle 1, rolling average 2,400$/u)).toHaveLength(1);
  });
});

describe("the reading's tip stays in the chart and off the point", () => {
  const plot = { width: 600, height: 200 };
  const size = { width: 160, height: 90 };
  const inside = (place: { left: number; top: number }) => {
    expect(place.left).toBeGreaterThanOrEqual(0);
    expect(place.top).toBeGreaterThanOrEqual(0);
    expect(place.left + size.width).toBeLessThanOrEqual(plot.width);
    expect(place.top + size.height).toBeLessThanOrEqual(plot.height);
  };
  const clear = (place: { left: number; top: number }, point: { x: number; y: number }) => {
    const covers =
      point.x + 6 > place.left &&
      point.x - 6 < place.left + size.width &&
      point.y + 6 > place.top &&
      point.y - 6 < place.top + size.height;
    expect(covers).toBe(false);
  };

  it("sits right of the point, centred on it", () => {
    const point = { x: 100, y: 100 };
    const place = placeTip(plot, size, point);
    expect(place.side).toBe("right");
    expect(place.left).toBe(112);
    expect(place.top).toBe(55);
    inside(place);
    clear(place, point);
  });

  it("bounces to the left near the right edge and stays inside at the top and bottom", () => {
    for (const point of [
      { x: 560, y: 100 },
      { x: 590, y: 4 },
      { x: 590, y: 196 },
      { x: 8, y: 4 },
      { x: 8, y: 196 },
    ]) {
      const place = placeTip(plot, size, point);
      expect(place.side).toBe(point.x > 300 ? "left" : "right");
      inside(place);
      clear(place, point);
    }
  });

  it("goes above or below the point when neither side has room", () => {
    const narrow = { width: 200, height: 200 };
    const low = placeTip(narrow, size, { x: 100, y: 150 });
    expect(low.side).toBe("above");
    const high = placeTip(narrow, size, { x: 100, y: 40 });
    expect(high.side).toBe("below");
    for (const place of [low, high]) {
      expect(place.left).toBeGreaterThanOrEqual(0);
      expect(place.left + size.width).toBeLessThanOrEqual(narrow.width);
    }
    expect(low.top + size.height).toBeLessThanOrEqual(150 - 6);
    expect(high.top).toBeGreaterThanOrEqual(40 + 6);
  });
});
