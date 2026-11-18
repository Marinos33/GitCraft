// Open tabs, their per-tab view state and commit drafts.
import type { StateCreator } from "zustand";
import type { Store } from "../../store";
import { sameFolder } from "../../agents";
import { defaultUi, newId } from "../ids";
import type { Draft, Tab, TabUi } from "../types";

export interface TabsSlice {
  tabs: Tab[];
  activeId: string;
  closed: Tab[];
  ui: Record<string, TabUi>;
  drafts: Record<string, Draft>;
  renameTab: (id: string, title: string) => void;
  openRepoTab: (path: string, name: string, group?: string) => void;
  newTab: () => void;
  openLaunchpad: () => void;
  closeTab: (id: string) => void;
  reopenTab: () => void;
  setActive: (id: string) => void;
  /** Move a tab so it sits just before `beforeId` (null: at the end). Dragging tabs and Move Tab Left / Right use it. */
  moveTab: (id: string, beforeId: string | null) => void;
  replaceTab: (id: string, tab: Tab) => void;
  setUi: (id: string, patch: Partial<TabUi>) => void;
  setDraft: (path: string, patch: Partial<Draft>) => void;
}

export const createTabsSlice: StateCreator<Store, [], [], TabsSlice> = (set, get) => {
  const start: Tab = { id: newId(), kind: "start", title: "New Tab" };
  return {
    tabs: [start],
    activeId: start.id,
    closed: [],
    ui: {},
    drafts: {},
    renameTab: (id, title) => {
      const s = get();
      const t = s.tabs.find((x) => x.id === id);
      if (!t) return;
      const aliases = { ...s.settings.aliases };
      if (t.path) {
        if (title.trim()) aliases[t.path] = title.trim();
        else delete aliases[t.path];
      }
      set({
        tabs: s.tabs.map((x) => (x.id === id ? { ...x, title: title.trim() || (x.path ? x.path.split(/[\\/]/).pop()! : x.title) } : x)),
        settings: { ...s.settings, aliases },
      });
    },
    openRepoTab: (path, name, group) => {
      const s = get();
      const existing = s.tabs.find((t) => t.kind === "repo" && !!t.path && sameFolder(t.path, path));
      const recents = [path, ...s.settings.recents.filter((r) => !sameFolder(r, path))].slice(0, 20);
      if (existing) {
        set({ activeId: existing.id, settings: { ...s.settings, recents } });
        return;
      }
      const active = s.tabs.find((t) => t.id === s.activeId);
      const tab: Tab = { id: newId(), kind: "repo", path, title: s.settings.aliases?.[path] || name };
      // Opening from a "New Tab" replaces it.
      if (active && active.kind === "start") {
        set({
          tabs: s.tabs.map((t) => (t.id === active.id ? tab : t)),
          activeId: tab.id,
          ui: { ...s.ui, [tab.id]: defaultUi() },
          settings: { ...s.settings, recents },
        });
      } else {
        // UI Customization > Automatically group worktree tabs together: a linked worktree opens next to its repository.
        const norm = (p?: string) => (p ?? "").replace(/\\/g, "/");
        const at = group && s.settings.autogroupWorktreeTabs !== false ? s.tabs.map((t) => sameFolder(norm(t.path), norm(group))).lastIndexOf(true) : -1;
        const tabs = at >= 0 ? [...s.tabs.slice(0, at + 1), tab, ...s.tabs.slice(at + 1)] : [...s.tabs, tab];
        set({ tabs, activeId: tab.id, ui: { ...s.ui, [tab.id]: defaultUi() }, settings: { ...s.settings, recents } });
      }
    },
    newTab: () => {
      const tab: Tab = { id: newId(), kind: "start", title: "New Tab" };
      set((s) => ({ tabs: [...s.tabs, tab], activeId: tab.id }));
    },
    openLaunchpad: () => {
      const s = get();
      const lp = s.tabs.find((t) => t.kind === "launchpad");
      if (lp) return set({ activeId: lp.id });
      const tab: Tab = { id: newId(), kind: "launchpad", title: "Launchpad" };
      set({ tabs: [tab, ...s.tabs], activeId: tab.id });
    },
    closeTab: (id) => {
      const s = get();
      const idx = s.tabs.findIndex((t) => t.id === id);
      if (idx < 0) return;
      const tab = s.tabs[idx];
      let tabs = s.tabs.filter((t) => t.id !== id);
      if (tabs.length === 0) tabs = [{ id: newId(), kind: "start", title: "New Tab" }];
      const activeId = s.activeId === id ? tabs[Math.min(idx, tabs.length - 1)].id : s.activeId;
      const ui = { ...s.ui }; delete ui[id];
      set({ tabs, activeId, ui, closed: tab.kind === "repo" ? [...s.closed, tab].slice(-10) : s.closed });
    },
    reopenTab: () => {
      const s = get();
      const last = s.closed[s.closed.length - 1];
      if (!last) return;
      set({ closed: s.closed.slice(0, -1) });
      get().openRepoTab(last.path!, last.title);
    },
    setActive: (id) => set({ activeId: id }),
    moveTab: (id, beforeId) =>
      set((s) => {
        const tab = s.tabs.find((t) => t.id === id);
        if (!tab || id === beforeId) return {};
        const rest = s.tabs.filter((t) => t.id !== id);
        const at = beforeId ? rest.findIndex((t) => t.id === beforeId) : -1;
        const tabs = at < 0 ? [...rest, tab] : [...rest.slice(0, at), tab, ...rest.slice(at)];
        return tabs.every((t, i) => t === s.tabs[i]) ? {} : { tabs };
      }),
    replaceTab: (id, tab) => set((s) => ({ tabs: s.tabs.map((t) => (t.id === id ? tab : t)), activeId: tab.id, ui: { ...s.ui, [tab.id]: defaultUi() } })),
    setUi: (id, patch) => set((s) => ({ ui: { ...s.ui, [id]: { ...(s.ui[id] ?? defaultUi()), ...patch } } })),
    setDraft: (path, patch) =>
      set((s) => ({ drafts: { ...s.drafts, [path]: { ...(s.drafts[path] ?? { summary: "", description: "", amend: false }), ...patch } } })),
  };
};
