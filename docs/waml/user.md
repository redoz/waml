---
type: uml.Actor
title: User
description: A person who opens a WAML bundle to read it.
---

# User

## Relationships
- associates [Native Editor](./architecture/concepts/runtime/native-editor.md): 1 user to 1 editor

## Notes
- A user opens a bundle and reads its views. A user makes no change to the content.
- A user can receive a bundle as a share link. The user then needs no installation and no account.
- An [Author](./architecture/concepts/workflows/author.md) is a user who also changes the content.
