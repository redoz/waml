import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import prettier from "eslint-config-prettier";
import globals from "globals";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "packages/okf/test/fixtures/**",
      "packages/web/public/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,

  // Repo-wide rule tuning
  {
    rules: {
      // Intentional at boundaries (Fastify req.body, React Flow data). Kept
      // visible as a warning to tighten over time, not block the build.
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", caughtErrorsIgnorePattern: "^_" },
      ],
    },
  },

  // Browser React app
  {
    files: ["packages/web/**/*.{ts,tsx}"],
    languageOptions: { globals: { ...globals.browser } },
    plugins: { "react-hooks": reactHooks, "react-refresh": reactRefresh },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },

  // Node code (server + shared lib)
  {
    files: ["packages/server/**/*.ts", "packages/okf/**/*.ts"],
    languageOptions: { globals: { ...globals.node } },
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
