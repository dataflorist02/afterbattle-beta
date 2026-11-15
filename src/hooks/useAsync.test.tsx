import { act, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useAsync } from "./useAsync";

/** A promise the test resolves or rejects by hand. */
function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (error: unknown) => void = () => undefined;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("useAsync (Q2, Q3, spec 29 §1)", () => {
  it("loads, then shows the data; a dependency change shows loading by default", async () => {
    const calls = [deferred<string>(), deferred<string>()];
    let index = 0;
    const { result, rerender } = renderHook(
      ({ id }) => useAsync(() => calls[index++].promise, [id]),
      {
        initialProps: { id: 1 },
      },
    );
    expect(result.current.state).toEqual({ status: "loading" });
    expect(result.current.refreshing).toBe(false);
    calls[0].resolve("one");
    await settle();
    expect(result.current.state).toEqual({ status: "ready", data: "one" });
    expect(result.current.refreshing).toBe(false);
    rerender({ id: 2 });
    expect(result.current.state).toEqual({ status: "loading" });
    calls[1].resolve("two");
    await settle();
    expect(result.current.state).toEqual({ status: "ready", data: "two" });
  });

  it("keeps the previous data while new dependencies load with keepData", async () => {
    const calls = [deferred<string>(), deferred<string>()];
    let index = 0;
    const { result, rerender } = renderHook(
      ({ id }) => useAsync(() => calls[index++].promise, [id], { keepData: true }),
      { initialProps: { id: 1 } },
    );
    expect(result.current.state).toEqual({ status: "loading" });
    expect(result.current.refreshing).toBe(false);
    calls[0].resolve("one");
    await settle();
    rerender({ id: 2 });
    await settle();
    expect(result.current.state).toEqual({ status: "ready", data: "one" });
    expect(result.current.refreshing).toBe(true);
    calls[1].resolve("two");
    await settle();
    expect(result.current.state).toEqual({ status: "ready", data: "two" });
    expect(result.current.refreshing).toBe(false);
  });

  it("ignores the result of an outdated request", async () => {
    const calls = [deferred<string>(), deferred<string>()];
    let index = 0;
    const { result, rerender } = renderHook(
      ({ id }) => useAsync(() => calls[index++].promise, [id], { keepData: true }),
      { initialProps: { id: 1 } },
    );
    rerender({ id: 2 });
    calls[1].resolve("two");
    await settle();
    calls[0].resolve("one");
    await settle();
    expect(result.current.state).toEqual({ status: "ready", data: "two" });
    expect(result.current.refreshing).toBe(false);
  });

  it("ignores an outdated error and reports the current one with its code", async () => {
    const calls = [deferred<string>(), deferred<string>(), deferred<string>()];
    let index = 0;
    const { result, rerender } = renderHook(
      ({ id }) => useAsync(() => calls[index++].promise, [id]),
      {
        initialProps: { id: 1 },
      },
    );
    rerender({ id: 2 });
    calls[0].reject({ code: "old" });
    await settle();
    expect(result.current.state).toEqual({ status: "loading" });
    calls[1].reject({ code: "database_error" });
    await settle();
    expect(result.current.state).toEqual({ status: "error", code: "database_error" });
    expect(result.current.refreshing).toBe(false);
    act(() => result.current.reload());
    expect(result.current.state).toEqual({ status: "loading" });
    calls[2].reject(new Error("no code"));
    await settle();
    expect(result.current.state).toEqual({ status: "error", code: undefined });
  });

  it("reports no code for an error without a string code", async () => {
    for (const error of [{ code: 42 }, null, undefined, "database_error", 7, {}]) {
      const { result, unmount } = renderHook(() => useAsync(() => Promise.reject(error), []));
      await settle();
      expect(result.current.state, String(error)).toEqual({ status: "error", code: undefined });
      unmount();
    }
  });

  it("reload keeps the data on screen and marks it as refreshing", async () => {
    const calls = [deferred<string>(), deferred<string>()];
    let index = 0;
    const { result } = renderHook(() => useAsync(() => calls[index++].promise, []));
    calls[0].resolve("one");
    await settle();
    act(() => result.current.reload());
    expect(result.current.state).toEqual({ status: "ready", data: "one" });
    expect(result.current.refreshing).toBe(true);
    calls[1].reject({ code: "late" });
    await settle();
    expect(result.current.state).toEqual({ status: "error", code: "late" });
    expect(result.current.refreshing).toBe(false);
  });

  it("reloads on a new refresh key with the data kept, and only once on mount", async () => {
    const calls = [deferred<string>(), deferred<string>(), deferred<string>()];
    let index = 0;
    const { result, rerender } = renderHook(
      ({ id, version }) => useAsync(() => calls[index++].promise, [id], { refreshKey: version }),
      { initialProps: { id: 1, version: 0 } },
    );
    expect(index).toBe(1);
    calls[0].resolve("one");
    await settle();
    rerender({ id: 1, version: 1 });
    await settle();
    expect(index).toBe(2);
    expect(result.current.state).toEqual({ status: "ready", data: "one" });
    expect(result.current.refreshing).toBe(true);
    calls[1].resolve("one, refreshed");
    await settle();
    expect(result.current.state).toEqual({ status: "ready", data: "one, refreshed" });
    // A change of the dependencies themselves still shows loading by default.
    rerender({ id: 2, version: 1 });
    expect(result.current.state).toEqual({ status: "loading" });
    expect(index).toBe(3);
  });

  it("does not reload when the refresh key stays the same", async () => {
    let calls = 0;
    const { rerender } = renderHook(
      ({ version }) => useAsync(() => Promise.resolve(++calls), [], { refreshKey: version }),
      { initialProps: { version: 5 } },
    );
    await settle();
    rerender({ version: 5 });
    await settle();
    expect(calls).toBe(1);
  });
});
