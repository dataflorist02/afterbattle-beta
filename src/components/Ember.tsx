/**
 * Layers of Żar (spec 33): the breath of the one hot spot, the sweep over a new thing, the heat
 * behind a changed number, the flash of a completed goal and the sparks over a warmer session.
 * All are spans hidden from readers; only their opacity and transform move (base.css).
 */
import { useEffect } from "react";
import type { CSSProperties, ReactNode } from "react";
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { sparkCount, type Heat, type Temperature } from "../lib/ember";
import { prefersReducedMotion } from "../lib/motion";

export const TEMPERATURE_KEYS: Record<Temperature, UiKey> = {
  warmer: "tempWarmer",
  usual: "tempUsual",
  cooler: "tempCooler",
};

/** "cieplej niż zwykle" / "jak zwykle" / "chłodniej niż zwykle": the color said in words. */
export function TemperatureWord({ heat }: { heat: Heat | null }) {
  const { tr } = useLanguage();
  if (!heat) return null;
  return <span className="temp-word">{tr(TEMPERATURE_KEYS[heat.temperature])}</span>;
}

/**
 * The breath of the hot spot (author, 22.11: no frame). Inside the main number (`ember-figure`)
 * it carries the number's `text` and draws a soft glow of the same shape behind it; only its
 * opacity breathes. Without text (the Damast spark) it is the round dot's glow.
 */
export function EmberBreath({ text }: { text?: string }) {
  return <span className="ember-breath" aria-hidden="true" data-text={text} />;
}

/**
 * The main number of a place; when the place is the hot spot it glows (author, 22.11): ember
 * core, with the breathing glow of its own text behind it. `children` follow the text (e.g. the
 * heat of a changed number).
 */
export function EmberNumber({
  hot,
  className,
  text,
  children,
}: {
  hot: boolean;
  className: string;
  text: string;
  children?: ReactNode;
}) {
  return (
    <span className={hot ? `${className} ember-figure` : className}>
      {text}
      {hot && <EmberBreath text={text} />}
      {children}
    </span>
  );
}

/** One pass of a bright band and an edge that cools; the parent carries `ember-swept`. */
export function EmberSweep() {
  return (
    <>
      <span className="ember-sweep" aria-hidden="true" />
      <span className="ember-edge" aria-hidden="true" />
    </>
  );
}

/** The heat behind a number that just changed; a new `key` starts it again. */
export function EmberHeat() {
  return <span className="ember-heat" aria-hidden="true" />;
}

export function EmberFlash() {
  return <span className="ember-flash" aria-hidden="true" />;
}

/** Sparks in fixed places (the same card looks the same every time), none with reduced motion. */
export function Sparks({ heat }: { heat: Heat | null }) {
  const count = prefersReducedMotion() ? 0 : sparkCount(heat);
  if (count === 0) return null;
  return (
    <span className="ember-sparks" aria-hidden="true">
      {Array.from({ length: count }, (_, index) => {
        const size = 2 + (index % 3);
        return (
          <span
            key={index}
            className="ember-spark"
            style={
              {
                left: `${((index * 61 + 7) % 96) + 2}%`,
                bottom: `${((index * 37 + 3) % 55) + 2}%`,
                width: `${size}px`,
                height: `${size}px`,
                opacity: 0.35 + ((index * 13) % 60) / 100,
                "--spark-index": index,
              } as CSSProperties
            }
          />
        );
      })}
    </span>
  );
}

/**
 * Marks the page while the app window is in the background (no focus or hidden), which is where
 * it is while the game is played: the breath of the hot spot then stands still. Nothing here
 * looks at the game; the window's own focus is all it knows.
 */
export function useWindowBackground(): void {
  useEffect(() => {
    const root = document.documentElement;
    const set = (background: boolean) => {
      if (background) root.dataset.window = "background";
      else delete root.dataset.window;
    };
    const onBlur = () => set(true);
    const onFocus = () => set(false);
    const onVisibility = () => set(document.visibilityState === "hidden");
    window.addEventListener("blur", onBlur);
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("blur", onBlur);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
      set(false);
    };
  }, []);
}
