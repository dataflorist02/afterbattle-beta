import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { BrandMark, MARK_BAR, MARK_OUTLINE } from "../../components/BrandMark";
import { CARD_LOGO_VARIANT, CARD_WORDMARK, brandLogoSvg } from "./logo";

describe("the card's logo (E14 polish, point 1)", () => {
  it("is the BrandMark of the navigation rail, rendered from the component", () => {
    const svg = brandLogoSvg(40, () => "");
    expect(svg.startsWith('<svg xmlns="http://www.w3.org/2000/svg" ')).toBe(true);
    expect(svg).toContain('viewBox="0 0 64 64"');
    expect(svg).toContain(`points="${MARK_OUTLINE}"`);
    expect(svg).toContain(`points="${MARK_BAR}"`);
    // The same markup as the component on screen (the rail's icon), ids aside.
    const { container } = render(<BrandMark variant={CARD_LOGO_VARIANT} size={40} />);
    const withoutIds = (markup: string) =>
      markup
        .replace(' xmlns="http://www.w3.org/2000/svg"', "")
        .replace(/brand[a-z0-9]+(navy|silver)/gi, "id-$1");
    expect(withoutIds(svg)).toBe(withoutIds(container.innerHTML));
    expect(CARD_WORDMARK).toBe("AFTERBATTLE");
  });

  it("resolves the colour tokens, since an SVG image does not see the page's CSS", () => {
    const tokens: Record<string, string> = { "--navy": "#16223a", "--accent": "#f08a3c" };
    const svg = brandLogoSvg(64, (name) => tokens[name] ?? "");
    expect(svg).toContain("stroke: #f08a3c;");
    expect(svg).toContain("stop-color: #16223a;");
    // A token without a value stays as it was (the browser then uses its default).
    expect(svg).toMatch(/var\(--navy-hi\)/);
  });
});
