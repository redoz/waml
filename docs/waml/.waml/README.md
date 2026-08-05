# `.waml/`

This directory holds editor state for **this project**, written by the WAML
editor. Nothing in here is part of your model -- the model is the markdown in
the project itself, and the editor will load it exactly the same whether this
directory exists or not. Deleting `.waml/` costs you nothing but the layout
you had set up.

`settings.json` is where that layout lives: things like how wide you dragged
the model tree and the inspector columns. It is versioned, so a newer editor
can read a file an older one wrote.

## Should I commit this?

Most people will want to add `.waml/` to their `.gitignore`. These are your
personal window proportions, and they will churn in every diff and conflict
with everyone else's while telling no one anything about the model.

The editor deliberately does **not** write a `.gitignore` entry for you. If a
team wants to agree on a shared starting layout for a project -- a wide tree
for a deeply nested model, say -- committing this file is a perfectly
reasonable thing to do, and that should be a decision someone made on purpose
rather than one the editor made quietly on their behalf.
