import js from "@eslint/js";
import ts from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import svelte from "eslint-plugin-svelte";

export default [
  js.configs.recommended,
  {
    // AGENTS.md 11.1: funciones ≤40 líneas, complejidad ≤10, archivos ≤400.
    rules: {
      complexity: ["error", 10],
      "max-lines-per-function": ["error", { max: 40, skipBlankLines: false }],
      "max-lines": ["error", { max: 400 }],
    },
  },
  {
    files: ["**/*.ts"],
    languageOptions: { parser: tsParser },
    plugins: { "@typescript-eslint": ts },
    rules: { "no-console": ["error", { allow: ["warn", "error"] }] },
  },
  ...svelte.configs["flat/recommended"],
  { ignores: [".svelte-kit/", "build/", "node_modules/"] },
];
