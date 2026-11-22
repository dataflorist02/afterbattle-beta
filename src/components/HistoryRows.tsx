/**
 * History rows (E5-flare §5, mockup `Mapy-Historia.dc.html`): a strip of the map picture on the
 * right fading in from the left, survival in words, losses dimmed and named, rows grouped under
 * day headers ("Today", "Yesterday", a date). Pictures load only for visible rows.
 */
import { Fragment } from "react";
import type { CSSProperties } from "react";
import { rowEntranceDelay } from "../lib/motion";

const NO_ROWS: ReadonlySet<string> = new Set();
import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { daysBetween, formatDate, formatNumber, formatTime } from "../lib/format";
import type { BattleHistoryItem, Language } from "../types";
import { outcomeKey } from "./BattleTable";
import { MapBackdrop } from "./MapBackdrop";
import { useTankLabel } from "./TankName";
import { VehicleImage } from "./ui";
import { heatProps, type Heat } from "../lib/ember";
import { EmberNumber, EmberSweep } from "./Ember";

type Translate = (key: UiKey, params?: Record<string, string | number>) => string;

export function dayLabel(at: number, now: number, language: Language, tr: Translate): string {
  const days = daysBetween(at, now);
  if (days === 0) return tr("dayToday");
  if (days === 1) return tr("dayYesterday");
  return formatDate(at, language);
}

/** Consecutive rows of the same calendar day, newest first as the page gives them. */
export function groupByDay<T extends { battleTime: number }>(
  items: T[],
  now: number,
): { days: number; items: T[] }[] {
  const groups: { days: number; items: T[] }[] = [];
  for (const item of items) {
    const days = daysBetween(item.battleTime, now);
    const last = groups[groups.length - 1];
    if (last && last.days === days) last.items.push(item);
    else groups.push({ days, items: [item] });
  }
  return groups;
}

export function HistoryRows({
  items,
  onOpenBattle,
  now = Date.now(),
  entering = NO_ROWS,
  heat = () => null,
  hotId = null,
  swept = NO_ROWS,
}: {
  items: BattleHistoryItem[];
  onOpenBattle: (battleId: string) => void;
  now?: number;
  /** Rows that enter with the cascade (the first rows of the first page, once). */
  entering?: ReadonlySet<string>;
  /** Spec 33 §3: a row's damage against the player's own average on its tank. */
  heat?: (item: BattleHistoryItem) => Heat | null;
  /** The record of the rows shown: the screen's one hot spot. */
  hotId?: string | null;
  /** Rows that just arrived: one sweep each. */
  swept?: ReadonlySet<string>;
}) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  const order = new Map(items.map((item, index) => [item.battleId, index]));
  const columns = (
    <div className="hrow-head" aria-hidden="true">
      <span>{tr("colResult")}</span>
      <span>{tr("colTank")}</span>
      <span>{tr("colMap")}</span>
      <span className="right">{tr("colDamage")}</span>
      <span className="right">{tr("colAssist")}</span>
      <span className="right col-extra">{tr("destroyed")}</span>
      <span className="right">{tr("survival")}</span>
      <span className="right">{tr("colTime")}</span>
    </div>
  );
  return (
    <section className="section history-rows" aria-labelledby="history-rows-h">
      <h2 id="history-rows-h" className="visually-hidden">
        {tr("historyBattles")}
      </h2>
      {columns}
      {groupByDay(items, now).map((group) => (
        <Fragment key={`${group.days}-${group.items[0].battleId}`}>
          <h3 className="day-header">{dayLabel(group.items[0].battleTime, now, language, tr)}</h3>
          <ul className="hrow-list">
            {group.items.map((item) => {
              const tank = label(item.vehicleKey, item.tankName);
              const loss = item.outcome === "defeat";
              const map = item.mapName ?? "—";
              const temperature = heat(item);
              const hot = item.battleId === hotId;
              const sweep = swept.has(item.battleId);
              return (
                <li
                  key={item.battleId}
                  data-flip={item.battleId}
                  className={entering.has(item.battleId) ? "row-enter" : undefined}
                  style={
                    entering.has(item.battleId)
                      ? ({
                          "--row-delay": `${rowEntranceDelay(order.get(item.battleId) ?? 0, entering.size) ?? 0}ms`,
                        } as CSSProperties)
                      : undefined
                  }
                >
                  <MapBackdrop
                    as="button"
                    mapKey={item.mapKey}
                    width={420}
                    className={[
                      "hrow",
                      loss ? "hrow-loss" : "",
                      hot ? "ember-hot" : "",
                      sweep ? "ember-swept" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    onClick={() => onOpenBattle(item.battleId)}
                  >
                    {temperature && (
                      <span
                        className="hrow-temp temp-mark"
                        aria-hidden="true"
                        {...heatProps(temperature)}
                      />
                    )}
                    {sweep && <EmberSweep />}
                    <span className={loss ? "hrow-result subtle" : "hrow-result"}>
                      {tr(outcomeKey(item.outcome))}
                    </span>
                    <span className="cell">
                      <VehicleImage vehicleKey={item.vehicleKey} label={tank.full} />
                      <span className="truncate" title={tank.full}>
                        {tank.short}
                      </span>
                    </span>
                    <span className="truncate hrow-map" title={map}>
                      {map}
                    </span>
                    <EmberNumber
                      hot={hot}
                      className="num right hrow-damage"
                      text={formatNumber(item.damage, language)}
                    />
                    <span className="num right hrow-assist">
                      {formatNumber(item.assist, language)}
                    </span>
                    <span className="num right col-extra">
                      {formatNumber(item.frags, language)}
                    </span>
                    <span className="right hrow-survival">
                      {item.survived === null
                        ? "—"
                        : tr(item.survived ? "survivedWord" : "destroyedWord")}
                    </span>
                    <span className="right hrow-time">{formatTime(item.battleTime, language)}</span>
                  </MapBackdrop>
                </li>
              );
            })}
          </ul>
        </Fragment>
      ))}
    </section>
  );
}
