import { useShallow } from "zustand/react/shallow";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openHistory } from "@/lib/repo";
import { Dialog, inputClass } from "../ui/dialog";
import { cn } from "@/lib/utils";

/** Ctrl+Shift+H: search for a file to view its history / blame. */
export function FileFinder({ onClose }: { onClose: () => void }) {
  const s = useStore(useShallow((st) => ({ activeId: st.activeId, tabs: st.tabs })));
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path : undefined;
  const q = useQuery({ queryKey: ["tracked", path], queryFn: ({ signal }) => api.trackedFiles(path!, signal), enabled: !!path });
  const [text, setText] = useState("");
  const [idx, setIdx] = useState(0);
  const words = text.toLowerCase().split(/\s+/).filter(Boolean);
  const list = (q.data ?? []).filter((f) => words.every((w) => f.toLowerCase().includes(w))).slice(0, 200);
  const pick = (f: string) => {
    onClose();
    openHistory(f, "history");
  };
  return (
    <Dialog open onClose={onClose} title="Search for a file to view its history / blame" width={560}>
      {!path && <div className="text-fg3 text-[13px]">Open a repository first.</div>}
      {path && (
        <>
          <input
            data-dialog-initial-focus
            aria-label="Filter files"
            className={cn(inputClass, "w-full mb-2")}
            placeholder="Type part of a file name"
            value={text}
            onChange={(e) => (setText(e.target.value), setIdx(0))}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") (e.preventDefault(), setIdx((i) => Math.min(list.length - 1, i + 1)));
              if (e.key === "ArrowUp") (e.preventDefault(), setIdx((i) => Math.max(0, i - 1)));
              if (e.key === "Enter" && list[idx]) pick(list[idx]);
            }}
          />
          <div className="max-h-[320px] overflow-auto rounded border border-white/5">
            {q.isLoading && <div className="p-2 text-fg3 text-[12px]">Loading files…</div>}
            {list.map((f, i) => (
              <button key={f} onClick={() => pick(f)} className={cn("w-full text-left px-2.5 py-1 text-[12.5px] truncate", i === idx ? "bg-accent/40 text-white" : "text-fg hover:bg-hoverrow")}>
                <span className="text-white">{f.split("/").pop()}</span> <span className="text-fg3">{f}</span>
              </button>
            ))}
            {!q.isLoading && list.length === 0 && <div className="p-2 text-fg3 text-[12px]">No matching files.</div>}
          </div>
        </>
      )}
    </Dialog>
  );
}
