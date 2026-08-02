# Format Canonically

**Goal:** Serialization is deterministic, so a diff shows only what changed.

**Why:** Two authors editing the same bundle with the same tool must not
produce different bytes for the same model. Nondeterminism turns every review
into noise.

**Done when:** Serializing the same model twice, in either surface, on either
platform, produces identical bytes; and a semantic edit produces a diff whose
every hunk is explainable by that edit.

**Status:** done — unverified
**MVP:** yes

## Notes

- Canonical serialization is an established workflow concept with formatter
  tests behind it.
- Ordering is the classic failure mode: any map iterated without a stable order
  breaks this silently and only sometimes.
- Platform line endings are a real cross-platform risk here, since the editor
  runs on Windows and the Pages build does not.
