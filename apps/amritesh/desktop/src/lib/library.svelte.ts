import { invoke } from "@tauri-apps/api/core";
import type { Recording } from "$lib/recorder.svelte";

export type Folder = { folder_id: string; name: string };
type Item = { type: string; job_id?: string; deleted_at?: number };
type Place = { folderId: string; index: number };

const call = <T>(tool: string, args: Record<string, unknown> = {}) =>
  invoke<T>("library", { tool, arguments: args });

// Folders live in the user's WhipScribe library and are managed over MCP.
class Library {
  signedIn = $state(false);
  signingIn = $state(false);
  folders = $state<Folder[]>([]);
  error = $state("");

  // job_id → the folder item that files it
  #places = $state<Record<string, Place>>({});

  async init() {
    this.signedIn = await invoke<boolean>("whipscribe_signed_in");
    if (this.signedIn) await this.load();
  }

  async signIn() {
    this.signingIn = true;
    this.error = "";
    try {
      await invoke("whipscribe_sign_in");
      this.signedIn = true;
      await this.load();
    } catch (e) {
      this.#fail(e);
    }
    this.signingIn = false;
  }

  async load() {
    try {
      const { folders } = await call<{ folders: Folder[] }>("library_list_folders");
      const places: Record<string, Place> = {};
      await Promise.all(
        folders.map(async ({ folder_id }) => {
          const { folder } = await call<{ folder: { items: Item[] } }>("library_get_folder", { folder_id });
          folder.items.forEach((item, index) => {
            if (item.type === "transcript" && item.job_id && !item.deleted_at) {
              places[item.job_id] = { folderId: folder_id, index };
            }
          });
        }),
      );
      this.folders = folders;
      this.#places = places;
      this.error = "";
    } catch (e) {
      this.#fail(e);
    }
  }

  folderOf(recording: Recording) {
    return recording.jobId ? (this.#places[recording.jobId]?.folderId ?? null) : null;
  }

  async move(recording: Recording, folderId: string | null) {
    const jobId = recording.jobId;
    if (!jobId) return;
    const current = this.#places[jobId];
    try {
      if (current) {
        await call("library_trash_item", { folder_id: current.folderId, item_index: current.index });
      }
      if (folderId) {
        const item = { type: "transcript", job_id: jobId, title: recording.title };
        await call("library_add_item", { folder_id: folderId, item });
      }
    } catch (e) {
      this.#fail(e);
    }
    await this.load();
  }

  async createFolder(name: string) {
    const { folder } = await call<{ folder: Folder }>("library_create_folder", { name });
    this.folders = [...this.folders, folder];
    return folder.folder_id;
  }

  #fail(e: unknown) {
    const message = String(e);
    if (message === "not_connected") this.signedIn = false;
    else this.error = message === "offline" ? "You're offline, so folders can't be updated right now." : message;
  }
}

export const library = new Library();
