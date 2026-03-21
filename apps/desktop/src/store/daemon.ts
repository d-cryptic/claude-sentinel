import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface DaemonStatus {
  running: boolean;
  active_timers: number;
}

export interface SwitchEvent {
  timestamp: string;
  from_profile: string;
  from_session: string;
  to_profile: string;
  to_session: string;
  reason: string;
  detail: string;
}

export interface SchedulerEntry {
  profile: string;
  detected_at: string;
  refill_at: string;
  time_until_refill: string;
  auto_switch_back: boolean;
  switched_back: boolean;
}

interface DaemonStore {
  status: DaemonStatus;
  switchLog: SwitchEvent[];
  schedulerEntries: SchedulerEntry[];
  fetch: () => Promise<void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
}

export const useDaemonStore = create<DaemonStore>((set) => ({
  status: { running: false, active_timers: 0 },
  switchLog: [],
  schedulerEntries: [],

  fetch: async () => {
    const [status, switchLog, schedulerEntries] = await Promise.all([
      invoke<DaemonStatus>("daemon_status"),
      invoke<SwitchEvent[]>("get_switch_log"),
      invoke<SchedulerEntry[]>("get_scheduler_state"),
    ]);
    set({ status, switchLog, schedulerEntries });
  },

  start: async () => {
    await invoke("daemon_start");
    const status = await invoke<DaemonStatus>("daemon_status");
    set({ status });
  },

  stop: async () => {
    await invoke("daemon_stop");
    const status = await invoke<DaemonStatus>("daemon_status");
    set({ status });
  },
}));
