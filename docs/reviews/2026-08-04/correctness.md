# Correctness review

- Dimension: Correctness
- Date: 2026-08-04
- Files examined: 14 (deep reads) + targeted greps across `crates/waml`, `crates/waml-syntax`, `crates/waml-cli`, `crates/waml-editor`

Overall verdict: the top-risk surfaces (incremental reparse, pulldown seam, share/bundle transport codecs, write-back transaction, LSP position mapping, boot selection) are unusually defensive — checked arithmetic, explicit error enums, and adversarial-input tests are the norm. No critical or high findings. The items below are real but bounded.

### [C-1] LSP diagnostic fallback relocates out-of-range diagnostics to line 0
Severity: medium
File: C:\dev\waml\crates\waml-cli\src\lsp\map.rs:213-223
Evidence:
```rust
let requested_line = d.line.saturating_sub(1);
let (line, line_text) = document.text().shared().lines().enumerate()
    .nth(requested_line)
    .map(|(line, text)| (line as u32, text))
    .unwrap_or((0, ""));
```
Why it's wrong: when a core `Diagnostic` carries a `line` past the document's last line (or points at a trailing empty line, which `str::lines()` never yields for a text ending in `\n`), the diagnostic silently jumps to line 0 with an empty range. The user sees the squiggle at the top of the file instead of near the real location. Clamping to the last line would preserve locality.
Suggested fix: on `nth` miss, clamp to the last `(index, line)` pair (or `line_count - 1` with an empty line text) instead of `(0, "")`.
Confidence: CONFIRMED (code reading; behavior follows directly from `unwrap_or((0, ""))`).

### [C-2] write_back rejects case-differing paths even on case-sensitive filesystems
Severity: low
File: C:\dev\waml\crates\waml-cli\src\io.rs:275-281
Evidence:
```rust
let key = target.to_string_lossy().to_lowercase();
if !targets.insert(key) {
    return Err(... "duplicate filesystem target: {logical}" ...);
}
```
Why it's wrong: duplicate detection lowercases unconditionally, so a bundle legitimately containing `A.md` and `a.md` on Linux (where they are distinct files) is refused. The failure is conservative (refuse, not corrupt) and probably a deliberate portability guard, but the code makes it a hard error on every platform without saying so. Also, Unicode `to_lowercase` is not NTFS's case-folding, so the guard is approximate in both directions.
Suggested fix: keep the check but scope it (`#[cfg(windows/macos)]`) or document the deliberate cross-platform restriction in the error message.
Confidence: CONFIRMED (code reading).

### [C-3] Stale doc comment on `utf16_col` describes a different function
Severity: low
File: C:\dev\waml\crates\waml-cli\src\lsp\map.rs:179-188
Evidence:
```rust
/// True iff the document's frontmatter declares a recognized WAML `type:`.
/// ... (frontmatter-scan rationale) ...
/// UTF-16 code-unit offset of byte offset `byte_col` within `line_text`.
pub fn utf16_col(...)
```
Why it's wrong: the first six doc lines describe a frontmatter WAML-filter function that no longer lives here; only the final line documents `utf16_col`. Misleading for the next reader of the byte→UTF-16 seam.
Suggested fix: delete the frontmatter paragraph or move it to wherever the filter now lives.
Confidence: CONFIRMED.

### [C-4] Bundle-envelope part markers are matched by substring, not line-anchored
Severity: low
File: C:\dev\waml\crates\waml\src\bundle_envelope.rs:256
Evidence:
```rust
let Some(relative) = text[body_start..].find(&active_prefix) else { ... };
```
Why it's wrong: `split_bundle` accepts a part marker anywhere in the byte stream, including mid-line. The encoder's nonce-collision retry (line 321-323) guarantees its own output never trips this, so round-trips are exact; but a hand-edited envelope whose body quotes the active marker text will be mis-split or fail with a marker error at a surprising offset. Anchoring markers to line starts would make hand-edited bundles behave predictably. (I verified cross-boundary prefix formation is impossible: the prefix contains `<` only at position 0.)
Suggested fix: require the match to be at `body_start` or preceded by `\n` before treating it as a marker.
Confidence: CONFIRMED (behavior); impact PLAUSIBLE (only hand-authored envelopes).

