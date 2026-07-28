# Task 4 report

## Implementation

- Added a pulldown-cmark offset mapper with explicit protected-container depth and normalized protected ranges.
- Added lossless shell parsing for clean, malformed, and unclosed initial frontmatter without consulting OKF type semantics.
- Unclosed recovery synchronizes at the first confirmed top-level H1 or H2 only when every preceding candidate line is a plausible flat entry or blank line; otherwise the thematic rule remains raw Markdown.
- Implemented normative trivia ownership: inter-token horizontal whitespace leads the following token, pre-newline horizontal whitespace leads `NewlineToken`, EOF horizontal whitespace leads the missing EOF token, and all trailing trivia is empty.
- Added exact diagnostics and missing tokens for malformed entries and missing frontmatter fences.
- Added fixture-driven deterministic recovery goldens containing kind, range, missing flag, escaped leading/text/trailing bytes, child path, and diagnostic code/range.

## Coverage

- BOM, CRLF, Unicode, and separate LF coverage.
- Arbitrary, missing, and unknown frontmatter type values.
- Clean, malformed, H1-unclosed, H2-unclosed, and false-positive frontmatter cases.
- Later thematic rules; top-level H1/H2; H3-H6 retained as raw shell content.
- Quote, ordered/unordered list item, fenced/indented code, HTML block, table, footnote, and definition-list containers.
- HTML comments and trailing/pre-newline/EOF horizontal whitespace.
- Every fixture asserts byte-exact roundtrip, leaf concatenation exactly once, bounded ranges, child-width sums, parser progress, and sorted non-overlapping protected ranges.

## Verification

- `rtk cargo test -p waml-syntax --test shell_roundtrip`
- `rtk cargo test -p waml-syntax`
- Workspace check, formatting check, and diff check are recorded by the fix-round handoff after their final fresh run.

## Concerns

- None known within Task 4 scope. Task 5 island semantics remain intentionally untouched.

## Fix round 2

- Corrected structured-line advancement at an unterminated final line: headings, authored close fences, and recovered frontmatter now advance only through their significant structural bytes.
- Remaining horizontal EOF whitespace is owned exactly once by leading trivia on the missing EOF token.
- Missing-close-fence and `FrontmatterNotClean` recovery ranges now end at the consumed structural boundary, before EOF trivia.
- Added deterministic escaped-source fixtures and goldens for `# Title   `, a clean close fence ending in spaces, and recovered frontmatter ending in spaces, all without a final newline.
- Routed the BOM/CRLF/Unicode case through the generalized roundtrip, leaf-concatenation, range, width, progress, and protected-range invariant helper.
