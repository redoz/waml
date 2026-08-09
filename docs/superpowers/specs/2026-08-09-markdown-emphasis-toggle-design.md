# Markdown Emphasis Toggle

## Goal

Let a user switch each source or Markdown editor tab between `Code` and
`Layout` emphasis. A per-user setting supplies the initial emphasis for new
tabs. Changing one tab does not change the setting or any other open tab.

## Ownership

- `EditorConfig` owns the per-user default. It serializes the value as `code`
  or `layout`, and an absent field defaults to `code` for compatibility with
  existing configuration files.
- The application session reads the configured default and makes it available
  when it creates a source or Markdown document view.
- Each source or Markdown document view owns its current emphasis after
  creation. The value is not shared with other tabs.
- `MarkdownEditor` remains the authority that applies an `EditorEmphasis` to
  its installed presentation through `set_emphasis`.

The project-level `.waml/settings.json` does not own this preference. Emphasis
is a user presentation preference, not bundle or project state.

## Header Action

Add one compact action button to the source or Markdown tab header. Follow the
existing eye/source action pattern: the button shows the destination state,
not the current state.

- Current mode `Code`: show the layout destination icon and the tooltip
  `Use layout emphasis`.
- Current mode `Layout`: show the code destination icon and the tooltip
  `Use code emphasis`.
- Activate the button to switch the current tab to the other emphasis.

The action is available only when the document body is backed by a Markdown
editor that supports emphasis. It does not change the user configuration.

## Creation and Data Flow

1. Load `markdown_emphasis` from the per-user editor configuration. Use `Code`
   when the field is absent.
2. Store the loaded value in application-session state.
3. When the application creates a source or Markdown document view, copy the
   session default into that view.
4. When the view mounts or installs its Markdown editor, apply the copied value
   through `MarkdownEditorRef::set_emphasis`.
5. When the header action is activated, invert the view-owned value, apply it
   to that tab's editor, and refresh the header action projection.

Already-open tabs keep their current value if the session default changes.
New tabs use the session default current at their creation time.

## Persistence

This change wires the setting into the configuration schema and load path but
does not add settings UI. The tab action never stores configuration. A future
settings surface can change the per-user default through the configuration
authority without changing tab behavior.

## Failure Behavior

- Invalid serialized emphasis values use normal configuration-load failure
  handling; they do not create a second fallback path.
- If a Markdown editor is not mounted, the view retains its emphasis and
  applies it when the editor becomes available.
- Reapplying the current emphasis remains a no-op through the existing
  `MarkdownEditorRef::set_emphasis` contract.

## Testing

Add focused tests that prove:

- old user configuration without `markdown_emphasis` loads `Code`;
- `code` and `layout` round-trip through the user configuration;
- a new source or Markdown view copies the session default;
- the header action projects the opposite, destination mode;
- activating the action switches `Code` to `Layout` and `Layout` to `Code`;
- two tabs can hold different emphasis values;
- changing one tab does not change the other tab or the session default.

Use test-first development. Each production behavior must first have a focused
test that fails because that behavior is absent.

## Non-goals

- A settings or preferences user interface.
- Updating the user default when the tab action is used.
- Persisting per-tab emphasis across application restarts.
- Changing Markdown syntax, presentation compilation, or the two existing
  emphasis profiles.
