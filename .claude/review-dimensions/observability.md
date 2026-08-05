# Observability

Evaluate whether a running build gives enough information to understand what is
happening and to diagnose a problem without reading the source.

waml is a desktop and web editor, not a server. "Operator" here means the person
using the editor and the person debugging it — including you, one session later.

## Checklist

### Diagnostics to the User
- When a document fails to parse, validate, or solve, does the user see a diagnostic that names the cause and the place?
- Does the diagnostic carry a usable span, or does it point at the whole file?
- Is the message written for the person who wrote the document, not for the person who wrote the parser?

### Error Visibility
- When something goes wrong, does anything record it, or does the error disappear into a discarded `Result`?
- Is a swallowed error deliberate and marked, or accidental?
- Does a failure degrade the view silently — a blank panel, a missing label, an edge that does not draw — with nothing said anywhere?

### Log Quality
- Are log messages structured enough to search, and do they carry which document, which node, which operation?
- Is logging present on the paths that actually fail: load, parse, export, web boot, shader and font setup?
- Is there excessive per-frame logging that will bury a real problem?

### Web Parity
- Does the wasm build report the same failures as native, or does the browser build fail quietly where native prints?
- Is a panic in the web build visible in the console, and does it name its cause?

### Verifiability
- Can a claim about this change be checked by something other than reading the code? A test, a measurement, a screenshot, a console line.
- If the change is a fix, does the codebase now show evidence when the bug returns?

## Scope Guidance

- **Full evaluation**: Review diagnostic coverage across parse, validate, solve, and export, plus the native and web failure paths.
- **Change review**: Focus on whether new failure modes are reported, whether errors are dropped, and whether the failure is visible on web as well as native.
