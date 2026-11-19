import { useShallow } from "zustand/react/shallow";
import { SERVICES, integrations } from "@/lib/integrations";
import { useQuery, useInfiniteQuery } from "@tanstack/react-query";
import { Globe } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath } from "@/lib/repo";
import { Dialog, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { hosting as hostingApi } from "@/lib/hosting";
import { FolderRow, FormRow, SERVICE_SOURCES, SourceList } from "./fields";

/** Where a repository can come from or go to: the left list of the Initialize / Clone dialogs. */
export function CloneDialog({ onClose }: { onClose: () => void }) {
  const s = { ...useStore(useShallow((st) => ({ setBusy: st.setBusy, setSettings: st.setSettings, toast: st.toast }))), settings: useStore(useShallow((st) => ({ defaultCloneFolder: st.settings.defaultCloneFolder }))) };
  const [source, setSource] = useState("url");
  const [url, setUrl] = useState("");
  const [dir, setDir] = useState(s.settings.defaultCloneFolder);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [shallow, setShallow] = useState(false);
  const [limitBy, setLimitBy] = useState<"depth" | "since">("depth");
  const [depth, setDepth] = useState("1");
  const [since, setSince] = useState("");
  const [branch, setBranch] = useState("");
  const [flags, setFlags] = useState("");
  const [sparse, setSparse] = useState(false);
  const [sparsePaths, setSparsePaths] = useState("");
  const [blobless, setBlobless] = useState(false);
  const sinceError = !shallow || limitBy !== "since" ? "" : !/^\d{4}-\d{2}-\d{2}$/.test(since) || Number.isNaN(Date.parse(since)) ? "Enter a valid date in 'YYYY-MM-DD' format" : new Date(since).getTime() > Date.now() ? "Date cannot be in the future" : "";
  const go = async () => {
    if (!url.trim() || !dir.trim() || sinceError) return;
    setBusy(true);
    setError("");
    s.setBusy("Cloning…");
    try {
      const path = await api.cloneRepo(url, dir, {
        depth: shallow && limitBy === "depth" ? Math.max(1, Number(depth) || 1) : undefined,
        since: shallow && limitBy === "since" ? since : undefined,
        branch: shallow ? branch.trim() || undefined : undefined,
        custom_flags: shallow ? flags.trim() || undefined : undefined,
        sparse,
        sparse_paths: sparse ? sparsePaths.split("\n").map((p) => p.trim()).filter(Boolean) : undefined,
        blobless: sparse && blobless,
      });
      s.setSettings({ defaultCloneFolder: dir });
      onClose();
      await openRepoPath(path);
      s.toast(true, "Clone finished");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
      s.setBusy(null);
    }
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title="Clone a Repository"
      width={780}
      sidebar={<SourceList active={source} onPick={setSource} items={[{ id: "url", label: "Clone with URL", icon: <Globe size={14} /> }, ...SERVICE_SOURCES]} />}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={busy} onClick={go}>
            {busy ? "Cloning…" : "Clone the repo!"}
          </Button>
        </>
      }
    >
      <div className="text-[15px] text-white mb-3">Clone a Repo</div>
      <FolderRow label="Where to clone to" value={dir} onChange={setDir} title="Choose where to clone to" />
      {source !== "url" && <HostedRepoPicker service={source} onPick={setUrl} />}
      <FormRow label="URL">
        <input data-dialog-initial-focus className={inputClass + " w-full"} value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://github.com/owner/project.git" />
      </FormRow>
      {url && dir && (
        <FormRow label="Full path">
          <span className="text-[12.5px] text-fg2 break-all">
            {dir.replace(/[\\/]$/, "")}\{url.replace(/\/$/, "").split(/[/\\:]/).pop()?.replace(/\.git$/, "")}
          </span>
        </FormRow>
      )}
      <FormRow label="Shallow Clone">
        <input type="checkbox" className="accent-[#80B1EA]" checked={shallow} onChange={(e) => setShallow(e.target.checked)} aria-label="Shallow clone" />
      </FormRow>
      {shallow && (
        <div className="ml-[142px] mb-3 border-l border-white/10 pl-3">
          <FormRow label="Limit history by">
            <select aria-label="Limit history by" className={inputClass + " w-full"} value={limitBy} onChange={(e) => setLimitBy(e.target.value as "depth" | "since")}>
              <option value="depth">Commit depth</option>
              <option value="since">Since date</option>
            </select>
          </FormRow>
          {limitBy === "depth" ? (
            <FormRow label="Depth">
              <input aria-label="Depth" value={depth} onChange={(e) => setDepth(e.target.value.replace(/\D/g, ""))} className={cn(inputClass, "w-24")} />
              <div className="text-[11px] text-fg3 mt-0.5">How many commits to clone from the target branch.</div>
            </FormRow>
          ) : (
            <FormRow label="Since date">
              <input aria-label="Since date" value={since} onChange={(e) => setSince(e.target.value)} placeholder="YYYY-MM-DD" className={cn(inputClass, "w-40")} />
              <div className={cn("text-[11px] mt-0.5", sinceError ? "text-[#ef7a76]" : "text-fg3")}>{sinceError || "Commits older than this date will not be cloned."}</div>
            </FormRow>
          )}
          <FormRow label="Branch to clone">
            <input aria-label="Branch to clone" value={branch} onChange={(e) => setBranch(e.target.value)} placeholder="default branch" className={inputClass + " w-full"} />
            <div className="text-[11px] text-fg3 mt-0.5">The default branch is usually main or master, but you can specify a different one.</div>
          </FormRow>
          <FormRow label="Custom flags">
            <input aria-label="Custom flags" value={flags} onChange={(e) => setFlags(e.target.value)} className={inputClass + " w-full mono"} />
            <div className="text-[11px] text-fg3 mt-0.5">Any additional flags for git clone, entered exactly as on the command line.</div>
          </FormRow>
        </div>
      )}
      <FormRow label="Sparse Checkout">
        <input type="checkbox" className="accent-[#80B1EA]" checked={sparse} onChange={(e) => setSparse(e.target.checked)} aria-label="Sparse checkout" />
      </FormRow>
      {sparse && (
        <div className="ml-[142px] mb-3 border-l border-white/10 pl-3">
          <FormRow label="Blobless clone">
            <input type="checkbox" className="accent-[#80B1EA]" checked={blobless} onChange={(e) => setBlobless(e.target.checked)} aria-label="Blobless clone" />
            <div className="text-[11px] text-fg3 mt-0.5">Fetches trees and commits without file contents at first, reducing clone size. Contents are fetched when needed.</div>
          </FormRow>
          <FormRow label="Paths to include">
            <textarea aria-label="Paths to include" rows={5} value={sparsePaths} onChange={(e) => setSparsePaths(e.target.value)} placeholder={"e.g.\nsrc/\ndocs/\nREADME.md"} className={cn(inputClass, "w-full h-auto py-1 mono resize-y")} />
            <div className="text-[11px] text-fg3 mt-0.5">One path per line. Only these paths are checked out; with none, only the files in the top folder.</div>
          </FormRow>
        </div>
      )}
      {error && <div className="text-[12px] text-[#ef7a76] whitespace-pre-wrap">{error}</div>}
    </Dialog>
  );
}

