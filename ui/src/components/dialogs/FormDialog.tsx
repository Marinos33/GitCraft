import { useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import type { FormField } from "@/lib/store";
import { Dialog, Field, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";

/** A small form: text fields (optionally with a folder Browse button) and checkboxes. */
export function FormDialog({
  title,
  fields,
  confirm,
  help,
  onSubmit,
  onClose,
  action,
}: {
  title: string;
  fields: FormField[];
  confirm: string;
  help?: string;
  onSubmit: (v: Record<string, string | boolean>) => void;
  onClose: () => void;
  action?: { label: string; run: (v: Record<string, string | boolean>) => Promise<Record<string, string | boolean>> };
}) {
  const [v, setV] = useState<Record<string, string | boolean>>(Object.fromEntries(fields.map((f) => [f.key, f.value])));
  const [running, setRunning] = useState(false);
  const submit = () => {
    onClose();
    onSubmit(v);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      width={500}
      footer={
        <>
          {action && (
            <Button
              variant="ai"
              className="mr-auto"
              disabled={running}
              onClick={async () => {
                setRunning(true);
                try {
                  setV(await action.run(v));
                } catch (e) {
                  useStore.getState().toast(false, String(e));
                } finally {
                  setRunning(false);
                }
              }}
            >
              {running ? "Working…" : action.label}
            </Button>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={submit}>
            {confirm}
          </Button>
        </>
      }
    >
      {fields.map((f, i) =>
        f.type === "checkbox" ? (
          <label key={f.key} className="flex items-center gap-2 text-[13px] text-fg mb-3 cursor-pointer">
            <input type="checkbox" checked={!!v[f.key]} onChange={(e) => setV({ ...v, [f.key]: e.target.checked })} className="accent-[#80B1EA]" />
            {f.label}
          </label>
        ) : (
          <Field key={f.key} label={f.label}>
            <div className="flex gap-2">
              <input
                data-dialog-initial-focus={i === 0 ? "" : undefined}
                className={cn(inputClass, "flex-1")}
                value={String(v[f.key] ?? "")}
                placeholder={f.placeholder}
                onChange={(e) => setV({ ...v, [f.key]: e.target.value })}
                onKeyDown={(e) => e.key === "Enter" && submit()}
              />
              {f.browse && (
                <Button
                  onClick={async (e) => {
                    e.preventDefault();
                    const p = await api.pickFolder(f.label);
                    if (p) setV({ ...v, [f.key]: p });
                  }}
                >
                  Browse
                </Button>
              )}
            </div>
          </Field>
        ),
      )}
      {help && <div className="text-[12px] text-fg3">{help}</div>}
    </Dialog>
  );
}
