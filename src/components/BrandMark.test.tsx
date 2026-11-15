import { readFileSync } from "node:fs";
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BrandMark, MARK_BAR, MARK_OUTLINE } from "./BrandMark";

const brand = (name: string) => readFileSync(`docs/design/brand/${name}`, "utf8");

describe("brand mark (spec 29 §4)", () => {
  it("draws the shapes of the approved brand files, from one source", () => {
    for (const file of [
      "afterbattle-icon.svg",
      "afterbattle-mark.svg",
      "afterbattle-mark-small.svg",
      "afterbattle-mark-light.svg",
      "afterbattle-loading-reference.svg",
    ]) {
      const svg = brand(file);
      expect(svg, file).toContain(`points="${MARK_OUTLINE}"`);
      expect(svg, file).toContain(`points="${MARK_BAR}"`);
    }
    const { container } = render(<BrandMark variant="mark" size={48} />);
    expect(container.querySelector("polygon")).toHaveAttribute("points", MARK_OUTLINE);
    expect(container.querySelector("polyline")).toHaveAttribute("points", MARK_BAR);
  });

  it("is decorative, sized, and colored only from tokens", () => {
    for (const variant of ["icon", "mark", "mark-small"] as const)
      for (const size of [16, 40, 96]) {
        const { container } = render(<BrandMark variant={variant} size={size} />);
        const svg = container.querySelector("svg")!;
        expect(svg).toHaveAttribute("aria-hidden", "true");
        expect(svg).toHaveAttribute("width", String(size));
        expect(svg).toHaveAttribute("height", String(size));
        expect(svg).toHaveAttribute("viewBox", "0 0 64 64");
        const markup = container.innerHTML;
        expect(markup, `${variant} ${size}`).not.toMatch(/#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/);
        expect(markup, `${variant} ${size}`).toMatch(/var\(--/);
      }
  });

  it("puts the icon on a navy tile with a steel outline and an ember bar", () => {
    const { container } = render(<BrandMark variant="icon" size={40} />);
    const tile = container.querySelector("rect")!;
    expect(tile).toHaveAttribute("rx", "15");
    expect(container.querySelectorAll("radialGradient stop")[0].getAttribute("style")).toContain(
      "var(--navy-hi)",
    );
    expect(container.querySelector("polygon")!.getAttribute("style")).toMatch(/url\("?#brand/);
    expect(container.querySelector("polygon")).toHaveAttribute("stroke-width", "4.4");
    expect(container.querySelector("polyline")!.getAttribute("style")).toContain("var(--accent)");
  });

  it("simplifies the icon below 32 px: flat colors, no gradients, the same thick lines", () => {
    const { container } = render(<BrandMark variant="icon" size={16} />);
    expect(container.querySelector("defs")).toBeNull();
    expect(container.querySelectorAll("rect")).toHaveLength(1);
    expect(container.querySelector("rect")!.getAttribute("style")).toContain("var(--navy)");
    expect(container.querySelector("polygon")!.getAttribute("style")).toContain("var(--text-soft)");
    expect(container.querySelector("polygon")).toHaveAttribute("stroke-width", "4.4");
    const at32 = render(<BrandMark variant="icon" size={32} />);
    expect(at32.container.querySelector("defs")).not.toBeNull();
  });

  it("heats only the bar of the mark, with the ember gradient", () => {
    const { container } = render(<BrandMark variant="mark" size={96} />);
    const stops = Array.from(container.querySelectorAll("linearGradient stop")).map((stop) =>
      stop.getAttribute("style"),
    );
    expect(stops.join(" ")).toMatch(/--ember-white.*--accent-soft.*--ember-lo.*--ember-deep/);
    expect(container.querySelector("polygon")).toHaveAttribute("stroke-width", "1.1");
    expect(container.querySelector("polyline")!.getAttribute("style")).toMatch(/url\("?#brand/);
    const small = render(<BrandMark variant="mark-small" size={24} />);
    expect(small.container.querySelector("defs")).toBeNull();
    expect(small.container.querySelector("polygon")).toHaveAttribute("stroke-width", "3.4");
  });

  it("gives two marks on one page their own gradient ids", () => {
    const { container } = render(
      <>
        <BrandMark variant="icon" size={40} />
        <BrandMark variant="icon" size={40} />
      </>,
    );
    const ids = Array.from(container.querySelectorAll("[id]")).map((node) => node.id);
    expect(ids.length).toBeGreaterThanOrEqual(4);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of ids) expect(id).toMatch(/^brand[a-z0-9]+$/i);
  });
});
