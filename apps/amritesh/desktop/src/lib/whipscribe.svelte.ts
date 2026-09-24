import { invoke } from "@tauri-apps/api/core";
import { recorder, type Recording } from "$lib/recorder.svelte";

export type Segment = { start: number; end: number; speaker: string | null; text: string };

export type Transcript = {
  text: string;
  segments: Segment[];
  speech_detected?: boolean;
  suggestion?: string;
};

type JobStatus = {
  status: "queued" | "processing" | "done" | "failed";
  progress: number;
  error: string | null;
  locked: boolean;
};

export type Job =
  | { phase: "uploading" }
  | { phase: "transcribing"; progress: number }
  | { phase: "failed"; message: string; resubmit: boolean };

const POLL_MS = 3000;
const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

class WhipScribe {
  connected = $state(false);
  askForKey = $state(false);
  jobs = $state<Record<string, Job>>({});

  #pending: string | null = null;

  async init() {
    this.connected = await invoke<boolean>("whipscribe_status");
  }

  async connect(key: string) {
    await invoke("whipscribe_connect", { key });
    this.connected = true;
    this.askForKey = false;
    if (this.#pending) this.transcribe(this.#pending);
    this.#pending = null;
  }

  cancelConnect() {
    this.askForKey = false;
    this.#pending = null;
  }

  async transcribe(id: string, resubmit = false) {
    if (!this.connected) {
      this.#pending = id;
      this.askForKey = true;
      return;
    }
    this.jobs[id] = { phase: "uploading" };
    try {
      const jobId = await invoke<string>("transcribe", { id, retry: resubmit });
      await this.#poll(id, jobId);
    } catch (e) {
      this.#fail(id, e);
    }
  }

  resume(recordings: Recording[]) {
    for (const r of recordings) {
      if (r.jobId && !r.transcribed && !this.jobs[r.id]) {
        this.#poll(r.id, r.jobId).catch((e) => this.#fail(r.id, e));
      }
    }
  }

  async #poll(id: string, jobId: string) {
    this.jobs[id] = { phase: "transcribing", progress: 0 };
    while (true) {
      const status = await invoke<JobStatus>("job_status", { jobId }).catch((e) => {
        if (String(e) === "offline") return null;
        throw e;
      });
      if (status?.status === "failed") {
        return this.#fail(id, status.error ?? "WhipScribe couldn't transcribe this recording.", true);
      }
      if (status?.status === "done") {
        if (status.locked) {
          return this.#fail(id, "This transcript is locked. Add credit on whipscribe.com to unlock it.");
        }
        await invoke("transcript", { id });
        delete this.jobs[id];
        return recorder.refresh();
      }
      if (status) this.jobs[id] = { phase: "transcribing", progress: status.progress };
      await sleep(POLL_MS);
    }
  }

  #fail(id: string, e: unknown, resubmit = false) {
    const message = String(e);
    if (message === "not_connected") {
      delete this.jobs[id];
      this.connected = false;
      return this.transcribe(id);
    }
    this.jobs[id] = {
      phase: "failed",
      message: message === "offline" ? "You're offline. Try again when you're connected." : message,
      resubmit,
    };
  }
}

export const whipscribe = new WhipScribe();
