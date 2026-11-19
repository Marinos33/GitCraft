import { useState } from "react";
import { Dialog, Field, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { PROFILE_COLORS, ProfileMark } from "../Icons";
import { cn } from "@/lib/utils";
import { addProfile } from "@/lib/prefs";

/** "Add Profile": pick a picture, name it, set the Git author, and choose whether to start from the current settings. */
export function AddProfileDialog({ onClose }: { onClose: () => void }) {
  const [icon, setIcon] = useState(0);
  const [name, setName] = useState("");
  const [gitName, setGitName] = useState("");
  const [email, setEmail] = useState("");
  const [copy, setCopy] = useState(true);
  const step = (d: number) => setIcon((i) => (i + d + PROFILE_COLORS.length) % PROFILE_COLORS.length);
  return (
    <Dialog
      open
      onClose={onClose}
      title="Add Profile"
      width={584}
      footer={
        <>
          <label className="flex items-center gap-2 text-[12.5px] text-fg mr-auto">
            <input type="checkbox" className="accent-[#80B1EA]" checked={copy} onChange={(e) => setCopy(e.target.checked)} />
            Copy settings from current profile
          </label>
          <Button onClick={onClose}>Cancel</Button>
          <Button
            variant="primary"
            disabled={!name.trim()}
            onClick={() => {
              addProfile(name, gitName, email, { icon, copySettings: copy });
              onClose();
            }}
          >
            Add Profile
          </Button>
        </>
      }
    >
      <div className="text-[12.5px] text-fg mb-2">Select a profile icon</div>
      <div className="flex items-center justify-center gap-3 mb-4">
        <button aria-label="Previous icon" className="text-fg2 hover:text-white px-1" onClick={() => step(-1)}>
          ‹
        </button>
        {PROFILE_COLORS.map((c, i) => (
          <button
            key={c}
            aria-label={`Icon ${i + 1}`}
            aria-pressed={icon === i}
            onClick={() => setIcon(i)}
            className={cn("w-[56px] h-[56px] rounded flex items-center justify-center border", icon === i ? "border-accent bg-white/10" : "border-transparent hover:bg-white/5")}
          >
            <ProfileMark color={c} size={36} />
          </button>
        ))}
        <button aria-label="Next icon" className="text-fg2 hover:text-white px-1" onClick={() => step(1)}>
          ›
        </button>
      </div>
      <div className="flex items-end gap-3 mb-3">
        <ProfileMark color={PROFILE_COLORS[icon]} size={36} />
        <label className="flex-1 flex flex-col gap-1 text-[12px] text-fg2">
          Profile Name
          <input data-dialog-initial-focus className={inputClass} value={name} onChange={(e) => setName(e.target.value)} placeholder="New Profile" />
        </label>
      </div>
      <Field label="Author Name">
        <input className={inputClass} value={gitName} onChange={(e) => setGitName(e.target.value)} />
      </Field>
      <Field label="Author Email">
        <input className={inputClass} value={email} onChange={(e) => setEmail(e.target.value)} />
      </Field>
    </Dialog>
  );
}
