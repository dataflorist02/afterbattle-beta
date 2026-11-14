/**
 * Map-led parts of the Session screen (E5-flare §2, mockup `Mapy-Sesja.dc.html`): the header on
 * the best battle's map with the headline sentence and the numbers, the battle-by-battle strip
 * and the session moments. Every rule comes from `get_session_story` (Rust, deterministic).
 */
import type { CSSProperties, ReactNode } from "react";
import { useLanguage } from "../../i18n/language";
import type { UiKey } from "../../i18n/ui";
import { formatNumber, formatPercent } from "../../lib/format";
import type { Battle, BattleOutcome, SessionHeadline, SessionMoment } from "../../types";
import { outcomeKey } from "../BattleTable";
import { MapBackdrop } from "../MapBackdrop";
import { useTankLabel } from "../TankName";

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
}: {
  mapKey: string | null;
  mapName: string | null;
  meta: ReactNode;
  headline: SessionHeadline | null;
  fallbackTitle: string;
  figures: ReactNode;
  notes: string[];
  action: ReactNode;
}) {
  const { tr } = useLanguage();
  const parts = headlineParts(headline, fallbackTitle, tr);
  return (
    <MapBackdrop
      as="section"
      mapKey={mapKey}
      width={1400}
      className="flare-hero session-flare-hero"
      aria-labelledby="session-headline"
      label={mapKey ? tr("bestMapBackground", { map: mapName ?? mapKey }) : undefined}
    >
      <div className="flare-hero-text">
        <div className="flare-hero-meta">{meta}</div>
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
        <div className="flare-hero-notes">
          {notes.map((note) => (
            <p key={note}>{note}</p>
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
}: {
  value: string;
  label: string;
  main?: boolean;
}) {
  return (
    <div className={main ? "figure figure-main" : "figure"}>
      <span className="num figure-value">{value}</span>{" "}
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
}: {
  battles: Battle[];
  bestBattleId: string | null;
  newBattleIds: readonly string[];
  onOpenBattle: (battleId: string) => void;
}) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  return (
    <section className="section" aria-labelledby="strip-h">
      <div className="card-head">
        <h2 id="strip-h" className="section-title">
          {tr("stripTitle")}
        </h2>
      </div>
      <ol className="battle-strip" aria-labelledby="strip-h">
        {battles.map((battle, index) => {
          const best = battle.arenaId === bestBattleId;
          const loss = lossLike(battle.outcome);
          const result = tr(outcomeKey(battle.outcome));
          const bestResult = tr("bestBattleResult", {
            result: result.toLocaleLowerCase(language === "pl" ? "pl-PL" : "en-GB"),
          });
          const tank = label(battle.vehicleKey, battle.tankName);
          const map = battle.mapName ?? "—";
          return (
            <li
              key={battle.arenaId}
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
                  best ? "tile-best" : "",
                  newBattleIds.includes(battle.arenaId) ? "tile-new" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => onOpenBattle(battle.arenaId)}
                aria-label={tr("tileLabel", {
                  result: best ? bestResult : result,
                  damage: formatNumber(battle.damage, language),
                  map,
                  tank: tank.full,
                })}
              >
                {newBattleIds.includes(battle.arenaId) && (
                  <span className="tile-flash" aria-hidden="true" />
                )}
                <span className="tile-result">{best ? bestResult : result}</span>
                <span className="tile-body">
                  <span className="num tile-damage">{formatNumber(battle.damage, language)}</span>
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
