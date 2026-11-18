import { useShallow } from "zustand/react/shallow";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { Dialog, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { Check } from "./fields";
import { Integrations } from "./PrefsDialog";

/** Identity, credentials, SSH keys and GPG signing (Git's own settings, so other tools agree). */
export function AccountsDialog({ onClose }: { onClose: () => void }) {
  const s = useStore(useShallow((st) => ({ activeId: st.activeId, tabs: st.tabs, toast: st.toast })));
  const tab = s.tabs.find((t) => t.id === s.activeId);
  const path = tab?.kind === "repo" ? tab.path! : null;
  const q = useQuery({ queryKey: ["accounts", path], queryFn: ({ signal }) => api.accountInfo(path, signal) });
  const a = q.data;
  const [edits, setEdits] = useState<Record<string, string>>({});
  const val = (k: string, fallback: string) => edits[k] ?? fallback;
  const save = async (scope: "global" | "local", key: string, value: string) => {
    try {
      await api.setGitConfig(path, scope, key, value);
      s.toast(true, `Saved ${key}`);
      q.refetch();
    } catch (e) {
      s.toast(false, String(e));
    }
  };
  const row = (label: string, key: string, scope: "global" | "local", value: string, placeholder = "") => (
    <div className="flex items-center gap-2 mb-2">
      <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">{label}</span>
      <input className={cn(inputClass, "flex-1")} value={val(scope + key, value)} placeholder={placeholder} onChange={(e) => setEdits({ ...edits, [scope + key]: e.target.value })} />
      <Button size="sm" disabled={edits[scope + key] === undefined} onClick={() => save(scope, key, edits[scope + key] ?? "")}>
        Save
      </Button>
    </div>
  );
  const head = (t: string) => <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">{t}</div>;
  if (!a) return <Dialog open onClose={onClose} title="Accounts" width={620}>{q.isError ? String(q.error) : "Loading…"}</Dialog>;
  return (
    <Dialog open onClose={onClose} title="Accounts" width={620}>
      <Integrations />
      {head("IDENTITY (EVERY REPOSITORY)")}
      {row("Name", "user.name", "global", a.name, "Your Name")}
      {row("Email", "user.email", "global", a.email, "you@example.com")}
      {path && head("THIS REPOSITORY ONLY (OVERRIDES THE ABOVE)")}
      {path && row("Name", "user.name", "local", a.repo_name, "(same as above)")}
      {path && row("Email", "user.email", "local", a.repo_email, "(same as above)")}

      {head("HTTPS CREDENTIALS")}
      <div className="text-[12.5px] text-fg mb-2">
        Credential helper: <span className="mono text-white">{a.credential_helper || "none"}</span>
        {a.gcm ? <span className="text-good"> · Git Credential Manager is installed</span> : <span className="text-fg3"> · Git Credential Manager not found</span>}
      </div>
      {a.gcm && a.credential_helper !== "manager" && (
        <Button size="sm" onClick={() => save("global", "credential.helper", "manager")}>
          Use Git Credential Manager (signs in to GitHub, GitLab, Bitbucket and Azure DevOps in your browser)
        </Button>
      )}

      {head("SSH KEYS")}
      <div className="text-[12px] text-fg3 mb-1">{a.ssh_dir}</div>
      {a.ssh_keys.length === 0 && <div className="text-[12.5px] text-fg3 mb-2">No SSH keys yet.</div>}
      {a.ssh_keys.map((k) => (
        <div key={k.path} className="flex items-center gap-2 mb-1.5 text-[12.5px]">
          <span className="mono text-white/85 truncate flex-1" title={k.public}>
            {k.path.split(/[\\/]/).pop()} <span className="text-fg3">({k.kind})</span>
          </span>
          <Button size="sm" onClick={() => navigator.clipboard.writeText(k.public).then(() => s.toast(true, "Public key copied — paste it into your Git host's SSH keys page"))}>
            Copy public key
          </Button>
        </div>
      ))}
      <Button
        size="sm"
        disabled={!a.ssh_keygen}
        title={a.ssh_keygen ? "Creates ~/.ssh/id_ed25519 without a passphrase" : "ssh-keygen was not found"}
        onClick={async () => {
          try {
            const pub = await api.generateSshKey(a.email || "gitcraft", "id_ed25519");
            await navigator.clipboard.writeText(pub);
            s.toast(true, "SSH key created and its public key copied");
            q.refetch();
          } catch (e) {
            s.toast(false, String(e));
          }
        }}
      >
        Generate a new SSH key
      </Button>

      {head("COMMIT SIGNING (GPG)")}
      {!a.gpg && <div className="text-[12.5px] text-fg3 mb-2">GPG was not found. Install Gpg4win (or use SSH signing) to sign commits.</div>}
      {a.gpg_keys.length > 0 && (
        <div className="flex items-center gap-2 mb-2">
          <span className="w-[150px] text-[12.5px] text-fg2 shrink-0">Signing key</span>
          <select className={cn(inputClass, "flex-1")} value={a.signing_key} onChange={(e) => save("global", "user.signingkey", e.target.value)}>
            <option value="">(none)</option>
            {a.gpg_keys.map((k) => (
              <option key={k.id} value={k.id}>
                {k.id} — {k.uid}
              </option>
            ))}
          </select>
        </div>
      )}
      {a.gpg_keys.length === 0 && row("Signing key", "user.signingkey", "global", a.signing_key, "key id")}
      <Check label="Sign every commit (commit.gpgsign)" value={a.sign_commits} onChange={(v) => save("global", "commit.gpgsign", v ? "true" : "")} />
    </Dialog>
  );
}
