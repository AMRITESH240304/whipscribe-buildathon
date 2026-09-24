import { invoke } from "@tauri-apps/api/core";

export type Status = {
  title: string;
  elapsedSecs: number;
  paused: boolean;
  micLevel: number;
  systemLevel: number;
  systemError: string | null;
};

export type Recording = {
  id: string;
  title: string;
  startedAt: number;
  durationSecs: number;
  recovered: boolean;
  jobId: string | null;
  transcribed: boolean;
  path: string;
};

const SILENCE_HINT_SECS = 5;

class Recorder {
  status = $state<Status | null>(null);
  recordings = $state<Recording[]>([]);
  loaded = $state(false);
  error = $state("");
  heardSound = $state(false);

  #poll?: ReturnType<typeof setInterval>;

  get silent() {
    return (
      !!this.status && !this.status.paused && !this.heardSound && this.status.elapsedSecs > SILENCE_HINT_SECS
    );
  }

  async init() {
    await this.refresh();
    this.status = await invoke<Status | null>("recording_status");
    if (this.status) this.#startPolling();
  }

  async refresh() {
    this.recordings = await invoke<Recording[]>("list_recordings");
    this.loaded = true;
  }

  async start(title: string) {
    this.error = "";
    try {
      await invoke("start_recording", { title });
      this.heardSound = false;
      this.#startPolling();
    } catch (e) {
      this.error = String(e);
    }
  }

  async setPaused(paused: boolean) {
    await invoke("set_paused", { paused });
    if (this.status) this.status.paused = paused;
  }

  async stop() {
    clearInterval(this.#poll);
    const id = await invoke<string>("stop_recording").catch((e) => {
      this.error = String(e);
      return null;
    });
    this.status = null;
    await this.refresh();
    return id;
  }

  #startPolling() {
    clearInterval(this.#poll);
    this.#poll = setInterval(async () => {
      this.status = await invoke<Status | null>("recording_status");
      if (this.status && this.status.micLevel > 0.01) this.heardSound = true;
    }, 200);
  }
}

export const recorder = new Recorder();

export function formatDuration(totalSecs: number) {
  const secs = Math.floor(totalSecs);
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = String(secs % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}
