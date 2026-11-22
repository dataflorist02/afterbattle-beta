/**
 * The app's logo on the Damast card (E14 polish, point 1): the same monogram as the navigation
 * rail and the title bar, rendered from `BrandMark` itself (no copy of its SVG), next to the
 * app's name in the brand's type (docs/design/brand: Big Shoulders 300, spaced). A canvas
 * cannot draw a React component, so the component is rendered once to SVG markup; the colour
 * tokens it uses are resolved to their values, since an SVG drawn as an image does not see the
 * page's CSS.
 */
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import { BrandMark } from "../../components/BrandMark";
import { APP_NAME } from "../../release";

/** The monogram's variant on the card: the icon of the navigation rail. */
export const CARD_LOGO_VARIANT = "icon" as const;
/** The name next to the monogram, as the brand writes it in the logo. */
export const CARD_WORDMARK = APP_NAME.toUpperCase();

/** `BrandMark` as SVG markup, `size` CSS pixels, with `var(--token)` resolved by `token`. */
export function brandLogoSvg(size: number, token: (name: string) => string): string {
  const host = document.createElement("div");
  const root = createRoot(host);
  flushSync(() => root.render(<BrandMark variant={CARD_LOGO_VARIANT} size={size} />));
  const markup = host.innerHTML;
  root.unmount();
  return markup
    .replace(/var\(--([\w-]+)\)/g, (whole, name: string) => token(`--${name}`) || whole)
    .replace(/^<svg /, '<svg xmlns="http://www.w3.org/2000/svg" ');
}

/** The tokens of the app's stylesheet. */
function appToken(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** The monogram as an image to draw on the card. */
export function loadBrandLogo(size: number): Promise<HTMLImageElement> {
  const svg = brandLogoSvg(size, appToken);
  return new Promise((resolve, reject) => {
    const image = new Image(size, size);
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("damast: the logo did not load"));
    image.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
  });
}
