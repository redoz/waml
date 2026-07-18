export const meta = {
  name: 'implement-plan',
  description:
    'Implement ONE approved implementation plan (a single top-level docs/superpowers/plans/*.md, handed by exact path) in its own isolated git worktree, using a single BATCHED, self-rotating implementer that pushes each green unit straight to origin/main. Each generation reads progress MECHANICALLY from git "Plan-Tasks" commit trailers, burns small committable green UNITS (implement -> full gate: cargo test --workspace && pnpm -r test && pnpm lint && pnpm build -> commit with a Plan-Tasks trailer -> rebase onto origin/main, re-gate, and fast-forward-push HEAD:main), and self-rotates near its quality zone (~100k context tokens); a fresh generation then resumes from the trailers, until every plan task has landed on main. Then ONE deep end-review of the cumulative diff fix-forwards the critical/high/medium defects THIS change introduced (pushing each fix), and the plan file is archived to completed/ on main. The LOCAL main checkout is left untouched — pull after.',
  whenToUse:
    'Run when ONE written implementation plan is ready to implement. Pass {plan:"docs/superpowers/plans/<file>.md"} — the exact top-level plan path (NOT drafts/, NOT completed/, NOT a deeper subdirectory) — OR a plan DIRECTORY {plan:"docs/superpowers/plans/<dir>/"} that holds a README.md (task index) plus task-N-*.md files (one task each). Pass {mode:"dry-run"} to implement + gate + commit per unit WITHOUT pushing, fix-forwarding, or archiving (an informational review only). Pass {local:true} (or {mode:"local"}) for a fully-LOCAL run: implement against the LOCAL main as the trunk on the plan branch with NO fetch/rebase/push and NO origin-sync — review + archive commit onto the branch, main is left untouched, and you land the branch yourself afterward. Optionally pass {planReviewConcerns:["..."]} — a deduped checklist of prior plan-review concerns, used purely as the deep end-review\'s focusing checklist.',
  phases: [
    { title: 'Preflight', detail: 'verify clean main in sync with origin and that the toolchain is present; validate the plan path' },
    {
      title: 'Implement',
      detail:
        'one worktree off origin/main; batched self-rotating generations — each burns small green units (implement -> full gate -> commit Plan-Tasks -> rebase onto origin/main, re-gate, ff-push HEAD:main) and self-rotates near the quality zone; fresh generations resume from git trailers',
    },
    {
      title: 'Review',
      detail:
        'one deep end-review of the cumulative diff vs the base commit; fix-forward the critical/high/medium defects this change introduced (pushing each) via the same per-unit mechanic; then archive the plan file on main',
    },
  ],
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
// Tool-level args may arrive as a JSON string; a nested workflow() call delivers an object.
let A = args || {}
if (typeof A === 'string') { try { A = JSON.parse(A) } catch { A = {} } }

// ===========================================================================
// PROJECT — the ONLY project-specific block. Everything below the line is a
// generic engine that reads from here. Re-targeting a copy of this workflow to
// another repo is a one-block edit: change PROJECT, touch nothing else. (Prior
// copies drifted because identity was smeared across every prompt string — see
// the "wrong-repo-template" failure mode. Keep it all here.)
// ===========================================================================
const PROJECT = {
  name: 'waml', // used verbatim in prompts wherever the project is named
  repoDir: 'C:/dev/waml', // main repo root (for worktree cleanup; never checked out off the base branch)
  remote: 'origin', // the remote whose baseBranch each green unit is fast-forward-pushed to
  baseBranch: 'main', // the trunk: worktrees fork off <remote>/<baseBranch> and every unit ff-pushes back onto it
  // Capability flags drive the SCOPE-AWARE gate: a plan is gated by only the toolchains it actually touches.
  rust: true, // repo is a cargo workspace (crates/*)
  pnpm: true, // repo is a pnpm monorepo (packages/*)
  gate: {
    // Rust half: fmt + clippy + tests. fmt/clippy were historically missing and slipped violations onto main.
    rust: 'cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace',
    // pnpm half: every package's vitest suite, eslint, then the build (which type-checks via tsc). Only run for
    // plans that touch packages/** — a pure-Rust plan must NOT build the (OWOX-origin, off-limits) @waml/* packages.
    pnpm: 'pnpm -r test && pnpm lint && pnpm build',
    // Clean-room recompile of the pushed SHA in a fresh throwaway worktree (a fresh tree can't be dirty, so it
    // can't false-green). Cheap check-only compile of the whole workspace.
    verify: 'cargo check --workspace --all-targets',
  },
  // Commit-message footer appended to every unit/fix/archive commit. '' disables. (waml history carries no
  // Co-Authored-By banner; keep '' unless the repo convention changes.) Conventional Commits either way.
  commitFooter: '',
}

const PLANS_DIR = 'docs/superpowers/plans'
const COMPLETED_DIR = 'docs/superpowers/plans/completed'
const WORKTREES_DIR = '.claude/worktrees'
const REPO_DIR = PROJECT.repoDir // main repo root (for worktree cleanup; never checked out off the base branch)
const REMOTE = PROJECT.remote // the remote whose main each green unit is fast-forward-pushed to
const BASE_BRANCH = PROJECT.baseBranch // the trunk: worktrees fork off REMOTE/main and every unit ff-pushes back onto it
const MODE = A.mode || 'merge' // 'merge' (per-unit ff-push to origin/main; default) | 'dry-run' (per-unit gate + commit, no push, no fix-forward, no archive) | 'local' (see LOCAL)
const PLAN = A.plan // the ONE plan to implement: a repo-relative top-level plan path OR a plan DIRECTORY (README.md + task-N-*.md).

// Fully-LOCAL run mode (A.local===true, or A.mode==='local'): implement against the LOCAL baseBranch as the trunk with
// ZERO origin interaction — NO git fetch, NO rebase-onto-remote, NO push, NO origin-sync preflight requirement. The plan
// BRANCH is itself the trunk: each green unit is COMMITTED onto it (carrying the same Plan/Plan-Tasks trailers, so
// cross-generation resume-from-trailers still works), and the deep review + fix-forward + archive all run as LOCAL
// commits on that branch. CRITICAL (Windows): local `main` is CHECKED OUT in another worktree (C:/dev/waml), so we must
// NEVER move / `git branch -f` / push the base ref during a local run — it would fail or corrupt that checkout. ALL work
// stays on the plan branch; `main` is left untouched and the user lands it themselves. Distinct from dry-run: local DOES
// review + archive (on the branch); dry-run does neither. When !LOCAL, every code path below behaves exactly as before.
const LOCAL = A.local === true || A.mode === 'local'

// Adaptive rigor: the plan file's "> **Rigor:**" header tunes in-prompt TDD strictness (one-shot | tdd-per-task).
// A.rigor is an ad-hoc override.
const RIGOR_OVERRIDE = A.rigor === 'one-shot' || A.rigor === 'tdd-per-task' ? A.rigor : null

// Optional plan-review focusing checklist for the deep end-review (plain strings, treated as an opaque checklist).
const PLAN_REVIEW_CONCERNS = Array.isArray(A.planReviewConcerns)
  ? A.planReviewConcerns.filter((s) => typeof s === 'string' && s.trim()).map((s) => s.trim())
  : []

// Per-agent model tiers (cost calibration). The session model (opus, inherited when no `model` is set) is RESERVED
// for the agents that exercise real engineering JUDGEMENT — the deep end-review and the fix-forward. The mechanical
// agents drop a tier to MID:
//   MID (sonnet) — preflight, setup, probe, archive: rule-following shell (branch/tree checks; worktree provisioning +
//                  plan parse; git-trailer reading; git mv + cleanup). Consequential, but not judgement-heavy.
// NOTE: the progress probe used to run on haiku, but haiku 502s DETERMINISTICALLY at the local inference gateway
// ("Unable to safely convert buffered response to SSE") — a null probe then read as landedCommits=0 and false-parked a
// run that had actually advanced. The probe is on MID now; do not drop it back to haiku until the gateway is fixed.
const MID = 'sonnet'

// The IMPLEMENTER generation tier. Sonnet by default: for the DETAILED plans this repo writes (exact code blocks + TDD
// steps + exact commands) implementation is near-mechanical transcribe -> gate -> commit, so opus is wasted there — and
// the escalate/bounded-retry guards make a weaker model PARK rather than corrupt on divergence. Bump to opus for a
// thin/exploratory plan that needs real mid-implementation judgement via {implementerModel:'opus'}. The deep end-review
// + fix-forward are deliberately NOT pinned — they inherit the session model (opus), where the judgement actually pays.
const IMPLEMENTER_MODEL = A.implementerModel || MID

// Absolute path to the self-rotation helper (committed locally). The plan worktree is branched off the base branch,
// which HAS this script once committed — but agents run it via Bash by ABSOLUTE path to be robust regardless.
const SCRIPTS_DIR = REPO_DIR + '/.claude/scripts'
const CONTEXT_FILL = SCRIPTS_DIR + '/context-fill.py' // reads the agent's own transcript -> rotation verdict (exit 10)

// Rotation cap: an ABSOLUTE context-token quality-zone cap (~10% of a 1M window), not a window fraction — model
// sharpness degrades long before the window fills, and the still-sharp zone is ~absolute, not a percentage.
const ROTATE_MAX_TOKENS = 100000

// Backstops on the generation loop. The no-progress guard (a generation that lands no NEW Plan-Tasks trailer parks)
// already bounds it to <= one generation per unit; the cap is a defensive net against a pathological loop.
const MAX_GENERATIONS = 25
const MAX_REVIEW_FIX_ROUNDS = 3 // deep-review -> fix-forward -> re-review iterations (crit/high hard-gate; medium soft-gate: warn + proceed once it's all that's left after the cap)
const MAX_ATTEMPTS = 3 // bounded local green-gate fix-and-retry attempts inside a generation

// SCOPE-AWARE green-gate. This repo is a cargo workspace AND a pnpm monorepo, but a plan is gated by only the
// toolchains it actually TOUCHES: a pure-Rust plan must not build the (OWOX-origin, off-limits) @waml/* packages, and a
// docs-only plan need not do either. gateFor(files, override) returns the '&&'-joined command for a plan given the
// repo-relative paths it changes (from the setup step) and an optional plan-header override.
//   - touches crates/** or *.rs or Cargo.* -> the Rust half (fmt + clippy + test --workspace)
//   - touches packages/**                  -> the pnpm half (vitest + eslint + build)
//   - plan header "> **Gate:** rust-only | full | auto" forces the shape (auto = derive from files; the default)
// Rust is the fallback when a plan's touched files are unknown/empty, so a mis-detected plan over-gates (safe) rather
// than under-gates. fmt + clippy are ALWAYS in the Rust half — they were historically absent and slipped onto main.
function gateFor(files, override) {
  const fs = Array.isArray(files) ? files : []
  const touchesRust = PROJECT.rust && fs.some((f) => /\.rs$|(^|\/)crates\/|(^|\/)Cargo\.(toml|lock)$/i.test(f))
  const touchesPnpm = PROJECT.pnpm && fs.some((f) => /(^|\/)packages\//i.test(f))
  let wantRust = touchesRust
  let wantPnpm = touchesPnpm
  if (override === 'rust-only') { wantRust = PROJECT.rust; wantPnpm = false }
  else if (override === 'full') { wantRust = PROJECT.rust; wantPnpm = PROJECT.pnpm }
  if (!wantRust && !wantPnpm) wantRust = PROJECT.rust // fallback: never run an empty gate
  const parts = []
  if (wantRust) parts.push(PROJECT.gate.rust)
  if (wantPnpm) parts.push(PROJECT.gate.pnpm)
  return parts.join(' && ')
}

// Commit-message footer appended to every unit/fix/archive commit. '' disables. Conventional Commits for the subject
// line either way. (Sourced from the PROJECT block so identity stays in one place.)
const COMMIT_FOOTER = PROJECT.commitFooter || ''

// Normalize any plan reference (slug, filename, DIRECTORY, or repo-relative path) to a bare slug: strip a trailing
// slash (a directory ref may carry one), the directory prefix, the .md extension (files only — a directory has none),
// and a leading YYYY-MM-DD- date prefix. Works for BOTH a single .md file and a bare plan directory:
//   docs/superpowers/plans/2026-07-15-foo.md                        -> foo
//   docs/superpowers/plans/2026-07-17-ontology-substrate-seam-slice1/ -> ontology-substrate-seam-slice1
function toSlug(s) {
  return String(s)
    .replace(/\\/g, '/')
    .replace(/\/+$/, '') // drop a directory ref's trailing slash BEFORE popping (else pop() would return '')
    .split('/')
    .pop()
    .replace(/\.md$/i, '') // no-op for a directory (it has no .md); strips the extension for a file
    .replace(/^\d{4}-\d{2}-\d{2}-/, '')
}

// True iff the (normalized, no trailing slash) ref is a plan DIRECTORY rather than a single .md file: a bare name
// directly under plans/ with no .md extension. Shape only — existence/contents are confirmed by the Setup agent.
function isPlanDirRef(norm) {
  return /^docs\/superpowers\/plans\/[^/]+$/.test(norm) && !/\.md$/i.test(norm)
}

// Structural validation (fail-closed): the input must be EITHER a markdown file directly under docs/superpowers/plans/
// OR a bare plan DIRECTORY directly under it (README.md + task-N-*.md live inside) — not drafts/, not completed/, not a
// deeper-nested path. A directory ref may carry a trailing slash. String shape is checked here; the file's/dir's actual
// existence is confirmed by the Setup agent (setupOk=false).
function planPathError(p) {
  if (!p || typeof p !== 'string')
    return 'no plan path provided (pass {plan:"' + PLANS_DIR + '/<file>.md"} or a plan DIRECTORY {plan:"' + PLANS_DIR + '/<dir>/"})'
  const norm = p.replace(/\\/g, '/').replace(/\/+$/, '') // tolerate a trailing slash on a directory ref
  const isFile = /^docs\/superpowers\/plans\/[^/]+\.md$/.test(norm)
  // A directory ref must be a bare name under plans/ (no deeper nesting) and must NOT be a reserved dir — drafts/ and
  // completed/ are directories directly under plans/ too, so they'd otherwise match the directory shape.
  const dir = /^docs\/superpowers\/plans\/([^/]+)$/.exec(norm)
  const isDir = !!dir && !/\.md$/i.test(dir[1]) && dir[1] !== 'drafts' && dir[1] !== 'completed'
  if (!isFile && !isDir)
    return 'plan must be a markdown file OR a directory directly under ' + PLANS_DIR + '/ (NOT drafts/, NOT completed/, NOT a deeper subdirectory): ' + p
  return null
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------
const PREFLIGHT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: { ok: { type: 'boolean' }, reason: { type: 'string' } },
  required: ['ok', 'reason'],
}

// Setup for one plan: create the worktree, capture baseSha, and read the plan's task-ids (NOT the full task bodies —
// the batched implementer reads the plan file itself), touched files + summary (for the deep review), and rigor tag.
const SETUP_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    setupOk: { type: 'boolean' },
    worktreePath: { type: 'string' },
    branch: { type: 'string' },
    baseSha: { type: 'string' },
    // The plan's ordered task-ids exactly as written, e.g. ["Task 1", "Task 2"]. Empty for a thin/one-shot plan
    // (no "### Task" sections) — the implementer then treats the whole plan as a single unit.
    taskIds: { type: 'array', items: { type: 'string' } },
    files: { type: 'array', items: { type: 'string' } },
    summary: { type: 'string' },
    rigor: { type: 'string', enum: ['one-shot', 'tdd-per-task'] },
    gate: { type: 'string', enum: ['auto', 'rust-only', 'full'] },
    notes: { type: 'string' },
  },
  required: ['setupOk', 'worktreePath', 'branch', 'taskIds', 'files', 'summary'],
}

