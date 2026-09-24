<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ApiKeyForm from "$lib/components/ApiKeyForm.svelte";
  import TranscribeStatus from "$lib/components/TranscribeStatus.svelte";
  import { library } from "$lib/library.svelte";
  import { formatDuration, recorder, type Recording } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  type Match = { id: string; start: number; text: string };

  let { onOpen }: { onOpen: (id: string, at?: number) => void } = $props();

  let query = $state("");
  let folder = $state("all");
  let matches = $state<Match[]>([]);

  const titles = $derived(new Map(recorder.recordings.map((r) => [r.id, r.title])));
  const folderNames = $derived(new Map(library.folders.map((f) => [f.folder_id, f.name])));
  const shown = $derived(
    folder === "all" ? recorder.recordings : recorder.recordings.filter((r) => library.folderOf(r) === folder),
  );

  $effect(() => {
    const q = query;
    const timer = setTimeout(async () => {
      matches = await invoke<Match[]>("search_transcripts", { query: q });
    }, 150);
    return () => clearTimeout(timer);
  });

  const startedAt = (ms: number) =>
    new Date(ms).toLocaleString(undefined, {
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "numeric",
      minute: "2-digit",
    });

  function highlight(text: string) {
    const i = text.toLowerCase().indexOf(query.trim().toLowerCase());
    const n = query.trim().length;
    return i < 0 ? [text, "", ""] : [text.slice(0, i), text.slice(i, i + n), text.slice(i + n)];
  }

  function folderName(recording: Recording) {
    const id = library.folderOf(recording);
    return id ? folderNames.get(id) : undefined;
  }
</script>

<section aria-labelledby="recordings-heading">
  <div class="mb-2 flex items-center justify-between gap-3">
    <h2 id="recordings-heading" class="text-xs font-medium tracking-wide text-zinc-500 uppercase">Recordings</h2>
    <div class="flex items-center gap-2">
      <label for="search" class="sr-only">Search transcripts</label>
      <input
        id="search"
        type="search"
        bind:value={query}
        placeholder="Search transcripts"
        class="w-48 rounded-md border border-zinc-300 bg-transparent px-3 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-blue-600 dark:border-zinc-700"
      />
      {#if library.signedIn}
        <label for="folder-filter" class="sr-only">Show folder</label>
        <select
          id="folder-filter"
          bind:value={folder}
          class="rounded-md border border-zinc-300 bg-transparent px-2 py-1.5 text-sm dark:border-zinc-700"
        >
          <option value="all">All recordings</option>
          {#each library.folders as f (f.folder_id)}
            <option value={f.folder_id}>{f.name}</option>
          {/each}
        </select>
      {:else}
        <button class="btn" onclick={() => library.signIn()} disabled={library.signingIn}>
          {library.signingIn ? "Finish in your browser…" : "Sign in for folders"}
        </button>
      {/if}
    </div>
  </div>

  {#if library.error}
    <p class="mb-3 text-sm text-red-700 dark:text-red-400" role="alert">{library.error}</p>
  {/if}
  {#if whipscribe.askForKey}
    <ApiKeyForm />
  {/if}

  {#if query.trim()}
    {#if matches.length === 0}
      <p class="rounded-lg border border-dashed border-zinc-300 px-6 py-8 text-center text-sm text-zinc-500 dark:border-zinc-700">
        Nothing said “{query.trim()}” in your transcripts.
      </p>
    {:else}
      <ul class="divide-y divide-zinc-200 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800" aria-label="Search results">
        {#each matches as match, i (i)}
          {@const [before, hit, after] = highlight(match.text)}
          <li>
            <button class="w-full px-4 py-3 text-left hover:bg-zinc-50 dark:hover:bg-zinc-900" onclick={() => onOpen(match.id, match.start)}>
              <p class="text-xs text-zinc-500 tabular-nums">{titles.get(match.id)} · {formatDuration(match.start)}</p>
              <p class="mt-0.5 text-sm">{before}<mark class="rounded bg-amber-200 px-0.5 dark:bg-amber-700/60 dark:text-white">{hit}</mark>{after}</p>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if !recorder.loaded}
    <div class="h-14 animate-pulse rounded-lg bg-zinc-100 dark:bg-zinc-900" aria-busy="true"></div>
  {:else if shown.length === 0}
    <p class="rounded-lg border border-dashed border-zinc-300 px-6 py-8 text-center text-sm text-zinc-500 dark:border-zinc-700">
      {folder === "all"
        ? "No recordings yet. Press Record, or record a meeting when it starts."
        : "No recordings in this folder. Open a recording to move it here."}
    </p>
  {:else}
    <ul class="divide-y divide-zinc-200 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800">
      {#each shown as recording (recording.id)}
        {@const inFolder = folderName(recording)}
        <li class="flex items-center gap-4 pr-4">
          <button class="min-w-0 flex-1 px-4 py-3 text-left hover:bg-zinc-50 dark:hover:bg-zinc-900" onclick={() => onOpen(recording.id)}>
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
            <span class="rounded-full bg-zinc-100 px-2 py-0.5 text-xs text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300">{inFolder}</span>
          {/if}
          <TranscribeStatus {recording} />
        </li>
      {/each}
    </ul>
  {/if}
</section>
