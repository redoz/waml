import js from "@eslint/js";
import tseslint from "typescript-eslint";
import prettier from "eslint-config-prettier";
import globals from "globals";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/.claude/**",
      "**/.superpowers/**",
      "packages/okf/test/fixtures/**",
      "packages/web/public/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,

  // Repo-wide rule tuning
  {
    rules: {
      // Intentional at boundaries (React Flow data). Kept visible as a
      // warning to tighten over time, not block the build.
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", caughtErrorsIgnorePattern: "^_" },
      ],
    },
  },

  // Browser Svelte app (plain-TS modules; .svelte files are checked by svelte-check)
  {
    files: ["packages/web/**/*.ts"],
    languageOptions: { globals: { ...globals.browser } },
  },

  // Node code (shared lib)
  {
    files: ["packages/okf/**/*.ts"],
    languageOptions: { globals: { ...globals.node } },
  },

  // Framework-free core (browser-coupled: localStorage, document, location, …)
  {
    files: ["packages/core/**/*.{ts,tsx}"],
    languageOptions: { globals: { ...globals.browser } },
  },

  // Tests: fixtures and boundary mocks legitimately need `any`
  {
    files: ["**/*.test.{ts,tsx}", "**/test/**/*.{ts,tsx}"],
    languageOptions: { globals: { ...globals.node } },
    rules: { "@typescript-eslint/no-explicit-any": "off" },
  },

  // Turn off stylistic rules that Prettier owns — keep last.
  prettier,
);
