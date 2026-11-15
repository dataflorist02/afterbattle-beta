// Mutation testing of the frontend (spec 29 §9: at least 85% for new and changed logic).
// Run from the repository root (pass the files to mutate, `file:line-line` narrows to lines):
//   node tools/mutants/node_modules/@stryker-mutator/core/bin/stryker.js run tools/mutants/stryker.config.mjs --mutate src/lib/motion.ts
/** @type {import("@stryker-mutator/api/core").PartialStrykerOptions} */
export default {
  testRunner: "vitest",
  plugins: ["@stryker-mutator/vitest-runner"],
  vitest: { configFile: "vite.config.ts", related: false },
  coverageAnalysis: "perTest",
  reporters: ["clear-text", "progress", "json"],
  jsonReporter: { fileName: "tools/mutants/reports/mutation.json" },
  thresholds: { high: 90, low: 85, break: 85 },
  tempDirName: "tools/mutants/.stryker-tmp",
  cleanTempDir: "always",
  concurrency: 3,
  timeoutMS: 20000,
};
