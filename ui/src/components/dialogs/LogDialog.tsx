import { useQuery } from "@tanstack/react-query";
import { useRef } from "react";
import { api } from "@/lib/api";
import { Dialog } from "../ui/dialog";

export function LogDialog({ onClose }: { onClose: () => void }) {
  const snapshot = useRef<{ version?: number; entries: import("@/lib/api").LogEntry[] }>({ entries: [] });
  const q = useQuery({ queryKey: ["log"], queryFn: async () => {
    const update = await api.activityLogSnapshot(snapshot.current.version);
    if (update) snapshot.current = { version: update[0], entries: update[1] };
    return snapshot.current.entries;
  }, refetchInterval: 1000 });
  const entries = (q.data ?? []).slice().reverse();
  return (
    <Dialog open onClose={onClose} title="Activity Log" width={640}>
      {entries.length === 0 && <div className="text-fg3 text-[13px]">No Git commands have run yet in this session.</div>}
      <div className="flex flex-col gap-1.5">
        {entries.map((e, i) => (
          <div key={i} className="rounded bg-field border border-white/5 px-2.5 py-1.5">
            <div className="flex items-center gap-2 text-[12px]">
              <span className={e.ok ? "text-good" : "text-[#ef7a76]"}>{e.ok ? "✓" : "✗"}</span>
              <span className="text-fg3">{e.time}</span>
              <span className="mono text-white/85 truncate">{e.command}</span>
            </div>
            {e.output && <pre className="mono text-[11px] text-fg2 whitespace-pre-wrap mt-1 max-h-[90px] overflow-auto select-text">{e.output}</pre>}
          </div>
        ))}
      </div>
    </Dialog>
  );
}
