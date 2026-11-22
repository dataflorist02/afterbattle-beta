/**
 * Map-led parts of the Session screen (E5-flare §2, mockup `Mapy-Sesja.dc.html`): the header on
 * the best battle's map with the headline sentence and the numbers, the battle-by-battle strip
 * and the session moments. Every rule comes from `get_session_story` (Rust, deterministic).
 */
import { useRef } from "react";
import type { CSSProperties, ReactNode } from "react";
import { useLanguage } from "../../i18n/language";
import type { UiKey } from "../../i18n/ui";
import { formatNumber, formatPercent } from "../../lib/format";
import type { Battle, BattleOutcome, SessionHeadline, SessionMoment } from "../../types";
import { outcomeKey } from "../BattleTable";
import { MapBackdrop } from "../MapBackdrop";
import { useTankLabel } from "../TankName";
import { prefersReducedMotion, useFlip, type Direction } from "../../lib/motion";
import { heatProps, type Heat } from "../../lib/ember";
import { EmberHeat, EmberNumber, EmberSweep, Sparks, TemperatureWord } from "../Ember";
import { HelpLink } from "../HelpLink";

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

/** The headline sentence; the percent is returned apart so it can carry the accent. */
export function headlineParts(
  headline: SessionHeadline | null,
  fallback: string,
  tr: Translate,
): { before: string; accent: string | null; after: string } {
  if (!headline) return { before: fallback, accent: null, after: "" };
  switch (headline.kind) {
    case "strong":
    case "tough": {
      const key: UiKey = headline.kind === "strong" ? "headlineStrong" : "headlineTough";
      const [before, after] = tr(key, { percent: "\u0000" }).split("\u0000");
      const accent = tr(headline.kind === "strong" ? "headlineHigher" : "headlineLower", {
        percent: headline.percent ?? 0,
      });
      return { before, accent, after: after ?? "" };
    }
    case "calm":
      return { before: tr("headlineCalm"), accent: null, after: "" };
    default:
      return {
        before: tr("headlineNeutral", { battles: headline.battles }),
        accent: null,
        after: "",
      };
  }
}

export function SessionHero({
  mapKey,
  mapName,
  meta,
  headline,
  fallbackTitle,
  figures,
  notes,
  action,
  notesId,
  heat = null,
}: {
  mapKey: string | null;
  mapName: string | null;
  meta: ReactNode;
  headline: SessionHeadline | null;
  fallbackTitle: string;
  figures: ReactNode;
  notes: string[];
  action: ReactNode;
  /** Lets a summary reference highlight the damage note (spec 30 §8). */
  notesId?: string;
  /** Spec 33 §3: the session against the own average warms or cools the card, with sparks. */
  heat?: Heat | null;
}) {
  const { tr } = useLanguage();
  const parts = headlineParts(headline, fallbackTitle, tr);
  const temperature = heatProps(heat);
  return (
    <MapBackdrop
      as="section"
      mapKey={mapKey}
      width={1400}
      className={
        heat ? "flare-hero session-flare-hero ember-tinted" : "flare-hero session-flare-hero"
      }
      aria-labelledby="session-headline"
      label={mapKey ? tr("bestMapBackground", { map: mapName ?? mapKey }) : undefined}
      data-temp={temperature["data-temp"]}
      style={temperature.style}
    >
      <Sparks heat={heat} />
      <div className="flare-hero-text">
        <div className="flare-hero-meta">
          {meta}
          {heat && (
            <p>
              <TemperatureWord heat={heat} />
            </p>
          )}
        </div>
        <h1 id="session-headline" className="flare-headline">
          {parts.before}
          {parts.accent && <span className="flare-accent">{parts.accent}</span>}
          {parts.after}
        </h1>
      </div>
      <div className="flare-figures">
        {figures}
        <div className="flare-hero-action">{action}</div>
      </div>
      {notes.length > 0 && (
        <div id={notesId} className="flare-hero-notes">
          {notes.map((note, index) => (
            <p key={note}>
              {note}
              {/* Spec 33 §4: how the average is counted lives in "Jak to działa". */}
              {index === 0 && <HelpLink entry="average" />}
            </p>
          ))}
        </div>
      )}
    </MapBackdrop>
  );
}

