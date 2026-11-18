import { LoaderCircle, Sparkles } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useStore } from "@/lib/store";
import { Dialog, Field } from "../ui/dialog";
import { Button } from "../ui/button";

/** A longer message, with optional AI generation that stays editable until saved. */
export function MessageDialog({ title, label, value, confirm, help, generate, onSubmit, onClose }: {
  title: string; label: string; value: string; confirm: string; help?: string;
  generate?: () => Promise<string>; onSubmit: (v: string) => void; onClose: () => void;
}) {
  const [v, setV] = useState(value);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState("");
  const active = useRef(true);
  const inFlight = useRef(false);
  const editor = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    active.current = true;
    return () => { active.current = false; };
  }, []);
  const writeWithAi = async () => {
    if (!generate || inFlight.current) return;
    inFlight.current = true;
    setGenerating(true);
    setError("");
    try {
      const message = await generate();
      if (active.current) {
        if (!message.trim()) throw new Error("AI returned an empty message. Try again.");
        setV(message);
        editor.current?.focus();
      }
    } catch (e) {
      if (active.current) setError(String(e));
    } finally {
      inFlight.current = false;
      if (active.current) setGenerating(false);
    }
  };
  const submit = () => {
    if (!v.trim() || inFlight.current) return;
    onClose();
    onSubmit(v.trim());
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      width={560}
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={!v.trim() || generating} onClick={submit}>
            {confirm}
          </Button>
        </>
      }
    >
      <Field label={label}>
        <textarea
          ref={editor}
          data-dialog-initial-focus
          value={v}
          readOnly={generating}
          aria-busy={generating}
          onChange={(e) => setV(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && submit()}
          className="h-[180px] px-2 py-1.5 rounded-[4px] bg-field border border-white/10 text-fg text-[13px] outline-none focus:border-accent resize-y"
        />
      </Field>
      {generate && (
        <div className="flex flex-col gap-2 mb-3">
          <div className="flex items-center gap-3">
            <Button variant="ai" disabled={generating} onClick={() => void writeWithAi()}>
              {generating ? <LoaderCircle size={14} className="spin" /> : <Sparkles size={14} />}
              {generating ? "Writing message…" : "Write with AI"}
            </Button>
            {v !== value && <Button variant="ghost" disabled={generating} onClick={() => setV(value)}>Restore original</Button>}
          </div>
          <div className="text-[12px] text-fg2">Uses this commit's saved changes and your selected AI connection.</div>
          <span className="sr-only" role="status">{generating ? "Writing a commit message with AI." : ""}</span>
          {error && (
            <div role="alert" className="text-[12px] text-[#ef7a76] whitespace-pre-wrap">
              {error}
              {error.includes("Preferences > AI") && (
                <button className="block mt-1 text-link hover:underline" onClick={() => useStore.getState().setDialog({ kind: "prefs", section: "ai" })}>Open AI Preferences</button>
              )}
            </div>
          )}
        </div>
      )}
      {help && <div className="text-[12px] text-fg3">{help}</div>}
    </Dialog>
  );
}
