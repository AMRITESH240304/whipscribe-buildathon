<script lang="ts">
  import { library } from "$lib/library.svelte";
  import type { Recording } from "$lib/recorder.svelte";

  const NEW = "__new";

  let { recording }: { recording: Recording } = $props();

  let creating = $state(false);
  let name = $state("");
  let saving = $state(false);

  const current = $derived(library.folderOf(recording) ?? "");

  async function choose(value: string) {
    if (value === NEW) {
      creating = true;
      return;
    }
    saving = true;
    await library.move(recording, value || null);
    saving = false;
  }

  async function create(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    try {
      const folderId = await library.createFolder(name.trim());
      await library.move(recording, folderId);
      creating = false;
      name = "";
    } catch (e) {
      library.error = String(e);
    }
    saving = false;
  }
</script>

{#if creating}
  <form class="flex gap-2" onsubmit={create}>
    <label for="folder-name" class="sr-only">New folder name</label>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      id="folder-name"
      bind:value={name}
      autofocus
      placeholder="Folder name"
      class="w-40 rounded-md border border-zinc-300 bg-transparent px-2 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-blue-600 dark:border-zinc-700"
    />
    <button class="btn-primary" disabled={!name.trim() || saving}>Create</button>
    <button type="button" class="btn" onclick={() => (creating = false)}>Cancel</button>
  </form>
{:else}
  <label for="folder" class="sr-only">Folder</label>
  <select
    id="folder"
    value={current}
    disabled={saving}
    onchange={(e) => choose(e.currentTarget.value)}
    class="rounded-md border border-zinc-300 bg-transparent px-2 py-1.5 text-sm disabled:opacity-50 dark:border-zinc-700"
  >
    <option value="">No folder</option>
    {#each library.folders as folder (folder.folder_id)}
      <option value={folder.folder_id}>{folder.name}</option>
    {/each}
    <option value={NEW}>New folder…</option>
  </select>
{/if}
