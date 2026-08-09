# `.waml/`

This directory holds settings for **this project**, written by WAML tooling.
Nothing in here is part of your model -- the model is the markdown in the
project itself, and it will load whether this directory exists or not.

There are two files, and they belong to different people:

`editor.json` is yours. It holds the layout you set up: things like how wide
you dragged the model tree and the inspector columns.

`project.json` is the project's. It holds settings about how the model itself
is read -- currently `max_view_depth`, how far a folder view will follow its
view chain before giving up. Any WAML tool reads it, not just the editor.

Both are versioned, so newer tooling can read a file an older version wrote.

## Should I commit this?

Different answers for the two files, which is why they are two files.

Most people will want to gitignore `editor.json`. Those are your personal
window proportions: they churn in every diff and conflict with everyone
else's, while telling no one anything about the model.

`project.json` is usually worth committing. It says something true about the
model rather than about your screen, and a team is better off agreeing on it
than each discovering it separately.

The tooling deliberately does **not** write a `.gitignore` entry for you.
Committing your layout is a perfectly reasonable thing for a team to do --
a wide tree for a deeply nested model, say -- and that should be a decision
someone made on purpose rather than one made quietly on their behalf.
