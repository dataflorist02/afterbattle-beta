import { api } from "../api";
import { outcomeKey } from "../components/BattleTable";
import { useTankLabel } from "../components/TankName";
import { MapBackdrop } from "../components/MapBackdrop";
import { Recap } from "../components/Recap";
import { Figure } from "../components/session/SessionStory";
import { Async, Card, VehicleImage } from "../components/ui";
import { useAsync } from "../hooks/useAsync";
import { useDataVersion } from "../app/dataVersion";
import { useLanguage } from "../i18n/language";
import {
  formatDate,
  formatDuration,
  formatNumber,
  formatPercent,
  formatThousands,
  formatTime,
} from "../lib/format";
import { BATTLE_TARGETS, battleFactLabel, battleSentences } from "../lib/recap/battle";
import { heatAgainst, heatProps } from "../lib/ember";
import { prefersReducedMotion } from "../lib/motion";
import { EmberHeat, EmberNumber, TemperatureWord } from "../components/Ember";
import { damastBestBattleId } from "../features/damast/stats";
import type { Battle, BattleDetailView, TeamComparison, TeamTotals } from "../types";

/**
 * Spec 33 §3: what the battle is compared with — the player's own average damage on its tank —
 * and whether it is the best battle of its session (then it is the screen's hot spot).
 */
function useBattleContext(battle: Battle): { average: number | null; best: boolean } {
  const version = useDataVersion();
  const context = useAsync(
    async () => {
      const [tanks, plate] = await Promise.all([
        api.vehicleHistoryOverview().catch(() => null),
        battle.sessionId
          ? Promise.resolve()
              .then(() => api.damastSession(battle.sessionId!))
              .catch(() => null)
          : null,
      ]);
      const tank =
        tanks?.vehicles.find((item) => item.vehicleId === battle.wgTankId) ??
        tanks?.vehicles.find((item) => item.vehicleKey === battle.vehicleKey);
      return {
        average: tank?.averageDamage ?? null,
        // The best battle of the session is its highest r (author's decision after E14).
        best: damastBestBattleId(plate) === battle.arenaId,
      };
    },
    [battle.arenaId],
    { refreshKey: version },
  );
  return context.state.status === "ready" ? context.state.data : { average: null, best: false };
}

/** "Podsumowanie bitwy" (spec 30 §2): Polish only, only for a counted battle with 3 sentences. */
function BattleRecap({ battleId }: { battleId: string }) {
  const { language, address, tr } = useLanguage();
  const label = useTankLabel();
  const version = useDataVersion();
  const recap = useAsync(() => api.recap("battle", battleId).catch(() => null), [battleId], {
    refreshKey: version,
  });
  const data = recap.state.status === "ready" ? recap.state.data : null;
  if (language !== "pl" || data?.kind !== "battle") return null;
  const context = { address, tank: (key: string, name: string | null) => label(key, name).short };
  const sentences = battleSentences(data, context);
  if (sentences.length === 0) return null;
  return (
    <Recap
      id="battle-recap"
      title={tr("recapBattleTitle")}
      sentences={sentences}
      facts={data.facts.map((fact) => ({
        id: fact.id,
        label: battleFactLabel(fact.fact, context),
        target: BATTLE_TARGETS[fact.fact.key] ?? null,
      }))}
    />
  );
}

function tiers(team: TeamTotals): string {
  if (team.tierMin === null || team.tierMax === null) return "—";
  return team.tierMin === team.tierMax ? String(team.tierMin) : `${team.tierMin}–${team.tierMax}`;
}

function TeamBar({ own, enemy }: { own: number | null; enemy: number | null }) {
  const total = (own ?? 0) + (enemy ?? 0);
  const share = total > 0 ? ((own ?? 0) / total) * 100 : 50;
  return (
    <div className="team-bar" aria-hidden="true">
      <div className="team-own" style={{ width: `${share}%` }} />
      <div className="team-enemy" style={{ width: `${100 - share}%` }} />
    </div>
  );
}

