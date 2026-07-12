# Classifier Document Editor — Design (deferred)

**Date:** 2026-07-12
**Status:** Parked. Captured so the idea isn't lost; not scheduled. Depends on the package/navigator work (`2026-07-12-uaml-packages-navigator-design.md`), which stubs *View/edit properties* to the existing Inspector until this lands.

## Goal

Replace the Inspector, for classifier editing, with a focused **document editor** — a large modal shaped like an A4 sheet to give editing a "document" feel, matching the fact that each classifier *is* a markdown document on disk.

## Sketch (to be brainstormed into a full spec later)

- **A4 document modal.** Opened from the navigator's classifier action menu (*View/edit properties*) and anywhere a classifier is opened for editing.
- **Markdown-aware fields.** Any field that accepts markdown gets a live preview of the rendered markdown as you type.
- **Raw-markdown escape hatch.** Bail out of the structured editor into the underlying raw OKF markdown for the document, edit directly, and come back.
- **Part D — LSP integration.** When running under a `uaml serve` scenario, the raw-markdown editor connects to the UAML language server for completions and diagnostics. This rides the existing LSP track (`2026-07-12-uaml-lsp-design.md`) and is a further sub-step.

## Why deferred

A per-field markdown preview, a structured⇄raw toggle, and live LSP form their own project. Folding them into the package/navigator spec would stall packages behind an editor overhaul. Ship packages first (with the Inspector stub), then brainstorm this into a full design.
