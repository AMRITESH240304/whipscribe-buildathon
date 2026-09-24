import { invoke } from "@tauri-apps/api/core";

type RawEvent = {
  id: string;
  title: string;
  start: string;
  end: string;
  allDay: boolean;
  meetingUrl: string | null;
  attendees: number;
};

export type CalendarEvent = Omit<RawEvent, "start" | "end"> & { start: Date; end: Date };

const DAY_MS = 86_400_000;

// All-day events come as "YYYY-MM-DD"; parse them as local midnight, not UTC.
const toDate = (value: string, allDay: boolean) => new Date(allDay ? `${value}T00:00` : value);

export const isConnected = () => invoke<boolean>("google_status");
export const connect = () => invoke<void>("google_connect");
export const disconnect = () => invoke<void>("google_disconnect");

export async function upcomingEvents(days = 7): Promise<CalendarEvent[]> {
  const now = Date.now();
  const raw = await invoke<RawEvent[]>("list_events", {
    timeMin: new Date(now).toISOString(),
    timeMax: new Date(now + days * DAY_MS).toISOString(),
  });
  return raw.map((e) => ({ ...e, start: toDate(e.start, e.allDay), end: toDate(e.end, e.allDay) }));
}
