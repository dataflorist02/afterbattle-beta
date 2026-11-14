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

/**
 * Loads data and exposes loading / error / ready, so no screen swallows an error (Q2, Q3).
 * `reload` repeats the request; results of outdated requests are ignored.
 */
export function useAsync<T>(load: () => Promise<T>, deps: readonly unknown[]) {
  const [state, setState] = useState<AsyncState<T>>({ status: "loading" });
  const generation = useRef(0);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const run = useCallback(load, deps);
  const reload = useCallback(() => {
    const current = ++generation.current;
    setState((previous) => (previous.status === "ready" ? previous : { status: "loading" }));
    run().then(
      (data) => {
        if (generation.current === current) setState({ status: "ready", data });
      },
      (error: unknown) => {
        if (generation.current === current) setState({ status: "error", code: errorCode(error) });
      },
    );
  }, [run]);
  useEffect(() => {
    setState({ status: "loading" });
    reload();
  }, [reload]);
  return { state, reload };
}
