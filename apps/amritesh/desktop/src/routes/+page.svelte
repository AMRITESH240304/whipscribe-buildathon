<script lang="ts">
  import { onMount } from "svelte";
  import Calendar from "$lib/components/Calendar.svelte";
  import RecordBar from "$lib/components/RecordBar.svelte";
  import RecordingView from "$lib/components/RecordingView.svelte";
  import Recordings from "$lib/components/Recordings.svelte";
  import { library } from "$lib/library.svelte";
  import { recorder } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let openId = $state<string | null>(null);
  let openAt = $state<number | undefined>();

  const open = $derived(recorder.recordings.find((r) => r.id === openId));

  onMount(async () => {
    await Promise.all([recorder.init(), whipscribe.init(), library.init()]);
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
      {#key open.id}
        <RecordingView recording={open} at={openAt} onBack={() => (openId = null)} />
      {/key}
    {:else}
      <div class="mx-auto w-full max-w-2xl space-y-10 px-6 py-6">
        <Calendar />
        <Recordings onOpen={(id, at) => ((openId = id), (openAt = at))} />
      </div>
    {/if}
  </main>
</div>