/** Team totals only (D17): no player list anywhere. */
function Teams({ teams }: { teams: TeamComparison }) {
  const { language, tr } = useLanguage();
  const rows: [string, number | null, number | null, (value: number | null) => string][] = [
    [tr("colDamage"), teams.own.damage, teams.enemy.damage, (v) => formatThousands(v, language)],
    [tr("spotted"), teams.own.spotted, teams.enemy.spotted, (v) => formatNumber(v, language)],
    [tr("assist"), teams.own.assist, teams.enemy.assist, (v) => formatThousands(v, language)],
    [tr("survivors"), teams.own.survivors, teams.enemy.survivors, (v) => formatNumber(v, language)],
  ];
  return (
    <Card labelledBy="teams-h" className="section">
      <h2 id="teams-h" className="section-title">
        {tr("teams")}
      </h2>
      <p className="muted">
        {tr("teamsMeta", {
          own: tiers(teams.own),
          enemy: tiers(teams.enemy),
          ownPlatoons: teams.own.platoons,
          enemyPlatoons: teams.enemy.platoons,
        })}
      </p>
      <p className="muted">
        <span>{tr("yourTeam")}</span> / <span>{tr("enemies")}</span>
      </p>
      <div className="teams">
        {rows.map(([label, own, enemy, format]) => (
          <div key={label} style={{ display: "contents" }}>
            <span className="stat-label">{label}</span>
            <TeamBar own={own} enemy={enemy} />
            <span className="num right">
              {format(own)} / {format(enemy)}
            </span>
          </div>
        ))}
      </div>
    </Card>
  );
}

/** "You on <map>": every counted battle of the player on this map (E5-flare §4). */
function MapRecordCard({
  mapKey,
  map,
  onOpenMap,
}: {
  mapKey: string;
  map: string;
  onOpenMap: (mapKey: string, mapName: string | null) => void;
}) {
  const { language, tr } = useLanguage();
  const version = useDataVersion();
  const record = useAsync(() => api.mapRecord(mapKey), [mapKey], { refreshKey: version });
  if (record.state.status !== "ready" || record.state.data.battles === 0) return null;
  const data = record.state.data;
  return (
    <MapBackdrop
      as="section"
      mapKey={mapKey}
      width={900}
      className="card map-record"
      aria-labelledby="map-record-h"
      label={tr("mapBackground", { map })}
    >
      <div className="map-record-title">
        <h2 id="map-record-h" className="section-title">
          {tr("mapRecordTitle", { map })}
        </h2>
        <span className="muted">{tr("mapRecordNote")}</span>
      </div>
      <Figure value={formatNumber(data.battles, language)} label={tr("mapRecordBattles")} />
      <Figure value={formatPercent(data.winRate, language)} label={tr("mapRecordWins")} />
      <Figure value={formatNumber(data.averageDamage, language)} label={tr("mapRecordDamage")} />
      <button type="button" className="link map-record-link" onClick={() => onOpenMap(mapKey, map)}>
        {tr("mapRecordAll")}
      </button>
    </MapBackdrop>
  );
}

