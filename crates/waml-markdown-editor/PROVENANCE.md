# Makepad CodeEditor adaptation provenance

The source behavior characterized here comes from
https://github.com/redoz/makepad.git at commit
`c38f529984eda61e258ca69fb50c6712d85c74c1`, package
`makepad-code-editor` 2.0.0, licensed `MIT OR Apache-2.0`. WAML uses the MIT License grant;
retain `Copyright (c) 2023 Makepad B.V.` with this record.

| WAML module | Upstream source | Retained behavior | Material changes |
|---|---|---|---|
| `selection.rs` | `code_editor/src/selection.rs` | ordered multi-selection, overlap normalization, affinity | UTF-8 `TextSize` positions, revision binding, Unicode grapheme boundaries, geometry-driven vertical movement |
| `history.rs` | `code_editor/src/history.rs` | grouped undo/redo and selection restoration | exact `MarkdownEdit` transactions and immutable snapshots replace mutable `Text` edits |
| `input.rs` | `code_editor/src/session.rs`, `code_editor/src/code_editor.rs` | insert/delete/paste/cut/indent/navigation/click-count behavior | commands lower before mutation; read-only and IME are explicit typed state; IME preedit remains uncommitted and accepted edits cancel active composition |
| `widget.rs` | `code_editor/src/code_editor.rs`, `code_editor/src/code_view.rs`, `code_editor/src/draw_selection.rs` | focus, clipboard, pointer selection, caret visibility, scrolling, drawing | exact change actions replace full-string actions; variable-metric `LayoutSnapshot`, visible-block virtualization, explicit draw order, no upstream `CodeEditor` or `Markdown` dependency |

The new layout implementation does not copy `code_editor/src/layout.rs` or
`wrap.rs`; those modules assume a fixed cell grid. Every deliberate behavioral
divergence has a named test in `tests/widget_parity.rs`.
