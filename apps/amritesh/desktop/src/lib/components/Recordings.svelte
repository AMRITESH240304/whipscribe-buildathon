<script lang="ts">
  import ApiKeyForm from "$lib/components/ApiKeyForm.svelte";
  import TranscribeStatus from "$lib/components/TranscribeStatus.svelte";
  import { library } from "$lib/library.svelte";
  import { formatDuration, recorder, type Recording } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let { folderId, onOpen }: { folderId: string | null; onOpen: (id: string) => void } = $props();

  const folderNames = $derived(new Map(library.folders.map((f) => [f.folder_id, f.name])));
  const title = $derived(folderId ? (folderNames.get(folderId) ?? "Folder") : "All recordings");
  const shown = $derived(
    folderId ? recorder.recordings.filter((r) => library.folderOf(r) === folderId) : recorder.recordings,
  );

  const startedAt = (ms: number) =>
    new Date(ms).toLocaleString(undefined, {
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "numeric",
      minute: "2-digit",
    });

  function folderName(recording: Recording) {
    const id = library.folderOf(recording);
    return !folderId && id ? folderNames.get(id) : undefined;
  }
</script>

<section aria-labelledby="recordings-heading">
  <h2 id="recordings-heading" class="mb-6 text-xl font-semibold">
    {title}
    {#if shown.length}<span class="ml-1 text-base font-normal text-zinc-500">{shown.length}</span>{/if}
  </h2>

  {#if whipscribe.askForKey}
    <ApiKeyForm />
  {/if}

  {#if !recorder.loaded}
    <div class="h-14 animate-pulse rounded-lg bg-zinc-100 dark:bg-zinc-900" aria-busy="true"></div>
  {:else if shown.length === 0}
    <div class="rounded-lg border border-dashed border-zinc-300 px-6 py-10 text-center dark:border-zinc-700">
      {#if folderId}
        <h3 class="font-semibold">This folder is empty</h3>
        <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
          Open a transcribed recording and pick this folder to file it here.
        </p>
      {:else}
        <h3 class="font-semibold">No recordings yet</h3>
        <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
          Press New recording, or record a meeting from Upcoming meetings when it starts.
        </p>
      {/if}
    </div>
  {:else}
    <ul class="divide-y divide-zinc-200 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800">
      {#each shown as recording (recording.id)}
        {@const inFolder = folderName(recording)}
        <li class="flex items-center gap-3 pr-4">
          <button
            class="min-w-0 flex-1 px-4 py-3 text-left hover:bg-zinc-50 dark:hover:bg-zinc-900"
            onclick={() => onOpen(recording.id)}
          >
            <p class="truncate font-medium">{recording.title}</p>
            <p class="text-xs text-zinc-500 tabular-nums">
              {startedAt(recording.startedAt)} · {formatDuration(recording.durationSecs)}
            </p>
          </button>
          {#if recording.recovered}
            <span
              class="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800 dark:bg-amber-950 dark:text-amber-300"
              title="The app closed during this recording. Everything up to that moment was saved."
            >
              Recovered
            </span>
          {/if}
          {#if inFolder}
            <span class="rounded-full bg-zinc-100 px-2 py-0.5 text-xs text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300">
              {inFolder}
            </span>
          {/if}
          <TranscribeStatus {recording} />
        </li>
      {/each}
    </ul>
  {/if}
</section>
