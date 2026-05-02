// ESLint flat config (ESLint 9+) — combines @eslint/js, typescript-eslint, and eslint-plugin-vue.
// Patterns adapted from SayIt's configuration (see doc/reference/sayit-frontend-analysis.md#10).
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import pluginVue from "eslint-plugin-vue";
import globals from "globals";

export default [
  {
    // Ignored paths — auto-generated, build artifacts, and Rust workspace
    ignores: [
      "node_modules/**",
      "dist/**",
      "dist-ssr/**",
      "coverage/**",
      "playwright-report/**",
      "test-results/**",
      "src-tauri/**",
      // shadcn-vue auto-generated primitives — added in C3
      "src/components/ui/**",
      // Auto-generated type declarations
      "**/*.d.ts",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs["flat/recommended"],
  {
    // Vue SFCs need the TypeScript parser inside <script lang="ts"> blocks
    files: ["**/*.vue"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },
  {
    // Browser globals for runtime code
    files: ["src/**/*.{ts,tsx,vue}"],
    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },
  },
  {
    // Node globals for tooling configs
    files: ["*.config.{ts,js,mjs}", "vite.config.ts", "vitest.config.ts", "playwright.config.ts"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
  {
    rules: {
      // shadcn-vue components are intentionally single-word (Button, Switch, etc.)
      "vue/multi-word-component-names": "off",
      // Defer unused-var detection to vue-tsc (avoids noisy duplicate diagnostics)
      "@typescript-eslint/no-unused-vars": "off",
      // Tauri IPC boundaries occasionally need any; warn instead of error
      "@typescript-eslint/no-explicit-any": "warn",
      // Browser globals are provided by the env config above
      "no-undef": "off",
    },
  },
];
