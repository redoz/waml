---
type: uml.Actor
title: Author
description: A person who creates or imports an OKF Bundle and responds to diagnostics.
---

# Author

## Relationships
- associates [OKF Bundle](../model/okf-bundle.md): 1 author to 0..* bundles
- associates [Diagnostic](../model/diagnostic.md): 1 author to 0..* diagnostics
- associates [Editor](./editor.md): 1 author to 1 editor

## Notes
- The author creates or imports a bundle. The author corrects the content that the diagnostics identify.
- The author can work in the editor, in a text editor, or with the command-line tool. The bundle is the same in all three.
- The author states the arrangement of a view as intent. The author does not give coordinates.
