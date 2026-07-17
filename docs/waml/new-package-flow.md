---
type: uml.Sequence
title: New Package flow
---

# New Package flow

## Lifelines
- [User](./user.md)
- [NewPackageDialog](./new-package-dialog.md) as dialog
- [CanvasInner](./canvas-inner.md) as canvas
- [ModelStore](./model-store.md) as store
- [Wasm](./wasm.md) as wasm

## Messages
- User calls dialog: `click New Template`
- User calls dialog: `select starter + name`
- User calls dialog: `submit()`
- dialog calls canvas: `onAdd(payload)`
- alt
  - when `tier == empty`
    - canvas calls store: `createGhostPackage(parent, name)`
    - store replies canvas: `key (ghost, no op emitted)`
  - else
    - canvas calls store: `insertPackage(parent, slug, docs)`
    - store calls wasm: `apply_ops(bundle, pkg.insert)`
    - alt
      - when `path collides`
        - wasm replies store: `throws`
        - store replies canvas: `false (onError surfaced)`
      - else
        - wasm replies store: `next bundle`
        - store replies canvas: `true`
        - canvas calls canvas: `layoutAll()`
- dialog calls canvas: `showNewPackage = false`
