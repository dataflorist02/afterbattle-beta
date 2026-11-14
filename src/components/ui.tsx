import { useEffect, useRef } from "react";
import type { CSSProperties, KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";
import { useGameImage } from "../hooks/useGameImage";
import type { AsyncState } from "../hooks/useAsync";
import { errorMessage } from "../i18n/errors";
import { useLanguage } from "../i18n/language";

export function Card({
  children,
  hero = false,
  className = "",
  labelledBy,
  label,
}: {
  children: ReactNode;
  hero?: boolean;
  className?: string;
  labelledBy?: string;
  label?: string;
}) {
  return (
    <section
      className={`card${hero ? " card-hero" : ""}${className ? ` ${className}` : ""}`}
      aria-labelledby={labelledBy}
      aria-label={label}
    >
      {children}
    </section>
  );
}

export function StatRow({
  label,
  value,
  suffix,
}: {
  label: string;
  value: string;
  suffix?: string;
}) {
  return (
    <div className="card stat-row">
      <span className="stat-label">{label}</span>
      <span>
        <span className="num stat-value">{value}</span>
        {suffix && <span className="stat-suffix">{suffix}</span>}
      </span>
    </div>
  );
}

export function Loading() {
  const { tr } = useLanguage();
  return (
    <div aria-busy="true" role="status" className="grid">
      <span className="visually-hidden">{tr("loading")}</span>
      <div className="skeleton" />
      <div className="skeleton" />
    </div>
  );
}

export function Empty({ text, action }: { text: string; action?: ReactNode }) {
  return (
    <div className="card state">
      <p className="muted">{text}</p>
      {action}
    </div>
  );
}

export function ErrorState({ code, onRetry }: { code: string | undefined; onRetry?: () => void }) {
  const { language, tr } = useLanguage();
  return (
    <div className="card state" role="alert">
      <p className="state-error">{errorMessage(code, language)}</p>
      {onRetry && (
        <button type="button" className="button" onClick={onRetry}>
          {tr("retry")}
        </button>
      )}
    </div>
  );
}

/** Renders loading / error / ready for one request. */
export function Async<T>({
  state,
  onRetry,
  children,
}: {
  state: AsyncState<T>;
  onRetry: () => void;
  children: (data: T) => ReactNode;
}) {
  if (state.status === "loading") return <Loading />;
  if (state.status === "error") return <ErrorState code={state.code} onRetry={onRetry} />;
  return <>{children(state.data)}</>;
}

function TankGlyph() {
  return (
    <svg
      aria-hidden="true"
      width="30"
      height="16"
      viewBox="0 0 30 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect x="2" y="9" width="24" height="5" rx="2.5" />
      <rect x="7" y="4.5" width="11" height="4.5" rx="1.5" />
      <line x1="18" y1="6.5" x2="28" y2="6.5" />
    </svg>
  );
}

function MapGlyph() {
  return (
    <svg
      aria-hidden="true"
      width="16"
      height="16"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <polygon points="3 6 9 4 15 6 21 4 21 18 15 20 9 18 3 20" />
    </svg>
  );
}

/** A vehicle icon from the game folder, or a neutral placeholder (D19). */
export function VehicleImage({
  vehicleKey,
  label,
  width = 52,
  height = 28,
}: {
  vehicleKey: string | null | undefined;
  label: string;
  width?: number;
  height?: number;
}) {
  const url = useGameImage("vehicle", vehicleKey)?.url ?? null;
  return (
    <span
      className="vehicle-image subtle"
      role="img"
      aria-label={label}
      data-loaded={url ? "true" : "false"}
      style={{ width, height }}
    >
      {url ? <img src={url} alt="" /> : <TankGlyph />}
    </span>
  );
}

/** A map image from the game folder, or a neutral placeholder (D19). */
export function MapImage({
  mapKey,
  label,
  width = 44,
  height = 30,
}: {
  mapKey: string | null | undefined;
  label: string;
  width?: number;
  height?: number;
}) {
  const url = useGameImage("map", mapKey)?.url ?? null;
  return (
    <span
      className="map-image subtle"
      role="img"
      aria-label={label}
      data-loaded={url ? "true" : "false"}
      style={{ width, height }}
    >
      {url ? <img src={url} alt="" /> : <MapGlyph />}
    </span>
  );
}

/** Modal dialog with focus inside, Escape to close and focus returned afterwards (U7). */
export function Dialog({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const panel = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const focusable = () =>
      Array.from(
        panel.current?.querySelectorAll<HTMLElement>(
          "button, [href], input, select, textarea, [tabindex]:not([tabindex='-1'])",
        ) ?? [],
      ).filter((element) => !element.hasAttribute("disabled"));
    focusable()[0]?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      } else if (event.key === "Tab") {
        const items = focusable();
        if (items.length === 0) return;
        const first = items[0];
        const last = items[items.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus();
        }
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      previous?.focus();
    };
  }, [onClose]);
  return (
    <div className="dialog-backdrop">
      <div
        ref={panel}
        className="card dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title"
      >
        <h2 id="dialog-title" className="section-title">
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
}

/**
 * One frame with segments, the active one filled (E5-ui §3). A radio group: Tab enters at the
 * checked segment, arrow keys move and select, Home/End jump to the ends.
 */
export function SegmentedControl<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: readonly { value: T; label: string; ariaLabel?: string }[];
  onChange: (value: T) => void;
}) {
  const group = useRef<HTMLDivElement>(null);
  const move = (index: number) => {
    const count = options.length;
    const next = options[((index % count) + count) % count];
    onChange(next.value);
    const radios = group.current?.querySelectorAll<HTMLButtonElement>("[role='radio']");
    radios?.item(options.indexOf(next))?.focus();
  };
  const onKeyDown = (event: ReactKeyboardEvent, index: number) => {
    const keys: Record<string, number> = {
      ArrowRight: index + 1,
      ArrowDown: index + 1,
      ArrowLeft: index - 1,
      ArrowUp: index - 1,
      Home: 0,
      End: options.length - 1,
    };
    if (!(event.key in keys)) return;
    event.preventDefault();
    move(keys[event.key]);
  };
  return (
    <div
      ref={group}
      className="segmented"
      role="radiogroup"
      aria-label={label}
      style={
        {
          "--segment-count": options.length,
          "--segment-index": Math.max(
            0,
            options.findIndex((option) => option.value === value),
          ),
        } as CSSProperties
      }
    >
      <span className="segment-indicator" aria-hidden="true" />
      {options.map((option, index) => {
        const checked = value === option.value;
        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            className="segment"
            aria-checked={checked}
            aria-label={option.ariaLabel}
            tabIndex={checked ? 0 : -1}
            onClick={() => onChange(option.value)}
            onKeyDown={(event) => onKeyDown(event, index)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
