<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { whipscribe } from "$lib/whipscribe.svelte";

  let key = $state("");
  let saving = $state(false);
  let error = $state("");

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    saving = true;
    error = "";
    try {
      await whipscribe.connect(key);
    } catch (e) {
      error = String(e) === "offline" ? "You're offline. Connect to the internet and try again." : String(e);
    }
    saving = false;
  }
</script>

<form
  class="mb-4 rounded-lg border border-zinc-200 p-5 dark:border-zinc-800"
  onsubmit={submit}
  aria-labelledby="api-key-heading"
>
  <h3 id="api-key-heading" class="font-semibold">Connect WhipScribe to transcribe</h3>
  <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
    Paste your WhipScribe API key. It's stored in your Mac's Keychain, and transcriptions use your
    account's credit.
    <button type="button" class="underline hover:text-zinc-900 dark:hover:text-white" onclick={() => openUrl("https://whipscribe.com/apis/keys")}>
      Get an API key
    </button>
  </p>
  <label for="api-key" class="sr-only">WhipScribe API key</label>
  <div class="mt-4 flex gap-2">
    <input
      id="api-key"
      type="password"
      bind:value={key}
      placeholder="tk_…"
      autocomplete="off"
      class="min-w-0 flex-1 rounded-md border border-zinc-300 bg-transparent px-3 py-1.5 text-sm focus-visible:outline-2 focus-visible:outline-blue-600 dark:border-zinc-700"
    />
    <button class="btn-primary" disabled={!key.trim() || saving}>
      {saving ? "Checking…" : "Connect"}
    </button>
    <button type="button" class="btn" onclick={() => whipscribe.cancelConnect()}>Not now</button>
  </div>
  {#if error}
    <p class="mt-2 text-sm text-red-700 dark:text-red-400" role="alert">{error}</p>
  {/if}
</form>
