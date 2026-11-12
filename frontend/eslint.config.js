import js from "@eslint/js";
import ts from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import svelte from "eslint-plugin-svelte";
import svelteParser from "svelte-eslint-parser";

// Globals del navegador y temporizadores (la UI corre en el navegador).
const browserGlobals = {
  fetch: "readonly",
  TextDecoder: "readonly",
  ReadableStream: "readonly",
  Uint8Array: "readonly",
  setTimeout: "readonly",
  clearTimeout: "readonly",
  console: "readonly",
};

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
    languageOptions: { parser: tsParser, globals: browserGlobals },
    plugins: { "@typescript-eslint": ts },
    rules: {
      "no-console": ["error", { allow: ["warn", "error"] }],
      // La regla base marca falsos positivos en nombres de parámetros de tipos
      // de función; la de TS los entiende correctamente.
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
  },
  ...svelte.configs["flat/recommended"],
  {
    // Svelte 5 con <script lang="ts">: el parser de Svelte delega en el de TS.
    files: ["**/*.svelte"],
    languageOptions: {
      parser: svelteParser,
      parserOptions: { parser: tsParser },
      globals: browserGlobals,
    },
    plugins: { "@typescript-eslint": ts },
    rules: {
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
  },
  { ignores: [".svelte-kit/", "build/", "node_modules/"] },
];
