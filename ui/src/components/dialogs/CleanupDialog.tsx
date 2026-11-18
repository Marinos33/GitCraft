import { useShallow } from "zustand/react/shallow";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act } from "@/lib/repo";
import { fullDate } from "@/lib/utils";
import { Dialog, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";

/** "Clean Up Local Branches": filter by age, merge status and remote status, pick, delete. */
export function CleanupDialog({ onClose }: { onClose: () => void }) {
  const s = useStore(useShallow((st) => ({ activeId: st.activeId, tabs: st.tabs })));
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path! : "";
  const q = useQuery({ queryKey: ["cleanup", path], queryFn: ({ signal }) => api.cleanupList(path, signal), enabled: !!path });
  const [days, setDays] = useState("0");
  const [merge, setMerge] = useState<"any" | "merged" | "unmerged">("merged");
  const [up, setUp] = useState<"any" | "has" | "none">("any");
  const [picked, setPicked] = useState<string[]>([]);
  const [remotes, setRemotes] = useState<string[]>([]);
  const now = Date.now() / 1000;
  const list = (q.data ?? []).filter(
    (b) =>
      now - b.time >= Number(days || 0) * 86400 &&
      (merge === "any" || (merge === "merged") === b.merged) &&
      (up === "any" || (up === "has") === !!b.upstream),
  );
  const toggle = (arr: string[], x: string) => (arr.includes(x) ? arr.filter((y) => y !== x) : [...arr, x]);
  const sel = "h-7 px-1 rounded-[4px] bg-field border border-white/10 text-[12.5px] text-fg outline-none";
  const remoteCount = remotes.filter((r) => picked.some((p) => (q.data ?? []).find((b) => b.name === p)?.upstream === r)).length;
  return (
    <Dialog
      open
      onClose={onClose}
      title="Clean Up Local Branches"
      width={640}
      footer={
        <>
          {remoteCount > 0 && <span className="text-[12px] text-[#ef7a76] mr-auto self-center">Warning: {remoteCount} remote branch{remoteCount === 1 ? "" : "es"} will be deleted. This cannot be undone.</span>}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="danger"
            disabled={!picked.length}
            onClick={() => {
              onClose();
              const ups = picked.map((p) => (q.data ?? []).find((b) => b.name === p)?.upstream).filter((u): u is string => !!u && remotes.includes(u));
              void act(path, { type: "deleteBranches", names: picked, remotes: ups });
            }}
          >
            Delete {picked.length} branch{picked.length === 1 ? "" : "es"}
          </Button>
        </>
      }
    >
      <div className="flex flex-wrap gap-3 items-center text-[12.5px] text-fg2 mb-3">
        <label className="flex items-center gap-1.5">
          Older than (days):
          <input value={days} onChange={(e) => setDays(e.target.value.replace(/\D/g, ""))} className={cn(inputClass, "w-14 h-7")} />
        </label>
        <label className="flex items-center gap-1.5">
          Merge status:
          <select value={merge} onChange={(e) => setMerge(e.target.value as typeof merge)} className={sel}>
            <option value="any">Any</option>
            <option value="merged">Merged</option>
            <option value="unmerged">Not merged</option>
          </select>
        </label>
        <label className="flex items-center gap-1.5">
          Remote status:
          <select value={up} onChange={(e) => setUp(e.target.value as typeof up)} className={sel}>
            <option value="any">Any</option>
            <option value="has">Has upstream</option>
            <option value="none">Doesn't have upstream</option>
          </select>
        </label>
      </div>
      <div className="text-[12px] text-fg3 mb-1.5">{list.length === 1 ? "1 branch matching filters" : `${list.length} branches matching filters`}</div>
      <div className="max-h-[300px] overflow-auto rounded border border-white/5">
        {q.isLoading && <div className="p-2 text-fg3 text-[12px]">Loading…</div>}
        {!q.isLoading && list.length === 0 && <div className="p-3 text-fg3 text-[12.5px]">No branches match the current filters</div>}
        {list.map((b) => (
          <div key={b.name} className="flex items-center gap-2 px-2 h-[30px] border-b border-white/5 text-[12.5px]">
            <input type="checkbox" checked={picked.includes(b.name)} onChange={() => setPicked(toggle(picked, b.name))} className="accent-[#80B1EA]" />
            <span className="flex-1 truncate text-white/90">{b.name}</span>
            <span className={cn("text-[11px]", b.merged ? "text-good" : "text-fg3")}>{b.merged ? "merged" : "not merged"}</span>
            <span className="w-[90px] text-[11px] text-fg3 text-right">{fullDate(b.time).split(",")[0]}</span>
            <span className="w-[150px] truncate text-[11px] text-fg2">{b.upstream || "None"}</span>
            {b.upstream && (
              <label className="flex items-center gap-1 text-[11px] text-fg3" title={`Also delete remote branch ${b.upstream}`}>
                <input type="checkbox" checked={remotes.includes(b.upstream)} onChange={() => setRemotes(toggle(remotes, b.upstream))} className="accent-[#d9413d]" />
                remote
              </label>
            )}
          </div>
        ))}
      </div>
      <button className="mt-2 text-[12px] text-link hover:underline" onClick={() => setPicked(list.map((b) => b.name))}>
        Select all branches
      </button>
    </Dialog>
  );
}
