import { X } from "lucide-react";
import { useState } from "react";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";

export function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const drop = useStore((s) => s.dropToast);
  // UI Customization > Notification Location (default: bottom left).
  const where = useStore((s) => s.settings.notificationLocation) ?? "bottom-left";
  const pos = { "top-left": "left-3 top-[100px]", "top-right": "right-3 top-[100px]", "bottom-left": "left-3 bottom-9", "bottom-right": "right-3 bottom-9" }[where];
  return (
    <div className={cn("fixed z-[70] flex flex-col gap-2 w-[300px] pointer-events-none", pos, where.startsWith("top") && "flex-col-reverse")}>
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn("toast-in pointer-events-auto rounded-md px-3 py-2 text-[12.5px] shadow-xl border bg-[var(--menu-bg)] whitespace-pre-wrap break-words max-h-[160px] overflow-auto", t.ok ? "border-good/60" : "border-bad/80")}
        >
          <div className="flex items-start gap-2">
            <div className="flex-1 min-w-0" role={t.ok ? "status" : "alert"}>
              <span aria-hidden="true" className={t.ok ? "text-good" : "text-[#ef7a76]"}>{t.ok ? "✓ " : "✗ "}</span>
              {t.text}
            </div>
            <button type="button" className="shrink-0 text-fg2 hover:text-white" aria-label="Dismiss notification" onClick={() => drop(t.id)}>
              <X size={14} />
            </button>
          </div>
          {!t.ok && <CopyErrorButton text={t.text} />}
        </div>
      ))}
    </div>
  );
}

/** "Copy error message to clipboard" on an error notice. */
function CopyErrorButton({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <button
      className="block mt-1 text-[11.5px] text-link hover:underline"
      title="Copy error message to clipboard"
      onClick={() => {
        navigator.clipboard.writeText(text).then(() => setDone(true), () => undefined);
      }}
    >
      {done ? "Copied" : "Copy error message"}
    </button>
  );
}
