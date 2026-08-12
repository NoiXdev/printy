import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { FolderEvent, JobEvent, QueueEvent } from "./types";

export function onJobEvent(cb: (e: JobEvent) => void): Promise<UnlistenFn> {
  return listen<JobEvent>("printy://job", (e) => cb(e.payload));
}

export function onFolderEvent(cb: (e: FolderEvent) => void): Promise<UnlistenFn> {
  return listen<FolderEvent>("printy://folder", (e) => cb(e.payload));
}

export function onQueueEvent(cb: (e: QueueEvent) => void): Promise<UnlistenFn> {
  return listen<QueueEvent>("printy://queue", (e) => cb(e.payload));
}
