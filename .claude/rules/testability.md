# Testability

Evaluate whether the code is structured for effective testing and is actually tested.

## Checklist

### Structure
- Is effectful code separated from pure logic? Can model, parse, and solve logic be tested without a window, a GPU, or a filesystem?
- Does new logic live in a headless crate (`waml`, `waml-syntax`) where it can be tested, or is it trapped inside a widget?
- Are external dependencies (filesystem, clock, network, host environment) injectable rather than called directly?
- Are there implicit dependencies (globals, ambient config, process state) that make a test order-dependent?

### Coverage
- Is there test coverage for the critical paths? What is missing?
- Are existing tests meaningful — do they assert behaviour, or restate the implementation?
- Are tests reliable? No timing dependence, no order dependence, no assertion on an incidental format.
- For parser and solver changes: is there a property test or a fixture that would have caught this class of bug?

### Fixtures & Snapshots
- Do new fixtures live with the others, and are shared fixtures left unmodified by the test run?
- Does a snapshot or golden file assert something a human can review, or is it an opaque blob that will be blindly re-blessed?

### GUI Limits
- Widget behaviour that the gate cannot assert must be verified visually, and the verification must be stated. A green gate is not evidence for a drawing change.
- Could the logic under a widget be lifted out and unit-tested, leaving only the drawing untested?
- Could a new contributor write a test for a bug fix without understanding the whole editor?

## Scope Guidance

- **Full evaluation**: Review coverage across the workspace crates and the vscode extension, plus fixture and harness quality.
- **Change review**: Focus on whether new logic is testable in isolation, whether effectful code is separated from pure logic, and whether this change should have brought a test.
