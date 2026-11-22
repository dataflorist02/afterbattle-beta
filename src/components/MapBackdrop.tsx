/**
 * A map picture from the game files as a background (E5-flare): headers, tiles, row strips and
 * cards. The picture loads only when the element is near the screen. E5-maps-hd: it is asked
 * for at the element's measured size in device pixels, so the backend sends it sharp at that
 * size; a picture that would still need more than 1.25× is never stretched: it stays at that
 * scale on the right, fading into a blurred copy of itself. Without the game's picture a
 * gradient placeholder stays, chosen from the map key so the same map always looks the same.
 * The element says in its label which map it shows.
 */
import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import {
  MAX_UPSCALE,
  boxSize,
  needsSoftFit,
  useGameImage,
  useVisible,
} from "../hooks/useGameImage";
import type { Size } from "../hooks/useGameImage";

/** Height of a map picture for its width (500 × 235), the estimate before layout. */
const ESTIMATED_PROPORTION = 235 / 500;

function pixelRatio(): number {
  return typeof window !== "undefined" && window.devicePixelRatio > 0 ? window.devicePixelRatio : 1;
}

/** The element's box in device pixels; before layout (or without it), estimated from `width`. */
function useBox(element: HTMLElement | null, width: number): Size {
  const estimate = () => boxSize(width, width * ESTIMATED_PROPORTION, pixelRatio());
  const [box, setBox] = useState<Size>(estimate);
  useEffect(() => {
    if (!element) return;
    const measure = () => {
      const rect = element.getBoundingClientRect();
      const next =
        rect.width > 0 && rect.height > 0
          ? boxSize(rect.width, rect.height, pixelRatio())
          : estimate();
      setBox((old) => (old.width === next.width && old.height === next.height ? old : next));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [element, width]);
  return box;
}

export const PLACEHOLDERS = 6;

/** Stable placeholder index for a map key (sum of character codes). */
export function placeholderIndex(mapKey: string | null | undefined): number {
  if (!mapKey) return 0;
  let sum = 0;
  for (const char of mapKey) sum += char.charCodeAt(0);
  return sum % PLACEHOLDERS;
}

export function MapBackdrop({
  mapKey,
  width,
  className = "",
  label,
  children,
  as = "div",
  onClick,
  style,
  ...rest
}: {
  mapKey: string | null | undefined;
  /** Estimated CSS width before the element is measured. */
  width: number;
  className?: string;
  /** Accessible description of the background, e.g. "Background: the map Synthetic Hills". */
  label?: string;
  children?: ReactNode;
  as?: "div" | "section" | "header" | "button";
  onClick?: () => void;
  style?: CSSProperties;
  "aria-labelledby"?: string;
  "aria-label"?: string;
  title?: string;
  "data-testid"?: string;
  /** Spec 33: the temperature of the card against the player's own average. */
  "data-temp"?: string;
  /** Lets a summary reference highlight this element (spec 30 §8). */
  id?: string;
}) {
  const [element, setElement] = useState<HTMLElement | null>(null);
  const visible = useVisible(element);
  const box = useBox(element, width);
  const image = useGameImage("map", mapKey, { box, enabled: visible });
  const url = image?.url ?? null;
  const soft = image ? needsSoftFit(box, image) : false;
  const Tag = as;
  return (
    <Tag
      ref={setElement as never}
      type={as === "button" ? "button" : undefined}
      onClick={onClick}
      className={`map-backdrop map-ph-${placeholderIndex(mapKey)} ${className}`.trim()}
      data-loaded={url ? "true" : "false"}
      data-fit={image ? (soft ? "soft" : "cover") : undefined}
      style={
        {
          ...style,
          ...(url ? { "--map-picture": `url("${url}")` } : {}),
          ...(image && soft
            ? {
                "--map-picture-width": `${Math.round((image.width * MAX_UPSCALE) / pixelRatio())}px`,
              }
            : {}),
        } as CSSProperties
      }
      {...rest}
    >
      <span className="map-backdrop-picture" aria-hidden="true" />
      {soft && <span className="map-backdrop-sharp" aria-hidden="true" />}
      {label && <span className="visually-hidden">{label}</span>}
      {children}
    </Tag>
  );
}