// One batched implementer generation's report. allDone is a HINT (the orchestrator verifies doneness mechanically
// from git trailers via the probe). escalate stops the run (plan wrong/infeasible OR the gate cannot go green).
const GEN_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    done: { type: 'boolean' }, // returned gracefully (rotated, finished, or escalated) rather than dying mid-run
    allDone: { type: 'boolean' }, // believes every plan task-id is now landed — a HINT, verified by the probe
    rotated: { type: 'boolean' }, // stopped because context-fill said rotate (exit 10), not because allDone
    escalate: { type: 'boolean' }, // plan wrong/infeasible OR the gate cannot go green after the bounded retries — STOP
    unitsLanded: { type: 'integer' }, // units committed this generation, for logging
    handoffNote: { type: 'string' }, // SOFT context for the next generation only — never progress
    problems: { type: 'string' }, // escalation reason, else empty
  },
  required: ['done', 'allDone', 'escalate', 'handoffNote'],
}

// Mechanical progress probe: numbers read straight from git trailers, no judgement.
const PROBE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    // THIS plan's task-ids found across landed "Plan-Tasks" trailers, intersected with the plan task-ids.
    coveredIds: { type: 'array', items: { type: 'string' } },
    // Count of commits carrying a "Plan: <slug>" trailer since baseSha (the progress signal for the no-progress guard).
    landedCommits: { type: 'integer' },
    // Short SHAs of those commits (for logging).
    landedShas: { type: 'array', items: { type: 'string' } },
    headSha: { type: 'string' },
    notes: { type: 'string' },
  },
  required: ['coveredIds', 'landedCommits', 'landedShas'],
}

// Deep end-review findings, partitioned for the fix-forward gate. critical/high/medium hold ONLY findings THIS change
// INTRODUCED — they drive the fix-forward pass. low = introduced nits (informational). deferred = NOT fixed here:
// pre-existing issues in surrounding code, or explicitly out-of-scope future work (informational).
const QUALITY_REVIEW_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    critical: { type: 'array', items: { type: 'string' } },
    high: { type: 'array', items: { type: 'string' } },
    medium: { type: 'array', items: { type: 'string' } },
    low: { type: 'array', items: { type: 'string' } },
    deferred: { type: 'array', items: { type: 'string' } },
    assessment: { type: 'string' },
  },
  required: ['critical', 'high', 'medium', 'low', 'assessment'],
}

// Deep-review fix-forward report (introduced defects fixed + committed green on the branch).
const FIX_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    fixed: { type: 'boolean' }, // all listed findings resolved AND committed green on the branch
    commits: { type: 'array', items: { type: 'string' } },
    problems: { type: 'string' },
  },
  required: ['fixed', 'problems'],
}

// Clean-room verify report: does the pushed trunk tip compile from a FRESH worktree (can't be dirty -> can't false-green)?
const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    green: { type: 'boolean' }, // the pushed trunk tip compiles clean from a throwaway worktree
    headSha: { type: 'string' },
    problems: { type: 'string' }, // first compile error if not green, else empty
  },
  required: ['green', 'problems'],
}

// Plan-file archive report.
const ARCHIVE_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    archived: { type: 'boolean' },
    headShaAfter: { type: 'string' },
    problems: { type: 'string' },
  },
  required: ['archived', 'problems'],
}

// ---------------------------------------------------------------------------
// Prompt builders (no backticks inside; use plain quotes for code/commands)
// ---------------------------------------------------------------------------
function preflightPrompt() {
  // Step 3 checks origin-sync in a normal (merge/dry-run) run; a LOCAL run has NO remote to sync against, so it instead
  // just confirms the local base exists. Everything else (clean tree, toolchain, base-green) is identical either way.
  const step3 = LOCAL
    ? [
        '3. LOCAL run — there is NO remote to sync against: DO NOT run "git fetch" and do NOT compare to any ' + REMOTE + '/*',
        '   ref. Just confirm the local base branch exists:  git rev-parse --verify ' + BASE_BRANCH + '  (exit 0). If it does',
        '   not exist, set ok=false, reason "local ' + BASE_BRANCH + ' branch missing". Leave ' + BASE_BRANCH + ' UNTOUCHED —',
        '   it is checked out in another worktree and must not be moved.',
      ]
    : [
        '3. Local ' + BASE_BRANCH + ' has not DIVERGED from ' + REMOTE + '. Run "git fetch ' + REMOTE + '", then:',
        '   - if HEAD == ' + REMOTE + '/' + BASE_BRANCH + ': fine;',
        '   - if ' + REMOTE + '/' + BASE_BRANCH + ' is an ancestor of HEAD (unpushed local commits): fine, leave them UNTOUCHED —',
        '     plans branch off and push onto ' + REMOTE + '/' + BASE_BRANCH + ', so your local-only commits are never moved;',
        '   - if HEAD is an ancestor of ' + REMOTE + '/' + BASE_BRANCH + ' (local behind): run "git merge --ff-only ' + REMOTE + '/' + BASE_BRANCH + '" to catch up;',
        '   - otherwise the branch has diverged: set ok=false, reason "' + BASE_BRANCH + ' diverged from ' + REMOTE + '".',
        '   (Test ancestry with: git merge-base --is-ancestor <maybe-ancestor> <descendant> ; exit 0 means yes.)',
      ]
  // Step 6 (base-green) runs a fresh REMOTE/base checkout in a normal run; a LOCAL run checks the already-checked-out
  // local base in place (no remote to diff against).
  const step6 = LOCAL
    ? [
        '6. BASE IS GREEN — do not branch off a red trunk. On the local ' + BASE_BRANCH + ' checkout run a fast compile-only',
        '   check of the whole workspace:  cargo check --workspace --all-targets',
        '   If it FAILS, the trunk is already broken and every unit would fail to integrate — set ok=false, reason',
        '   "' + BASE_BRANCH + ' base is RED: <first error line>". Do NOT try to fix the base here.',
      ]
    : [
        '6. BASE IS GREEN — do not branch off a red trunk. From a clean ' + REMOTE + '/' + BASE_BRANCH + ' checkout run a fast',
        '   compile-only check of the whole workspace:  cargo check --workspace --all-targets',
        '   If it FAILS, the trunk is already broken (often a concurrent session\'s in-flight commit) and every unit would',
        '   fail to integrate against it — set ok=false, reason "' + BASE_BRANCH + ' base is RED: <first error line>". Do NOT',
        '   try to fix the base here. (A compile-only check is used deliberately — running the full test suite in preflight',
        '   would be slow; a non-compiling base is the failure that reliably blocks every integrate.)',
      ]
  const tail = LOCAL
    ? 'Do NOT change any tracked files. Return ok=true only if the branch is ' + BASE_BRANCH + ', the tracked tree is clean,\nthe local ' + BASE_BRANCH + ' exists, the toolchain is present, AND the base compiles; otherwise ok=false with a precise reason.'
    : 'Other than an allowed fast-forward of ' + BASE_BRANCH + ' in step 3, do NOT change any tracked files. Return ok=true\nonly if the branch is ' + BASE_BRANCH + ', the tracked tree is clean, ' + BASE_BRANCH + ' has not diverged, the\ntoolchain is present, AND the base compiles; otherwise ok=false with a precise reason.'
  return [
    'You are the preflight check for an autonomous plan-implementation run on the ' + PROJECT.name + ' repo.',
    'Host: Windows, PowerShell (a Bash tool is also available). This is a Rust cargo workspace (crates/*) AND a pnpm monorepo (packages/*).',
    'Working directory is the repo root (' + REPO_DIR + '). Verify ALL of the following and report:',
    '1. Current branch is "' + BASE_BRANCH + '"  (git rev-parse --abbrev-ref HEAD).',
    '2. Working tree is clean of TRACKED changes  (git status --porcelain shows no staged/modified tracked files;',
    '   untracked files are fine — e.g. new docs).',
    ...step3,
    '4. The toolchain is present: "pnpm --version" succeeds, "node --version" reports >= 20, and "cargo --version" succeeds',
    '   (the green-gate runs "cargo test --workspace", so a missing Rust toolchain must fail preflight).',
    '5. Dependencies are installed: node_modules exists at the repo root (if "pnpm -r exec true" fails with a missing-',
    '   module/ELIFECYCLE error, run "pnpm install" once and re-check). Do NOT upgrade or change any dependency.',
    ...step6,
    tail,
  ].join('\n')
}

