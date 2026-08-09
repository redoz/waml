# Publish a Site

**Goal:** Each push to the main branch publishes the web artifact again. A
person does no step.

**Why:** A publication step that a person must remember does not occur.

**Done when:** A push to the main branch makes a complete artifact that
operates, or the workflow fails and reports the failure.

**Status:** partial
**MVP:** yes

## Shipped behavior

#### BROWSER-012 — publication accepts only a complete web artifact

**Applies to:** browser

**Given** the Pages workflow has built a web artifact
**When** the workflow prepares the artifact for publication
**Then** it continues only when every referenced artifact file exists

**Evidence:** `.github/workflows/pages.yml:184` and `scripts/verify-web-artifact.mjs:22`

## Verification gaps

- BROWSER-012 — target: browser; The verifier checks artifact structure before publication, but no browser test asserts a deployed artifact has every referenced resource.

## Notes

- The web-artifact build waits for the full CI job. It then removes unused
  fonts, adds the brand and runtime shell, and verifies the complete artifact.
- The build tool uses the same exact graphics-library revision as the editor.
