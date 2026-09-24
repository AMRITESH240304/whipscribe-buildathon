<script lang="ts">
  import { onMount } from "svelte";
  import Calendar from "$lib/components/Calendar.svelte";
  import RecordBar from "$lib/components/RecordBar.svelte";
  import Recordings from "$lib/components/Recordings.svelte";
  import Transcript from "$lib/components/Transcript.svelte";
  import { recorder, type Recording } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let open = $state<Recording | null>(null);

  onMount(async () => {
    await Promise.all([recorder.init(), whipscribe.init()]);
    if (whipscribe.connected) whipscribe.resume(recorder.recordings);
  });
</script>

<div class="flex h-screen flex-col">
  <header class="flex items-center justify-between border-b border-zinc-200 px-6 py-3 dark:border-zinc-800">
    <h1 class="text-sm font-semibold">WhipScribe Recorder</h1>
    {#if !recorder.status}
      <button class="btn-primary" onclick={() => recorder.start("Untitled recording")}>
        <span class="size-2 rounded-full bg-red-500" aria-hidden="true"></span>
        Record
      </button>
    {/if}
  </header>

  <RecordBar />

  <main class="flex-1 overflow-y-auto">
    {#if open}
      <Transcript recording={open} onBack={() => (open = null)} />
    {:else}
      <div class="mx-auto w-full max-w-2xl space-y-10 px-6 py-6">
        <Calendar />
        <Recordings onOpen={(recording) => (open = recording)} />
      </div>
    {/if}
  </main>
</div>
