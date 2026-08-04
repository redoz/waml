# Security review — full evaluation

- Dimension: Security
- Date: 2026-08-04
- Files examined: 14 (share.rs, bundle_envelope.rs, source.rs, frontmatter.rs, site_boot via site.rs, waml-cli io.rs / site.rs / main.rs / commands.rs / serve/mod.rs, waml-editor browser_boot.rs, workspace + crate Cargo.tomls, markdown scan/inline/block spot checks, uml/syntax/parser.rs spot check)

---

### [S-1] Unbounded recursion in frontmatter value parsing — hostile document can overflow the stack

- Severity: high
- File: `crates/waml/src/frontmatter.rs:147-155` (also `render_value` :229-255 and the untagged serde `FmValue` derive :10-17)
- Evidence:
  ```rust
  pub(crate) fn parse_value(s: &str) -> FmValue {
      if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
          let items = inner
              .split(',')
              .map(|x| parse_value(x.trim()))
  ```
- Why it's wrong: `parse_value` recurses once per bracket-nesting level with no depth cap. A frontmatter value of `[[[[…]]]]` (one line, ~2 bytes per level, so a ~200 KB line gives ~100k levels) drives recursion until stack overflow — an abort, not a diagnostic. This is reachable from any hostile `.waml` document, share link, or bundle; per the charter, a panic/abort in the LSP or the wasm build takes the whole session down. `render_value` and the untagged serde `Deserialize` for `FmValue` (`List(Vec<FmValue>)`) share the same unbounded recursion, so round-tripping or wire-decoding an already-parsed deep value hits it too.
- Suggested fix: Thread a depth counter through `parse_value` (and `render_value`) and degrade to `FmValue::Str` past a small cap (e.g. 32); for serde, deserialize via a depth-limited visitor or reject deep `extra` objects.
- Confidence: CONFIRMED (by code reading; recursion has no guard anywhere on the path)

### [S-2] Bearer token designed to travel in the URL query string

- Severity: medium
- File: `crates/waml-editor/src/browser_boot.rs:25-26, 53-56`
- Evidence:
  ```rust
  /// `?api=<base>[&token=<token>]`: a live model server.
  Api { base: String, token: Option<String> },
  ...
  token: value("token").map(str::to_owned),
  ```
- Why it's wrong: A secret carried in the query string ends up in browser history, proxy/server access logs, and potentially `Referer` headers. `waml serve` is unimplemented (`Api` currently falls through to the start screen, `app.rs:751`) and `subtle` is already a dependency for constant-time comparison, so the auth design is being built around a leaky channel now.
- Suggested fix: Before `serve` lands, move the token to the URL fragment (never sent to servers) or a one-time cookie/localStorage exchange; strip it from the address bar via `history.replaceState` immediately after read.
- Confidence: CONFIRMED (the parse exists; the leak is latent until serve ships)

### [S-3] Bundle paths admit NTFS alternate data streams (`a:b.md`) on Windows

- Severity: medium
- File: `crates/waml/src/source.rs:37-47` (`BundlePath::parse`), enforced downstream at `crates/waml-cli/src/io.rs:426-441` (`validate_relative`)
- Evidence:
  ```rust
  || normalized
      .as_bytes()
      .get(1)
      .is_some_and(|byte| *byte == b':')
  ```
