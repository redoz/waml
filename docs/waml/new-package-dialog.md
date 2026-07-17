---
type: uml.Class
title: NewPackageDialog
description: Collects the starter, name, and placement.
---

# NewPackageDialog

## Notes
- Renders one flat starter list: empty, the four diagram kinds, then committed templates.
- Emits a NewPackagePayload through onAdd on submit.
- Blocks submit on an empty name or a path collision.
