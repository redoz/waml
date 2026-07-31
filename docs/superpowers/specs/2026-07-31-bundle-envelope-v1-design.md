# WAML Bundle Envelope v1 Design

## Status

Approved in conversation on 2026-07-31.

## Problem

The current concatenated-document transport uses comments such as
`<!-- shop/order.md -->` as document separators. The decoder searches the full
input for any line-shaped comment that ends in `.md`. An ordinary Markdown
document can therefore activate bundle decoding by accident. When that happens,
the decoder discards all bytes before the first matching comment.

The current format also has no explicit version, does not distinguish transport
metadata from authored comments, and handles LF and CRLF differently.

## Goals

- Bundle detection must never discard an authored prefix.
- Ordinary Markdown comments and fenced examples must remain document content.
- A bundle must identify itself at byte zero.
- One marker must both declare the envelope and open the first document.
- Later part boundaries must be unique to that envelope.
- Common paths must remain readable.
- Encoding and decoding must preserve every document byte exactly.
- Malformed envelopes must produce explicit errors.
- LF and CRLF marker line endings must both work.

## Non-goals

- The envelope does not provide authentication, confidentiality, or integrity.
- The nonce is not a security token.
- The old headerless `<!-- path.md -->` transport syntax will not remain
  supported.
- This change does not alter WAML document syntax, parsing, or semantic models.
- This change does not define a streaming or HTTP wire protocol.

## Format

Each document starts with one HTML comment marker:

```markdown
<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/order.md -->
# Order

<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/customer.md -->
# Customer
```

The first marker starts at byte zero. It has this logical grammar:

```text
"<!-- waml/1 part " NONCE " " ENCODED_PATH " -->" LINE_ENDING
```

- `NONCE` is exactly 32 lowercase hexadecimal characters.
- `ENCODED_PATH` is a percent-encoded UTF-8 bundle path.
- `LINE_ENDING` is either LF or CRLF.
- The spaces and fixed tokens shown in the grammar are exact.
- The marker has no trailing spaces.

The first marker is both the envelope sentinel and the first part boundary.
There is no separate envelope header.

## Path encoding

Path bytes in the RFC 3986 unreserved set remain literal. `/` also remains
literal so normal bundle paths stay readable. All other UTF-8 bytes use uppercase
`%HH` escapes. This includes whitespace, `%`, `<`, and `>`.

Examples:

```text
shop/order.md
shop/special%20order.md
shop/%E6%B3%A8%E6%96%87.md
```

The decoder rejects malformed percent escapes, invalid UTF-8, and decoded paths
that fail `BundlePath` validation. Encoding and then decoding a valid bundle path
must recover the exact normalized path.

## Detection and decoding

The decoder examines only byte zero when deciding whether the input is a bundle:

1. If the input does not start with `<!-- waml/`, it is plain Markdown.
2. If it starts with `<!-- waml/` but the version or first marker is malformed,
   decoding returns an envelope error. It does not downgrade the input to plain
   Markdown.
3. A valid `waml/1` first marker activates bundle mode and supplies the envelope
   nonce.
4. The decoder splits only on complete `waml/1 part` markers that contain that
   exact nonce. Later markers can start at any byte offset; only the first marker
   must start at byte zero.
5. Text that resembles a marker but has another nonce remains document content.
6. A same-nonce marker prefix with malformed syntax is an envelope error.
7. Each document contains all bytes after its marker line and before the next
   matching marker. The final document ends at end-of-input.

Because detection occurs only at byte zero, any preamble makes the entire input
plain Markdown. No prefix can be silently dropped.

The decoder must distinguish three outcomes in its API:

```rust
Result<Option<Vec<(String, String)>>, BundleEnvelopeError>
```

- `Ok(None)` means ordinary Markdown.
- `Ok(Some(parts))` means a valid bundle envelope.
- `Err(error)` means the byte-zero WAML sentinel was present but malformed.

The implementation can retain the existing public name or introduce a clearer
codec API, but callers must not infer detection from a magic fallback path such
as `pasted/doc.md`.

## Encoding and nonce selection

