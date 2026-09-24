<script lang="ts">
  import { formatDuration, recorder } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let panel = $state<HTMLElement>();

  const segments = $derived(recorder.live.flatMap((clip) => clip.segments));

  $effect(() => {
    if (segments.length && panel) panel.scrollTop = panel.scrollHeight;
  });
</script>

{#if recorder.status}
  <section
    bind:this={panel}
    class="max-h-48 overflow-y-auto border-b border-zinc-200 px-6 py-3 dark:border-zinc-800"
    aria-label="Live transcript"
    aria-live="polite"
  >
    <p class="mb-1 text-xs font-medium tracking-wide text-zinc-500 uppercase">
      Live transcript <span class="font-normal normal-case">· rough, the full transcript follows when you stop</span>
    </p>
    {#if !whipscribe.connected}
      <p class="text-sm text-zinc-500">Connect your WhipScribe API key to see words as they're spoken.</p>
    {:else if segments.length === 0}
      <p class="text-sm text-zinc-500">Listening… words appear a few seconds after someone speaks.</p>
    {:else}
      {#each segments as segment, i (i)}
        <p class="flex gap-4 py-0.5 text-sm leading-relaxed">
          <span class="w-12 shrink-0 pt-0.5 text-right text-xs text-zinc-500 tabular-nums">
            {formatDuration(segment.start)}
          </span>
          <span>{segment.text}</span>
        </p>
      {/each}
    {/if}
  </section>
{/if}
