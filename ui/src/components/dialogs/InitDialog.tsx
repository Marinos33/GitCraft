import { useShallow } from "zustand/react/shallow";
import { useQuery } from "@tanstack/react-query";
import { Laptop } from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath } from "@/lib/repo";
import { Dialog, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { FolderRow, FormRow, SERVICE_SOURCES, SourceList } from "./fields";

export function InitDialog({ onClose }: { onClose: () => void }) {
  const s = { ...useStore(useShallow((st) => ({ toast: st.toast }))), settings: useStore(useShallow((st) => ({ defaultBranch: st.settings.defaultBranch, defaultCloneFolder: st.settings.defaultCloneFolder }))) };
  const [name, setName] = useState("");
  const [dir, setDir] = useState(s.settings.defaultCloneFolder);
  const [error, setError] = useState("");
  const [branch, setBranch] = useState(s.settings.defaultBranch);
  const [ignore, setIgnore] = useState("");
  const [license, setLicense] = useState("");
  const [readme, setReadme] = useState(false);
  const [lfs, setLfs] = useState(false);
  const templates = useQuery({ queryKey: ["gitignoreTemplates"], queryFn: api.gitignoreTemplates, staleTime: Infinity });
  const go = async () => {
    if (!name.trim() || !dir.trim()) return;
    try {
      const result = await api.initRepo(dir, name, { branch: branch || s.settings.defaultBranch, gitignore: ignore ? [ignore] : [], license, readme, lfs });
      onClose();
      await openRepoPath(result.path);
      s.toast(!result.warning, result.warning ?? "Repository created");
    } catch (e) {
      setError(String(e));
    }
  };
  const selectClass = cn(inputClass, "w-full");
  return (
    <Dialog
      open
      onClose={onClose}
      title="Initialize a Repository"
      width={780}
      sidebar={
        <SourceList
          active="local"
          onPick={() => undefined}
          items={[
            { id: "local", label: "Local Only", icon: <Laptop size={14} /> },
            ...SERVICE_SOURCES.map((x) => ({ ...x, disabled: true, title: "Creating a repository on a service isn't available yet. Initialize it here, then add the remote." })),
          ]}
        />
      }
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={go}>
            Create Repository
          </Button>
        </>
      }
    >
      <div className="text-[15px] text-white mb-3">Initialize a Repo</div>
      <FormRow label="Name">
        <input data-dialog-initial-focus className={inputClass + " w-full"} value={name} onChange={(e) => setName(e.target.value)} placeholder="my-project" />
      </FormRow>
      <FolderRow label="Initialize in" value={dir} onChange={setDir} title="Choose a parent folder" />
      <FormRow label="Full path">
        <span className="text-[12.5px] text-fg2 break-all">
          {dir.replace(/[\\/]$/, "")}\{name.trim()}
        </span>
      </FormRow>
      <FormRow label="Default branch name">
        <input className={inputClass + " w-full"} value={branch} onChange={(e) => setBranch(e.target.value)} placeholder="main" />
      </FormRow>
      <FormRow label=".gitignore Template (optional)">
        <select aria-label=".gitignore template" className={selectClass} value={ignore} onChange={(e) => setIgnore(e.target.value)}>
          <option value="">Select...</option>
          {(templates.data ?? []).map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </FormRow>
      <FormRow label="License (optional)">
        <select aria-label="License" className={selectClass} value={license} onChange={(e) => setLicense(e.target.value)}>
          <option value="">Select...</option>
          <option value="MIT">MIT</option>
        </select>
      </FormRow>
      <FormRow label="Initialize with LFS">
        <input type="checkbox" className="accent-[#80B1EA]" checked={lfs} onChange={(e) => setLfs(e.target.checked)} aria-label="Initialize with LFS" />
      </FormRow>
      <FormRow label="Add a README">
        <input type="checkbox" className="accent-[#80B1EA]" checked={readme} onChange={(e) => setReadme(e.target.checked)} aria-label="Add a README" />
      </FormRow>
      {error && <div className="text-[12px] text-[#ef7a76] mt-2 whitespace-pre-wrap">{error}</div>}
    </Dialog>
  );
}
