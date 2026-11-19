import { useShallow } from "zustand/react/shallow";
import { useState } from "react";
import { useStore } from "@/lib/store";
import { Dialog, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { deleteWorkspace, openWorkspace, saveWorkspace } from "@/lib/prefs";

/** Local workspaces: named groups of repositories you open together. */
export function WorkspacesDialog({ onClose }: { onClose: () => void }) {
  const s = { ...useStore(useShallow((st) => ({ tabs: st.tabs }))), settings: useStore(useShallow((st) => ({ recents: st.settings.recents, workspaces: st.settings.workspaces }))) };
  const [name, setName] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const candidates = [...new Set([...s.tabs.filter((t) => t.kind === "repo").map((t) => t.path!), ...s.settings.recents])];
  return (
    <Dialog open onClose={onClose} title="Workspaces" width={560}>
      {s.settings.workspaces.length === 0 && <div className="text-[12.5px] text-fg3 mb-2">No workspaces yet.</div>}
      {s.settings.workspaces.map((w) => (
        <div key={w.id} className="flex items-center gap-2 h-[34px] border-b border-white/5 text-[13px]">
          <span className="flex-1 truncate text-fg">
            {w.name} <span className="text-fg3">· {w.repos.length} repositories</span>
          </span>
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              onClose();
              void openWorkspace(w.id);
            }}
          >
            Open all
          </Button>
          <Button size="sm" variant="danger" onClick={() => deleteWorkspace(w.id)}>
            Delete
          </Button>
        </div>
      ))}
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">NEW WORKSPACE</div>
      <input className={cn(inputClass, "w-full mb-2")} placeholder="Workspace name" value={name} onChange={(e) => setName(e.target.value)} />
      <div className="max-h-[180px] overflow-auto rounded border border-white/5 mb-2">
        {candidates.length === 0 && <div className="p-2 text-[12px] text-fg3">Open some repositories first.</div>}
        {candidates.map((p) => (
          <label key={p} className="flex items-center gap-2 px-2 h-[26px] text-[12.5px] text-fg cursor-pointer hover:bg-hoverrow">
            <input type="checkbox" className="accent-[#80B1EA]" checked={picked.includes(p)} onChange={() => setPicked(picked.includes(p) ? picked.filter((x) => x !== p) : [...picked, p])} />
            <span className="truncate">{p}</span>
          </label>
        ))}
      </div>
      <Button
        variant="primary"
        disabled={!name.trim() || picked.length === 0}
        onClick={() => {
          saveWorkspace(name, picked);
          setName("");
          setPicked([]);
        }}
      >
        Create Workspace
      </Button>
    </Dialog>
  );
}
