---
type: uml.UseCase
title: Save and Undo
description: An author saves work and moves through edit history.
---

# Save and Undo

## Relationships
- associates [Author](../actors/author.md)

## Owning goal

- [Save and Undo](../../goals/author-in-the-editor/save-and-undo.md)


## Scenarios

- [NATIVE-027](../../goals/author-in-the-editor/save-and-undo.md#native-027-—-an-accepted-source-edit-advances-once)
- [NATIVE-028](../../goals/author-in-the-editor/save-and-undo.md#native-028-—-undo-and-redo-exchange-reciprocal-history-entries)
- [NATIVE-029](../../goals/author-in-the-editor/save-and-undo.md#native-029-—-undo-to-a-savepoint-clears-dirty-state)
- [NATIVE-030](../../goals/author-in-the-editor/save-and-undo.md#native-030-—-a-failed-final-save-keeps-dirty-work-open)
- [NATIVE-031](../../goals/author-in-the-editor/save-and-undo.md#native-031-—-a-successful-bundle-open-clears-the-save-error)
- [NATIVE-048](../../goals/author-in-the-editor/save-and-undo.md#native-048-—-a-new-edit-after-undo-clears-redo-history)
- [NATIVE-049](../../goals/author-in-the-editor/save-and-undo.md#native-049-—-an-invalid-proposal-does-not-change-session-history)
- [NATIVE-056](../../goals/author-in-the-editor/save-and-undo.md#native-056-—-a-failed-save-blocks-close-and-keeps-document-state)
- [NATIVE-057](../../goals/author-in-the-editor/save-and-undo.md#native-057-—-a-successful-save-permits-close-and-clears-document-state)
- [NATIVE-062](../../goals/author-in-the-editor/save-and-undo.md#native-062-—-an-accepted-source-proposal-becomes-current)

