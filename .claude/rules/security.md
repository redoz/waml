# Security

Evaluate whether the system resists adversarial input and protects its boundaries.

waml parses documents it did not write. A `.waml` file, a shared bundle, or a
workspace opened from a link can come from anyone.

## Checklist

### Untrusted Input
- Is `.waml` source validated before it reaches the model? (unbounded nesting, absurd counts, cyclic references, huge literals)
- Can a malformed or hostile document panic the parser or the solver instead of producing a diagnostic? A panic in the LSP or the wasm build takes the session down.
- Are there denial-of-service vectors? (a document that makes layout or edge routing run unbounded, a fuzz-reachable quadratic path)
- Is bundle and frontmatter input treated as untrusted, including fields the current version does not read?

### Filesystem & Paths
- Can a path from a document, a bundle, or a share link escape the intended directory? (`..`, absolute paths, symlinks, Windows device names)
- Are files written only where the user asked, and never overwritten without the caller knowing what is there?

### Web & Export
- Is exported or embedded HTML escaped? A node name, label, or comment reaches the exported site verbatim if nothing escapes it.
- Does the web build fetch or execute anything it did not ship with?
- Are user-controlled strings kept out of positions where they become markup or script.

### Secrets & Dependencies
- Are secrets handled properly? No hardcoded keys, no credentials in source, no tokens written to logs or to a bundle.
- Are dependencies pinned? A git dependency must be pinned to a commit, never to a branch tip.
- Does the change add a dependency, and is that dependency worth its supply-chain surface?

## Scope Guidance

- **Full evaluation**: Review every parse entry point, every path that touches the filesystem, the export and share paths, and the dependency set.
- **Change review**: Focus on whether the change accepts new external input, touches paths or export output, or adds a dependency.