/** "GitCraft will help you find stuff on {service} when cloning": your repositories on connected services. */
function HostedRepoPicker({ onPick, service }: { onPick: (url: string) => void; service?: string }) {
  const s = { ...useStore(useShallow((st) => ({ setDialog: st.setDialog }))), settings: useStore(useShallow((st) => ({ integrations: st.settings.integrations }))) };
  const st = useQuery({ queryKey: ["integrationStatuses", s.settings.integrations], queryFn: () => integrations.statuses(SERVICES.filter((x) => x.kind === "hosting").map((x) => x.id)) });
  const connected = (st.data ?? []).filter((x) => x.connected && !x.error);
  const [svc, setSvc] = useState("");
  const [q, setQ] = useState("");
  const id = service ?? (svc || connected[0]?.id || "");
  const repos = useInfiniteQuery({ queryKey: ["hostedReposPages", id], initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) => hostingApi.reposPage(id, pageParam, signal), getNextPageParam: (page) => page.cursor ?? undefined, enabled: !!id });
  if (service ? !connected.some((c) => c.id === service) : !connected.length)
    return (
      <div className="text-[12px] text-fg3 mb-2">
        {service ? "This service is not connected yet. Connect it to pick from your repositories, or paste a URL below." : "Connect GitHub, GitLab, Bitbucket or Azure DevOps to pick from your repositories."}{" "}
        <button className="text-link hover:underline" onClick={() => s.setDialog({ kind: "prefs", section: "integrations" })}>
          Integrations
        </button>
      </div>
    );
  const list = [...new Map((repos.data?.pages.flatMap((page) => page.items) ?? []).map((repo) => [repo.full_name, repo])).values()].filter((r) => r.full_name.toLowerCase().includes(q.toLowerCase()));
  return (
    <div className="mb-3">
      <div className="flex gap-2 mb-1.5">
        <select hidden={!!service} className={cn(inputClass, "w-[180px]")} value={id} onChange={(e) => setSvc(e.target.value)}>
          {connected.map((c) => (
            <option key={c.id} value={c.id}>
              {c.label}
            </option>
          ))}
        </select>
        <input className={cn(inputClass, "flex-1")} placeholder="Search your repositories" value={q} onChange={(e) => setQ(e.target.value)} />
      </div>
      <div className="h-[150px] overflow-auto rounded border border-white/8 bg-white/[0.02]">
        {repos.isLoading && <div className="p-2 text-[12px] text-fg3">Loading…</div>}
        {repos.error && <div className="p-2 text-[12px] text-[#ef7a76]">{String(repos.error)} <button onClick={() => repos.isFetchNextPageError ? repos.fetchNextPage() : repos.refetch()}>Retry</button></div>}
        {list.map((r) => (
          <button key={r.full_name} className="w-full flex items-center gap-2 px-2 h-[26px] text-left text-[12.5px] hover:bg-hoverrow" title={r.description} onClick={() => onPick(r.https_url || r.ssh_url)}>
            <span className="truncate text-fg flex-1">{r.full_name}</span>
            {r.private && <span className="text-[11px] text-fg3">private</span>}
          </button>
        ))}
        {repos.hasNextPage && <button className="w-full text-link text-[12px] py-1" disabled={repos.isFetchingNextPage} onClick={() => repos.fetchNextPage()}>{repos.isFetchingNextPage ? "Loading more…" : "Load more repositories"}</button>}
        {!list.length && !repos.isLoading && !repos.isError && <div className="p-2 text-fg3 text-[12px]">No matches in loaded repositories.{repos.hasNextPage ? " Load more to search the next page, or paste a repository URL." : ""}</div>}
      </div>
    </div>
  );
}
