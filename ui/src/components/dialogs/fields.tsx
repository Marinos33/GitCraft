import type { ReactNode } from "react";
import { api } from "@/lib/api";
import { Field, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { Cloud, Github, Gitlab } from "lucide-react";

export function FolderField({ label, value, onChange, title }: { label: string; value: string; onChange: (v: string) => void; title: string }) {
  return (
    <Field label={label}>
      <div className="flex gap-2">
        <input className={cn(inputClass, "flex-1")} value={value} onChange={(e) => onChange(e.target.value)} placeholder="C:\\Users\\you\\Projects" />
        <Button
          onClick={async (e) => {
            e.preventDefault();
            const p = await api.pickFolder(title);
            if (p) onChange(p);
          }}
        >
          Browse
        </Button>
      </div>
    </Field>
  );
}

export function SourceList({ items, active, onPick }: { items: { id: string; label: string; icon: ReactNode; disabled?: boolean; title?: string }[]; active: string; onPick: (id: string) => void }) {
  return (
    <div role="listbox" aria-label="Source">
      {items.map((i) => (
        <button
          key={i.id}
          role="option"
          aria-selected={active === i.id}
          disabled={i.disabled}
          title={i.title}
          onClick={() => onPick(i.id)}
          className={cn("w-full flex items-center gap-2 h-[31px] px-3 text-[13px] text-left disabled:opacity-40", active === i.id ? "bg-selrow text-white" : "text-fg hover:bg-hoverrow")}
        >
          <span className="w-[16px] flex justify-center text-white/80">{i.icon}</span>
          <span className="truncate">{i.label}</span>
        </button>
      ))}
    </div>
  );
}

/** A form row with the label on the left, right-aligned, like the Initialize / Clone dialogs. */
export function FormRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[130px_1fr] items-center gap-x-3 mb-3">
      <span className="text-right text-[12.5px] text-fg leading-tight">{label}</span>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

export function FolderRow({ label, value, onChange, title }: { label: string; value: string; onChange: (v: string) => void; title: string }) {
  return (
    <FormRow label={label}>
      <div className="flex gap-2">
        <input className={cn(inputClass, "flex-1 min-w-0")} value={value} onChange={(e) => onChange(e.target.value)} placeholder="C:\\Users\\you\\Projects" />
        <Button
          onClick={async (e) => {
            e.preventDefault();
            const p = await api.pickFolder(title);
            if (p) onChange(p);
          }}
        >
          Browse
        </Button>
      </div>
    </FormRow>
  );
}

export function Check({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex items-center gap-2 text-[13px] text-fg py-1 cursor-pointer">
      <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} className="accent-[#80B1EA]" />
      {label}
    </label>
  );
}

export const SERVICE_SOURCES: { id: string; label: string; icon: ReactNode }[] = [
  { id: "github", label: "GitHub.com", icon: <Github size={14} /> },
  { id: "github-enterprise", label: "GitHub Enterprise Server", icon: <Github size={14} /> },
  { id: "gitlab", label: "GitLab.com", icon: <Gitlab size={14} /> },
  { id: "gitlab-self", label: "GitLab (Self-Managed)", icon: <Gitlab size={14} /> },
  { id: "bitbucket", label: "Bitbucket.org", icon: <Cloud size={14} /> },
  { id: "bitbucket-server", label: "Bitbucket Data Center", icon: <Cloud size={14} /> },
  { id: "azure", label: "Azure DevOps", icon: <Cloud size={14} /> },
];
