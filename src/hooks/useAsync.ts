import { useCallback, useEffect, useRef, useState } from "react";

export type AsyncState<T> =
  | { status: "loading" }
  | { status: "error"; code: string | undefined }
  | { status: "ready"; data: T };

function errorCode(error: unknown): string | undefined {
  if (error && typeof error === "object" && "code" in error) {
    const code = (error as { code?: unknown }).code;
    return typeof code === "string" ? code : undefined;
  }
  return undefined;
}

export interface AsyncOptions {
  /**
   * Keep the data on screen while new dependencies load (spec 29 §1): the state stays `ready`
   * with the previous data until the new data arrives, so a screen never shows a loading frame
   * for a change it already has something to show for.
   */
  keepData?: boolean;
  /**
   * The data version this request depends on (spec 29 §2): a new value reloads with the data kept
   * on screen, whatever `keepData` says; the dependencies keep their own rule.
   */
  refreshKey?: unknown;
}

/**
 * Loads data and exposes loading / error / ready, so no screen swallows an error (Q2, Q3).
 * `reload` repeats the request; results of outdated requests are ignored. `refreshing` is true
 * while a newer request runs behind data already shown.
 */
export function useAsync<T>(
  load: () => Promise<T>,
  deps: readonly unknown[],
  { keepData = false, refreshKey }: AsyncOptions = {},
) {
  const [state, setState] = useState<AsyncState<T>>({ status: "loading" });
  const [pending, setPending] = useState(false);
  const generation = useRef(0);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const run = useCallback(load, deps);
  const reload = useCallback(() => {
    const current = ++generation.current;
    setState((previous) => (previous.status === "ready" ? previous : { status: "loading" }));
    setPending(true);
    run().then(
      (data) => {
        if (generation.current !== current) return;
        setState({ status: "ready", data });
        setPending(false);
      },
      (error: unknown) => {
        if (generation.current !== current) return;
        setState({ status: "error", code: errorCode(error) });
        setPending(false);
      },
    );
  }, [run]);
  useEffect(() => {
    if (!keepData) setState({ status: "loading" });
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reload]);
  // A new data version reloads in place; the first value is covered by the load above.
  const shownKey = useRef(refreshKey);
  useEffect(() => {
    if (Object.is(shownKey.current, refreshKey)) return;
    shownKey.current = refreshKey;
    reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshKey]);
  return { state, reload, refreshing: pending && state.status === "ready" };
}
