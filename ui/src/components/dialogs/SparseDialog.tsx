import { useShallow } from "zustand/react/shallow";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { act } from "@/lib/repo";
import { Dialog } from "../ui/dialog";
import { Button } from "../ui/button";

/** Sparse checkout: keep only some folders in the working folder. */
export function SparseDialog({ onClose }: { onClose: () => void }) {
  const s = useStore(useShallow((st) => ({ activeId: st.activeId, tabs: st.tabs })));
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path! : "";
  const q = useQuery({ queryKey: ["sparse", path], queryFn: ({ signal }) => api.sparseInfo(path, signal), enabled: !!path });
  const [text, setText] = useState<string | null>(null);
  const value = text ?? (q.data?.patterns.join("\n") || "/src/\n/docs/");
  return (
    <Dialog
      open
      onClose={onClose}
      title="Sparse Checkout"
      width={480}
      footer={
        <>
          {q.data?.enabled && (
            <Button
              variant="danger"
              onClick={() => {
                onClose();
                void act(path, { type: "sparseDisable" });
              }}
            >
              Turn Off
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            onClick={() => {
              onClose();
              void act(path, { type: "sparseSet", patterns: value.split(/\r?\n/).map((x) => x.trim()).filter(Boolean) });
            }}
          >
            {q.data?.enabled ? "Update" : "Turn On"}
          </Button>
        </>
      }
    >
      <p className="text-[12.5px] text-fg2 mb-2 leading-relaxed">Only files matching these patterns stay in the working folder (one per line, .gitignore style). Everything else is still in the repository.</p>
      <textarea
        value={value}
        onChange={(e) => setText(e.target.value)}
        className="w-full h-[140px] mono text-[12px] bg-field border border-white/10 rounded-[3px] px-2 py-1.5 text-fg outline-none focus:border-accent resize-none"
      />
    </Dialog>
  );
}
