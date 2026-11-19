// Dialogs, the busy note and the command palette.
import type { StateCreator } from "zustand";
import type { Store } from "../../store";
import type { DialogState } from "../types";

export interface SessionSlice {
  dialog: DialogState;
  busy: string | null;
  /** Command palette: closed, the command list, or one of its pickers. */
  palette: string | null;
  setPalette: (p: string | null) => void;
  setDialog: (d: DialogState) => void;
  setBusy: (b: string | null) => void;
}

export const createSessionSlice: StateCreator<Store, [], [], SessionSlice> = (set) => {
  return {
    dialog: null,
    busy: null,
    palette: null,
    setPalette: (palette) => set({ palette }),

    setDialog: (dialog) => set({ dialog }),
    setBusy: (busy) => set({ busy }),
  };
};
