<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { connect, disconnect, isConnected, upcomingEvents, type CalendarEvent } from "$lib/google";
  import { recorder } from "$lib/recorder.svelte";

  type View = "checking" | "signedOut" | "connecting" | "loading" | "ready" | "error";

  const REFRESH_MS = 5 * 60_000;

  let view = $state<View>("checking");
  let connected = $state(false);
  let events = $state<CalendarEvent[]>([]);
  let errorMessage = $state("");
  let now = $state(Date.now());

  const days = $derived(groupByDay(events, now));

  onMount(() => {
    start();
    const clock = setInterval(() => (now = Date.now()), 30_000);
    const poll = setInterval(() => view === "ready" && load(true), REFRESH_MS);
    return () => {
      clearInterval(clock);
      clearInterval(poll);
    };
  });

  async function start() {
    connected = await isConnected();
    if (connected) await load();
    else view = "signedOut";
  }

  async function load(silent = false) {
    if (!silent) view = "loading";
    try {
      events = await upcomingEvents();
      view = "ready";
    } catch (e) {
      if (!silent || String(e) === "not_connected") fail(e);
    }
  }

  async function onConnect() {
    view = "connecting";
    try {
      await connect();
      connected = true;
      await load();
    } catch (e) {
      fail(e);
    }
  }

  async function onDisconnect() {
    await disconnect().catch(() => {});
    connected = false;
    events = [];
    view = "signedOut";
  }

  function fail(e: unknown) {
    const message = String(e);
    if (message === "not_connected") {
      connected = false;
      view = "signedOut";
      return;
    }
    errorMessage =
      message === "offline" ? "You're offline. Check your connection and try again." : message;
    view = "error";
  }

  function groupByDay(list: CalendarEvent[], nowMs: number) {
    const today = new Date(nowMs);
    today.setHours(0, 0, 0, 0);
    const groups = new Map<string, CalendarEvent[]>();
    for (const event of list) {
      const day = event.start < today ? today : event.start;
      const key = day.toDateString();
      groups.set(key, [...(groups.get(key) ?? []), event]);
    }
    return [...groups].map(([key, items]) => ({ key, label: dayLabel(new Date(key), today), items }));
  }

  function dayLabel(day: Date, today: Date) {
    const diff = Math.round((day.getTime() - today.getTime()) / 86_400_000);
    if (diff === 0) return "Today";
    if (diff === 1) return "Tomorrow";
    return day.toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "short" });
  }

  const clockTime = (d: Date) => d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });

  function timeRange(event: CalendarEvent) {
    return event.allDay ? "All day" : `${clockTime(event.start)} – ${clockTime(event.end)}`;
  }

  function badge(event: CalendarEvent, nowMs: number) {
    if (event.allDay) return null;
    const start = event.start.getTime();
    if (start <= nowMs && nowMs < event.end.getTime()) return "Now";
    const minutes = Math.ceil((start - nowMs) / 60_000);
    return minutes > 0 && minutes <= 60 ? `In ${minutes} min` : null;
  }

  function details(event: CalendarEvent) {
    const host = event.meetingUrl ? new URL(event.meetingUrl).hostname : "";
    const provider = host.includes("meet.google")
      ? "Google Meet"
      : host.includes("zoom.us")
        ? "Zoom"
        : host.includes("teams.")
          ? "Microsoft Teams"
          : host && "Video call";
    const people =
      event.attendees > 0 && `${event.attendees} ${event.attendees === 1 ? "person" : "people"}`;
    return [provider, people].filter(Boolean).join(" · ");
  }
</script>

<section aria-labelledby="calendar-heading">
  <div class="mb-6 flex items-center justify-between">
    <h2 id="calendar-heading" class="text-xl font-semibold">Upcoming meetings</h2>
    {#if connected && view !== "connecting"}
      <div class="flex gap-2">
        <button class="btn" onclick={() => load()} disabled={view === "loading"}>Refresh</button>
        <button class="btn" onclick={onDisconnect}>Disconnect calendar</button>
      </div>
    {/if}
  </div>

  {#if view === "signedOut"}
    <div class="rounded-lg border border-dashed border-zinc-300 px-6 py-10 text-center dark:border-zinc-700">
      <h3 class="font-semibold">Connect your calendar</h3>
      <p class="mx-auto mt-1 max-w-sm text-sm text-zinc-500 dark:text-zinc-400">
        See your upcoming meetings here so you're ready to record them. We only read your events
        and never change them.
      </p>
      <button class="btn-primary mt-5" onclick={onConnect}>Connect Google Calendar</button>
    </div>
  {:else if view === "connecting"}
    <div class="rounded-lg border border-zinc-200 px-6 py-10 text-center dark:border-zinc-800" aria-live="polite">
      <div
        class="mx-auto size-6 animate-spin rounded-full border-2 border-zinc-300 border-t-zinc-900 dark:border-zinc-700 dark:border-t-white"
        aria-hidden="true"
      ></div>
      <h3 class="mt-4 font-semibold">Finish signing in with Google</h3>
      <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
        We opened your browser. This updates as soon as you're done.
      </p>
    </div>
  {:else if view === "loading" || view === "checking"}
    <div class="space-y-3" aria-busy="true" aria-label="Loading your meetings">
      {#each { length: 3 } as _}
        <div class="h-14 animate-pulse rounded-lg bg-zinc-100 dark:bg-zinc-900"></div>
      {/each}
    </div>
  {:else if view === "error"}
    <div class="rounded-lg border border-zinc-200 px-6 py-10 text-center dark:border-zinc-800" role="alert">
      <h3 class="font-semibold">Couldn't load your calendar</h3>
      <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">{errorMessage}</p>
      <button class="btn mt-4" onclick={start}>Try again</button>
    </div>
  {:else if events.length === 0}
    <p class="rounded-lg border border-dashed border-zinc-300 px-6 py-8 text-center text-sm text-zinc-500 dark:border-zinc-700">
      No meetings in the next 7 days. New events show up here automatically.
    </p>
  {:else}
    {#each days as day (day.key)}
      <h3 class="mt-4 mb-2 text-sm font-medium text-zinc-700 first:mt-0 dark:text-zinc-300">{day.label}</h3>
      <ul class="divide-y divide-zinc-200 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800">
        {#each day.items as event (event.id)}
          {@const status = badge(event, now)}
          <li class="flex items-center gap-4 px-4 py-3">
            <span class="w-32 shrink-0 text-sm text-zinc-500 tabular-nums">{timeRange(event)}</span>
            <div class="min-w-0 flex-1">
              <p class="truncate font-medium">{event.title}</p>
              {#if details(event)}
                <p class="truncate text-xs text-zinc-500">{details(event)}</p>
              {/if}
            </div>
            {#if status}
              <span
                class={[
                  "rounded-full px-2 py-0.5 text-xs font-medium",
                  status === "Now"
                    ? "bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-300"
                    : "bg-zinc-100 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300",
                ]}
              >
                {status}
              </span>
            {/if}
            {#if event.meetingUrl}
              <button class="btn" onclick={() => openUrl(event.meetingUrl!)} aria-label={`Join ${event.title}`}>
                Join
              </button>
            {/if}
            {#if status && !recorder.status}
              <button
                class="btn-primary"
                onclick={() => recorder.start(event.title)}
                aria-label={`Record ${event.title}`}
              >
                Record
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/each}
  {/if}
</section>
