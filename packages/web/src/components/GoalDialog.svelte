<script lang="ts">
  // Mirrors packages/web/src/components/GoalDialog.tsx.
  import { untrack } from "svelte";
  import { CheckCircle2 } from "lucide-svelte";
  import { NICHE_PRESETS, type BusinessGoal, type NichePreset } from "@uaml/core/state/goal";

  const DISCLAIMER =
    "Capture the business objective behind this model — the niche you operate in and the " +
    "goal you're optimising for. It's saved locally in your browser alongside the model.";

  let { current, onConfirm, onClear, onClose }: {
    current: BusinessGoal | null;
    onConfirm: (g: BusinessGoal) => void;
    onClear: () => void;
    onClose: () => void;
  } = $props();

  // One-time seeds from `current` (mirrors React's useState(current) — snapshot at
  // mount, NOT reactively resynced). untrack() reads the prop in a non-tracking
  // closure, making the deliberate one-time capture explicit and keeping
  // svelte-check clean (a bare read here trips state_referenced_locally).
  const initialNiche = NICHE_PRESETS.find(n => n.label === current?.niche) ?? null;
  const seededNiche = untrack<NichePreset | { label: string } | null>(
    () => initialNiche ?? (current ? { label: current.niche } : null),
  );
  const seededGoal = untrack(() => current?.goal ?? "");
  const seededApplied = untrack(() => current);

  let niche: NichePreset | { label: string } | null = $state(seededNiche);
  let goal: string = $state(seededGoal);

  // A goal that's currently in effect — drives the green success notice. Seeded
  // from `current` so reopening with a saved goal shows the notice right away.
  let appliedGoal: BusinessGoal | null = $state(seededApplied);
  // Briefly true right after Apply to pulse the notice green and pull focus.
  let highlight = $state(false);
  let noticeEl: HTMLDivElement | undefined = $state();

  let presetGoals = $derived(niche && "goals" in niche ? niche.goals : []);
  let canApply = $derived(!!niche?.label && goal.trim().length > 0);

  function handleApply() {
    if (!canApply) return;
    const g: BusinessGoal = { niche: niche!.label, goal: goal.trim() };
    onConfirm(g);
    appliedGoal = g;
    highlight = true;
  }

  // After Apply, scroll the notice into view and move focus to it so the user
  // can see — and a screen reader announces — that the goal took effect.
  $effect(() => {
    if (!highlight || !noticeEl) return;
    noticeEl.scrollIntoView?.({ behavior: "smooth", block: "nearest" });
    noticeEl.focus();
    const t = setTimeout(() => { highlight = false; }, 2200);
    return () => clearTimeout(t);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30" onclick={onClose}>
  <div
    class="bg-white rounded-xl shadow-xl w-[460px] max-w-[92vw] max-h-[88vh] overflow-y-auto p-5"
    onclick={e => e.stopPropagation()}
    style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Inter, system-ui, sans-serif;"
  >
    <h3 class="text-[15px] font-[650] text-slate-900 mb-2">Business Goal</h3>
    <p class="text-[12px] text-slate-500 leading-[1.5] mb-4">{DISCLAIMER}</p>

    <!-- Success notice — stays visible while a goal is set. Pulses green and
         grabs focus right after Apply so the user knows it worked. -->
    {#if appliedGoal}
      <div
        bind:this={noticeEl}
        tabindex="-1"
        class={`flex items-start gap-2 rounded-lg border px-3 py-[10px] mb-4 outline-none transition-shadow ${
          highlight
            ? "border-[#10b981] bg-[#ecfdf5] ring-2 ring-[#10b981]/50"
            : "border-[#a7f3d0] bg-[#ecfdf5]"
        }`}
      >
        <CheckCircle2 size={16} class="text-[#059669] mt-[1px] flex-shrink-0" />
        <div class="text-[12px] leading-[1.5] text-[#065f46]">
          <strong class="font-semibold">Goal applied.</strong> It's saved with your model in
          this browser.
        </div>
      </div>
    {/if}

    <!-- Step 1 — niche -->
    <span class="block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">Niche</span>
    <div class="grid grid-cols-2 gap-2 mb-4">
      {#each NICHE_PRESETS as n (n.id)}
        <button
          onclick={() => { niche = n; goal = ""; }}
          class={`text-[12.5px] text-left px-3 py-2 rounded-lg border ${niche?.label === n.label ? "border-[#1e88e5] bg-[#e6f1fb] text-[#1e88e5]" : "border-[#d8dee8] text-slate-900 hover:bg-[#f1f3f7]"}`}
        >
          {n.label}
        </button>
      {/each}
    </div>

    <!-- Step 2 — goal -->
    <label for="goal-dialog-input" class="block text-[11px] font-semibold text-slate-500 uppercase tracking-[0.3px] mb-[6px]">Goal</label>
    <div class="flex flex-col gap-2 mb-3">
      {#each presetGoals as g (g)}
        <button
          onclick={() => { goal = g; }}
          class={`text-[12.5px] text-left px-3 py-2 rounded-lg border ${goal === g ? "border-[#1e88e5] bg-[#e6f1fb] text-[#1e88e5]" : "border-[#d8dee8] text-slate-900 hover:bg-[#f1f3f7]"}`}
        >
          {g}
        </button>
      {/each}
    </div>
    <input
      id="goal-dialog-input"
      type="text"
      value={goal}
      oninput={e => { goal = e.currentTarget.value; }}
      placeholder="…or type your own goal"
      class="w-full text-[13px] px-[10px] py-2 border border-[#d8dee8] rounded-lg text-slate-900 focus:outline-none focus:border-[#1e88e5] focus:ring-2 focus:ring-[#e6f1fb] mb-4"
    />

    <div class="flex items-center gap-2">
      <button
        disabled={!canApply}
        onclick={handleApply}
        class="text-[13px] font-[550] bg-[#1e88e5] text-white rounded-lg px-4 py-[8px] cursor-pointer hover:bg-[#1976d2] disabled:opacity-40 disabled:cursor-not-allowed"
      >
        Apply
      </button>
      {#if current}
        <button onclick={onClear} class="text-[13px] text-slate-500 px-3 py-[8px] rounded-lg hover:bg-[#f1f3f7]">
          Clear
        </button>
      {/if}
      <div class="flex-1"></div>
      <button onclick={onClose} class="text-[13px] text-slate-500 px-3 py-[8px] rounded-lg hover:bg-[#f1f3f7]">
        Cancel
      </button>
    </div>
  </div>
</div>
