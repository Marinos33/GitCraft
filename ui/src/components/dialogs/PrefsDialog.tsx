import { useShallow } from "zustand/react/shallow";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useStore } from "@/lib/store";
import { refresh } from "@/lib/repo";
import { Dialog, Field, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { hosting as hostingApi } from "@/lib/hosting";
import { THEMES } from "@/lib/prefs";
import { Check } from "./fields";

export function PrefsDialog({ onClose }: { onClose: () => void }) {
  const s = useStore(useShallow((st) => ({ activeId: st.activeId, setDialog: st.setDialog, setSettings: st.setSettings, settings: st.settings, tabs: st.tabs })));
  const set = s.settings;
  const tab = s.tabs.find((t) => t.id === s.activeId);
  return (
    <Dialog open onClose={onClose} title="Preferences" width={480}>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mb-2">UI CUSTOMIZATION</div>
      <Field label="Theme">
        <select className={inputClass} value={set.theme} onChange={(e) => s.setSettings({ theme: e.target.value })}>
          {THEMES.map((t) => (
            <option key={t.id} value={t.id}>
              {t.label}
            </option>
          ))}
        </select>
      </Field>
      <Check label="Automatically download GitCraft updates (from GitHub Releases)" value={set.autoUpdate} onChange={(v) => s.setSettings({ autoUpdate: v })} />
      <Check label="Enable spell checking (commit messages)" value={set.spellcheck} onChange={(v) => s.setSettings({ spellcheck: v })} />
      <Check label="Show a notice after successful actions" value={set.notifySuccess} onChange={(v) => s.setSettings({ notifySuccess: v })} />
      <Check label="Show ghost branch/tag when hovering over or selecting a commit" value={set.ghostRefs} onChange={(v) => s.setSettings({ ghostRefs: v })} />
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">GENERAL</div>
      <Check label="Show toolbar icon labels" value={set.toolbarLabels} onChange={(v) => s.setSettings({ toolbarLabels: v })} />
      <Check label="Push after each commit" value={set.pushAfterCommit} onChange={(v) => s.setSettings({ pushAfterCommit: v })} />
      <Check label="Show left panel" value={set.showLeft} onChange={(v) => s.setSettings({ showLeft: v })} />
      <Check label="Show commit details panel" value={set.showRight} onChange={(v) => s.setSettings({ showRight: v })} />
      <Field label="Default pull behaviour" className="mt-3">
        <select className={inputClass} value={set.pullMode} onChange={(e) => s.setSettings({ pullMode: e.target.value as typeof set.pullMode })}>
          <option value="ff">Pull (fast-forward if possible)</option>
          <option value="ffonly">Pull (fast-forward only)</option>
          <option value="rebase">Pull (rebase)</option>
          <option value="fetch">Fetch All</option>
        </select>
      </Field>
      <Field label="Default folder for clone and init">
        <input className={inputClass} value={set.defaultCloneFolder} onChange={(e) => s.setSettings({ defaultCloneFolder: e.target.value })} />
      </Field>
      {tab?.path && (
        <Button size="sm" onClick={() => refresh(tab.path!)}>
          Reload repository
        </Button>
      )}
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">REMOTES</div>
      <Field label="Auto-Fetch Interval (minutes between 0 and 60; 0 turns it off)">
        <input
          className={inputClass}
          value={set.autoFetchMin}
          onChange={(e) => s.setSettings({ autoFetchMin: Math.max(0, Math.min(60, Number(e.target.value.replace(/\D/g, "") || 0))) })}
        />
      </Field>
      <Check label="Auto-Prune (remove remote branches that were deleted on the server)" value={set.autoPrune} onChange={(v) => s.setSettings({ autoPrune: v })} />
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">COMMIT</div>
      <Check label="Skip git hooks by default when committing" value={set.skipHooks} onChange={(v) => s.setSettings({ skipHooks: v })} />
      <Field label="Commit Template: Summary" className="mt-2">
        <input className={inputClass} value={set.templateSummary} placeholder="e.g. feat: " onChange={(e) => s.setSettings({ templateSummary: e.target.value })} />
      </Field>
      <Field label="Commit Template: Description">
        <textarea
          className="h-[60px] px-2 py-1 rounded-[4px] bg-field border border-white/10 text-fg text-[13px] outline-none focus:border-accent resize-none"
          value={set.templateDescription}
          onChange={(e) => s.setSettings({ templateDescription: e.target.value })}
        />
      </Field>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">EXTERNAL TOOLS</div>
      <div className="flex gap-3">
        {(["diffTool", "mergeTool"] as const).map((k) => (
          <Field key={k} label={k === "diffTool" ? "External Diff Tool" : "External Merge Tool"} className="flex-1">
            <select className={inputClass} value={set[k]} onChange={(e) => s.setSettings({ [k]: e.target.value })}>
              <option value="">Git's configured tool</option>
              {["vscode", "meld", "kdiff3", "bc", "p4merge", "winmerge", "tortoisemerge", "vimdiff"].map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </Field>
        ))}
      </div>
      <Field label="Default Branch Name (for new repositories)" className="mt-2">
        <input className={inputClass} value={set.defaultBranch} onChange={(e) => s.setSettings({ defaultBranch: e.target.value })} />
      </Field>
      <div className="flex gap-2 mt-3">
        <Button size="sm" onClick={() => s.setDialog({ kind: "accounts" })}>
          Accounts: identity, credentials, SSH and GPG...
        </Button>
        <Button size="sm" onClick={() => s.setDialog({ kind: "profiles" })}>
          Profiles...
        </Button>
        <Button size="sm" onClick={() => s.setDialog({ kind: "workspaces" })}>
          Workspaces...
        </Button>
      </div>
    </Dialog>
  );
}

/** GitHub / GitLab tokens, with a shortcut to shared AI connection preferences. */
export function Integrations() {
  const s = { ...useStore(useShallow((st) => ({ setDialog: st.setDialog, setSettings: st.setSettings, toast: st.toast }))), settings: useStore(useShallow((st) => ({ gitlabHost: st.settings.gitlabHost }))) };
  const q = useQuery({ queryKey: ["hostingStatus", s.settings.gitlabHost], queryFn: hostingApi.status });
  const [vals, setVals] = useState<Record<string, string>>({});
  const save = async (name: "github-token" | "gitlab-token", value: string) => {
    try {
      await hostingApi.setSecret(name, value);
      setVals({ ...vals, [name]: "" });
      s.toast(true, value ? "Saved to the system credential store" : "Removed");
      q.refetch();
    } catch (e) {
      s.toast(false, String(e));
    }
  };
  const row = (name: "github-token" | "gitlab-token", label: string, connected: string | null | boolean, error: string | null, help: string) => (
    <div className="mb-3">
      <div className="flex items-center gap-2">
        <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">{label}</span>
        <input
          type="password"
          className={cn(inputClass, "flex-1")}
          placeholder={connected ? "•••••••• (saved)" : "paste token"}
          value={vals[name] ?? ""}
          onChange={(e) => setVals({ ...vals, [name]: e.target.value })}
        />
        <Button size="sm" disabled={!vals[name]} onClick={() => save(name, vals[name] ?? "")}>
          Save
        </Button>
        {connected && (
          <Button size="sm" variant="danger" onClick={() => save(name, "")}>
            Remove
          </Button>
        )}
      </div>
      <div className="text-[11.5px] ml-[158px] mt-0.5">
        {error ? <span className="text-[#ef7a76]">{error}</span> : connected ? <span className="text-good">{typeof connected === "string" ? `Connected as ${connected}` : "Saved"}</span> : <span className="text-fg3">{help}</span>}
      </div>
    </div>
  );
  const st = q.data;
  return (
    <>
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-1 mb-2">INTEGRATIONS</div>
      {row("github-token", "GitHub token", st?.github ?? null, st?.github_error ?? null, "A personal access token with repo access (github.com › Settings › Developer settings).")}
      {row("gitlab-token", "GitLab token", st?.gitlab ?? null, st?.gitlab_error ?? null, "A personal access token with the api scope.")}
      <div className="flex items-center gap-2 mb-3">
        <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">Self-managed GitLab</span>
        <input className={cn(inputClass, "flex-1")} placeholder="gitlab.example.com (optional)" value={s.settings.gitlabHost} onChange={(e) => s.setSettings({ gitlabHost: e.target.value.trim() })} />
      </div>
      <Button size="sm" onClick={() => s.setDialog({ kind: "prefs", section: "ai" })}>Configure local or hosted AI</Button>
      <div className="text-[11.5px] text-fg3 mb-2">Tokens and keys are stored in the system's credential store, not in GitCraft's settings file.</div>
    </>
  );
}
