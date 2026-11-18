// The window state, built from slices (src/lib/store/slices): tabs, notifications, settings and the
// session (dialogs, progress, launch information).
import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import { createNoticesSlice, type NoticesSlice } from "./store/slices/notices";
import { createSessionSlice, type SessionSlice } from "./store/slices/session";
import { createSettingsSlice, type SettingsSlice } from "./store/slices/settings";
import { createTabsSlice, type TabsSlice } from "./store/slices/tabs";
import { defaultUi, newId } from "./store/ids";
import type { Tab, TabUi } from "./store/types";

export * from "./store/types";
export * from "./store/defaults";
export { newId } from "./store/ids";

export type Store = TabsSlice & NoticesSlice & SettingsSlice & SessionSlice;

export const useStore = create<Store>()((...a) => ({
  ...createTabsSlice(...a),
  ...createNoticesSlice(...a),
  ...createSettingsSlice(...a),
  ...createSessionSlice(...a),
}));

/** Bring back the saved repository tabs in one step (start-up, General > Remember tabs): names from the alias or the
 *  folder, the saved active tab showing. Nothing is read here; each tab loads its repository when first shown. */
export function restoreTabs(paths: string[], active: number) {
  const st = useStore.getState();
  const tabs: Tab[] = paths.map((path) => ({ id: newId(), kind: "repo", path, title: st.settings.aliases?.[path] || path.split(/[\\/]/).filter(Boolean).pop() || path }));
  if (tabs.length === 0) return;
  const ui = { ...st.ui };
  for (const t of tabs) ui[t.id] = defaultUi();
  // The empty "New Tab" shown before start-up finished is replaced; anything else stays after the restored tabs.
  const keep = st.tabs.filter((t) => t.kind !== "start");
  useStore.setState({ tabs: [...tabs, ...keep], ui, activeId: (tabs[active] ?? tabs[0]).id });
}

export function tabUi(id: string): TabUi {
  return useStore.getState().ui[id] ?? defaultUi();
}

export function useTabUi(id: string): TabUi {
  return useStore((s) => s.ui[id]) ?? defaultUiConst;
}
const defaultUiConst = defaultUi();

/** The whole store except the busy note, job progress and toasts. Those change several times during every
 *  action; big panels (file lists, branch lists, diffs) use this so they don't re-render for them. */
export function usePanelStore() {
  return useStore(
    useShallow((st: Store) => {
      const { busy, job, toasts, ...rest } = st;
      void busy;
      void job;
      void toasts;
      return rest;
    }),
  );
}