function setupPrompt(plan) {
  const wt = WORKTREES_DIR + '/plan-' + plan.slug
  // Fork the worktree off the trunk. Normal (merge/dry-run) run: fetch, branch off REMOTE/base. LOCAL run: no remote, so
  // branch off the LOCAL base — and if the plan branch already exists (RESUMING a prior local run), REUSE it rather than
  // deleting it, so its committed units + Plan-Tasks trailers survive for resume-from-trailers (in merge mode a prior
  // run's work lives on the remote, so a stale local branch is safely discarded; in local mode it is the ONLY copy).
  const forkBlock = LOCAL
    ? [
        '  (LOCAL run — no remote: do NOT run "git fetch".)',
        '  If branch plan/' + plan.slug + ' does NOT yet exist, create it off the LOCAL base:',
        '    git worktree add ' + wt + ' -b plan/' + plan.slug + ' ' + BASE_BRANCH,
        '  If branch plan/' + plan.slug + ' ALREADY exists (you are RESUMING a prior local run — its commits carry the',
        '  Plan-Tasks trailers this run reads back), do NOT delete it. Attach a worktree to the EXISTING branch (no -b):',
        '    git worktree add ' + wt + ' plan/' + plan.slug,
        '  If a stale/broken worktree dir is in the way, "git worktree remove --force ' + wt + '" (then "git worktree prune")',
        '  and re-add — but NEVER "git branch -D plan/' + plan.slug + '" in a local run (that would delete the only copy of the work).',
      ]
    : [
        '  Run:  git fetch ' + REMOTE,
        '  Run:  git worktree add ' + wt + ' -b plan/' + plan.slug + ' ' + REMOTE + '/' + BASE_BRANCH,
        '  - If branch plan/' + plan.slug + ' already exists from a stale run, remove the stale worktree',
        '    ("git worktree remove --force ..." then "git branch -D plan/' + plan.slug + '") and retry.',
      ]
  // Validation + task-reading differ for a single-file (.md) vs a directory (README.md + task-N-*.md) plan.
  const validateBlock = plan.isDir
    ? [
        '  FIRST confirm the plan DIRECTORY exists DIRECTLY under ' + PLANS_DIR + '/ and holds a README.md plus one or more',
        '  task-N-*.md files (top level ONLY — NOT ' + PLANS_DIR + '/drafts/, NOT ' + COMPLETED_DIR + '/). If it is missing or',
        '  has no README.md, set setupOk=false with a notes reason and STOP (return empty taskIds[], files[], summary).',
      ]
    : [
        '  FIRST confirm the plan file exists DIRECTLY under ' + PLANS_DIR + '/ (top level ONLY — NOT ' + PLANS_DIR + '/drafts/,',
        '  NOT ' + COMPLETED_DIR + '/, NOT any subdirectory). If it is missing or not a top-level plan, set setupOk=false with a',
        '  notes reason and STOP (return empty taskIds[], empty files[], and an empty summary).',
      ]
  return [
    'You are the SETUP step for ONE approved implementation plan in the ' + PROJECT.name + ' repo (batched implementer).',
    'Project: ' + PROJECT.name + ' — a Rust cargo workspace (crates/*) under a pnpm monorepo (packages/*).',
    'Host: Windows, PowerShell (Bash tool also available). You run in the MAIN repository directory (' + REPO_DIR + ').',
    '',
    (plan.isDir ? 'Plan DIRECTORY (README.md + task-N-*.md): ' : 'Plan file: ') + plan.path + '   (slug: ' + plan.slug + ')',
    '',
    'STEP A — validate the path, then create an isolated worktree branched off the trunk (' +
      (LOCAL ? 'local ' + BASE_BRANCH : REMOTE + '/' + BASE_BRANCH) + '):',
    ...validateBlock,
    ...forkBlock,
    '  Then capture the absolute worktree path:  git -C ' + wt + ' rev-parse --show-toplevel',
    '  And capture baseSha — the trunk commit this branch starts from (for the deep end-review):',
    '    git -C ' + wt + ' rev-parse HEAD',
    '  Do NOT switch the MAIN worktree off ' + BASE_BRANCH + ', and do NOT modify the local ' + BASE_BRANCH + ' branch.',
    '',
    'STEP A.5 — make the worktree buildable. This is a pnpm monorepo; a fresh worktree shares the repo history but has',
    '  its OWN node_modules. From INSIDE the worktree (cd ' + wt + ') run "pnpm install"',
    '  once so the packages resolve. (pnpm links from the shared store, so this is fast.) Do not change any lockfile.',
    '',
    ...(plan.isDir
      ? [
          'STEP B — extract the plan\'s ordered TASK-IDS DETERMINISTICALLY. This is a DIRECTORY plan: the tasks are ONE PER',
          '  FILE — each "task-N-*.md" is "Task N". List the task files and transcribe their numbers (do NOT eyeball):',
          '      ls "' + plan.absPath + '"      (PowerShell: Get-ChildItem "' + plan.absPath + '" -Name)',
          '  For EVERY file named task-<N>-*.md return the id "Task <N>" (e.g. task-1-uml-module-and-seam.md -> "Task 1"),',
          '  ordered by N ASCENDING — e.g. ["Task 1", "Task 2", "Task 3", "Task 4", "Task 5"]. Cross-check the count and order',
          '  against the README\'s "## Task Index" section (read "' + plan.absPath + '/README.md"); if they disagree, PREFER the',
          '  task-N-*.md filenames (they are the authoritative per-task bodies). If there are NO task-N-*.md files, return an',
          '  empty taskIds[] and set setupOk=true (the implementer treats README.md as a single unit).',
        ]
      : [
          'STEP B — extract the plan\'s ordered TASK-IDS DETERMINISTICALLY (do NOT eyeball a long file — a large plan has been',
          '  mis-counted as "2 tasks" that way, landing 0 commits while still reporting done). Run this EXACT command against the',
          '  plan file and transcribe its matches — do not summarize or infer:',
          '      grep -nE "^### Task " "' + plan.absPath + '"      (PowerShell: Select-String -Path "' + plan.absPath + '" -Pattern "^### Task ")',
          '  Each matching line looks like "### Task 3: Reshape the edge". Return taskIds as the bare id of each match in file',
          '  order — the text between "### " and the first ":" — e.g. ["Task 1", "Task 2", "Task 3"]. The COUNT of taskIds MUST',
          '  equal the number of grep matches; if they differ, re-run the grep and recount. A one-shot/thin plan may have NO',
          '  "### Task" matches — return an empty taskIds[] and still set setupOk=true (the implementer treats the whole plan as',
          '  a single unit). "## File Structure" / "## Notes for the implementer" headings are "## " not "### Task ", so the',
          '  grep excludes them automatically.',
        ]),
    '',
    'STEP C — summarize the plan as a whole (for the deep end-review):',
    '  - files: every repo-relative file path the plan will create or modify, collected from the "**Files:**" blocks' +
      (plan.isDir ? ' across the README.md and every task-N-*.md' : ''),
    '    (lines like "- Create: path", "- Modify: path", "- Test: path"). Strip surrounding backticks and any trailing',
    '    parenthetical note; keep only the bare path. De-duplicate.',
    '  - summary: one or two sentences on what the plan delivers.',
    '',
    'STEP D — read the plan\'s HEADER TAGS (blockquote lines near the top' + (plan.isDir ? ' of README.md' : '') + '):',
    '  - RIGOR:  a line like  > **Rigor:** one-shot  -> return rigor="one-shot"; otherwise rigor="tdd-per-task" (default).',
    '    This tunes how strict the implementer is about test-first. A one-shot plan is ALLOWED to have NO "### Task"s.',
    '  - GATE:  a line like  > **Gate:** rust-only   (or "full", or "auto") -> return gate accordingly; if absent return',
    '    gate="auto" (the default — the engine then derives the gate from the files[] this plan touches). "rust-only"',
    '    forces the cargo-only gate (use for a plan that must NOT build the pnpm packages); "full" forces both halves.',
    '',
    'Do NOT implement anything and do NOT modify tracked files — only create the worktree, install deps, and read the plan.',
    'Return setupOk (true only if the plan is a valid top-level ' + (plan.isDir ? 'directory (README.md present)' : 'file') + ' AND the worktree exists on branch plan/' + plan.slug + '),',
    'worktreePath (the absolute path), baseSha (the base commit captured above), branch (plan/' + plan.slug + '), taskIds[],',
    'files[] (the plan\'s touched repo-relative paths), summary (what the plan delivers), rigor ("one-shot"|"tdd-per-task"),',
    'gate ("auto"|"rust-only"|"full"), and notes.',
  ].join('\n')
}

