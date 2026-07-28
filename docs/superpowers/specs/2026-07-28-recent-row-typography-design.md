# Recent Row Typography and Spacing

## Goal

Make each start-screen Recent row feel smaller and better balanced while preserving its current 48 px pitch and interaction behavior.

## Considered approaches

1. Change the shared `fonts.text_label` and `fonts.text_menu` tokens. This would shrink unrelated labels, menus, tabs, and inspector text, so the blast radius is too broad.
2. Add compact typography roles to the centralized scale. This preserves the typography gate while leaving every existing consumer unchanged. This is the selected approach.
3. Override the font sizes locally in `RecentRowView`. The typography gate intentionally rejects this because chrome sizes must come from `mod.fonts`.

## Design

- Keep the Recent row at 48 px tall.
- Render the title at 10 px.
- Render both the relative time and path at 9 px.
- Keep the title and relative time in the same horizontal title row with vertical centering, so their visual centerlines align.
- Retain the title-over-path stack, but replace the negative inter-line spacing with non-negative spacing and explicit bottom breathing room so the path no longer sits against the row's lower edge.
- Leave the package glyph, pin anchor, hover behavior, colors, and data flow unchanged.

The centralized scale gains `text_compact_label` (IBM Plex Sans Medium, 10 px) and `text_micro` (IBM Plex Sans Regular, 9 px). The Fonts style-guide overlay exposes both roles. Existing role values and consumers remain unchanged.

## Verification

- Use the existing chrome typography gate as the regression check against ad-hoc size overrides.
- Update the Fonts-overlay coverage test from eight to ten roles using a red/green cycle.
- Run the focused `waml-editor` Recent-row tests and the crate test suite.
- Build and launch the editor from the isolated worktree, then capture the native window to verify the visual hierarchy, centerline alignment, and lower spacing at the actual HiDPI scale.

## Scope

Only the Recent item row consumes the two new roles. The Fonts style-guide overlay documents them; start-screen headings, action links, list capacity, row interactions, and existing typography-role values remain unchanged.
