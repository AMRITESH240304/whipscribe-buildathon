<script lang="ts">
  import type { Recording } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let { recording }: { recording: Recording } = $props();

  const job = $derived(whipscribe.jobs[recording.id]);
</script>

{#if job?.phase === "uploading" || job?.phase === "transcribing"}
  <div class="w-28 text-xs text-zinc-500" role="status">
    {job.phase === "uploading" ? "Uploading…" : `Transcribing ${Math.round(job.progress * 100)}%`}
    <div class="mt-1 h-1 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
      <div
        class={["h-full bg-zinc-900 dark:bg-white", job.phase === "uploading" && "w-1/3 animate-pulse"]}
        style:width={job.phase === "transcribing" ? `${job.progress * 100}%` : undefined}
      ></div>
    </div>
  </div>
{:else if job?.phase === "failed"}
  <div class="flex items-center gap-3">
    <p class="max-w-56 text-xs text-red-700 dark:text-red-400" role="alert">{job.message}</p>
    <button class="btn" onclick={() => whipscribe.transcribe(recording.id, job.resubmit)}>Try again</button>
  </div>
{:else if !recording.transcribed}
  <button class="btn-primary" onclick={() => whipscribe.transcribe(recording.id)}>Transcribe</button>
{/if}