- Why it's wrong: only a colon at byte index 1 (drive letters, `C:`) is rejected. A path like `notes:payload.md` or `dir/ab:x.md` passes `BundlePath::parse` and, being a single `Component::Normal`, passes `validate_relative` in `write_back`. On Windows/NTFS a colon names an alternate data stream, so a hostile bundle applied via `write_back` (fmt --write, ops apply) can target `notes:payload.md` — a stream on file `notes`, invisible to `collect_md` and to the user, rather than a regular `.md` file. It cannot escape the root, but it writes somewhere the caller cannot see.
- Suggested fix: reject `:` anywhere in a bundle path segment in `BundlePath::parse` (there is no legitimate use of a colon in a model path).
- Confidence: PLAUSIBLE (validation gap confirmed in code; the exact behavior of `fs::rename` onto an ADS target under the `\\?\` root was not executed)

### [S-4] `write_back` symlink/type check races the rename (TOCTOU)

- Severity: low
- File: `crates/waml-cli/src/io.rs:290-299` (check) vs `:353-364` (renames)
- Evidence:
  ```rust
  let metadata = match fs::symlink_metadata(&target) {
      Ok(metadata) if metadata.is_file() => Some(metadata),
      Ok(_) => { return Err(... "bundle target is not a file" ...) }
  ```
  … the `ops.rename(&write.desired, &write.target)` happens later, after all staging.
- Why it's wrong: between the `symlink_metadata` screen and the commit-phase rename, a concurrent actor can swap `target` for a symlink or directory. `rename` replaces the symlink itself rather than following it on both platforms, which caps the damage, but the backup/rollback bookkeeping then operates on a different object than the one screened. Exploitation requires an attacker already writing inside the bundle root, so severity is low for a local CLI.
- Suggested fix: none urgent; document the assumption that the bundle root is not concurrently attacker-writable, or re-verify metadata immediately before the commit renames.
- Confidence: CONFIRMED (the window exists by construction; exploitability is environment-dependent)

### [S-5] Fuzz targets for the hostile-input surface are not run by CI

- Severity: low
- File: `fuzz/fuzz_targets/{parse_write, syntax_edits, outer_mapping, uml_islands}.rs`; `Cargo.toml` (`fuzz/` in `exclude`); `.github/workflows/ci.yml`
- Evidence: MAP §4 and workspace manifest — `fuzz/` is its own workspace and no CI step invokes `cargo fuzz`.
- Why it's wrong: the parser is the primary adversarial boundary (share links, bundles) and the project owns fuzz targets plus seed corpora for it, but nothing exercises them automatically, so hostile-input regressions (like S-1) go undetected between manual runs.
- Suggested fix: add a scheduled/nightly CI job running each target for a bounded time (e.g. `cargo fuzz run <t> -- -max_total_time=120`); add a frontmatter-value target while at it.
- Confidence: CONFIRMED

### [S-6] `render_bundle_ts` interpolates `--export-name` unescaped into generated TypeScript

- Severity: low
- File: `crates/waml-cli/src/commands.rs:132-136`
- Evidence:
  ```rust
  out.push_str(&format!(
      "export const {export_name}: [string, string][] = [\n"
  ));
  ```
- Why it's wrong: `export_name` is a raw CLI argument; a value like `x = evil(); export const y` becomes live code in the emitted module. The attacker is the invoking user, so this is self-injection — but the output is described as "checked-in", so a poisoned name survives into a repo where others run it.
- Suggested fix: validate `export_name` against a JS identifier pattern (`^[A-Za-z_$][\w$]*$`) and refuse otherwise.
- Confidence: CONFIRMED

---

## Not findings

- `crates/waml/src/share.rs` — decode path is solid: `decompress_to_vec_with_limit` caps inflation at 64 MiB (`:32,:101`), lying lengths cannot pre-allocate (`:107` grows per record, with a regression test `:287`), truncation/UTF-8/base64 all return typed errors, hand-rolled base64url rejects out-of-alphabet bytes.
- `crates/waml-cli/src/site.rs` — `is_safe_relative_path` (`:164`) rejects absolute, backslash, empty, `.` and `..` segments; the site is fully assembled in memory before writing; non-`--force` refuses non-empty output dirs and `--force` replaces only site-owned paths.
- `crates/waml-cli/src/io.rs` traversal defense — `validate_relative` allows only `Component::Normal`, `validate_target` canonicalizes the nearest existing ancestor and requires it under the canonicalized root, and non-file targets (symlinks included, via `symlink_metadata`) are refused; the Windows `\\?\` verbatim root from `canonicalize` also defuses device names like `CON.md`.
- `crates/waml/src/bundle_envelope.rs` — `split_bundle` is iterative and offset-based, validates nonce hex, percent-decodes with typed errors, enforces `BundlePath` invariants and duplicate rejection; encode picks a collision-free nonce against the body.
- `crates/waml-editor/src/browser_boot.rs` — share > api > bundle precedence is pure and tested; a site's `waml-boot.txt` can never smuggle a share fragment (`select_site_boot` parses with an empty hash, test `:334`); fetched `?bundle=` must be a real envelope, so an HTML error page cannot open as a document.
- No HTML export of user content exists — the exported site ships the prebuilt editor plus `bundle.waml`; rendering is canvas/WebGL, so node names/labels never become markup. `render_bundle_json`/`render_bundle_ts` JSON-escape all user strings.
- Dependencies — all three git dependencies (makepad-widgets ×2, unicode-bidi) are pinned to commit `62f515dc…`, not a branch tip; registry deps are ordinary versioned crates; no new high-surface dependency observed.
- Secrets — repo-wide sweep for keys/tokens/passwords found nothing beyond test fixtures; the bundle nonce is documented as non-secret.
- Markdown and UML parsers — delimiter handling is stack-machine/iterative (`inline.rs:1088`), heading nesting uses a bounded `u8` depth with explicit stack pops (`uml/syntax/parser.rs:270-298`); no obvious recursive descent over attacker-controlled nesting (unlike frontmatter, S-1).
- `waml serve` stub — prints "not implemented" and exits 2; no listener, no route surface today. The pre-added `axum`/`rand`/`subtle` deps are unused supply-chain surface but were an explicit design decision.
- `collect_md` skips dot-directories during walks, so tool state (`.waml/`) cannot re-enter the model as a phantom document.