export function Figure({
  value,
  label,
  main = false,
  delta = null,
  reserve,
  id,
}: {
  value: string;
  label: string;
  main?: boolean;
  /** Lets a summary reference highlight this number (spec 30 §8). */
  id?: string;
  /** A number that just changed (spec 29 §6): an overlay in the direction color, fading out. */
  delta?: { direction: Direction; key: number } | null;
  /** Characters kept for the number while it counts, so the width never jumps. */
  reserve?: number;
}) {
  return (
    <div id={id} className={main ? "figure figure-main" : "figure"}>
      <span className="num figure-value" style={reserve ? { minWidth: `${reserve}ch` } : undefined}>
        {value}
        {delta && (
          <span
            key={delta.key}
            className={`figure-delta figure-${delta.direction}`}
            aria-hidden="true"
          >
            {value}
          </span>
        )}
        {/* Spec 33 §3: after the count the number heats up and cools down. */}
        {delta && <EmberHeat key={`heat-${delta.key}`} />}
      </span>{" "}
      <span className="figure-label">{label}</span>
    </div>
  );
}

const lossLike = (outcome: BattleOutcome) => outcome === "defeat";

/** One tile per battle, oldest first; losses dimmed and named; the best battle framed. */
export function BattleStrip({
  battles,
  bestBattleId,
  newBattleIds,
  onOpenBattle,
  hot = true,
  referenceBattleId = bestBattleId,
}: {
  battles: Battle[];
  bestBattleId: string | null;
  /**
   * The battle the summary's "najlepsza bitwa" names (spec 30 §8 reference); on an ended
   * session the marked best battle is the highest r of its plate and may be another one.
   */
  referenceBattleId?: string | null;
  newBattleIds: readonly string[];
  onOpenBattle: (battleId: string) => void;
  /** False when another place is the screen's hot spot (the Damast spark, spec 34 §3). */
  hot?: boolean;
}) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  // Spec 29 §6: when a battle arrives, the tiles that moved glide to their new place.
  const strip = useRef<HTMLOListElement>(null);
  useFlip(
    strip,
    battles.map((battle) => battle.arenaId).join(","),
    newBattleIds.length > 0 && !prefersReducedMotion(),
  );
  return (
    <section className="section" aria-labelledby="strip-h">
      <div className="card-head">
        <h2 id="strip-h" className="section-title">
          {tr("stripTitle")}
        </h2>
      </div>
      <ol ref={strip} className="battle-strip" aria-labelledby="strip-h">
        {battles.map((battle, index) => {
          const best = battle.arenaId === bestBattleId;
          const loss = lossLike(battle.outcome);
          const result = tr(outcomeKey(battle.outcome));
          const bestResult = tr("bestBattleResult", {
            result: result.toLocaleLowerCase(language === "pl" ? "pl-PL" : "en-GB"),
          });
          const tank = label(battle.vehicleKey, battle.tankName);
          const map = battle.mapName ?? "—";
          const fresh = newBattleIds.includes(battle.arenaId);
          const sweep = fresh && !prefersReducedMotion();
          return (
            <li
              key={battle.arenaId}
              data-flip={battle.arenaId}
              className="strip-item"
              style={{ "--stagger": index } as CSSProperties}
            >
              <MapBackdrop
                as="button"
                mapKey={battle.mapKey}
                width={160}
                className={[
                  "tile",
                  loss ? "tile-loss" : "",
                  // Spec 33 §3: the best battle of the session is the screen's one hot spot.
                  best ? "tile-best" : "",
                  best && hot ? "ember-hot" : "",
                  fresh ? "tile-new" : "",
                  sweep ? "ember-swept" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => onOpenBattle(battle.arenaId)}
                id={battle.arenaId === referenceBattleId ? "recap-target-best" : undefined}
                aria-label={tr("tileLabel", {
                  result: best ? bestResult : result,
                  damage: formatNumber(battle.damage, language),
                  map,
                  tank: tank.full,
                })}
              >
                {fresh && <span className="tile-flash" aria-hidden="true" />}
                {sweep && <EmberSweep />}
                <span className="tile-result">{best ? bestResult : result}</span>
                <span className="tile-body">
                  <EmberNumber
                    hot={best && hot}
                    className="num tile-damage"
                    text={formatNumber(battle.damage, language)}
                  />
                  <span className="tile-line truncate">{map}</span>
                  <span className="tile-line tile-tank truncate" title={tank.full}>
                    {tank.short}
                  </span>
                </span>
              </MapBackdrop>
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function MomentCard({
  label,
  value,
  caption,
  mapKey,
  mapName,
  onClick,
}: {
  label: string;
  value: string;
  caption: string;
  mapKey?: string | null;
  mapName?: string | null;
  onClick?: () => void;
}) {
  const { tr } = useLanguage();
  const body = (
    <>
      <span className="moment-label">{label}</span>{" "}
      <span className="num moment-value">{value}</span>{" "}
      <span className="moment-caption">{caption}</span>
    </>
  );
  if (mapKey !== undefined)
    return (
      <MapBackdrop
        as={onClick ? "button" : "div"}
        mapKey={mapKey}
        width={640}
        className="card moment moment-map"
        onClick={onClick}
        label={mapKey ? tr("mapBackground", { map: mapName ?? mapKey }) : undefined}
      >
        {body}
      </MapBackdrop>
    );
  return <div className="card moment">{body}</div>;
}

/** Cards appear only for rules that have data (the backend leaves the others out). */
export function SessionMoments({
  moments,
  onOpenBattle,
}: {
  moments: SessionMoment[];
  onOpenBattle: (battleId: string) => void;
}) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  if (moments.length === 0) return null;
  const number = (value: number | null | undefined) => formatNumber(value, language);
  return (
    <section className="section" aria-labelledby="moments-h">
      <h2 id="moments-h" className="section-title">
        {tr("momentsTitle")}
      </h2>
      <div className="moments">
        {moments.map((moment) => {
          if (moment.kind === "damage_trend")
            return (
              <MomentCard
                key="trend"
                label={tr(moment.rising ? "momentRising" : "momentFalling")}
                value={`${number(moment.from)} → ${number(moment.to)}`}
                caption={
                  moment.everyBattle
                    ? tr(moment.rising ? "momentEveryRising" : "momentEveryFalling")
                    : tr("momentThirds", { count: moment.battlesPerThird })
                }
              />
            );
          if (moment.kind === "weakest_tank") {
            const tank = label(moment.vehicleKey, moment.tankName).short;
            const value =
              moment.metric === "survival"
                ? tr("momentSurvived", {
                    survived: moment.survived ?? 0,
                    battles: moment.battles,
                  })
                : moment.metric === "damage"
                  ? tr("momentDamage", { value: number(moment.current) })
                  : moment.metric === "assist"
                    ? tr("momentAssist", { value: number(moment.current) })
                    : tr("momentWn8", { value: number(moment.current) });
            const usual =
              moment.metric === "survival"
                ? formatPercent(moment.baseline, language)
                : number(moment.baseline);
            return (
              <MomentCard
                key="weakest"
                label={tr("momentToImprove")}
                value={value}
                caption={tr(moment.metric === "survival" ? "momentUsualSurvival" : "momentUsual", {
                  tank,
                  value: usual,
                })}
                mapKey={moment.mapKey}
                mapName={moment.mapName}
              />
            );
          }
          const battle = moment.battle;
          return (
            <MomentCard
              key="best"
              label={tr("momentBest")}
              value={number(battle.damage)}
              caption={tr("momentBestCaption", {
                map: battle.mapName ?? "—",
                tank: label(battle.vehicleKey, battle.tankName).short,
              })}
              mapKey={battle.mapKey}
              mapName={battle.mapName}
              onClick={() => onOpenBattle(battle.battleId)}
            />
          );
        })}
      </div>
    </section>
  );
}
