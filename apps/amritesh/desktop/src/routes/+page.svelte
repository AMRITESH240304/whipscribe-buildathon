<script lang="ts">
  import { onMount } from "svelte";
  import Calendar from "$lib/components/Calendar.svelte";
  import LiveTranscript from "$lib/components/LiveTranscript.svelte";
  import RecordBar from "$lib/components/RecordBar.svelte";
  import RecordingView from "$lib/components/RecordingView.svelte";
  import Recordings from "$lib/components/Recordings.svelte";
  import SearchResults from "$lib/components/SearchResults.svelte";
  import Sidebar, { type View } from "$lib/components/Sidebar.svelte";
  import { library } from "$lib/library.svelte";
  import { recorder } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let view = $state<View>({ kind: "home" });
  let query = $state("");
  let openId = $state<string | null>(null);
  let openAt = $state<number | undefined>();

  const open = $derived(recorder.recordings.find((r) => r.id === openId));

  onMount(async () => {
    await Promise.all([recorder.init(), whipscribe.init(), library.init()]);
    if (whipscribe.connected) whipscribe.resume(recorder.recordings);
  });

  function navigate(next: View) {
    view = next;
    query = "";
    openId = null;
  }

  function openRecording(id: string, at?: number) {
    openId = id;
    openAt = at;
  }
</script>

<div class="flex h-screen">
  <Sidebar {view} bind:query onNavigate={navigate} />

  <div class="flex min-w-0 flex-1 flex-col">
    <main class="flex-1 overflow-y-auto">
      {#if open}
        {#key open.id}
          <RecordingView recording={open} at={openAt} onBack={() => (openId = null)} />
        {/key}
      {:else}
        <div class="mx-auto w-full max-w-3xl px-8 py-8">
          {#if query.trim()}
            <SearchResults {query} onOpen={openRecording} />
          {:else if view.kind === "home"}
            <div class="space-y-12">
              <Calendar />
              <Recordings
                folderId={null}
                limit={5}
                onOpen={openRecording}
                onViewAll={() => navigate({ kind: "all" })}
              />
            </div>
          {:else}
            <Recordings folderId={view.kind === "folder" ? view.id : null} onOpen={openRecording} />
          {/if}
        </div>
      {/if}
    </main>
    <LiveTranscript />
    <RecordBar />
  </div>
</div>
