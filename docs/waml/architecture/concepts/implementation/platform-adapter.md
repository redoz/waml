---
type: uml.Class
title: Platform Adapter
description: The native and browser boundary for bundle saving, browser boot selection, and external URLs.
stereotype: runtime
sources:
  - { id: native-save, resource: ../../../../../crates/waml-editor/src/native_save.rs, title: crates/waml-editor/src/native_save.rs }
  - { id: platform-browser, resource: ../../../../../crates/waml-editor/src/platform_browser.rs, title: crates/waml-editor/src/platform_browser.rs::PlatformBrowser }
  - { id: browser-boot, resource: ../../../../../crates/waml-editor/src/browser_boot.rs, title: crates/waml-editor/src/browser_boot.rs::BrowserBootSource }
  - { id: api-save, resource: ../../../../../crates/waml-editor/src/api_save.rs, title: crates/waml-editor/src/api_save.rs }
---

# Platform Adapter

## Notes
- `native_save.rs` validates and atomically writes native bundle changes.
- `browser_boot.rs` selects browser boot input. `api_save.rs` builds and interprets the browser API write contract.
- `PlatformBrowser` implements external URL opening separately for native and WebAssembly builds.
- Platform adapters perform external effects. They do not own editor session state or semantic analysis.
