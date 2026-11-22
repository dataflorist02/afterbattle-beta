import "@testing-library/jest-dom/vitest";

// jsdom has no canvas: a canvas gets no context in tests (instead of a "not implemented" error on
// every Damast plate); the tests that draw stub `getContext` themselves.
HTMLCanvasElement.prototype.getContext = (() =>
  null) as typeof HTMLCanvasElement.prototype.getContext;