// The core: ONE batched implementer generation. Burns committable green units, then returns gracefully when
// context-fill says it is near the quality zone — a fresh generation resumes from git trailers.
function implementerGenerationPrompt(plan, worktreePath, baseSha, gen, rigor, taskIds, handoffNote, marker, gate) {
  const tddHint = rigor === 'one-shot'
    ? 'RIGOR HINT (one-shot): this plan was tagged small / single-concern. You do NOT need strict test-first-per-task — make the change directly and ensure the gate is green; add or adjust a test for any BEHAVIOR change.'
    : 'RIGOR HINT (tdd-per-task): use Test-Driven Development for each behavioral slice — write the failing test FIRST, RUN it and watch it fail for the RIGHT reason, then minimal code to green. Pure docs/config steps need no new test — run exactly what the step says and observe the result.'
  const hasTasks = taskIds && taskIds.length
  const idsLine = hasTasks ? taskIds.join(', ') : '(none — thin/one-shot plan; treat the whole plan as a single unit)'
  const trailerTasksHint = hasTasks
    ? '<the task-ids this unit completed, e.g. "Task 1, Task 2">'
    : '<the plan slug "' + plan.slug + '", since this thin plan has no numbered tasks>'
  const footerLine = COMMIT_FOOTER
    ? '     On the VERY NEXT line after Plan-Tasks — NO blank line between them — add the attribution footer so all three\n' +
      '     stay in ONE contiguous trailer block (a blank line would split the block and git would stop parsing Plan-Tasks):  ' + COMMIT_FOOTER
    : ''
  // Integrate step by mode: LOCAL commits stay on the plan branch (no push/fetch/rebase — main is untouchable here);
  // dry-run keeps the commit local too; merge (default) rebases + fast-forward-pushes onto the trunk exactly as before.
  const integrateStep = LOCAL
    ? [
        '  d. LOCAL MODE: do NOT push, fetch, or rebase onto any remote. The committed unit stays on the plan branch',
        '     (plan/' + plan.slug + '), which IS the trunk for this run — its Plan/Plan-Tasks trailers are how the next',
        '     generation resumes. Do NOT touch the local ' + BASE_BRANCH + ' ref: it is checked out in another worktree and',
        '     must be left exactly where it is. Go straight to step e.',
      ].join('\n')
    : MODE === 'dry-run'
    ? '  d. DRY-RUN MODE: do NOT push. The committed unit stays local on the branch. Go straight to step e.'
    : [
        '  d. PUSH the unit straight to the trunk (' + REMOTE + '/' + BASE_BRANCH + '):',
        '       - CLEAN-TREE ASSERT (fail-closed against a false-green): run "git status --porcelain". It MUST be EMPTY.',
        '         If anything is staged/modified/untracked, the gate below would validate a DIRTY tree, not the committed',
        '         HEAD — exactly the slip that once pushed a non-compiling commit to main. Either commit the leftover into',
        '         this unit or stash/clean it, until "git status --porcelain" prints nothing. Do NOT push a dirty tree.',
        '       - git fetch ' + REMOTE,
        '       - git rebase ' + REMOTE + '/' + BASE_BRANCH + '   (a CONFLICT is the one thing you must not force-resolve blindly:',
        '         if it conflicts in a way the plan does not clearly dictate, run "git rebase --abort" and return escalate=true.)',
        '       - RE-RUN the gate on the rebased tree so what LANDS is what was tested:  ' + gate,
        '       - Re-assert "git status --porcelain" is EMPTY after the gate (the gate must not have dirtied the tree, e.g.',
        '         a formatter rewrite) — if it dirtied, commit the fixup into the unit and re-gate before pushing.',
        '       - git push ' + REMOTE + ' HEAD:' + BASE_BRANCH,
        '       - If the push is REJECTED (non-fast-forward — the trunk advanced under you), re-fetch, rebase onto',
        '         ' + REMOTE + '/' + BASE_BRANCH + ' again, re-gate, and retry, BOUNDED to ' + MAX_ATTEMPTS + ' attempts. NEVER force-push.',
        '       If the gate goes red on the rebased tree (a semantic conflict the text-merge missed), DEBUG + FIX it',
        '       (test-first for a behavior change), commit the fix into this unit, and retry the push, within the same',
        '       ' + MAX_ATTEMPTS + '-attempt budget. If it still cannot land, return escalate=true with the reason.',
      ].join('\n')
  return [
    'You are ONE generation of a BATCHED, self-rotating plan implementer for the ' + PROJECT.name + ' repo. You burn through',
    'as many committable green UNITS of an approved plan as you can, then RETURN gracefully when a context check says you',
    'are near your quality zone. A FRESH generation then resumes exactly where you left off, reading progress from git.',
    'You are generation #' + gen + '.',
    'Project: ' + PROJECT.name + ' — a Rust cargo workspace (crates/*) plus a pnpm/TypeScript monorepo (packages/*). Host: Windows, PowerShell (Bash also',
    'available). Conventions you MUST follow: Conventional Commits for the subject line.',
    '',
    'Work ENTIRELY inside this git worktree — cd into it and run all git/pnpm commands there:',
    '  ' + worktreePath,
    ...(plan.isDir
      ? [
          'You are on branch plan/' + plan.slug + '. Your COMPLETE instruction set is a plan DIRECTORY (docs/superpowers/ is',
          'gitignored, so it lives ONLY in the main repo, NOT inside your worktree). Read it by ABSOLUTE path:',
          '  ' + plan.absPath + '/README.md      — the overview + "## Task Index" (task order; may hold per-task "- [ ]" checkboxes)',
          '  ' + plan.absPath + '/task-N-*.md     — ONE file per task; each holds that task\'s body and its "- [ ]"/"- [x]" step checkboxes',
          'Read the README for the task index, then the relevant task-N-*.md before implementing that task.',
        ]
      : [
          'You are on branch plan/' + plan.slug + '. The plan file is your COMPLETE instruction set — READ IT by this ABSOLUTE',
          'path (docs/superpowers/ is gitignored, so the plan lives ONLY in the main repo and is NOT inside your worktree):',
          '  ' + plan.absPath,
        ]),
    'What it delivers: ' + plan.summary,
    'Its task-ids, in order: ' + idsLine,
    '',
    '=== STEP 0: DISCOVER YOUR OWN TRANSCRIPT (for self-rotation) ===',
    'Your unique context marker is:  ' + marker,
    'Run this ONCE to locate your own transcript and CACHE the path it prints (the "transcript" field of the JSON):',
    '    python ' + CONTEXT_FILL + ' --marker "' + marker + '" --max-tokens ' + ROTATE_MAX_TOKENS,
    'Use that cached path as --transcript on every later check (instant, no re-scan). If the exit code is already 10',
    'this early (unlikely), do ONE unit then rotate.',
    '',
    '=== STEP 1: ORIENT MECHANICALLY (never trust prose for what is done) ===',
    'Progress is derived from TWO signals, unioned. Never guess from prose.',
    '',
    'SIGNAL A — commit trailers. Every landed unit of THIS plan carries two trailers in its commit message body:',
    '    Plan: ' + plan.slug,
    '    Plan-Tasks: <comma-separated plan task-ids this unit completed>',
    ...(LOCAL
      ? [
          'LOCAL run: the plan branch (plan/' + plan.slug + ') is itself the trunk and every unit is committed onto it, so its',
          'OWN history holds every landed unit as an ancestor of HEAD — there is NO remote and NO fetch. Scan HEAD filtered',
          'to this plan\'s slug:',
          '    git log HEAD --grep="Plan: ' + plan.slug + '" --format=%h%x1f%(trailers:key=Plan,valueonly)%x1f%(trailers:key=Plan-Tasks,valueonly)',
        ]
      : [
          'Scan the WHOLE trunk, NOT just ' + baseSha + '..HEAD. Critical: this generation forked off the CURRENT ' + REMOTE + '/' +
            BASE_BRANCH + ' tip, so units a PRIOR run already pushed are now folded BELOW your fork point — a ' + baseSha +
            '..HEAD window would show them as undone and you would redo Task 1 over already-landed work. Fetch, then scan the',
          'trunk history filtered to this plan\'s slug:',
          '    git fetch ' + REMOTE,
          '    git log ' + REMOTE + '/' + BASE_BRANCH + ' --grep="Plan: ' + plan.slug + '" --format=%h%x1f%(trailers:key=Plan,valueonly)%x1f%(trailers:key=Plan-Tasks,valueonly)',
        ]),
    'Keep only commits whose Plan trailer is EXACTLY "' + plan.slug + '"; union their Plan-Tasks ids. ROBUSTNESS: git parses',
    'trailers only from the LAST contiguous block, so a stray blank line can leave %(trailers) empty though the body has',
    'the lines — for any --grep hit with an empty parsed Plan, fall back to "git log -1 --format=%B <sha>" and scan for',
    '"^Plan:" / "^Plan-Tasks:" body lines. (A wide history scan can be capped with --since or -n a few hundred if huge.)',
    '',
    'SIGNAL B — plan checkboxes (covers work landed WITHOUT trailers, e.g. tasks done by hand before this workflow ran).',
    ...(plan.isDir
      ? [
          'This is a DIRECTORY plan: read ' + plan.absPath + '/README.md AND each ' + plan.absPath + '/task-N-*.md. Treat "Task N"',
          'as DONE when its README "## Task Index" line is a CHECKED "- [x]" box (x or X), OR — when the README carries no',
          'per-task checkbox — when EVERY "- [ ]" step inside task-N-*.md is checked "- [x]". An unchecked box is NOT done.',
        ]
      : [
          'Read the plan file (' + plan.absPath + '). If a task-id\'s line is a CHECKED checkbox — "- [x] Task N" (any case, x or',
          'X) — treat that task as DONE even if no trailer covers it. An unchecked "- [ ]" is NOT done. Plans without a checkbox',
          'list contribute nothing from this signal — that is fine.',
        ]),
    '',
    'DONE = union(Signal A ids, Signal B checked ids), intersected with this plan\'s real task-ids. The NEXT unit starts at',
    'the LOWEST plan task-id NOT in DONE. If EVERY plan task-id is already in DONE (or, for a thin plan with no task-ids, at',
    'least one unit with this plan\'s slug has already landed on the trunk), you are finished — make no new commit and',
    'return done=true, allDone=true. When you DO land a unit whose task was only checkbox-done before (no trailer), still',
    'write the Plan/Plan-Tasks trailers so the record becomes trailer-backed going forward.',
    '',
    '=== STEP 2: DRIFT-CHECK (only when RESUMING — i.e. DONE is non-empty) ===',
    'If some units are already landed, a prior generation did work. Skim this plan\'s landed commits',
    '(git log ' + (LOCAL ? 'HEAD' : REMOTE + '/' + BASE_BRANCH) + ' --grep="Plan: ' + plan.slug + '" --oneline) and the handoff note below. If the',
    'landed work has DRIFTED from the plan, correct it before continuing. The PLAN IS',
    'LEADING — deviations from it are defects, not creativity. BUT if the PLAN ITSELF is wrong or infeasible (it asks for',
    'something that cannot be built as written), do NOT silently rewrite it: STOP and return escalate=true with the',
    'reason in problems. (On generation #1 with nothing landed, SKIP this step.)',
    'CONTINUOUS REVIEW (cheap — you are already reading the increment): while here, also eyeball the just-landed commits',
    'for any OBVIOUS correctness or security defect. Fix-forward anything clearly wrong NOW, landing the fix as its own',
    'unit (steps b–c). The deep end-review is the final whole-plan backstop, not the only review.',
    'Handoff note from the previous generation (SOFT context only — gotchas/deferrals; progress is NEVER here):',
    '  ' + (handoffNote ? handoffNote : '(none — this is the first generation)'),
    '',
    '=== STEP 3: BURN UNITS ===',
    tddHint,
    'A UNIT is the SMALLEST coherent, committable, GREEN slice of the plan — DEFAULT to ONE plan task per unit. Small',
    'batches give fast, precisely-localized feedback and a small blast radius when something fails. Bundle tasks into one',
    'unit ONLY when they genuinely cannot stand alone. Working from the NEXT not-done task, for EACH unit:',
    '  a. Implement it (per the rigor hint above).',
    '  b. GATE — run the SCOPE-AWARE green-gate for this plan in the worktree and make it pass:  ' + gate,
    '     (This gate was derived from the files this plan touches: the cargo half is fmt --check + clippy + test',
    '     --workspace; the pnpm half — vitest + eslint + build — is included only if the plan touches packages/**.)',
    '     A failure: DEBUG + FIX (test-first for a behavior change), re-run, BOUNDED to ' + MAX_ATTEMPTS + ' attempts. If it',
    '     still cannot go green, STOP and return escalate=true with the reason in problems.',
    '  c. Commit the unit (Conventional Commit subject), staging ONLY the files this unit changed, with BOTH trailers in',
    '     the commit message body (after a blank line, one trailer per line):',
    '         Plan: ' + plan.slug,
    '         Plan-Tasks: ' + trailerTasksHint,
    '     These trailers are how the next generation and the orchestrator know this unit landed — they are MANDATORY and',
    '     must be ACCURATE. List exactly the plan task-ids this commit completes.',
    footerLine,
    integrateStep,
    '  e. SELF-ROTATION CHECK — run this after EACH unit is committed (and, in merge mode, pushed):',
    '         python ' + CONTEXT_FILL + ' --transcript "<your cached path>" --max-tokens ' + ROTATE_MAX_TOKENS,
    '     Exit code 10 => you are at the quality-zone cap: STOP NOW (do NOT start another unit) and return gracefully with',
    '     done=true, rotated=true, allDone=false, and a short handoffNote. Exit 0 => continue to the next unit.',
    '',
    '=== WHEN TO RETURN ===',
    '  - Every plan task-id is now landed (DONE covers them all): return done=true, allDone=true.',
    '  - The rotation check said rotate (exit 10): return done=true, rotated=true, allDone=false, with a handoffNote.',
    '  - The plan is wrong/infeasible, the gate cannot go green, or a push cannot land (rebase conflict / lost race after',
    '    the bounded retries) after the bounded retries: return done=true, escalate=true with problems set.',
    'NEVER fabricate progress. Your allDone is a HINT — the orchestrator verifies it against the plan task-list from git.',
    '',
    'handoffNote: a few bullets of SOFT context for the next generation ONLY — gotchas, deferrals, patterns to watch.',
    'NEVER put progress or "what is done" in it (that is read from git trailers). Keep it short. Omit it (empty string)',
    'if you have nothing useful to pass on.',
    '',
    'Report: done (you returned gracefully rather than dying), allDone (you believe every plan task-id is landed — a',
    'HINT), rotated (you stopped because of the rotation check), escalate (plan wrong/infeasible, the gate could not go',
    'green, OR a push could not land after the bounded retries), unitsLanded (how many units you pushed this generation),',
    'handoffNote, and problems (the escalation reason, else empty).',
  ].join('\n')
}

