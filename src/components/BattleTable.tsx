import { useLanguage } from "../i18n/language";
import type { UiKey } from "../i18n/ui";
import { formatNumber, formatTime } from "../lib/format";
import type { BattleOutcome } from "../types";
import { TankName, useTankLabel } from "./TankName";
import { MapImage, VehicleImage } from "./ui";

export interface BattleRow {
  id: string;
  outcome: BattleOutcome;
  tankName: string | null;
  vehicleKey: string | null;
  mapName: string | null;
  mapKey: string | null;
  damage: number | null;
  assist: number | null;
  battleTime: number;
  frags?: number | null;
  survived?: boolean | null;
}

const outcomeKeys: Record<BattleOutcome, UiKey> = {
  victory: "victory",
  defeat: "defeat",
  draw: "draw",
  unknown: "unknownResult",
};

export function outcomeKey(outcome: BattleOutcome): UiKey {
  return outcomeKeys[outcome] ?? "unknownResult";
}

/** Rows from full battles (session) or history items. */
export function toRow(value: {
  arenaId?: string;
  battleId?: string;
  outcome: BattleOutcome;
  tankName: string | null;
  vehicleKey: string | null;
  mapName: string | null;
  mapKey: string | null;
  damage: number | null;
  assist: number | null;
  battleTime: number;
  frags?: number | null;
  survived?: boolean | null;
}): BattleRow {
  return { ...value, id: value.battleId ?? value.arenaId ?? "" };
}

export function BattleTable({
  title,
  rows,
  limit,
  totalLabel,
  onOpenAll,
  onOpenBattle,
  showTank = true,
  extended = false,
}: {
  title: string;
  rows: Parameters<typeof toRow>[0][];
  limit?: number;
  totalLabel?: string;
  onOpenAll?: () => void;
  onOpenBattle: (battleId: string) => void;
  /** False on the Tank screen, where the header already names the tank. */
  showTank?: boolean;
  /** History: destroyed and survival as extra columns, shown on wide screens (E5-ui §1). */
  extended?: boolean;
}) {
  const { language, tr } = useLanguage();
  const label = useTankLabel();
  const shown = (limit ? rows.slice(0, limit) : rows).map(toRow);
  return (
    <section className="card section" aria-labelledby="battles-h">
      <div className="stat-row">
        <h2 id="battles-h" className="section-title">
          {title}
        </h2>
        {onOpenAll && totalLabel && (
          <button type="button" className="link" onClick={onOpenAll}>
            {totalLabel}
          </button>
        )}
      </div>
      <div
        className={["table", showTank ? "" : "table-no-tank", extended ? "table-extended" : ""]
          .filter(Boolean)
          .join(" ")}
      >
        <div className="table-inner">
          <div className="table-head" aria-hidden="true">
            <span>{tr("colResult")}</span>
            {showTank && <span>{tr("colTank")}</span>}
            <span>{tr("colMap")}</span>
            <span className="right">{tr("colDamage")}</span>
            <span className="right">{tr("colAssist")}</span>
            {extended && <span className="right col-extra">{tr("destroyed")}</span>}
            {extended && <span className="col-extra">{tr("survival")}</span>}
            <span className="right">{tr("colTime")}</span>
          </div>
          {shown.map((row) => (
            <button
              key={row.id}
              type="button"

              className="table-row"
              onClick={() => onOpenBattle(row.id)}
            >
              <span className={row.outcome === "victory" ? "" : "subtle"}>
                {tr(outcomeKey(row.outcome))}
              </span>
              {showTank && (
                <span className="cell">
                  <VehicleImage
                    vehicleKey={row.vehicleKey}
                    label={label(row.vehicleKey, row.tankName).full}
                  />
                  <TankName vehicleKey={row.vehicleKey} tankName={row.tankName} />
                </span>
              )}
              <span className="cell">
                <MapImage mapKey={row.mapKey} label={row.mapName ?? ""} />
                <span className="truncate" title={row.mapName ?? undefined}>
                  {row.mapName ?? "—"}
                </span>
              </span>
              <span className="num right">{formatNumber(row.damage, language)}</span>
              <span className="num right">{formatNumber(row.assist, language)}</span>
              {extended && (
                <span className="num right col-extra">{formatNumber(row.frags, language)}</span>
              )}
              {extended && (
                <span className="col-extra">
                  {row.survived === null || row.survived === undefined
                    ? "—"
                    : tr(row.survived ? "survivedYes" : "survivedNo")}
                </span>
              )}
              <span className="right subtle">{formatTime(row.battleTime, language)}</span>
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}
