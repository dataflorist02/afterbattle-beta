/**
 * The ⓘ by the Damast heading on Session (author, 22.11): "Jak czytać tę płytkę". A small
 * button opens a popover next to it — not modal: Esc, a click outside or a second click on the
 * button close it, and the focus goes back to the button. Its rows read this session's plate
 * from the same numbers the plate is drawn from (guide.ts), each with a small legend mark, and
 * lead to the Damast entry of "Jak to działa". Not on the card PNG, not in History.
 */
import { useContext, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties, KeyboardEvent as ReactKeyboardEvent } from "react";
import { HowItWorksContext } from "../../components/HelpLink";
import { useLanguage } from "../../i18n/language";
import { prefersReducedMotion } from "../../lib/motion";
import type { DamastSession } from "../../types";
import { plateGuide, type GuideKind } from "./guide";
import type { DamastStats } from "./stats";

/** Room kept between the popover and the window's edges, and between it and the button. */
const MARGIN = 8;
const GAP = 6;

export interface PopoverPlace {
  /** Horizontal move from the button's left edge (negative: to the left). */
  shift: number;
  side: "below" | "above";
}

/**
 * Where the popover goes so it stays inside the window: under the button from its left edge,
 * moved left near the right edge (never past the left one), above the button when there is no
 * room below and more above.
 */
export function fitPopover(
  anchor: { left: number; top: number; bottom: number },
  size: { width: number; height: number },
  view: { width: number; height: number },
): PopoverPlace {
  let shift = 0;
  if (anchor.left + size.width > view.width - MARGIN)
    shift = view.width - MARGIN - size.width - anchor.left;
  if (anchor.left + shift < MARGIN) shift = MARGIN - anchor.left;
  const below = view.height - MARGIN - (anchor.bottom + GAP);
  const above = anchor.top - GAP - MARGIN;
  const side = below >= size.height || below >= above ? "below" : "above";
  return { shift, side };
}

/** The small mark in front of each row, drawn like the thing it names. */
function LegendMark({ kind }: { kind: GuideKind }) {
  return (
    <svg
      className="damast-legend-mark"
      data-kind={kind}
      viewBox="0 0 18 12"
      width={18}
      height={12}
      aria-hidden="true"
    >
      {kind === "order" && <path className="mark-line" d="M1 6h14M11 2.5 15 6l-4 3.5" />}
      {kind === "tint" && (
        <>
          <rect className="mark-warm" x={1} y={2} width={8} height={8} rx={2} />
          <rect className="mark-cool" x={9} y={2} width={8} height={8} rx={2} />
        </>
      )}
      {kind === "eyes" && <ellipse className="mark-line" cx={9} cy={6} rx={6} ry={3.5} />}
      {kind === "tiers" && <path className="mark-line" d="M1 2.5h16M1 5h16M1 7.5h16M1 10h16" />}
      {kind === "waves" && <path className="mark-line" d="M1 6c2.5-6 5.5-6 8 0s5.5 6 8 0" />}
      {kind === "spark" && <circle className="mark-spark" cx={9} cy={6} r={3.5} />}
    </svg>
  );
}

export function DamastInfo({
  damast,
  stats,
  mapOf,
}: {
  damast: DamastSession;
  stats: DamastStats;
  /** The map of a battle of this session (the spark's row names it). */
  mapOf: (battleId: string) => string | null;
}) {
  const { language, tr } = useLanguage();
  const openHow = useContext(HowItWorksContext);
  const [open, setOpen] = useState(false);
  const [place, setPlace] = useState<PopoverPlace>({ shift: 0, side: "below" });
  const id = `damast-info-${useId().replace(/[^\w-]/g, "")}`;
  const wrap = useRef<HTMLSpanElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  // Enter and Space toggle on the key; the click a browser may send after it is ignored.
  const keyed = useRef(false);

  useEffect(() => {
    if (!open) return;
    const close = (focus: boolean) => {
      setOpen(false);
      if (focus) button.current?.focus();
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      close(true);
    };
    const onDown = (event: MouseEvent) => {
      const target = event.target as Element | null;
      if (target && wrap.current?.contains(target)) return;
      // A click on another control keeps the focus there; anywhere else it goes back to ⓘ.
      close(!target?.closest?.("button, a, input, select, textarea, [tabindex]"));
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onDown);
    };
  }, [open]);

  useLayoutEffect(() => {
    if (!open || !wrap.current || !panel.current) return;
    const anchor = wrap.current.getBoundingClientRect();
    setPlace(
      fitPopover(
        anchor,
        { width: panel.current.offsetWidth, height: panel.current.offsetHeight },
        { width: window.innerWidth, height: window.innerHeight },
      ),
    );
  }, [open, language]);

  const onKeyDown = (event: ReactKeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    keyed.current = true;
    setOpen((value) => !value);
  };
  const onKeyUp = () => {
    setTimeout(() => {
      keyed.current = false;
    }, 0);
  };

  const rows = open
    ? plateGuide(damast, stats, mapOf, language, tr).map((row) => ({
        ...row,
        text: row.lines.join(" "),
      }))
    : [];
  return (
    <span ref={wrap} className="damast-info">
      <button
        ref={button}
        type="button"
        className="damast-info-button"
        aria-label={tr("damastInfoOpen")}
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => {
          if (keyed.current) return;
          setOpen((value) => !value);
        }}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
      >
        <svg viewBox="0 0 20 20" width={18} height={18} aria-hidden="true">
          <circle cx={10} cy={10} r={8.25} fill="none" stroke="currentColor" strokeWidth={1.5} />
          <path d="M10 9v5" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" />
          <circle cx={10} cy={6.2} r={1.1} fill="currentColor" />
        </svg>
      </button>
      {open && (
        <div
          ref={panel}
          id={id}
          role="dialog"
          aria-modal="false"
          aria-label={tr("damastInfoOpen")}
          className={
            prefersReducedMotion() ? "damast-info-panel" : "damast-info-panel damast-info-in"
          }
          data-side={place.side}
          style={{ "--info-shift": `${place.shift}px` } as CSSProperties}
        >
          <ul className="damast-guide">
            {rows.map((row) => (
              <li key={row.kind} data-kind={row.kind}>
                <LegendMark kind={row.kind} />
                <span>{row.text}</span>
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="link damast-info-more"
            onClick={() => {
              setOpen(false);
              openHow("damast");
            }}
          >
            {tr("damastInfoMore")}
          </button>
        </div>
      )}
    </span>
  );
}
