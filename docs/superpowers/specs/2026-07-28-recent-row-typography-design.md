# Recent Row Typography and Spacing

## Goal

Make each start-screen Recent row feel smaller and better balanced while preserving its current 48 px pitch and interaction behavior.

## Considered approaches

1. Change the shared `fonts.text_label` and `fonts.text_menu` tokens. This would shrink unrelated labels, menus, tabs, and inspector text, so the blast radius is too broad.
2. Add new shared typography roles. This would formalize the two sizes, but a single component does not justify expanding the global type scale.
3. Override the font sizes locally in `RecentRowView`. This keeps the change isolated and is the selected approach.

## Design

- Keep the Recent row at 48 px tall.
- Render the title at 10 px.
- Render both the relative time and path at 9 px.
- Keep the title and relative time in the same horizontal title row with vertical centering, so their visual centerlines align.
- Retain the title-over-path stack, but replace the negative inter-line spacing with non-negative spacing and explicit bottom breathing room so the path no longer sits against the row's lower edge.
- Leave the package glyph, pin anchor, hover behavior, colors, and data flow unchanged.

The sizes are component-local overrides based on the existing IBM Plex Sans font roles. They do not modify the global font scale.

## Verification

- Add a small layout-spec regression test before changing the declarative widget, proving the intended title, metadata, stack-spacing, and lower-padding values.
- Run the focused `waml-editor` Recent-row tests and the crate test suite.
- Build and launch the editor from the isolated worktree, then capture the native window to verify the visual hierarchy, centerline alignment, and lower spacing at the actual HiDPI scale.

## Scope

Only the Recent item row changes. Start-screen headings, action links, list capacity, row interactions, and shared typography tokens remain out of scope.