function BattleBody({
  detail,
  onBack,
  onOpenMap,
}: {
  detail: BattleDetailView;
  onBack: () => void;
  onOpenMap: (mapKey: string, mapName: string | null) => void;
}) {
  const { language, tr } = useLanguage();
  const battle = detail.view.battle;
  const tank = useTankLabel()(battle.vehicleKey, battle.tankName).full;
  const map = battle.mapName ?? "—";
  const counted = detail.view.feedback.every(
    (item) => item.code !== "battle_not_counted" && item.code !== "battle_partial",
  );
  const partial = battle.dataQuality !== "full";
  const survivedHp =
    battle.survived && detail.healthRemaining !== null && detail.maxHealth
      ? { hp: detail.healthRemaining, max: detail.maxHealth }
      : null;
  const teamShare =
    detail.teams?.own.damage && battle.damage !== null
      ? Math.round((battle.damage / detail.teams.own.damage) * 100)
      : null;
  // Spec 33 §3: once, on opening, the result heats up or cools down against the own average.
  const context = useBattleContext(battle);
  const average = counted ? context.average : null;
  const heat = heatAgainst(battle.damage, average);
  const temperature = heatProps(heat);
  const hot = counted && context.best;
  const stats: [string, string, string][] = [
    [
      battle.shots === null
        ? formatNumber(battle.directEnemyHits ?? battle.hits, language)
        : tr("figureOf", {
            value: formatNumber(battle.directEnemyHits ?? battle.hits, language),
            count: battle.shots,
          }),
      tr("statHits"),
      "recap-target-hits",
    ],
    [
      formatNumber(battle.piercingEnemyHits ?? battle.penetrations, language),
      tr("statPenetrations"),
      "recap-target-penetrations",
    ],
    [formatNumber(battle.frags, language), tr("statDestroyed"), "recap-target-destroyed"],
    [formatNumber(battle.assist, language), tr("statAssist"), "recap-target-assist"],
    [formatNumber(battle.spots, language), tr("statSpotted"), "recap-target-spotted"],
    [formatNumber(battle.blocked, language), tr("statBlocked"), "recap-target-blocked"],
  ];
  return (
    <>
      <MapBackdrop
        as="header"
        mapKey={battle.mapKey}
        width={1400}
        className={
          heat ? "flare-hero battle-flare-hero ember-tinted" : "flare-hero battle-flare-hero"
        }
        label={tr("mapBackground", { map })}
        data-temp={temperature["data-temp"]}
        style={temperature.style}
      >
        <div className="battle-hero-top">
          <button type="button" className="link" onClick={onBack}>
            ← {tr("back")}
          </button>
        </div>
        <div className="battle-hero-main">
          <div className="flare-hero-text">
            <p className="battle-hero-meta">
              {tr("battleWhen", {
                date: formatDate(battle.battleTime, language),
                time: formatTime(battle.battleTime, language),
                duration: formatDuration(battle.durationSeconds),
              })}
              {heat && (
                <>
                  {" · "}
                  <TemperatureWord heat={heat} />
                </>
              )}
            </p>
            <h1 id="recap-target-headline" className="flare-headline battle-headline">
              {tr("battleTitle", { result: tr(outcomeKey(battle.outcome)), map })}
            </h1>
            <p id="recap-target-tank" className="battle-hero-tank">
              <VehicleImage vehicleKey={battle.vehicleKey} label={tank} width={64} height={34} />
              <span>{tank}</span>
            </p>
          </div>
          <div
            id="recap-target-damage"
            className={hot ? "battle-hero-damage ember-hot" : "battle-hero-damage"}
          >
            <EmberNumber
              hot={hot}
              className="num battle-damage-value"
              text={formatNumber(battle.damage, language)}
            >
              {heat && !prefersReducedMotion() && <EmberHeat />}
            </EmberNumber>
            <span className="figure-label">
              {teamShare === null
                ? tr("damageWord")
                : tr(detail.mostDamageInTeam ? "damageLineTop" : "damageLineShare", {
                    share: teamShare,
                  })}
              {average !== null &&
                ` · ${tr("battleUsual", { value: formatNumber(average, language) })}`}
            </span>
          </div>
        </div>
      </MapBackdrop>
      {!counted && (
        <p className="muted" role="note">
          {tr(partial ? "partialBattle" : "notCounted")}
        </p>
      )}
      {counted && <BattleRecap battleId={battle.arenaId} />}
      <div className="battle-layout">
        <section className="card section" aria-labelledby="you-h">
          <h2 id="you-h" className="section-title">
            {tr("yourBattle")}
          </h2>
          <div className="stat-grid">
            {stats.map(([value, label, id]) => (
              <Figure key={label} id={id} value={value} label={label} />
            ))}
            {survivedHp ? (
              <Figure
                id="recap-target-survival"
                value={tr("hpValue", { hp: formatNumber(survivedHp.hp, language) })}
                label={tr("survivedOf", { max: formatNumber(survivedHp.max, language) })}
              />
            ) : (
              <Figure
                id="recap-target-survival"
                value={tr(battle.survived ? "survivedWord" : "destroyedWord")}
                label={tr("survival")}
              />
            )}
          </div>
        </section>
        {counted &&
          !partial &&
          (detail.teams ? (
            <Teams teams={detail.teams} />
          ) : (
            <p className="muted">{tr("teamsUnavailable")}</p>
          ))}
      </div>
      {battle.mapKey && <MapRecordCard mapKey={battle.mapKey} map={map} onOpenMap={onOpenMap} />}
    </>
  );
}

export function BattlePage({
  battleId,
  onBack,
  onOpenMap = () => undefined,
}: {
  battleId: string;
  onBack: () => void;
  onOpenMap?: (mapKey: string, mapName: string | null) => void;
}) {
  const { tr } = useLanguage();
  const version = useDataVersion();
  const { state, reload } = useAsync(() => api.battleView(battleId), [battleId], {
    refreshKey: version,
  });
  return state.status === "ready" ? (
    <BattleBody detail={state.data} onBack={onBack} onOpenMap={onOpenMap} />
  ) : (
    <>
      <button type="button" className="link" onClick={onBack}>
        ← {tr("back")}
      </button>
      <Async state={state} onRetry={reload}>
        {() => null}
      </Async>
    </>
  );
}