// Mechanical progress + completeness probe — numbers read from git trailers AND plan checkboxes, no judgement, no code
// changes. Authoritative: the generation's allDone is only a hint; this is what the completeness gate trusts.
function progressProbePrompt(plan, worktreePath, baseSha, taskIds) {
  const idsLine = taskIds && taskIds.length ? taskIds.join(', ') : '(none — thin/one-shot plan, no numbered tasks)'
  return [
    'You are a MECHANICAL progress probe for a batched plan implementation. You make NO code changes and exercise NO',
    'judgement — you only READ and report numbers. Host: Windows/PowerShell (Bash also available).',
    'cd into this worktree:  ' + worktreePath,
    'Branch: plan/' + plan.slug + '.',
    '',
    'Progress comes from TWO signals; report their union.',
    '',
    'SIGNAL A — commit trailers. Every landed unit of THIS plan carries "Plan: ' + plan.slug + '" and "Plan-Tasks: <ids>".',
    ...(LOCAL
      ? [
          'LOCAL run: the plan branch is the trunk and every unit is committed onto it, so HEAD\'s own history holds them all',
          '— there is NO remote and NO fetch. Run:',
          '    git log HEAD --grep="Plan: ' + plan.slug + '" --format=%h%x1f%(trailers:key=Plan,valueonly)%x1f%(trailers:key=Plan-Tasks,valueonly)',
        ]
      : [
          'Scan the WHOLE trunk, not a fork-point window: a prior run\'s units are folded BELOW this worktree\'s base, so a',
          baseSha + '..HEAD range would MISS them and falsely report 0 landed (the classic false-park). Run:',
          '    git fetch ' + REMOTE,
          '    git log ' + REMOTE + '/' + BASE_BRANCH + ' --grep="Plan: ' + plan.slug + '" --format=%h%x1f%(trailers:key=Plan,valueonly)%x1f%(trailers:key=Plan-Tasks,valueonly)',
        ]),
    'Consider ONLY commits whose "Plan:" trailer is EXACTLY "' + plan.slug + '" (the slug filter is REQUIRED — the trunk',
    'holds OTHER plans\' commits whose Plan-Tasks ids could collide). Union their Plan-Tasks ids.',
    'ROBUSTNESS — git parses trailers only from the LAST contiguous message block; a stray blank line can leave',
    '%(trailers) EMPTY though the body has the lines. For any --grep hit with an empty parsed Plan, FALL BACK to',
    '"git log -1 --format=%B <sha>" and scan for "^Plan:"/"^Plan-Tasks:" body lines. Count fallback commits identically.',
    '',
    ...(plan.isDir
      ? [
          'SIGNAL B — plan checkboxes (covers tasks completed WITHOUT trailers, e.g. by hand before the workflow ran).',
          'This is a DIRECTORY plan: read ' + plan.absPath + '/README.md AND each ' + plan.absPath + '/task-N-*.md. "Task N" counts',
          'as done when its README "## Task Index" line is a CHECKED "- [x]" box (x or X), OR — when the README has no per-task',
          'checkbox — when EVERY "- [ ]" step inside task-N-*.md is checked. "- [ ]" does not count.',
        ]
      : [
          'SIGNAL B — plan checkboxes (covers tasks completed WITHOUT trailers, e.g. by hand before the workflow ran). Read the',
          'plan file (' + plan.absPath + '). A task-id whose line is a CHECKED checkbox "- [x] Task N" (x or X) counts as done',
          'even with no commit trailer. "- [ ]" does not count. A plan with no checkbox list contributes nothing here.',
        ]),
    '',
    'This plan\'s task-ids are: ' + idsLine,
    '',
    'Report (read straight from git + the plan file — do NOT guess or infer):',
    '  - coveredIds: the UNION of Signal-A trailer ids and Signal-B checked ids, INTERSECTED with the plan task-ids above',
    '    (only real plan task-ids appear; empty for a thin plan).',
    '  - landedCommits: how many commits carry the "Plan: ' + plan.slug + '" trailer on the trunk (Signal A only — a',
    '    checkbox with no commit does NOT count here; this number is the run\'s no-progress signal).',
    '  - landedShas: the short SHAs of those commits.',
    '  - headSha: git rev-parse --short HEAD.',
    '  - notes: anything odd (e.g. a covered id that is not a known plan task-id, or tasks done only by checkbox).',
  ].join('\n')
}

// The deep END-REVIEW: one whole-plan review of the cumulative branch diff against the base point, across the standard
// quality lenses. It fix-forwards introduced critical/high/medium defects; unresolved INTRODUCED critical/high BLOCK
// the close (medium is pursued across the bounded rounds, then downgraded to a warning if it is all that is left).
function deepReviewPrompt(plan, worktreePath, baseSha, concerns) {
  const fileScope =
    plan.files && plan.files.length ? plan.files.map((f) => '"' + f + '"').join(' ') : '.'
  const hasConcerns = Array.isArray(concerns) && concerns.length
  const concernsBlock = hasConcerns
    ? [
        'This plan was gated by a prior plan-review; its critical/high concerns are supplied below. Use them as a',
        'checklist that the implementation actually handles each one: for each concern, verify the code CONVINCINGLY',
        'addresses it; if it does NOT, that is a finding here (at a severity matching the concern). This is a FOCUSING',
        'checklist, not a re-litigation of concerns already resolved. The concerns:',
        JSON.stringify(concerns, null, 2),
      ].join('\n')
    : '(No prior plan-review concerns were supplied for this run — SKIP this step.)'
  return [
    'You are the DEEP END-REVIEWER for a completed plan in the ' + PROJECT.name + ' repo (a Rust core under a pnpm/TypeScript monorepo).',
    'You review its WHOLE cumulative implementation and decide what THIS change must fix before it can close.',
    'Host: Windows/PowerShell (a Bash tool is also available). cd into this worktree:  ' + worktreePath,
    'Branch: plan/' + plan.slug + '.',
    '',
    '=== STEP 1: KNOW THE BAR ===',
    'If the repo has a CLAUDE.md or AGENTS.md at its root (or under .claude/), read it first — it carries the project\'s',
    'conventions. Review the change through these lenses, most load-bearing first:',
    '  - correctness: does it do what the plan specifies? edge cases, error paths, off-by-one, state handling.',
    '  - security: input handling, injection, unwrap/panic on untrusted input, unsafe HTML/eval, secret handling.',
    '  - maintainability & simplicity: dead code, needless complexity, duplication, unclear naming, wrong abstraction.',
    '  - testability & coverage: are the behavior changes covered by tests that would FAIL without the change?',
    '  - performance: only where it plausibly matters (render loops, large data, quadratic work).',
    '  - consistency: does it match the surrounding code\'s idioms, types, and file layout?',
    'Review each concern ONCE — do not re-report the same issue under multiple lenses.',
    '',
    '=== STEP 2: READ THE CHANGE ===',
    ...(LOCAL
      ? [
          'First compute the REVIEW BASE — the whole plan\'s starting point. LOCAL run: the plan branch is the trunk and HEAD',
          'is its tip; there is NO remote and NO fetch. Find this plan\'s OLDEST commit on the branch and diff from its parent:',
          '    REVIEW_BASE=$(git log HEAD --grep="Plan: ' + plan.slug + '" --reverse --format=%H | head -1)^',
          '    (if that finds NO commit — a thin plan, or work done only via checkboxes with no commits — fall back to ' + baseSha + ')',
          'Review the cumulative contribution over REVIEW_BASE..HEAD, scoped to the files this plan owns:',
          '    git diff $REVIEW_BASE HEAD -- ' + fileScope,
          'Skim the commit messages:  git log --oneline $REVIEW_BASE..HEAD  — then OPEN and read the',
        ]
      : [
          'First compute the REVIEW BASE — the whole plan\'s starting point on the trunk. Do NOT assume the worktree fork point:',
          'if a PRIOR run landed some units, they are folded BELOW this fork and a diff from the fork would miss them. Instead',
          'find this plan\'s OLDEST trunk commit and diff from its parent:',
          '    git fetch ' + REMOTE,
          '    REVIEW_BASE=$(git log ' + REMOTE + '/' + BASE_BRANCH + ' --grep="Plan: ' + plan.slug + '" --reverse --format=%H | head -1)^',
          '    (if that finds NO commit — a thin plan, or work done only via checkboxes with no commits — fall back to ' + baseSha + ')',
          'Review the cumulative contribution over REVIEW_BASE..' + REMOTE + '/' + BASE_BRANCH + ', scoped to the files this plan owns:',
          '    git diff $REVIEW_BASE ' + REMOTE + '/' + BASE_BRANCH + ' -- ' + fileScope,
          'Skim the commit messages:  git log --oneline $REVIEW_BASE..' + REMOTE + '/' + BASE_BRANCH + '  — then OPEN and read the',
        ]),
    'changed code. Confirm the plan is delivered as a whole, the per-unit pieces fit together coherently, and no defect',
    'was introduced ACROSS units (the kind a single-unit review would miss).',
    'Plan ' + (plan.isDir ? 'DIRECTORY (README.md + task-N-*.md)' : 'file') + ' (the requirements, ABSOLUTE path — gitignored, not in your worktree): ' + plan.absPath + '. What it delivers: ' + plan.summary,
    '',
    '=== STEP 3: PRIOR-REVIEW MEMORY (additive focusing — IN ADDITION to, never instead of, the review above) ===',
    concernsBlock,
    '',
    '=== STEP 4: CLASSIFY EVERY FINDING — INTRODUCED vs DEFERRED (this gate is load-bearing) ===',
    'We fix what THIS change introduced; we do NOT sign this plan up to fix the whole codebase. For each issue:',
    '  - INTRODUCED: the defect is in code this plan ADDED or MODIFIED — this change caused it. Put it in critical /',
    '    high / medium by severity. These DRIVE the fix-forward pass.',
    '  - low: an INTRODUCED nit (style/naming/minor inconsistency). Informational — logged, never fixed here.',
    '  - deferred: NOT fixed here — an issue in surrounding/untouched code (pre-existing), OR work the plan explicitly',
    '    scoped out, EVEN IF this change technically touched nearby. Logged as context.',
    'Do NOT modify anything — you only review and classify.',
    '',
    'Return the INTRODUCED findings in critical / high / medium (each a file:line + one-line — these get fix-forwarded),',
    'low (introduced nits, informational), deferred (pre-existing or out-of-scope, informational), and a one-line assessment.',
  ].join('\n')
}