### [C-5] Share-link base64url decoder accepts non-canonical encodings
Severity: low
File: C:\dev\waml\crates\waml\src\share.rs:166-187
Evidence:
```rust
if bits >= 8 { bits -= 8; out.push((acc >> bits) as u8); }
// leftover 1-7 bits at end are silently discarded; no length % 4 == 1 check
```
Why it's wrong: dangling trailing bits (including the invalid `len % 4 == 1` shape) and nonzero padding bits are ignored, so multiple distinct fragments decode to the same bundle. No security or data-loss consequence here (the payload is then deflate-validated), but decode is not the strict inverse of encode, which the module's determinism doc implies.
Suggested fix: reject `s.len() % 4 == 1` and require the discarded trailing bits to be zero.
Confidence: CONFIRMED.

### [C-6] `FmValue::Num(f64)` derives `PartialEq`
Severity: low
File: C:\dev\waml\crates\waml\src\frontmatter.rs:7-17
Evidence: `#[derive(Debug, Clone, PartialEq)] pub enum FmValue { ... Num(f64), ... }`
Why it's wrong: a NaN payload would make a frontmatter compare unequal to itself, breaking any change-detection that relies on `Frontmatter: PartialEq`. Currently unreachable — the line parser only admits `^-?\d+(\.\d+)?$` (line 5) and untagged JSON deserialize cannot produce NaN — so this is a latent trap, not a live bug.
Suggested fix: none required now; a comment noting the NaN-free invariant (or a total-order compare) would pin it.
Confidence: PLAUSIBLE (latent only).

## Not findings (checked, fine)

- `waml-syntax/src/incremental.rs` — `ChangeMap::checked` overlap/ordering rules (incl. insertion-at-same-point), `translate_boundary` insertion bias, and every offset op checked; the `.unwrap()`s in `transfer_mapped_annotations` are bounded by the u32 `TextSize` invariant of any constructible `SourceText`.
- `waml-syntax/src/markdown/scan/pulldown.rs` — every raw pulldown range screened via `range_is_well_formed` (bounds + char-boundary); unmapped tags keep the open-stack balanced; malformed ranges force a raw-text fallback.
- `waml-syntax/src/markdown/reparse.rs` — reference-definition/use guards are deliberately conservative (false positives only widen work); window-vs-full reference resolution divergence explicitly forces full fallback; splice/restore paths use checked widths throughout.
- `waml/src/share.rs` — decompression capped at 64 MiB, bogus counts don't pre-allocate, truncation/lying-length/UTF-8 all covered by tests; no panic path on hostile fragments.
- `waml/src/bundle_envelope.rs` — wasm `production_nonce` avoids `SystemTime::now()` (the wasm-panic trap); native uses `unwrap_or_default()`; encoder nonce-collision check covers path, encoded path, and body, and cross-boundary prefix formation is impossible.
- `waml-editor/src/browser_boot.rs` — Share > api > bundle > Start precedence pure and host-tested; site config cannot smuggle a Share fragment; percent-decode errors are diagnostics, not panics; `decode_boot_bundle` rejects HTML-at-the-URL with a reason.
- `waml-cli/src/io.rs write_back` — stage-then-rename with journal, ordered rollback, displaced-file reinstall on double failure, staging retained with its path named in the error when cleanup fails; symlink targets rejected via `symlink_metadata().is_file()`.
- `waml/src/analysis.rs` — `validate_disjoint_claims` correct on the BTreeMap shape; promoted-markdown update validation covers missing/stale/mismatch cases with typed reasons; no unwraps on the candidate path.
- `waml/src/ops/mod.rs`, `okf.rs` — the MAP's "panic density" counts are almost entirely `#[cfg(test)]` code; non-test unwraps are invariant-guarded (`f.node(...).unwrap()` fails only on u32 width overflow, unreachable for constructible sources).
- `waml/src/uml/syntax/parser.rs` (spot-checked) — lexer indexing guarded by `at < content_end` and single-byte match arms; bad atoms produce diagnostics (`BadToken`), not panics.
- `waml/src/solve/route.rs` — endpoint-on-border invariant explicitly handled in `connect_ends`/spread pass 2 and guarded by an in-file test ("Every route endpoint must stay ON the border").
- `waml-cli/src/lsp/map.rs` position math — CRLF-interior offsets rejected, UTF-16 round-trips tested incl. astral plane; `line_bounds` handles the final empty line.
- Widget hover discipline (sampled: canvas/behavior, inspector_panel, document_header, panel_splitter) — every `hover_in` has a matching `hover_out`/`drag_end`, incl. the drag-in-progress guard on `FingerHoverOut`; hit-tests use the same rect source as draw at the sampled sites.
- native/wasm cfg splits — confined to 8 files; the one behavioral divergence (nonce source) is documented and deliberate.
