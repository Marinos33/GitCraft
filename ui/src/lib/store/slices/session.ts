// Dialogs, progress, the command palette and launch information.
import type { StateCreator } from "zustand";
import type { Store } from "../../store";
import type { JobProgress, Metrics } from "../../api";
import { DEFAULT_METRICS } from "../defaults";
import type { DialogState } from "../types";

export interface SessionSlice {
  dialog: DialogState;
  busy: string | null;
  /** Running clone / fetch / pull / push with its latest progress line. */
  job: JobProgress | null;
  /** Command palette: closed, the command list, or one of its pickers. */
  palette: string | null;
  setPalette: (p: string | null) => void;
  screenshot: boolean;
  version: string;
  metrics: Metrics;
  setDialog: (d: DialogState) => void;
  setBusy: (b: string | null) => void;
}

export const createSessionSlice: StateCreator<Store, [], [], SessionSlice> = (set) => {
  return {
    dialog: null,
    busy: null,
    job: null,
    palette: null,
    setPalette: (palette) => set({ palette }),
    screenshot: false,
    version: "0.1.0",
    metrics: DEFAULT_METRICS,

    setDialog: (dialog) => set({ dialog }),
    setBusy: (busy) => set({ busy }),
  };
};
