---
type: uml.Class
title: CanvasInner
description: Hosts the dialog and realizes its choice.
---

# CanvasInner

## Notes
- handleNewPackageAdd routes the payload: empty tier becomes a ghost package.
- Diagram and template tiers go through insertPackage.
- After a successful insert, layoutAll re-positions the new nodes.
