import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

export interface Profile {
  name: string;
  auth_type: string;
  description: string;
  is_active: boolean;
  sessions: string[];
  color: string;
}

export interface ActiveState {
  profile: string;
  session: string;
}

interface ProfileStore {
  profiles: Profile[];
  active: ActiveState;
  loading: boolean;
  error: string | null;
  fetch: () => Promise<void>;
  switchTo: (profile: string, session: string) => Promise<void>;
  createProfile: (name: string, auth_type: string, template?: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
}

export const useProfileStore = create<ProfileStore>((set, get) => ({
  profiles: [],
  active: { profile: "", session: "" },
  loading: false,
  error: null,

  fetch: async () => {
    set({ loading: true, error: null });
    try {
      const [profiles, active] = await Promise.all([
        invoke<Profile[]>("list_profiles"),
        invoke<ActiveState>("get_active"),
      ]);
      set({ profiles, active, loading: false });
    } catch (e) {
      set({ error: String(e), loading: false });
    }
  },

  switchTo: async (profile, session) => {
    try {
      await invoke("switch_profile", { profile, session });
      set({ active: { profile, session }, error: null });
      await get().fetch();
    } catch (e) {
      set({ error: String(e) });
      // Re-sync to show actual backend state
      await get().fetch().catch(() => {});
    }
  },

  createProfile: async (name, auth_type, template) => {
    try {
      await invoke("create_profile", { name, authType: auth_type, template: template ?? null });
      set({ error: null });
      await get().fetch();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  deleteProfile: async (name) => {
    try {
      await invoke("delete_profile", { name });
      set({ error: null });
      await get().fetch();
    } catch (e) {
      set({ error: String(e) });
    }
  },
}));