// Fix-forward the deep review's INTRODUCED findings (critical/high/medium), committing each green on the branch.
function fixForwardPrompt(plan, worktreePath, actionable, gate) {
  const footerLine = COMMIT_FOOTER ? '     After a blank line, add the footer:  ' + COMMIT_FOOTER : ''
  return [
    'A deep end-review of a completed plan found defects THIS change INTRODUCED (critical/high/medium). Fix them and',
    'commit the fixes on the branch. Project: ' + PROJECT.name + ' (Rust core + pnpm/TypeScript monorepo). Host: Windows/',
    'PowerShell. Conventions: Conventional Commits.',
    'cd into this worktree:  ' + worktreePath,
    'Branch: plan/' + plan.slug + '. The plan is fully implemented on this branch; you are fixing review findings.',
    '',
    'Resolve these findings (and ONLY these, plus what is strictly required to make them correct). Each is prefixed with',
    'its severity — fix all of them:',
    JSON.stringify(actionable, null, 2),
    '',
    'Plan (intent/spec, ABSOLUTE path — gitignored, not in your worktree): ' + plan.absPath + ' — ' + plan.summary,
    '',
    'For each fix: keep it minimal and scoped; a behavior change needs a test that FAILS first. Then LAND it on the ' + (LOCAL ? 'branch:' : 'trunk:'),
    '  1. Run the scope-aware green-gate and make it pass:  ' + gate,
    '  2. Commit (Conventional Commit subject). A review-fix completes no NEW plan task, so it needs no Plan-Tasks trailer.',
    footerLine,
    ...(LOCAL
      ? [
          '  3. LOCAL MODE: do NOT push, fetch, or rebase. The fix commit stays on the plan branch (plan/' + plan.slug + '),',
          '     which is the trunk for this run. Do NOT touch the local ' + BASE_BRANCH + ' ref (checked out elsewhere). Before',
          '     committing, assert "git status --porcelain" is EMPTY after the gate so the committed HEAD is what was gated.',
        ]
      : [
          '  3. Push it straight to the trunk (' + REMOTE + '/' + BASE_BRANCH + '). Before pushing, assert "git status --porcelain"',
          '     is EMPTY (never push a dirty tree — the gate must have validated the committed HEAD, not loose changes):',
          '       git fetch ' + REMOTE + ' ; git rebase ' + REMOTE + '/' + BASE_BRANCH + ' ; re-run the gate ; git push ' + REMOTE + ' HEAD:' + BASE_BRANCH + '.',
          '     On a non-fast-forward rejection, re-fetch/rebase/re-gate and retry (bounded to ' + MAX_ATTEMPTS + '); on a rebase',
          '     CONFLICT you cannot cleanly resolve, STOP and report it. NEVER force-push.',
        ]),
    '',
    'Report: fixed (true ONLY if ALL listed findings are resolved AND ' +
      (LOCAL ? 'committed green on the plan branch' : 'pushed green to ' + REMOTE + '/' + BASE_BRANCH) + '), commits',
    '(short SHAs you created), and problems (precise reason if not fixed).',
  ].join('\n')
}

// Clean-room verify: an INDEPENDENT agent compiles the exact pushed trunk tip from a FRESH throwaway worktree. A fresh
// worktree cannot be dirty, so it cannot false-green the way an in-place gate over a dirty tree once did (a non-compiling
// commit reached main while the agent reported the gate green, because the gate saw uncommitted moved files). This is the
// deterministic backstop: if the pushed SHA does not compile from clean, park LOUDLY (RED-MAIN) instead of proceeding.
function cleanRoomVerifyPrompt(plan) {
  const vwt = WORKTREES_DIR + '/verify-' + plan.slug
  return [
    'You are the CLEAN-ROOM VERIFIER for the ' + PROJECT.name + ' repo. You make NO code changes — you only confirm that the',
    (LOCAL ? 'plan-branch tip' : 'pushed trunk tip') + ' COMPILES from a pristine checkout. Host: Windows/PowerShell (Bash also available). Run from the MAIN',
    'repo dir (' + REPO_DIR + ').',
    '',
    ...(LOCAL
      ? [
          'Every unit of this plan is committed on the local plan branch (plan/' + plan.slug + ') — there is NO remote. Verify',
          'that branch tip compiles from clean:',
          '  1. (LOCAL — no remote: skip git fetch.)',
          '  2. Create a FRESH throwaway worktree at the plan-branch tip (detached, so it does not collide with the branch\'s',
          '     own worktree):',
          '       git worktree add --detach "' + vwt + '" plan/' + plan.slug,
          '     (If it already exists from a stale run: git worktree remove --force "' + vwt + '" first, then re-add.)',
        ]
      : [
          'Every unit of this plan has been pushed to ' + REMOTE + '/' + BASE_BRANCH + '. Verify that tip compiles from clean:',
          '  1. git fetch ' + REMOTE,
          '  2. Create a FRESH throwaway worktree at the exact pushed tip (detached, no branch):',
          '       git worktree add --detach "' + vwt + '" ' + REMOTE + '/' + BASE_BRANCH,
          '     (If it already exists from a stale run: git worktree remove --force "' + vwt + '" first, then re-add.)',
        ]),
    '  3. Capture the SHA:  git -C "' + vwt + '" rev-parse --short HEAD',
    '  4. From INSIDE that worktree, run the compile-only workspace check and make NOTHING else:',
    '       ' + PROJECT.gate.verify,
    '     A fresh worktree cannot be dirty, so this validates the committed HEAD exactly. (Compile-only by design — this',
    '     is a fast fail-closed backstop against a non-compiling pushed commit, not a re-run of the whole test suite.)',
    '  5. CLEAN UP regardless of result (from ' + REPO_DIR + ', not inside the worktree):',
    '       git worktree remove --force "' + vwt + '"   (if that fails: rm -rf "' + vwt + '" ; git worktree prune)',
    '',
    'Report: green (true ONLY if step 4 exited 0), headSha (the SHA from step 3), problems (the first compile error line if',
    'not green, else empty).',
  ].join('\n')
}

