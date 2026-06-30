# Task 3 Report: Enable / Account Sheet Panels

## Status
COMPLETE. tsc clean. 211/211 tests pass. 1 local commit.

## Commit
`da33b31` — feat(web): Enable + Account sheet panels (re-host auth)

## Files created
- `packages/web/src/components/rail/EnablePanel.tsx` — signed-out auth panel
- `packages/web/src/components/rail/AccountPanel.tsx` — signed-in account panel
- `packages/web/src/components/rail/EnablePanel.test.tsx` — 3 tests (exact from brief)

## Files modified
- `packages/web/src/components/canvas/Canvas.tsx` — imports, handleEnable, handleRailOpen, ModelSheet children, TopBar wiring
- `packages/web/src/components/TopBar.tsx` — optional `onEnable` prop; "Enable" button (signed-out) / avatar-to-panel wiring (signed-in)

## Implementation decisions

### EnablePanel
- Intro copy is verbatim as specified (apostrophes escaped as entities in JSX).
- Perk rows (`Saves`, `Version history`) are `<div>` elements with no role/cursor, satisfying "not clickable" requirement.
- No "Named sharing" perk (test verifies this).
- Google/GitHub SVGs copied verbatim from `AccountDialog.tsx` (no new auth logic).
- Email input + "Send magic link" button call `onEmail(email.trim())` on click and Enter key.
- Legal note at 12px muted (`text-slate-400`), both links `target="_blank" rel="noreferrer"`, hrefs exactly as specified.

### AccountPanel
- Props: `{ email: string; onMyModels(): void; onSignOut(): void }` — per brief interface.
- Avatar: first char of email uppercased in blue circle.
- Shows "Signed in" (provider not in props so not displayed — aligns with brief interface).
- My Models closes the panel then opens MyModelsDialog; Sign out clears savedModelId + closes panel.

### Canvas.tsx gating
- `handleRailOpen` intercepts `models` and `history` clicks: when `!account`, redirects to `panel.open("enable")`.
- RightRail now uses `handleRailOpen` instead of bare `panel.open`.
- `handleEnable = () => panel.open(account ? "account" : "enable")` — passed as `onEnable` to TopBar.
- `signInWithGoogle`, `signInWithGitHub`, `signInWithEmail` destructured from `useAccount()`.

### TopBar.tsx
- `onEnable?: () => void` added (optional, no breaking change to existing callers).
- Signed-out path: new "Enable" button (blue-tinted) calling `onEnable`.
- Signed-in path: avatar button calls `onEnable` when provided, else falls back to dropdown.
  The old dropdown still renders when `onEnable` is not set (future-safe). Task 7 removes it.
- Old AccountDialog (`{showAccount && <AccountDialog .../>}`) remains untouched per "stays for now" instruction.

## Concerns / notes
- TopBar.tsx was not in the brief's "Modify" list, but adding the Enable control there was architecturally necessary. The change is additive (one optional prop) and backward-compatible.
- Clicking a gated rail icon (models/history while signed out) opens the Enable panel but the rail icon does NOT stay highlighted — there is no "enable" entry in the rail. Brief says "keep the clicked icon's intent", interpreted as: the enable panel opens in response to that intent (the user sees what they need to do). Fine-grained icon highlight tracking can be added in a later task if needed.
- The old AccountDialog modal is still reachable via the Save button (handleSave → setShowAccount when !account). This is intentional per "old buttons removed in Task 7".

---

# Task 3 Fix Report: Keep clicked rail icon highlighted during gate redirect

## Status
DONE. tsc clean. 213/213 tests pass. 1 local commit.

## Problem
When a signed-out user clicks "My Models" or "History", `handleRailOpen` routes to `panel.open("enable")`. Since `RightRail` computes `const on = active === it.id` and `panel.active` becomes `"enable"` (not a rail id), no icon highlights.

## Fix

### `packages/web/src/components/rail/RightRail.tsx`
- Added `highlightId?: RightPanelId | null` prop.
- Changed highlight computation to: `const on = it.id === (highlightId ?? active);` — when `highlightId` is set it takes precedence; falls back to `active` for normal (non-gated) navigation. `aria-current` follows `on`.

### `packages/web/src/components/canvas/Canvas.tsx`
- Added `const [visualRailId, setVisualRailId] = useState<RightPanelId | null>(null);` for tracking the clicked icon independently of `panel.active`.
- `handleRailOpen` now calls `setVisualRailId(id)` first (before possibly redirecting to `"enable"`).
- The auto-open-inspect `useEffect` (on node/edge selection) also calls `setVisualRailId("inspect")` so the Inspect icon stays in sync.
- Added `useEffect(() => { if (account) setVisualRailId(null); }, [account]);` — clears the highlight once the user signs in so the gated icon doesn't stay lit forever.
- `ModelSheet onClose` now includes `setVisualRailId(null)` so closing the sheet resets the highlight.
- `<RightRail>` receives `highlightId={visualRailId}`.

## Tests
Two new tests added to `src/components/rail/RightRail.test.tsx`:
- `highlights the icon from highlightId even when active is a different panel (gated redirect case)` — asserts `My Models` has `aria-current="true"` when `active="enable"` and `highlightId="models"`.
- Same case for `history`.

## Test commands run and output

```
pnpm --filter @mc/web exec vitest run src/components/rail/RightRail.test.tsx
→  ✓ src/components/rail/RightRail.test.tsx (5 tests) — all pass

pnpm --filter @mc/web exec tsc --noEmit
→ (no output — clean)

pnpm --filter @mc/web test
→ Test Files  40 passed (40)
      Tests  213 passed (213)
```
