/**
 * History → Sessions as a collection of Damast plates (spec 34 §3): every ended session with its
 * plate, the battle bar under it (no spark), the date, the number of battles, the temperature
 * and WN8. The plates come from the cache in the app's data folder; a plate missing there (an
 * old session, a new algorithm version) is drawn once in the worker and kept. Only the record
 * session glows.
 */
import { useMemo, useState } from "react";
import { api } from "../../api";
import { EmberNumber } from "../../components/Ember";
import { Async, Empty, ErrorState } from "../../components/ui";
import { useAsync } from "../../hooks/useAsync";
import { useDataVersion } from "../../app/dataVersion";
import { useLanguage } from "../../i18n/language";
import type { UiKey } from "../../i18n/ui";
import { heatProps, type Temperature } from "../../lib/ember";
import { formatDate, formatNumber } from "../../lib/format";
import type { DamastSession, DamastSessionCursor } from "../../types";
import { battlesLabel } from "./card";
import { DamastStrip } from "./SessionDamast";
import { damastStats, type DamastStats } from "./stats";
import { plateTile, tileKey } from "./tiles";

/** Sessions per page; "Pokaż kolejne" loads the next ones. */
export const DAMAST_PAGE = 30;

const TEMPERATURE_WORDS: Record<Temperature, UiKey> = {
  warmer: "damastWarmer",
  usual: "damastUsual",
  cooler: "damastCooler",
};

interface Plated {
  damast: DamastSession;
  stats: DamastStats;
}

/** The record session of the plates shown: the highest WN8, without WN8 the warmest. */
export function recordSession(items: readonly Plated[], withWn8: boolean): string | null {
  let best: Plated | null = null;
  const score = (item: Plated) =>
    withWn8 && item.damast.wn8 !== null ? item.damast.wn8 : (item.stats.change ?? -Infinity);
  for (const item of items) if (best === null || score(item) > score(best)) best = item;
  if (best === null || score(best) === -Infinity) return null;
  return best.damast.session.id;
}

function DamastTile({
  item,
  hot,
  withWn8,
  onOpen,
}: {
  item: Plated;
  hot: boolean;
  withWn8: boolean;
  onOpen: () => void;
}) {
  const { language, tr } = useLanguage();
  const { damast, stats } = item;
  const tile = useAsync(() => plateTile(stats.plate), [tileKey(stats.plate)], { keepData: true });
  const url = tile.state.status === "ready" ? tile.state.data : null;
  const caption = [
    formatDate(damast.session.startedAt, language),
    battlesLabel(stats.count, language, tr),
    ...(stats.heat ? [tr(TEMPERATURE_WORDS[stats.heat.temperature])] : []),
  ].join(" · ");
  return (
    <button
      type="button"
      className={hot ? "card damast-tile ember-hot" : "card damast-tile"}
      onClick={onOpen}
      {...heatProps(stats.heat)}
    >
      {url ? (
        <img className="damast-tile-image" src={url} alt="" decoding="async" loading="lazy" />
      ) : (
        <span className="damast-tile-image" aria-hidden="true" />
      )}
      <DamastStrip plate={stats.plate} best={stats.best} spark={false} decorative />
      <span className="damast-tile-caption">
        <span>{caption}</span>
        {withWn8 && damast.wn8 !== null && (
          <span>
            <EmberNumber hot={hot} className="num" text={formatNumber(damast.wn8, language)} /> WN8
          </span>
        )}
      </span>
    </button>
  );
}

export function DamastSessions({
  xvmOn,
  onOpenSession,
}: {
  xvmOn: boolean;
  onOpenSession: (sessionId: string) => void;
}) {
  const { tr } = useLanguage();
  const version = useDataVersion();
  const first = useAsync(() => api.damastSessions(null, DAMAST_PAGE), [], {
    refreshKey: version,
  });
  // Pages loaded with "Pokaż kolejne" stay below a refreshed first page.
  const [more, setMore] = useState<{
    items: DamastSession[];
    next: DamastSessionCursor | null;
  } | null>(null);
  const [moreError, setMoreError] = useState<string | undefined | null>(null);
  return (
    <Async state={first.state} onRetry={first.reload}>
      {(page) => (
        <Plates
          sessions={withMore(page.items, more?.items ?? [])}
          next={more ? more.next : page.nextCursor}
          xvmOn={xvmOn}
          onOpenSession={onOpenSession}
          error={moreError}
          empty={tr("sessionsEmpty")}
          onMore={(cursor) => {
            setMoreError(null);
            api.damastSessions(cursor, DAMAST_PAGE).then(
              (next) =>
                setMore({ items: [...(more?.items ?? []), ...next.items], next: next.nextCursor }),
              (error: { code?: string }) => setMoreError(error?.code),
            );
          }}
        />
      )}
    </Async>
  );
}

/** The first page, then the pages loaded later, each session once. */
function withMore(first: DamastSession[], later: DamastSession[]): DamastSession[] {
  const seen = new Set(first.map((item) => item.session.id));
  return [...first, ...later.filter((item) => !seen.has(item.session.id))];
}

function Plates({
  sessions,
  next,
  xvmOn,
  onOpenSession,
  error,
  empty,
  onMore,
}: {
  sessions: DamastSession[];
  next: DamastSessionCursor | null;
  xvmOn: boolean;
  onOpenSession: (sessionId: string) => void;
  error: string | undefined | null;
  empty: string;
  onMore: (cursor: DamastSessionCursor) => void;
}) {
  const { tr } = useLanguage();
  const items = useMemo(
    () =>
      sessions
        .map((damast) => ({ damast, stats: damastStats(damast) }))
        .filter((item) => item.stats.count > 0),
    [sessions],
  );
  if (items.length === 0) return <Empty text={empty} />;
  const record = recordSession(items, xvmOn);
  return (
    <>
      <ul className="damast-tiles">
        {items.map((item) => (
          <li key={item.damast.session.id}>
            <DamastTile
              item={item}
              hot={item.damast.session.id === record}
              withWn8={xvmOn}
              onOpen={() => onOpenSession(item.damast.session.id)}
            />
          </li>
        ))}
      </ul>
      {error !== null && <ErrorState code={error} />}
      {next && (
        <button type="button" className="button" onClick={() => onMore(next)}>
          {tr("loadMore")}
        </button>
      )}
    </>
  );
}
