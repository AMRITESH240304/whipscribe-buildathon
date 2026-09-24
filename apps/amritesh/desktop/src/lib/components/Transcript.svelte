<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { formatDuration, type Recording } from "$lib/recorder.svelte";
  import type { Segment, Transcript } from "$lib/whipscribe.svelte";

  let { recording, onBack }: { recording: Recording; onBack: () => void } = $props();

  let transcript = $state<Transcript | null>(null);
  let error = $state("");
  let copied = $state(false);

  // Speaker labels only appear when the API returns them.
  const blocks = $derived(
    (transcript?.segments ?? []).reduce<{ speaker: string | null; segments: Segment[] }[]>((acc, s) => {
      const last = acc.at(-1);
      if (last && last.speaker === s.speaker) last.segments.push(s);
      else acc.push({ speaker: s.speaker, segments: [s] });
      return acc;
    }, []),
  );

  onMount(load);

  async function load() {
    error = "";
    try {
      transcript = await invoke<Transcript>("transcript", { id: recording.id });
    } catch (e) {
      error = String(e) === "offline" ? "You're offline. This transcript hasn't been saved yet." : String(e);
    }
  }

  async function copy() {
    await navigator.clipboard.writeText(transcript?.text ?? "");
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  const speakerName = (speaker: string) => `Speaker ${Number(speaker.replace(/\D/g, "")) + 1}`;
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onBack()} />

<div class="mx-auto w-full max-w-2xl px-6 py-6">
  <div class="mb-6 flex items-start gap-4">
    <button class="btn" onclick={onBack} aria-label="Back to recordings">← Back</button>
    <div class="min-w-0 flex-1">
      <h2 class="truncate text-lg font-semibold">{recording.title}</h2>
      <p class="text-sm text-zinc-500 tabular-nums">
        {new Date(recording.startedAt).toLocaleString()} · {formatDuration(recording.durationSecs)}
      </p>
    </div>
    {#if transcript?.text}
      <button class="btn" onclick={copy}>{copied ? "Copied" : "Copy text"}</button>
    {/if}
  </div>

  {#if error}
    <div class="rounded-lg border border-zinc-200 px-6 py-10 text-center dark:border-zinc-800" role="alert">
      <h3 class="font-semibold">Couldn't open the transcript</h3>
      <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">{error}</p>
      <button class="btn mt-4" onclick={load}>Try again</button>
    </div>
  {:else if !transcript}
    <div class="space-y-3" aria-busy="true" aria-label="Loading transcript">
      {#each { length: 5 } as _}
        <div class="h-5 animate-pulse rounded bg-zinc-100 dark:bg-zinc-900"></div>
      {/each}
    </div>
  {:else if transcript.speech_detected === false || transcript.segments.length === 0}
    <div class="rounded-lg border border-dashed border-zinc-300 px-6 py-10 text-center dark:border-zinc-700">
      <h3 class="font-semibold">No speech in this recording</h3>
      <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
        {transcript.suggestion ?? "WhipScribe didn't find anything to transcribe."}
      </p>
    </div>
  {:else}
    <article class="space-y-6">
      {#each blocks as block, i (i)}
        <section>
          {#if block.speaker}
            <h3 class="mb-1 text-sm font-semibold">{speakerName(block.speaker)}</h3>
          {/if}
          {#each block.segments as segment}
            <p class="flex gap-4 py-0.5 leading-relaxed">
              <span class="w-12 shrink-0 pt-0.5 text-right text-xs text-zinc-500 tabular-nums">
                {formatDuration(segment.start)}
              </span>
              <span class="select-text">{segment.text}</span>
            </p>
          {/each}
        </section>
      {/each}
    </article>
  {/if}
</div>
