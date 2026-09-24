<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { formatDuration, recorder } from "$lib/recorder.svelte";

  const startedAt = (ms: number) =>
    new Date(ms).toLocaleString(undefined, {
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "numeric",
      minute: "2-digit",
    });
</script>

<section aria-labelledby="recordings-heading">
  <h2 id="recordings-heading" class="mb-2 text-xs font-medium tracking-wide text-zinc-500 uppercase">
    Recordings
  </h2>
  {#if !recorder.loaded}
    <div class="h-14 animate-pulse rounded-lg bg-zinc-100 dark:bg-zinc-900" aria-busy="true"></div>
  {:else if recorder.recordings.length === 0}
    <p class="rounded-lg border border-dashed border-zinc-300 px-6 py-8 text-center text-sm text-zinc-500 dark:border-zinc-700">
      No recordings yet. Press Record, or record a meeting when it starts.
    </p>
  {:else}
    <ul class="divide-y divide-zinc-200 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800">
      {#each recorder.recordings as recording (recording.id)}
        <li class="flex items-center gap-4 px-4 py-3">
          <div class="min-w-0 flex-1">
            <p class="truncate font-medium">{recording.title}</p>
            <p class="text-xs text-zinc-500 tabular-nums">
              {startedAt(recording.startedAt)} · {formatDuration(recording.durationSecs)}
            </p>
          </div>
          {#if recording.recovered}
            <span
              class="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-950 dark:text-amber-300"
              title="The app closed during this recording. Everything up to that moment was saved."
            >
              Recovered
            </span>
          {/if}
          <button
            class="btn"
            onclick={() => revealItemInDir(recording.path)}
            aria-label={`Show ${recording.title} in Finder`}
          >
            Show in Finder
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>
