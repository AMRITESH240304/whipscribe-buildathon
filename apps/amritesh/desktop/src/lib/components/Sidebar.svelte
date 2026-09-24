<script module lang="ts">
  export type View = { kind: "home" } | { kind: "all" } | { kind: "folder"; id: string };
</script>

<script lang="ts">
  import { library } from "$lib/library.svelte";
  import { formatDuration, recorder } from "$lib/recorder.svelte";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let {
    view,
    query = $bindable(),
    onNavigate,
  }: { view: View; query: string; onNavigate: (view: View) => void } = $props();

  let search: HTMLInputElement;
  let creating = $state(false);
  let name = $state("");

  const counts = $derived(
    recorder.recordings.reduce((acc, r) => {
      const folder = library.folderOf(r);
      if (folder) acc.set(folder, (acc.get(folder) ?? 0) + 1);
      return acc;
    }, new Map<string, number>()),
  );

  const isActive = (next: View) =>
    !query.trim() && next.kind === view.kind && (next.kind !== "folder" || view.kind !== "folder" || next.id === view.id);

  async function createFolder(event: SubmitEvent) {
    event.preventDefault();
    try {
      const id = await library.createFolder(name.trim());
      creating = false;
      name = "";
      onNavigate({ kind: "folder", id });
    } catch (e) {
      library.error = String(e);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "f" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      search.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#snippet navItem(label: string, target: View, count?: number)}
  <button
    class={[
      "flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm hover:bg-zinc-200/70 dark:hover:bg-zinc-800",
      isActive(target) && "bg-zinc-200 font-medium dark:bg-zinc-800",
    ]}
    aria-current={isActive(target) ? "page" : undefined}
    onclick={() => onNavigate(target)}
  >
    <span class="truncate">{label}</span>
    {#if count}
      <span class="text-xs text-zinc-500 tabular-nums">{count}</span>
    {/if}
  </button>
{/snippet}

<nav
  class="flex w-60 shrink-0 flex-col gap-4 border-r border-zinc-200 bg-zinc-50 p-3 dark:border-zinc-800 dark:bg-zinc-900/60"
  aria-label="Sidebar"
>
  <p class="px-2 pt-1 text-sm font-semibold">WhipScribe Recorder</p>

  {#if recorder.status}
    <button
      class="btn w-full border-red-300 bg-red-50 text-red-700 hover:bg-red-100 dark:border-red-900 dark:bg-red-950/40 dark:text-red-300"
      onclick={() => whipscribe.stopAndTranscribe()}
    >
      <span class="size-2 rounded-sm bg-red-600" aria-hidden="true"></span>
      Stop · <span class="tabular-nums">{formatDuration(recorder.status.elapsedSecs)}</span>
    </button>
  {:else}
    <button class="btn-primary w-full py-2" onclick={() => recorder.start()}>
      <span class="size-2 rounded-full bg-red-500" aria-hidden="true"></span>
      New recording
    </button>
  {/if}

  <div class="relative">
    <label for="search" class="sr-only">Search transcripts</label>
    <input
      id="search"
      type="search"
      bind:this={search}
      bind:value={query}
      placeholder="Search transcripts"
      class="w-full rounded-md border border-zinc-300 bg-white px-3 py-1.5 pr-9 text-sm focus-visible:outline-2 focus-visible:outline-blue-600 dark:border-zinc-700 dark:bg-zinc-950"
    />
    {#if !query}
      <kbd class="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-xs text-zinc-400">⌘F</kbd>
    {/if}
  </div>

  <div class="space-y-0.5">
    {@render navItem("Home", { kind: "home" })}
    {@render navItem("All recordings", { kind: "all" }, recorder.recordings.length)}
  </div>

  <div class="flex min-h-0 flex-1 flex-col">
    <div class="flex items-center justify-between px-2 pb-1">
      <h2 class="text-xs font-medium tracking-wide text-zinc-500 uppercase">Folders</h2>
      {#if library.signedIn}
        <button
          class="rounded px-1.5 text-lg leading-none text-zinc-500 hover:bg-zinc-200 hover:text-zinc-900 dark:hover:bg-zinc-800 dark:hover:text-white"
          onclick={() => (creating = true)}
          aria-label="New folder"
          title="New folder"
        >
          +
        </button>
      {/if}
    </div>

    {#if !library.signedIn}
      <p class="px-2 text-xs text-zinc-500">Sign in to WhipScribe to sort recordings into folders.</p>
      <button class="btn mt-2 w-full" onclick={() => library.signIn()} disabled={library.signingIn}>
        {library.signingIn ? "Finish in your browser…" : "Sign in to WhipScribe"}
      </button>
    {:else}
      <div class="min-h-0 flex-1 space-y-0.5 overflow-y-auto">
        {#if creating}
          <form onsubmit={createFolder}>
            <label for="new-folder" class="sr-only">New folder name</label>
            <!-- svelte-ignore a11y_autofocus -->
            <input
              id="new-folder"
              bind:value={name}
              autofocus
              placeholder="Folder name"
              onkeydown={(e) => e.key === "Escape" && (creating = false)}
              onblur={() => !name.trim() && (creating = false)}
              class="w-full rounded-md border border-zinc-300 bg-white px-2 py-1 text-sm focus-visible:outline-2 focus-visible:outline-blue-600 dark:border-zinc-700 dark:bg-zinc-950"
            />
          </form>
        {/if}
        {#each library.folders as folder (folder.folder_id)}
          {@render navItem(folder.name, { kind: "folder", id: folder.folder_id }, counts.get(folder.folder_id))}
        {:else}
          {#if !creating}
            <p class="px-2 text-xs text-zinc-500">No folders yet. Press + to make one.</p>
          {/if}
        {/each}
      </div>
    {/if}
    {#if library.error}
      <p class="mt-2 px-2 text-xs text-red-700 dark:text-red-400" role="alert">{library.error}</p>
    {/if}
  </div>
</nav>
