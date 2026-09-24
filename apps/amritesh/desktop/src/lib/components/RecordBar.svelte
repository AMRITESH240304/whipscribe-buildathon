<script lang="ts">
  import { formatDuration, recorder } from "$lib/recorder.svelte";

  const status = $derived(recorder.status);
</script>

{#if status}
  <div
    class="flex items-center gap-4 border-b border-red-200 bg-red-50 px-6 py-3 dark:border-red-900 dark:bg-red-950/40"
    role="status"
  >
    <span
      class={["size-2.5 shrink-0 rounded-full", status.paused ? "bg-zinc-400" : "animate-pulse bg-red-600"]}
      aria-hidden="true"
    ></span>
    <div class="min-w-0 flex-1">
      <p class="truncate text-sm font-medium">{status.title}</p>
      <p class="text-xs text-zinc-600 tabular-nums dark:text-zinc-400">
        {status.paused ? "Paused" : "Recording"} · {formatDuration(status.elapsedSecs)}
      </p>
    </div>
    <div class="h-1.5 w-24 overflow-hidden rounded-full bg-red-100 dark:bg-red-900/50" aria-hidden="true">
      <div
        class="h-full rounded-full bg-red-600 transition-[width] duration-150"
        style:width={`${Math.min(1, Math.sqrt(status.level)) * 100}%`}
      ></div>
    </div>
    <button class="btn" onclick={() => recorder.setPaused(!status.paused)}>
      {status.paused ? "Resume" : "Pause"}
    </button>
    <button class="btn-primary" onclick={() => recorder.stop()}>Stop</button>
  </div>
  {#if recorder.silent}
    <p class="border-b border-amber-200 bg-amber-50 px-6 py-2 text-sm text-amber-900 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200" role="alert">
      We can't hear your microphone. Check it isn't muted, and that WhipScribe Recorder is
      allowed in System Settings → Privacy & Security → Microphone.
    </p>
  {/if}
{:else if recorder.error}
  <p class="flex items-center justify-between gap-4 border-b border-red-200 bg-red-50 px-6 py-2 text-sm text-red-800 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200" role="alert">
    Couldn't record: {recorder.error}
    <button class="btn" onclick={() => (recorder.error = "")}>Dismiss</button>
  </p>
{/if}