The encoder emits one marker for every document. It uses one 128-bit nonce,
formatted as 32 lowercase hexadecimal characters, for the complete envelope.
It writes later markers immediately after the preceding document bytes. It does
not insert a newline or other separator padding, so bodies with and without a
final newline both round-trip exactly. The marker's own required line ending
belongs to the envelope, not to the following document body.

Before emission, it verifies that a marker prefix using the candidate nonce
cannot be recognized inside any path or document content. If a collision exists,
it requests another nonce and repeats the check. Tests use an injected nonce
source so collision and retry behavior are deterministic.

Nonce generation need not be cryptographically secure. Correctness comes from
checking the selected delimiter against the payload before emission. The
production generator should avoid a new heavyweight dependency unless the
implementation shows that one is necessary.

The encoder validates every logical path before writing output. An empty bundle,
duplicate path, invalid path, or exhausted nonce source produces an explicit
error rather than partial output.

## Ownership and call flow

The transport codec belongs with the source-bundle boundary, not with parsing or
UML analysis. One module owns:

- marker grammar and version constants;
- path percent encoding and decoding;
- envelope detection and decoding;
- envelope serialization with an injected nonce source;
- structured envelope errors.

CLI input expansion calls the decoder. `Ok(None)` keeps the original display
path and full text as one document. `Ok(Some(parts))` uses the decoded logical
paths. Decoder errors propagate to the CLI and produce a non-zero exit.

CLI commands that emit concatenated documents call the authoritative encoder
instead of formatting `<!-- path.md -->` comments locally.

The parser, syntax tree, incremental analysis, LSP, and editor do not learn the
marker grammar. They continue to consume validated source bundles.

## Compatibility and migration

The old headerless syntax becomes ordinary Markdown. All checked-in bundle
fixtures and expected command output migrate to v1 in the same change as the
decoder and encoder. No compatibility decoder is retained because it would
preserve the ambiguity that this design removes.

The migration must search the repository for old marker producers and fixtures,
not only the primary orders-domain fixture. Historical design documents do not
need rewriting unless a test consumes them as live fixtures.

## Errors

Structured errors must identify at least:

- unsupported envelope version;
- malformed first marker;
- malformed same-nonce part marker;
- invalid nonce syntax;
- invalid percent escape or path UTF-8;
- invalid bundle path;
- duplicate bundle path;
- empty bundle;
- nonce-source exhaustion or repeated collision.

Errors should include a byte offset or line number when the information is
available. User-facing adapters may add the physical input name.

## Verification

Focused tests must cover:

- a valid one-document envelope;
- a valid multi-document envelope;
- exact encode/decode round trips, including empty bodies and bodies with and
  without a final newline;
- LF and CRLF marker endings;
- readable paths and percent-encoded spaces, Unicode, `%`, `<`, and `>`;
- an ordinary `.md` HTML comment in Markdown;
- a marker example inside a fenced code block;
- non-empty content before a marker, preserved as one plain document;
- old `<!-- path.md -->` syntax treated as plain Markdown;
- a later marker with a different nonce preserved as content;
- matching and non-matching marker-like text at arbitrary body byte offsets;
- unsupported versions and malformed byte-zero markers;
- malformed percent escapes and invalid paths;
- duplicate paths;
- deterministic nonce collision followed by successful retry;
- authoritative CLI producer output using the v1 format;
- current checked-in bundle fixtures migrated to v1.

Property tests should verify that encoding then decoding arbitrary valid bundles
recovers the same ordered path/content pairs and that arbitrary plain Markdown
without the byte-zero sentinel is never split.

The final verification includes focused codec and CLI tests, the full workspace
suite, strict Clippy, formatting checks for changed Rust files, and fuzz or
property coverage proportional to the codec's small grammar.

## Acceptance criteria

- No decoder searches arbitrary document lines to decide whether bundle mode is
  active.
- No input prefix can be silently discarded.
- Only a valid byte-zero `waml/1` marker activates bundle mode.
- Only matching-nonce markers split the active envelope.
- All checked-in live bundle fixtures use the v1 form.
- All concatenated-bundle producers use one authoritative encoder.
- Plain Markdown, malformed envelopes, and valid envelopes have distinct API
  outcomes.
- The full workspace verification remains green.
