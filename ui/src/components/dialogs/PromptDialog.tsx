import { useState } from "react";
import { Dialog, Field, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";

export function PromptDialog({
  title,
  label,
  value,
  confirm,
  onSubmit,
  extraLabel,
  allowEmpty = false,
  onClose,
}: {
  title: string;
  label: string;
  value: string;
  confirm: string;
  onSubmit: (v: string, extra?: string) => void;
  extraLabel?: string;
  allowEmpty?: boolean;
  onClose: () => void;
}) {
  const [v, setV] = useState(value);
  const [x, setX] = useState("");
  const submit = () => {
    if (!v.trim() && !allowEmpty && !label.includes("optional")) return;
    onClose();
    onSubmit(v.trim(), extraLabel ? x : undefined);
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={submit}>
            {confirm}
          </Button>
        </>
      }
    >
      <Field label={label}>
        <input data-dialog-initial-focus className={inputClass} value={v} onChange={(e) => setV(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
      </Field>
      {extraLabel && (
        <Field label={extraLabel}>
          <input className={inputClass} value={x} onChange={(e) => setX(e.target.value)} onKeyDown={(e) => e.key === "Enter" && submit()} />
        </Field>
      )}
    </Dialog>
  );
}
