---
name: ship-overnight
description: Use when the user hands off a spec to be planned and fully implemented while they are away ("land it tonight", "I won't be around", AFK autopilot) and expects it landed on main by morning. Takes a spec all the way to landed-on-origin/main unattended.
user_invocable: true
---

# ship-overnight

Take **one spec** all the way to **landed on `origin/main`**, unattended, by morning.
The user is away and cannot answer questions. Pipeline:

**spec → segmented plan (Opus agent) → `implement-plan` → verify it landed.**

## The prime directive (read first)

The user is AFK **by choice**, trusting you to finish. Their standing instruction:

> On any judgment call, do it **the proper way** — the thorough, correct option, as if
> I'd said "we should do it the proper way, of course." Never cut scope, skip a gate, or
> hack around a problem just to finish faster.

"Land it tonight" is a **quality** bar, not a licence to rush. Fewer tasks fully landed
beats all tasks hacked. See the rationalization table below.

## Procedure

1. **Pin the spec.** Identify the one spec from the invocation args / current thread (a
   spec doc, a design file, a brainstorm result). If genuinely no spec is identifiable,
   you cannot proceed — stop and leave a note (see Stop conditions). Do not invent scope.

2. **Brief + dispatch the planning Opus agent.** Write a handoff-grade context packet for
   the planner — invoke the **handoff** skill's discipline (pointers over payloads;
   decisions, rationale, and ruled-out approaches inline; point to files it can read).
   Dispatch **one Opus agent** (`agentType: general-purpose`, or Task with a plan-focused
   agent) told to:
   - Follow the **superpowers:writing-plans** discipline to plan the spec.
   - Write the plan **in segments the way `implement-plan` expects** — see Plan format.
   - Write it under `docs/superpowers/plans/` and return the exact path it wrote.

3. **Gate the plan before implementing.** Read the plan the agent wrote. Verify the
   segment format is correct (Plan format below) — a malformed plan silently collapses to
   one giant unit. Fix format issues yourself or bounce it back to the agent.

4. **Get the plan onto `origin/main`.** `implement-plan`'s default mode forks a worktree
   off `origin/main`, so the plan must exist there first: commit the plan file/dir and
   push to `origin/main` (rebase if needed, as in [[integrate-often-push-origin]]).

5. **Run `implement-plan`.** Call the **Workflow** tool:
   `Workflow({name:"implement-plan", args:{plan:"docs/superpowers/plans/<file>.md"}})`.
   - `args` is a **real JSON object**, not a string.
   - Use **default mode** — it ff-pushes each green unit to `origin/main`, deep-reviews,
     and archives the plan. Do **NOT** pass `dry-run` or `local`: those never land.
   - It runs in the background; a task-notification fires on completion.

6. **Verify it actually landed** (do not trust the workflow's own summary):
   `git fetch origin` then confirm `origin/main` carries the unit commits (Plan-Tasks
   trailers) and the plan moved to `docs/superpowers/plans/completed/`. Report what landed
   and what didn't, with the decisive evidence.

## Plan format (what "in segments" means)

`implement-plan` accepts either shape — the planner must produce one:

- **Single file** `docs/superpowers/plans/<name>.md`: one segment per **`### ` (H3)**
  task/step heading. **`## ` (H2) headings silently collapse the whole plan to one unit**
  ([[implement-plan-needs-h3-task-headings]]) — the most common failure. Verify H3.
- **Plan directory** `docs/superpowers/plans/<name>/`: a `README.md` task index plus
  `task-N-*.md` files, one task each.

Each segment must be a small, independently committable, gate-passing unit.

## Stop conditions (leave a handoff, do not guess)

Autonomy means proper-way judgment calls — NOT plowing through a wall. Stop and emit a
**handoff** block (do not hack around) when:
- No spec is identifiable, or the spec is too ambiguous to plan without inventing scope.
- The gate cannot be made to pass honestly (a unit can't go green without disabling tests
  / lint / a check).
- A decision is destructive, irreversible, or outward-facing (deletes data, force-pushes
  history, publishes externally) and wasn't clearly authorized by the spec.

For these, land whatever units are safely green, then leave a crisp handoff of exactly
where you stopped and why — so the returning user resumes in one read.

## Rationalization table

| Late-night excuse | Reality |
|---|---|
| "Running out of time — ship the partial and skip the gate" | Land fewer units fully green. A skipped gate lands a bug on main unattended. |
| "This test is flaky, I'll disable it to get green" | That's hacking around, not the proper way. Fix or stop the unit. |
| "The spec is vague here; I'll pick something to keep moving" | Inventing scope while AFK = the user wakes to the wrong thing. Stop, leave a note. |
| "`dry-run`/`local` is safer, I'll just do that" | Those don't land on main. The whole job is *landed by morning*. Default mode. |
| "The workflow said done, so it's done" | Verify against `origin/main` yourself. Trust the git state, not the summary. |

## Red flags — STOP

- About to disable/skip a test, lint, or gate to get a unit green.
- About to guess at ambiguous spec intent rather than leave a note.
- About to run `implement-plan` in `dry-run`/`local` when the goal is to land tonight.
- Declaring success without a `git fetch` + check of `origin/main`.
