// Saved preferences and profiles.
import type { StateCreator } from "zustand";
import type { Store } from "../../store";
import { DEFAULT_SETTINGS } from "../defaults";
import type { Profile, Settings } from "../types";

export interface SettingsSlice {
  settings: Settings;
  profiles: Profile[];
  activeProfile: string;
  setSettings: (patch: Partial<Settings>) => void;
}

export const createSettingsSlice: StateCreator<Store, [], [], SettingsSlice> = (set) => {
  return {
    settings: DEFAULT_SETTINGS,
    profiles: [{ id: "default", name: "Default Profile", gitName: "", gitEmail: "", settings: null }],
    activeProfile: "default",
    setSettings: (patch) => set((s) => ({ settings: { ...s.settings, ...patch } })),
  };
};
