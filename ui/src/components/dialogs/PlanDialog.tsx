import { Dialog } from "../ui/dialog";
import { cn } from "@/lib/utils";

const STAGES: [string, string, string[]][] = [
  ["Stage 1", "Main window and everyday Git — done", ["Tabs, toolbar, panels, commit graph", "Stage, commit, amend, discard", "Branches, tags, stashes", "Fetch, pull, push", "Undo / redo, search, activity log"]],
  ["Stage 2", "Deeper graph and diffs (Monaco) — done", ["Hunk / inline / split diffs, line staging, edit and save", "File history and blame", "More graph columns, solo / hide branches", "Compare commits, load more history, image diffs"]],
  ["Stage 3", "Rewriting history and conflicts", ["Interactive rebase", "Merge conflict tool", "Reword, squash, drop"]],
  ["Stage 4", "Repository management and accounts", ["Remotes, worktrees, submodules", "Gitflow, LFS", "Credential Manager, SSH, GPG", "Job queue with progress"]],
  ["Stage 5", "Productivity", ["Command palette", "Built-in terminal", "Workspaces, profiles, preferences", "Themes; gix and SQLite where needed"]],
  ["Stage 6", "Online services, AI and shipping", ["Pull requests, issues, Launchpad", "Cloud patches, teams", "AI helpers, Agents view", "Installers and updates"]],
];

export function PlanDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog open onClose={onClose} title="GitCraft feature plan" width={560}>
      <div className="flex flex-col gap-3">
        {STAGES.map(([n, t, items], i) => (
          <div key={n} className={cn("rounded border px-3 py-2", i <= 1 ? "border-good/60 bg-good/10" : "border-white/10 bg-field")}>
            <div className="text-[13px] text-white font-semibold">
              {n} · {t}
            </div>
            <div className="text-[12px] text-fg2 mt-1">{items.join(" · ")}</div>
          </div>
        ))}
      </div>
    </Dialog>
  );
}