function archivePrompt(plan, worktreePath, planPath) {
  const footer = COMMIT_FOOTER ? ' (add the footer "' + COMMIT_FOOTER + '" after a blank line)' : ''
  const completedAbs = REPO_DIR + '/' + COMPLETED_DIR
  // For a directory plan, `git mv`/`mv` moves the WHOLE directory into completed/ (yielding completed/<dir>); for a
  // single-file plan it moves the one .md. The commands are identical either way because planPath/absPath already point
  // at the directory or the file — only the wording changes.
  const noun = plan.isDir ? 'plan directory' : 'plan file'
  const headRef = LOCAL ? 'plan/' + plan.slug : REMOTE + '/' + BASE_BRANCH
  // Step 1a (TRACKED): merge/dry-run push the archive commit onto the trunk; LOCAL commits it onto the plan branch only.
  const step1aLand = LOCAL
    ? [
        '     LOCAL MODE: do NOT push/fetch/rebase — the archive commit stays on the plan branch (which is the trunk for',
        '     this run). Do NOT touch the local ' + BASE_BRANCH + ' ref (checked out elsewhere). Set archived=true once the',
        '     archive commit is on plan/' + plan.slug + '.',
      ]
    : [
        '     Push it to the trunk (fast-forward). On non-fast-forward rejection re-fetch, rebase onto ' + REMOTE + '/' + BASE_BRANCH + ',',
        '     re-push; retry up to ' + MAX_ATTEMPTS + ' times; NEVER force-push:',
        '       git fetch ' + REMOTE + ' ; git rebase ' + REMOTE + '/' + BASE_BRANCH + ' ; git push ' + REMOTE + ' HEAD:' + BASE_BRANCH,
        '     Set archived=true only if the archive commit reached ' + REMOTE + '/' + BASE_BRANCH + '.',
      ]
  // Step 2 cleanup: merge/dry-run tears down the plan worktree + branch (work is safely on the remote). LOCAL MUST NOT —
  // the plan branch is the ONLY copy of the work; deleting it would destroy the run. Leave both for the user to land.
  const step2 = LOCAL
    ? [
        '2. LOCAL MODE — DO NOT clean up. The plan branch (plan/' + plan.slug + ') and its worktree are the ONLY copy of the',
        '   implemented work; the user still has to review and land them. Leave the worktree and the branch in place. NEVER',
        '   "git branch -D plan/' + plan.slug + '" or remove the worktree here.',
      ]
    : [
        '2. Clean up — run these from the MAIN repo dir, NOT from inside the worktree. On Windows "git worktree remove" can',
        '   fail "Directory not empty" because the worktree has its own node_modules; if so, delete the directory first,',
        '   then prune:',
        '     cd "' + REPO_DIR + '"',
        '     git worktree remove --force "' + worktreePath + '"   (if that fails: rm -rf "' + worktreePath + '" ; git worktree prune)',
        '     git branch -D plan/' + plan.slug,
      ]
  return [
    'Final step for a fully-implemented' + (LOCAL ? '' : ', fully-pushed') + ' plan: archive its ' + noun + ' and ' +
      (LOCAL ? 'report (do NOT tear down the branch). Host:' : 'clean up the worktree. Host:'),
    'Windows/PowerShell (Bash tool also available). Conventions: Conventional Commits.',
    'Do NOT touch the LOCAL ' + BASE_BRANCH + ' branch or the main working directory tree (beyond moving the ' + noun + ').',
    '',
    '0. DECIDE THE ARCHIVE MODE — is the ' + noun + ' tracked by git? Run from the MAIN repo dir (' + REPO_DIR + '):',
    '     git -C "' + REPO_DIR + '" ls-files --error-unmatch "' + planPath + '"',
    '   Exit 0 = TRACKED (do step 1a). Non-zero = UNTRACKED/gitignored (do step 1b). Let the command DECIDE — do NOT assume;',
    '   a plan may be committed (tracked) or gitignored-local depending on the repo/workflow.',
    '',
    '1a. TRACKED plan — archive it ' + (LOCAL ? '' : 'on the trunk ') + 'via the worktree. cd into the worktree (' + worktreePath + '), then' + footer + ':',
    '     git mv "' + planPath + '" ' + COMPLETED_DIR + '/     (mkdir -p ' + COMPLETED_DIR + ' first if it does not exist' + (plan.isDir ? '; moves the WHOLE directory' : '') + ')',
    '     git commit -m "chore(plans): archive ' + plan.slug + ' (implemented)"',
    ...step1aLand,
    '',
    '1b. UNTRACKED/gitignored plan — there is NOTHING to commit or push (git never tracked it). Just move the ' + (plan.isDir ? 'directory' : 'file') + ' ON DISK',
    '    in the MAIN repo so it leaves the active plans/ directory. Run from anywhere (absolute paths):',
    '       mkdir -p "' + completedAbs + '"',
    '       mv "' + plan.absPath + '" "' + completedAbs + '/"',
    '    This is a pure filesystem move, no git. Set archived=true once the ' + (plan.isDir ? 'directory' : 'file') + ' is moved. headSha is not applicable',
    '    (report the current ' + headRef + ' head:  git -C "' + REPO_DIR + '" rev-parse --short ' + headRef + ').',
    '',
    ...step2,
    '',
    'Report: archived (true once the ' + noun + ' is in ' + COMPLETED_DIR + '/ — committed' + (LOCAL ? ' on the branch' : '+pushed') + ' if it was tracked, else moved',
    'on disk), headShaAfter (git rev-parse --short of ' + headRef + '), and problems (empty if clean).',
  ].join('\n')
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Uniform result shape for one plan. `completed` = every plan task landed on the branch AND (in branch mode) the plan
// file was archived. `landedUnits` = how many unit commits are on the branch.
function planResult(plan, branch, worktreePath, fields) {
  const problems = fields.problems
  return {
    slug: plan.slug,
    branch,
    worktreePath,
    green: !!fields.green,
    completed: !!fields.completed,
    landedUnits: fields.landedUnits || 0,
    tasksTotal: fields.tasksTotal === undefined ? null : fields.tasksTotal,
    tasksAccounted: fields.tasksAccounted === undefined ? null : fields.tasksAccounted,
    commits: fields.commits || [],
    reviewWarnings: fields.reviewWarnings || [],
    reviewDeferred: fields.reviewDeferred || [],
    summary: fields.summary || '',
    problems: Array.isArray(problems) ? problems.join(' | ') : problems || '',
    reason: fields.reason || (fields.completed ? 'completed' : fields.green ? 'green (not archived)' : 'not green'),
  }
}

// Completeness gate: doneness is MECHANICAL — every plan task-id must appear in a landed "Plan-Tasks" trailer. A thin
// plan (no task-ids) is "done" once at least one unit has landed. Returns { complete, tasksTotal, tasksAccounted, missing }.
function completenessFromTrailers(taskIds, coveredIds, landedCommits) {
  const ids = Array.isArray(taskIds) ? taskIds : []
  const covered = new Set(Array.isArray(coveredIds) ? coveredIds : [])
  if (ids.length === 0) {
    const done = (landedCommits || 0) > 0
    return { complete: done, tasksTotal: 1, tasksAccounted: done ? 1 : 0, missing: [] }
  }
  const missing = ids.filter((id) => !covered.has(id))
  return { complete: missing.length === 0, tasksTotal: ids.length, tasksAccounted: ids.length - missing.length, missing }
}

// Implement ONE plan end-to-end in its own worktree via the batched, self-rotating engine. All units accumulate on the
// plan/<slug> branch; nothing is pushed. Always returns a planResult.
async function implementPlan(plan) {
  const branchGuess = 'plan/' + plan.slug
  try {
    // 1. Setup worktree (off the base branch) + install + capture baseSha + task-ids + files/summary/rigor.
    const setup = await agent(setupPrompt(plan), {
      schema: SETUP_SCHEMA,
      agentType: 'general-purpose',
      label: 'setup:' + plan.slug,
      phase: 'Implement',
      model: MID,
    })
    if (!setup || !setup.setupOk || !setup.worktreePath) {
      return planResult(plan, (setup && setup.branch) || branchGuess, (setup && setup.worktreePath) || '', {
        summary: 'setup failed',
        problems: setup ? 'worktree not ready; notes: ' + (setup.notes || '') : 'setup agent returned null',
        reason: 'setup failed',
      })
    }
    const worktreePath = setup.worktreePath
    const branch = setup.branch || branchGuess
    const baseSha = setup.baseSha || BASE_BRANCH
    plan.files = setup.files || [] // for the deep end-review's file scope
    plan.summary = setup.summary || '' // what the plan delivers
    const taskIds = Array.isArray(setup.taskIds) ? setup.taskIds : []
    const rigor = RIGOR_OVERRIDE || setup.rigor || 'tdd-per-task'
    // Scope-aware gate for THIS plan: derived from the files it touches, with the plan's "> **Gate:**" header as an
    // override. Computed once and threaded into every gate call site (per-unit gate, fix-forward, clean-room verify).
    const gate = gateFor(setup.files, setup.gate)
    log(plan.slug + ': worktree ready (' + worktreePath + '); ' + (taskIds.length || 'no numbered') + ' task(s); base ' + baseSha + '; rigor ' + rigor + '; gate [' + gate + '].')

    const problems = []
    let handoffNote = ''
    let lastProbe = null
    let prevLanded = 0 // landedCommits at the end of the previous generation (for the no-progress guard)
    let complete = false
    let coverage = completenessFromTrailers(taskIds, [], 0)

    // 2. Batched generation loop: spawn a generation, then probe git mechanically for progress + doneness.
    let gen = 0
    while (gen < MAX_GENERATIONS) {
      gen++
      if (handoffNote) log('gen ' + gen + ' resumes with handoff: ' + handoffNote)
      // Marker is unique per generation within this run (Date.now/Math.random are unavailable in the sandbox);
      // context-fill's newest-first scan resolves any cross-run collision to this agent's live transcript.
      const marker = 'CTXFILL::' + plan.slug + '::GEN' + gen
      const g = await agent(
        implementerGenerationPrompt(plan, worktreePath, baseSha, gen, rigor, taskIds, handoffNote, marker, gate),
        { schema: GEN_SCHEMA, agentType: 'general-purpose', label: 'impl-gen' + gen + ':' + plan.slug, phase: 'Implement', model: IMPLEMENTER_MODEL },
      )
      if (g && g.handoffNote) handoffNote = g.handoffNote

      // A returning generation that escalated (plan wrong/infeasible OR the gate could not go green) stops the run.
      if (g && g.escalate) {
        problems.push('gen ' + gen + ' escalated: ' + (g.problems || 'plan wrong/infeasible or gate could not go green'))
        const probe = await agent(progressProbePrompt(plan, worktreePath, baseSha, taskIds), {
          schema: PROBE_SCHEMA, agentType: 'general-purpose', label: 'probe' + gen + ':' + plan.slug, phase: 'Implement', model: MID,
        })
        if (probe) lastProbe = probe
        const cov = completenessFromTrailers(taskIds, lastProbe && lastProbe.coveredIds, lastProbe && lastProbe.landedCommits)
        log('ESCALATE ' + plan.slug + ' (gen ' + gen + '): ' + (g.problems || ''))
        return planResult(plan, branch, worktreePath, {
          completed: false,
          landedUnits: lastProbe ? lastProbe.landedCommits : 0,
          commits: (lastProbe && lastProbe.landedShas) || [],
          tasksTotal: cov.tasksTotal,
          tasksAccounted: cov.tasksAccounted,
          summary: 'escalated at gen ' + gen,
          problems,
          reason: 'escalated: plan wrong/infeasible or gate could not go green (' + (g.problems || '') + ')',
        })
      }

      // Mechanical progress + completeness probe (authoritative — the generation's allDone is only a hint).
      const probe = await agent(progressProbePrompt(plan, worktreePath, baseSha, taskIds), {
        schema: PROBE_SCHEMA, agentType: 'general-purpose', label: 'probe' + gen + ':' + plan.slug, phase: 'Implement', model: MID,
      })
      if (probe) lastProbe = probe
      const landed = lastProbe ? lastProbe.landedCommits : prevLanded
      coverage = completenessFromTrailers(taskIds, lastProbe && lastProbe.coveredIds, lastProbe && lastProbe.landedCommits)
      complete = coverage.complete
      if (g && g.allDone && !complete)
        log('NOTE ' + plan.slug + ' gen ' + gen + ': generation claims allDone but trailers show ' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' — continuing.')
      if (complete) {
        log('completeness gate PASSED for ' + plan.slug + ': ' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' task(s) landed.')
        break
      }

      // No-progress backstop: a whole generation that landed NO new Plan-Tasks trailer is stuck -> park (do not loop).
      if (landed <= prevLanded) {
        problems.push('gen ' + gen + ' landed no new Plan-Tasks trailer (no progress)' + (g ? '' : ' [generation agent died]'))
        log('NO-PROGRESS ' + plan.slug + ' at gen ' + gen + ' (' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' landed) -> parking.')
        return planResult(plan, branch, worktreePath, {
          completed: false,
          landedUnits: landed,
          commits: (lastProbe && lastProbe.landedShas) || [],
          tasksTotal: coverage.tasksTotal,
          tasksAccounted: coverage.tasksAccounted,
          summary: 'no progress in generation ' + gen + ' (' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' landed)',
          problems,
          reason: 'no progress (stuck) at generation ' + gen,
        })
      }
      prevLanded = landed
      // else: spawn the next generation — it resumes mechanically from the trailers.
    }

    if (!complete) {
      problems.push('generation cap (' + MAX_GENERATIONS + ') reached without completeness')
      log('GEN-CAP ' + plan.slug + ': ' + MAX_GENERATIONS + ' generations without completeness -> parking.')
      return planResult(plan, branch, worktreePath, {
        completed: false,
        landedUnits: lastProbe ? lastProbe.landedCommits : 0,
        commits: (lastProbe && lastProbe.landedShas) || [],
        tasksTotal: coverage.tasksTotal,
        tasksAccounted: coverage.tasksAccounted,
        summary: 'generation cap reached (' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' landed)',
        problems,
        reason: 'generation cap reached without completeness',
      })
    }

    const landedShas = (lastProbe && lastProbe.landedShas) || []
    const landedCommits = lastProbe ? lastProbe.landedCommits : 0

    // 3a. dry-run: units committed with trailers + the completeness gate passed. Run one INFORMATIONAL deep review
    //     (no fix-forward, no archive) and report.
    if (MODE === 'dry-run') {
      const review = await agent(deepReviewPrompt(plan, worktreePath, baseSha, PLAN_REVIEW_CONCERNS), {
        schema: QUALITY_REVIEW_SCHEMA, label: 'deep-review:' + plan.slug, phase: 'Review',
      })
      const crit = (review && review.critical) || []
      const high = (review && review.high) || []
      const med = (review && review.medium) || []
      const low = (review && review.low) || []
      const actionable = crit.length + high.length + med.length
      if (actionable)
        problems.push(
          'dry-run deep review (INTRODUCED, would fix-forward): ' +
            [...crit.map((c) => 'CRIT ' + c), ...high.map((h) => 'HIGH ' + h), ...med.map((m) => 'MED ' + m)].join('; '),
        )
      if (low.length) log('dry-run deep review LOWS (informational) for ' + plan.slug + ': ' + low.join('; '))
      return planResult(plan, branch, worktreePath, {
        green: !actionable,
        landedUnits: landedCommits,
        commits: landedShas,
        tasksTotal: coverage.tasksTotal,
        tasksAccounted: coverage.tasksAccounted,
        summary: (review && review.assessment) || 'dry-run: ' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' task(s) committed on the branch (not archived)',
        problems,
        reason: 'dry-run',
      })
    }

    // 3a-verify. CLEAN-ROOM COMPILE of the pushed trunk tip (merge mode only — dry-run pushes nothing). Deterministic
    //     backstop against a false-green: if what actually landed on the trunk does not compile from a pristine worktree,
    //     park LOUDLY as RED-MAIN rather than proceeding to review/archive over broken code.
    const verify = await agent(cleanRoomVerifyPrompt(plan), {
      schema: VERIFY_SCHEMA, agentType: 'general-purpose', label: 'verify:' + plan.slug, phase: 'Review', model: MID,
    })
    if (!verify || !verify.green) {
      // LOCAL parks against the plan-branch tip; merge/dry-run against the pushed trunk. Strings kept identical for merge.
      problems.push((LOCAL ? 'RED-MAIN: plan-branch tip does not compile from a clean worktree: ' : 'RED-MAIN: pushed trunk tip does not compile from a clean worktree: ') + (verify ? verify.problems : 'verify agent returned null'))
      log('RED-MAIN ' + plan.slug + ': ' + (LOCAL ? 'plan branch (plan/' + plan.slug + ')' : 'pushed ' + REMOTE + '/' + BASE_BRANCH) + ' tip fails ' + PROJECT.gate.verify + ' from clean — parking before review/archive.')
      return planResult(plan, branch, worktreePath, {
        completed: false,
        landedUnits: landedCommits,
        commits: landedShas,
        tasksTotal: coverage.tasksTotal,
        tasksAccounted: coverage.tasksAccounted,
        summary: LOCAL ? 'all task work landed but the plan-branch tip does not compile from clean (RED-MAIN)' : 'all task work landed but the trunk tip does not compile from clean (RED-MAIN)',
        problems,
        reason: LOCAL ? 'RED-MAIN: plan-branch tip does not compile from clean' : 'RED-MAIN: pushed tip does not compile from clean',
      })
    }
    log('clean-room verify PASSED for ' + plan.slug + ': ' + (LOCAL ? 'plan branch (plan/' + plan.slug + ')' : REMOTE + '/' + BASE_BRANCH) + ' tip (' + (verify.headSha || '?') + ') compiles from clean.')

    // 3b. Deep end-review; fix-forward the defects THIS change INTRODUCED (critical/high/medium) as new commits on the
    //     branch. low + deferred are informational. Gate: unresolved INTRODUCED critical/high BLOCK the close; medium is
    //     pursued across the bounded rounds, then if only medium/low remain we PROCEED and warn about the mediums.
    let review = await agent(deepReviewPrompt(plan, worktreePath, baseSha, PLAN_REVIEW_CONCERNS), {
      schema: QUALITY_REVIEW_SCHEMA, label: 'deep-review:' + plan.slug, phase: 'Review',
    })
    const reviewLists = (r) => ({
      critical: (r && r.critical) || [],
      high: (r && r.high) || [],
      medium: (r && r.medium) || [],
      low: (r && r.low) || [],
      deferred: (r && r.deferred) || [],
    })
    const actionableOf = (f) => [
      ...f.critical.map((c) => '[critical] ' + c),
      ...f.high.map((h) => '[high] ' + h),
      ...f.medium.map((m) => '[medium] ' + m),
    ]
    let f = reviewLists(review)
    for (let round = 0; round < MAX_REVIEW_FIX_ROUNDS && actionableOf(f).length; round++) {
      log(
        'Deep review of ' + plan.slug + ' raised ' + f.critical.length + ' critical / ' + f.high.length + ' high / ' +
          f.medium.length + ' medium introduced finding(s); fix-forward round ' + (round + 1) + '.',
      )
      const fix = await agent(fixForwardPrompt(plan, worktreePath, actionableOf(f), gate), {
        schema: FIX_SCHEMA, agentType: 'general-purpose', label: 'fix-fwd' + (round + 1) + ':' + plan.slug, phase: 'Review',
      })
      if (fix && !fix.fixed && fix.problems) log('fix-forward round ' + (round + 1) + ' incomplete: ' + fix.problems)
      review = await agent(deepReviewPrompt(plan, worktreePath, baseSha, PLAN_REVIEW_CONCERNS), {
        schema: QUALITY_REVIEW_SCHEMA, label: 'deep-review' + (round + 2) + ':' + plan.slug, phase: 'Review',
      })
      f = reviewLists(review)
    }
    // Informational findings — logged then dropped, regardless of the gate outcome.
    if (f.low.length) log('Deep review LOWS (informational, dropped) for ' + plan.slug + ': ' + f.low.join('; '))
    if (f.deferred.length)
      log('Deep review DEFERRED (pre-existing or out-of-scope, not fixed) for ' + plan.slug + ': ' + f.deferred.join('; '))
    // HARD gate: unresolved INTRODUCED critical/high after the bounded rounds block the close (work stays on the branch).
    if (f.critical.length || f.high.length) {
      problems.push(
        'deep review unresolved INTRODUCED critical/high: ' +
          [...f.critical.map((c) => 'CRIT ' + c), ...f.high.map((h) => 'HIGH ' + h)].join('; '),
      )
      return planResult(plan, branch, worktreePath, {
        completed: false,
        landedUnits: landedCommits,
        commits: landedShas,
        tasksTotal: coverage.tasksTotal,
        tasksAccounted: coverage.tasksAccounted,
        summary: 'deep review found unresolved critical/high issues introduced by this change (all task work is already on ' + (LOCAL ? 'branch ' + branch : REMOTE + '/' + BASE_BRANCH) + ')',
        problems,
        reason: 'deep review unresolved',
      })
    }
    // SOFT gate: only medium/low remain (crit/high cleared). PROCEED to archive, but warn about the unresolved mediums.
    const mediumWarnings = f.medium
    if (mediumWarnings.length)
      log(
        'WARN ' + plan.slug + ': proceeding to archive with ' + mediumWarnings.length +
          ' unresolved INTRODUCED medium(s) after ' + MAX_REVIEW_FIX_ROUNDS + ' fix round(s): ' + mediumWarnings.join('; '),
      )

    // 4. Archive the plan file on the branch.
    const arch = await agent(archivePrompt(plan, worktreePath, plan.path), {
      schema: ARCHIVE_SCHEMA, agentType: 'general-purpose', label: 'archive:' + plan.slug, phase: 'Review', model: MID,
    })
    const archived = !!(arch && arch.archived)
    if (!archived) problems.push('archive: ' + (arch ? arch.problems : 'agent returned null'))

    return planResult(plan, branch, worktreePath, {
      green: true,
      completed: archived,
      landedUnits: landedCommits,
      commits: landedShas,
      tasksTotal: coverage.tasksTotal,
      tasksAccounted: coverage.tasksAccounted,
      reviewWarnings: mediumWarnings,
      reviewDeferred: f.deferred,
      summary:
        ((review && review.assessment) || 'implemented ' + coverage.tasksAccounted + '/' + coverage.tasksTotal + ' task(s)') +
        (mediumWarnings.length ? ' [proceeded past ' + mediumWarnings.length + ' unresolved medium warning(s)]' : ''),
      problems: (problems || []).filter(Boolean),
      reason: archived ? 'completed + archived' : 'implemented; archive failed',
    })
  } catch (e) {
    return planResult(plan, branchGuess, WORKTREES_DIR + '/plan-' + plan.slug, {
      summary: 'exception during implementation',
      problems: 'exception: ' + ((e && e.message) || String(e)),
      reason: 'exception',
    })
  }
}

// ---------------------------------------------------------------------------
// Orchestration — single plan, no waves, no parallelism.
// ---------------------------------------------------------------------------
phase('Preflight')
const pf = await agent(preflightPrompt(), { schema: PREFLIGHT_SCHEMA, label: 'preflight', model: MID })
if (!pf || !pf.ok) {
  log('Preflight FAILED, aborting: ' + (pf ? pf.reason : 'preflight agent returned null'))
  return { aborted: true, reason: pf ? pf.reason : 'preflight null' }
}
log('Preflight OK — clean tree on ' + BASE_BRANCH + ', toolchain present.')

// Validate the single plan input (fail-closed: must be a top-level plan path).
const pathErr = planPathError(PLAN)
if (pathErr) {
  log('Plan validation FAILED, aborting: ' + pathErr)
  return { aborted: true, reason: pathErr }
}

// Normalize the ref (drop a directory's trailing slash) and remember its shape. `absPath` resolves to the directory
// for a dir plan (agents then read README.md + task-N-*.md under it) and to the file for a single-file plan.
const planNorm = PLAN.replace(/\\/g, '/').replace(/\/+$/, '')
const plan = { slug: toSlug(PLAN), path: planNorm, absPath: REPO_DIR + '/' + planNorm, isDir: isPlanDirRef(planNorm) }
if (MODE === 'dry-run') log('MODE = dry-run: the plan will be implemented and gated per unit but NOT fix-forwarded or archived.')
if (LOCAL) log('MODE = local: implementing against the LOCAL ' + BASE_BRANCH + ' as the trunk on branch plan/' + plan.slug + ' — NO fetch/rebase/push; review + archive commit onto the branch; ' + BASE_BRANCH + ' is left untouched.')
if (plan.isDir) log('Plan is a DIRECTORY (' + plan.path + '): agents read README.md + task-N-*.md under it.')
log('Implementing plan ' + plan.slug + ' (' + plan.path + ') via the batched self-rotating engine.')

phase('Implement')
const result = await implementPlan(plan)

// Where the landed work lives, for the final report: LOCAL keeps it on the plan branch (nothing pushed); merge puts it
// on the remote trunk. (dry-run has its own branch already.)
const landedWhere = LOCAL ? 'branch ' + result.branch : REMOTE + '/' + BASE_BRANCH
if (result.completed && result.reason === 'completed + archived') {
  log('DONE  ' + result.slug + ': ' + result.landedUnits + ' unit(s) on ' + landedWhere + ' — ' + result.reason)
} else if (result.green && MODE === 'dry-run') {
  log('OK    ' + result.slug + ' green (dry-run, committed on ' + result.branch + ', not pushed). worktree=' + (result.worktreePath || '?'))
} else if (result.tasksAccounted && result.tasksTotal && result.tasksAccounted === result.tasksTotal) {
  log('PARTIAL ' + result.slug + ': all task work is on ' + landedWhere + ' but ' + result.reason + '. ' + (result.problems || ''))
} else {
  log(
    'FAIL  ' + result.slug + ': ' + result.reason + '. ' + result.landedUnits + ' unit(s) landed before stopping. ' +
      'branch=' + result.branch + ' (worktree=' + (result.worktreePath || '?') + ') kept for inspection. ' + (result.problems || ''),
  )
}
if (MODE === 'dry-run') {
  log('NOTE: dry-run — work is committed on branch ' + result.branch + ' (worktree under ' + WORKTREES_DIR + '/), NOT pushed.')
} else if (LOCAL && result.landedUnits) {
  // LOCAL run: everything stayed on the plan branch; main was never touched and nothing was pushed. The user lands it.
  // NOTE the Windows constraint: `git branch -f main <branch>` is REFUSED while main is checked out in another worktree,
  // so the safe landing is a fast-forward merge FROM the main checkout (C:/dev/waml). This workflow does NOT do it.
  log(
    'NOTE: LOCAL run — the work stayed entirely on branch ' + result.branch + ' (worktree under ' + WORKTREES_DIR + '/); ' +
      'nothing was pushed and ' + BASE_BRANCH + ' was NOT moved. To land it: review branch "' + result.branch + '", then ' +
      'from the ' + BASE_BRANCH + ' checkout (' + REPO_DIR + ') run "git merge --ff-only ' + result.branch + '" (git refuses ' +
      '"git branch -f ' + BASE_BRANCH + '" while ' + BASE_BRANCH + ' is checked out there). This workflow did NOT land it for you.',
  )
} else if (result.landedUnits) {
  log(
    'NOTE: your LOCAL ' + BASE_BRANCH + ' was left untouched; ' + REMOTE + '/' + BASE_BRANCH + ' has the pushed work. Run ' +
      '"git pull --ff-only" (or "git pull --rebase" if you have local-only commits) when ready.',
  )
}

// Return the single planResult directly.
return result
