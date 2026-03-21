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
    await invoke("switch_profile", { profile, session });
    set((s) => ({ active: { profile, session } }));
    await get().fetch();
  },

  createProfile: async (name, auth_type, template) => {
    await invoke("create_profile", { name, authType: auth_type, template: template ?? null });
    await get().fetch();
  },

  deleteProfile: async (name) => {
    await invoke("delete_profile", { name });
    await get().fetch();
  },
}));
