/**
 * Damast at the end of a session (spec 34 §3): the plate, forged once after the session has
 * ended (never during it), the battle bar under it with one spark at the best battle, the
 * session's numbers and the two actions. Labels and numbers only (spec 33 §1.6); how the plate
 * is made is in Settings → "Jak to działa".
 */
import { useEffect, useMemo, useState } from "react";
import { EmberBreath } from "../../components/Ember";
import { ErrorState } from "../../components/ui";
import { useAsync } from "../../hooks/useAsync";
import { useLanguage } from "../../i18n/language";
import { formatTier } from "../../lib/format";
import type { DamastSession } from "../../types";
import { cardText, saveCard } from "./card";
import { DamastInfo } from "./DamastInfo";
import { DamastPlate } from "./DamastPlate";
import { damastStats, type PlateBattle } from "./stats";
import { hasTile, plateTile, tileKey } from "./tiles";

/**
 * Under every plate: a full dot for a win, an empty one otherwise, and the tier. With `spark`
 * (Session and the card) the best battle carries the one spark, the screen's hot spot.
 */
export function DamastStrip({
  plate,
  best,
  spark,
  decorative = false,
}: {
  plate: readonly PlateBattle[];
  best: number;
  spark: boolean;
  /** Inside a button (History's plates): spans hidden from readers, the caption says it all. */
  decorative?: boolean;
}) {
  const { tr } = useLanguage();
  if (decorative)
    return (
      <span className="damast-strip" aria-hidden="true">
        {plate.map((battle) => (
          <span
            key={battle.battleId}
            className={battle.win ? "damast-dot damast-win" : "damast-dot"}
          >
            <span className="damast-pip" />
            <span className="damast-tier">{formatTier(Math.round(battle.tier)) ?? ""}</span>
          </span>
        ))}
      </span>
    );
  return (
    <ol className="damast-strip" aria-label={tr("damastStrip")}>
      {plate.map((battle, index) => {
        const tier = formatTier(Math.round(battle.tier)) ?? "";
        const said = tr(battle.win ? "damastDotWin" : "damastDotLoss", { tier });
        const hot = spark && index === best;
        return (
          <li
            key={battle.battleId}
            className={[
              "damast-dot",
              battle.win ? "damast-win" : "",
              hot ? "damast-best ember-hot" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            aria-label={hot ? tr("damastDotBest", { battle: said }) : said}
          >
            {/* The hot spot breathes (spec 33 §1) in the round dot itself: no frame. */}
            <span className="damast-pip" aria-hidden="true">
              {hot && <EmberBreath />}
            </span>
            <span className="damast-tier" aria-hidden="true">
              {tier}
            </span>
          </li>
        );
      })}
    </ol>
  );
}

/** How long "Zapisano kartę." stays next to the buttons. */
export const SAVED_MS = 4_000;
/**
 * The plate's height on Session (author, 22.11): the full width of the section, 360 px tall,
 * a lower cut of the same pattern field at the width's scale (not stretched).
 */
export const SESSION_PLATE_HEIGHT = 360;

export function SessionDamast({
  damast,
  onOpenHistory,
  mapOf = () => null,
}: {
  damast: DamastSession;
  onOpenHistory: () => void;
  /** The map of a battle of this session, for "Jak czytać tę płytkę". */
  mapOf?: (battleId: string) => string | null;
}) {
  const { language, tr } = useLanguage();
  const stats = useMemo(() => damastStats(damast), [damast]);
  const key = tileKey(stats.plate);
  // Forged once: a plate whose tile is already kept was forged before and is shown finished.
  const kept = useAsync(() => hasTile(stats.plate), [key]);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | undefined | null>(null);
  // "Zapisano kartę." stays next to the buttons for 4 s.
  useEffect(() => {
    if (!saved) return;
    const timer = setTimeout(() => setSaved(false), SAVED_MS);
    return () => clearTimeout(timer);
  }, [saved]);
  if (stats.count === 0) return null;
  const text = cardText(damast, stats, language, tr);
  const save = () => {
    setSaving(true);
    setSaved(false);
    setError(null);
    saveCard(damast, stats, text).then(
      (done) => {
        setSaving(false);
        setSaved(done);
      },
      (failure: { code?: string }) => {
        setSaving(false);
        setError(failure?.code);
      },
    );
  };
  return (
    <section className="section damast" aria-labelledby="damast-h">
      {/* Author, 22.11: the ⓘ "Jak czytać tę płytkę" next to the heading (Session only). */}
      <div className="damast-head">
        <h2 id="damast-h" className="section-title">
          {tr("damastTitle")}
        </h2>
        <DamastInfo damast={damast} stats={stats} mapOf={mapOf} />
      </div>
      {/* Author, 22.11: no side column — the plate on the full width, the battle bar under it,
          then one row: the title over WN8 and three figures on the left, the actions with their
          message on the right. */}
      <div className="card damast-card">
        {kept.state.status === "ready" ? (
          <DamastPlate
            plate={stats.plate}
            forge={!kept.state.data}
            label={tr("damastPlate")}
            height={SESSION_PLATE_HEIGHT}
            // The tile is kept once the plate stands: from now on it is "forged".
            onReady={() => void plateTile(stats.plate)}
          />
        ) : (
          <div
            className="damast-plate"
            data-state="waiting"
            aria-hidden="true"
            style={{ height: `${SESSION_PLATE_HEIGHT}px` }}
          />
        )}
        <DamastStrip plate={stats.plate} best={stats.best} spark />
        <div className="damast-row">
          <div className="damast-summary">
            <p className="damast-title">{text.title}</p>
            <div className="damast-numbers">
              <div className="damast-headline">
                <span className="num damast-headline-value">{text.headline.value}</span>
                <span className="damast-headline-label">{text.headline.label}</span>
              </div>
              <div className="damast-figures">
                {text.figures.map((figure) => (
                  <div key={figure.label} className="damast-figure">
                    <span className="num damast-figure-value">{figure.value}</span>
                    <span className="damast-figure-label">{figure.label}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="damast-actions">
            {/* The message left of the buttons: when it appears, they stay where they are. */}
            <span className="damast-saved" aria-live="polite">
              {saved ? tr("damastSaved") : ""}
            </span>
            <button type="button" className="button" onClick={save} disabled={saving}>
              {tr("damastSaveCard")}
            </button>
            <button type="button" className="button" onClick={onOpenHistory}>
              {tr("damastToHistory")}
            </button>
            {error !== null && <ErrorState code={error} />}
          </div>
        </div>
      </div>
    </section>
  );
}
