import { api } from "./api";
import { useStore, type Settings } from "./store";

let timer: ReturnType<typeof setTimeout> | undefined;
let writes = Promise.resolve();
let dirty = false;
function snapshot() {
  const st = useStore.getState();
  const tabs = st.tabs.filter(t => t.kind === "repo");
  return { settings: st.settings, tabs: tabs.map(t => t.path), active: Math.max(0, tabs.findIndex(t => t.id === st.activeId)), profiles: st.profiles, activeProfile: st.activeProfile };
}
export function hasUnsavedPreferences() { return dirty; }
export async function flushPersistence() {
  clearTimeout(timer);
  for (;;) {
    if (!dirty) { await writes; if (!dirty) return; }
    const value = snapshot();
    dirty = false;
    const next = writes.catch(() => undefined).then(() => api.saveSettings(value));
    writes = next.then(() => undefined);
    // The failure is reported by `await next` below; mark the chained copy as handled too.
    writes.catch(() => undefined);
    let restored: Record<string, unknown> | null | undefined;
    try { restored = await next; } catch (error) { dirty = true; throw error; }
    // Commands that were not approved keep their previous values; show those again.
    if (restored && Object.keys(restored).length) useStore.getState().setSettings(restored as Partial<Settings>);
  }
}
export function startPersistence() {
  return useStore.subscribe((st, prev) => {
    if (st.settings === prev.settings && st.tabs === prev.tabs && st.activeId === prev.activeId && st.profiles === prev.profiles && st.activeProfile === prev.activeProfile) return;
    dirty = true;
    clearTimeout(timer);
    timer = setTimeout(() => { void flushPersistence().catch(error => useStore.getState().toast(false, "Could not save preferences: " + String(error))); }, 400);
  });
}

/** Prevent duplicate close requests until the latest preference snapshot is durable. */
export function preferenceCloseGuard(close: () => Promise<void>, report: (error: unknown) => void) {
  let closing = false;
  let allowClose = false;
  return (event: { preventDefault: () => void }) => {
    if (allowClose && !hasUnsavedPreferences()) return;
    event.preventDefault();
    if (closing) return;
    closing = true;
    void flushPersistence().then(() => { closing = false; allowClose = true; return close(); }).catch(error => {
      closing = false; allowClose = false; report(error);
    });
  };
}
