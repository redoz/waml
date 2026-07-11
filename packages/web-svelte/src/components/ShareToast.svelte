<script lang="ts">
  let { message, onClose }: { message: string; onClose: () => void } = $props();

  $effect(() => {
    const t = setTimeout(onClose, 3500);
    return () => clearTimeout(t);
  });
</script>

<div
  class="fixed bottom-4 right-4 z-50 flex items-center gap-2 rounded-xl border border-emerald-300 bg-white px-4 py-3 text-[13px] shadow-2xl"
>
  <span class="h-2 w-2 rounded-full bg-emerald-500 flex-shrink-0"></span>
  <span class="text-slate-800">{message}</span>
</div>
