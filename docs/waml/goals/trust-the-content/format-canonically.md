# Format Canonically

**Goal:** The tool writes the same bytes for the same model each time. Thus a
diff shows the change only.

**Why:** Two authors who edit the same bundle with the same tool must get the
same bytes for the same model. Different bytes make each review difficult.

**Done when:** To write the same model two times gives the same bytes. This is
true at each surface and on each platform. A semantic edit gives a diff in
which each hunk is a result of that edit.

**Status:** done — unverified
**MVP:** yes

## Notes

- Canonical serialization is an established behavior with formatter tests.
- The usual defect is order. A map with no stable order breaks this behavior
  silently and only in some conditions.
- Line ends are a risk. The editor operates on Windows. The publication build
  does not.
