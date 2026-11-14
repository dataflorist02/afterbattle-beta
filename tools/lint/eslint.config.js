// Lint gate (spec 20 D9): React hooks rules and no floating promises.
import { resolve } from "node:path";
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

const root = resolve(import.meta.dirname, "../..");

export default tseslint.config(
  { ignores: ["**/api/generated/**"] },
  {
    files: ["**/*.{ts,tsx}"],
    extends: [js.configs.recommended, ...tseslint.configs.recommended],
    languageOptions: {
      globals: globals.browser,
      parserOptions: { project: resolve(root, "tsconfig.json"), tsconfigRootDir: root },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "error",
      "@typescript-eslint/no-floating-promises": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
  },
);
