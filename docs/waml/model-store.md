---
type: uml.Class
title: ModelStore
description: Source of truth for model mutations.
---

# ModelStore

## Notes
- createGhostPackage registers an empty dir and emits no op — it materializes on first child.
- insertPackage builds a pkg.insert op and runs it through apply_ops.
- On path collision: keeps prior state, surfaces the error, returns false.
