import { useShallow } from "zustand/react/shallow";
import { useState } from "react";
import { useStore } from "@/lib/store";
import { Dialog, inputClass } from "../ui/dialog";
import { cn, MOD } from "@/lib/utils";
import { COMMANDS, comboOf, keysOf, type Group } from "@/lib/commands";

const SHORTCUTS: [string, string][] = [
  [`${MOD}+T`, "Open a new tab"],
  [`${MOD}+W`, "Close tab"],
  [`${MOD}+Tab / ${MOD}+Shift+Tab`, "Jump to the next / previous tab"],
  [`${MOD}+Shift+T`, "Reopen closed tab"],
  [`${MOD}+O`, "Open repository"],
  [`${MOD}+N`, "Clone repository"],
  [`${MOD}+I`, "Init repository"],
  [`${MOD}+B`, "Create branch"],
  [`${MOD}+F`, "Search commits"],
  [`${MOD}+L`, "Fetch all"],
  [`${MOD}+Z / ${MOD}+Y`, "Undo / Redo"],
  [`${MOD}+Shift+S`, "Stage all files"],
  [`${MOD}+Shift+U`, "Unstage all files"],
  [`${MOD}+Shift+M`, "Focus commit message"],
  [`${MOD}+Shift+H`, "Search for a file to view its history / blame"],
  [`${MOD}+S`, "Save the file being edited"],
  [`${MOD}+Click / Shift+Click`, "Select several commits (2 = compare)"],
  ["S / U", "Stage / unstage the open file"],
  [`${MOD}+Enter`, "Commit (in message box)"],
  [`${MOD}+Shift+Enter`, "Stage all + commit (in message box)"],
  [`${MOD}+J`, "Toggle left panel"],
  [`${MOD}+K`, "Toggle commit details panel"],
  [`${MOD}+Alt+F`, "Filter the left panel"],
  [`${MOD}+= / ${MOD}+- / ${MOD}+0`, "Zoom in / out / reset"],
  ["↑ ↓ or K J", "Select previous / next commit"],
  ["Esc", "Close view"],
  ["F11", "Toggle full screen"],
];

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const s = { ...useStore(useShallow((st) => ({ setSettings: st.setSettings }))), settings: useStore(useShallow((st) => ({ keybindings: st.settings.keybindings }))) };
  const [filter, setFilter] = useState("");
  const [recording, setRecording] = useState<string | null>(null);
  const groups: Group[] = ["Repo Actions", "Navigation", "Command Palette", "UI"];
  const f = filter.toLowerCase();
  const setKeys = (id: string, keys: string | undefined) => {
    const kb = { ...s.settings.keybindings };
    if (keys === undefined) delete kb[id];
    else kb[id] = keys;
    s.setSettings({ keybindings: kb });
  };
  const usedBy = (keys: string, id: string) => COMMANDS.find((c) => c.id !== id && keys && keysOf(c).toUpperCase() === keys.toUpperCase());
  return (
    <Dialog open onClose={onClose} title="Keyboard Shortcuts" width={600}>
      <input data-dialog-initial-focus aria-label="Filter shortcuts" className={cn(inputClass, "w-full mb-3")} placeholder={`Filter shortcuts (${MOD} + F)`} value={filter} onChange={(e) => setFilter(e.target.value)} />
      {groups.map((g) => {
        const list = COMMANDS.filter((c) => c.group === g && (!f || c.label.toLowerCase().includes(f) || keysOf(c).toLowerCase().includes(f)));
        if (!list.length) return null;
        return (
          <div key={g} className="mb-3">
            <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-1">{g.toUpperCase()}</div>
            {list.map((c) => {
              const keys = keysOf(c);
              const custom = s.settings.keybindings[c.id] !== undefined;
              const clash = keys ? usedBy(keys, c.id) : undefined;
              return (
                <div key={c.id} className="flex items-center gap-2 h-[30px] border-b border-white/5 text-[13px]">
                  <span className="flex-1 text-fg truncate">{c.label}</span>
                  {clash && <span className="text-[11px] text-warn" title={`Also used by ${clash.label}`}>clash</span>}
                  <button
                    className={cn("mono text-[11.5px] px-1.5 py-0.5 rounded border min-w-[90px]", recording === c.id ? "border-accent bg-accent/20 text-white" : "border-white/10 bg-white/8 text-fg hover:border-white/30")}
                    title="Click, then press the new keys (Esc cancels, Backspace removes)"
                    onClick={() => setRecording(c.id)}
                    onKeyDown={(e) => {
                      if (recording !== c.id) return;
                      e.preventDefault();
                      e.stopPropagation();
                      if (e.key === "Escape") return setRecording(null);
                      if (e.key === "Backspace") return (setKeys(c.id, ""), setRecording(null));
                      const combo = comboOf(e.nativeEvent);
                      if (combo && (/Ctrl|Alt/.test(combo) || /^F\d+$/.test(combo))) {
                        setKeys(c.id, combo);
                        setRecording(null);
                      }
                    }}
                    onBlur={() => setRecording(null)}
                  >
                    {recording === c.id ? "Press keys…" : keys || "—"}
                  </button>
                  <button className={cn("text-[11px] w-[40px]", custom ? "text-link hover:underline" : "invisible")} onClick={() => setKeys(c.id, undefined)}>
                    Reset
                  </button>
                </div>
              );
            })}
          </div>
        );
      })}
      <div className="text-[12px] text-fg3">Shortcuts need Ctrl or Alt (or an F key). Arrow keys, J / K, S / U and Esc work as before.</div>
    </Dialog>
  );
}
