# The makepad fork

WAML's editors build against a fork of makepad
([redoz/makepad](https://github.com/redoz/makepad)), pinned by rev in the
workspace manifest and nowhere else. This file is the inventory the audit found
missing: 44 commits and roughly +2,992/−563 against upstream `dev`, previously
described only in a manifest comment.

## Why a fork exists

Two payoffs, both real: one codebase runs native and web, and the SDF text and
shape rendering is crisp at every zoom. The taxes are equally real — hand-authored
icon splines, a bespoke UI-test harness, and repeated time lost to shader boot
and zoom framerates. The strategic question (upstream aggressively, or declare
the fork permanent and give it its own CI) is a decision row in the audit
ledger, not something this file answers.

## The branch

All fork work lands on the `waml` branch, and the manifest pins a **sha**, never
the branch name. The fork carries several rebase-duplicated lines whose commits
share messages but not shas, so a branch tip can silently rewind past work that
is already depended on. Before moving the pin, check drift in both directions:

```bash
git merge-base --is-ancestor <current-pin> <candidate>
git branch -a --contains <current-pin>
```

## Inventory

Classified so the fork's endgame is a decision about a list rather than about a
feeling:

- **upstream** — generally useful, should be proposed to makepad. Carrying these
  is the tax; landing them upstream is the exit.
- **product** — WAML-specific behaviour upstream has no reason to want. Ours to
  carry for as long as the fork exists.
- **dead** — reverted, superseded, or `wip:`. These should come out on the next
  rebase; they are pure carrying cost.

| Commit | Subject | Class | Note |
|---|---|---|---|
| `6534634a` | fix(widgets): give every Cx its own widget tree | upstream | Generally useful; propose upstream. |
| `7bb5e50a` | fix(platform): compile the headless backend on linux and windows | upstream | Generally useful; propose upstream. |
| `9e428ab8` | fix(widgets): align semantic snapshot rounding | upstream | Generally useful; propose upstream. |
| `51521304` | test(widgets): cover semantic window context | upstream | Generally useful; propose upstream. |
| `11a1aba8` | feat(widgets): expose semantic child items | product | WAML-specific behaviour; ours to carry. |
| `1fda4ec2` | feat(test): forward application arguments | upstream | Generally useful; propose upstream. |
| `c4937742` | revert(file_tree): drop the waml-only fold and draw hooks | dead | Reverted when waml took ownership of the tree row list; its pair 92df3316 is dead with it. |
| `92df3316` | feat(file_tree): expose the fold scale and whether a row was drawn | dead | Reverted by c4937742. |
| `1a5d1add` | feat(text_flow): expose the selection pair and a decoration gutter for list items | product | WAML-specific behaviour; ours to carry. |
| `62f515dc` | fix(web): copy download bytes out of wasm memory | upstream | Generally useful; propose upstream. |
| `f6ca0863` | feat(web): add generic file download bridge | upstream | Generally useful; propose upstream. |
| `2ad35404` | @ feat(file_tree): expose the animated folder open amount | dead | FileTree fold accessor, superseded by waml-owned tree rows. |
| `619d61be` | perf(web): compile every shader before querying any compile status | upstream | Generally useful; propose upstream. |
| `60f88a7b` | perf(web): issue every shader link before querying any of them | upstream | Generally useful; propose upstream. |
| `8ec3417f` | feat(web): add FromWasmFinishWebGLShaders batch-finish message | upstream | Generally useful; propose upstream. |
| `01ed72d8` | fix(web): apply the SLUG size cutoff on web as on linux | upstream | Generally useful; propose upstream. |
| `040c93e9` | fix(web): use the shared DrawTextSlug helper instead of inlining the solver | upstream | Generally useful; propose upstream. |
| `c9c95bd2` | fix(web): implement seconds_since_app_start instead of returning 0.0 | upstream | Generally useful; propose upstream. |
| `ba37e49b` | fix(web): compare uniform block contents instead of pointer identity | upstream | Generally useful; propose upstream. |
| `83a46646` | fix(script): drop path normalizer superseded by logical resource keys | dead | Removes the normalizer the note above designed. |
| `a3ad91ba` | fix(cargo-makepad): read cargo metadata from stdout alone | upstream | Generally useful; propose upstream. |
| `377ed372` | fix(cargo-makepad): resolve git dependency dirs via cargo metadata | upstream | Generally useful; propose upstream. |
| `19f694c6` | fix(script): name cross-crate widget resources | upstream | Generally useful; propose upstream. |
| `6231f89c` | fix(script): omit wasm manifest paths | upstream | Generally useful; propose upstream. |
| `e08887f6` | fix(script): use logical wasm resource keys | upstream | Generally useful; propose upstream. |
| `c2a8bb4c` | @ fix(text): make layout_params available on all targets | product | WAML-specific behaviour; ours to carry. |
| `8214dab5` | feat(text): add uncached layout API | upstream | Generally useful; propose upstream. |
| `308ce7dd` | feat(markdown): expose scroll anchors | product | WAML-specific behaviour; ours to carry. |
| `26da4f63` | fix(markdown): disambiguate fragment anchors | product | WAML-specific behaviour; ours to carry. |
| `fbb881c5` | feat(file-tree): allow app-owned folder toggles | product | WAML-specific behaviour; ours to carry. |
| `4648b151` | fix(markdown): stabilize fragment anchors | product | WAML-specific behaviour; ours to carry. |
| `e571efc2` | feat(markdown): add fragment anchors | product | WAML-specific behaviour; ours to carry. |
| `2159bab3` | docs(script): design logical wasm resources | dead | Design note, not code. |
| `647c5e39` | docs(script): design path normalization | dead | Design note, superseded by 83a46646. |
| `d545c874` | fix(platform): enable FXC optimization to fix trig-sdf shader spike | upstream | Generally useful; propose upstream. |
| `65b74840` | fix(web): paint in the same task as a resize realloc | upstream | Generally useful; propose upstream. |
| `bf4aa907` | feat(web): let a lone finger drive the mouse event stream | upstream | Generally useful; propose upstream. |
| `90930e5c` | fix(script): compare crate manifest paths separator-agnostically | upstream | Generally useful; propose upstream. |
| `5284de7b` | fix(web): dispatch Startup before indexing the window pool | upstream | Generally useful; propose upstream. |
| `c20d5d1c` | feat(file_tree): secondary-click row action for context menus | product | WAML-specific behaviour; ours to carry. |
| `4a7fb0e0` | fix(platform): honor handled_x/handled_y scroll occlusion in hits() | upstream | Generally useful; propose upstream. |
| `fdf20015` | wip: transparent DirectComposition popup (per-pixel alpha) | dead | A `wip:` commit baked into the pinned lineage. Rebase it out or finish it. |
| `f553709b` | chore(gitignore): ignore more tooling folders | dead | Tooling gitignore noise; drop on the next rebase. |
| `94fcf5c4` | feat(svg): fit DrawSvg bounds to STROKED extent, not bare centerline | upstream | Generally useful; propose upstream. |

## Keeping this file true

It is written against pin `6534634a60aa8101fa93bb1ddf0edde6949e520a`. Regenerate
the table when the pin moves:

```bash
git -C <makepad-clone> log --format="%h|%s" --no-merges upstream/dev..<new-pin>
```
