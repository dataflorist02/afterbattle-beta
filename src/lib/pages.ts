/**
 * Lists loaded page by page ("Show more") across a refresh (spec 29 §2): the refreshed first page
 * goes on top, the rows loaded after it stay, so a refresh never shortens the list or moves it.
 */
export function mergeRefreshedPage<T extends { battleId: string }>(
  fresh: readonly T[],
  loaded: readonly T[],
): T[] {
  const seen = new Set(fresh.map((item) => item.battleId));
  return [...fresh, ...loaded.filter((item) => !seen.has(item.battleId))];
}
