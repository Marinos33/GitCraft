import { useShallow } from "zustand/react/shallow";
import { useState } from "react";
import { useStore } from "@/lib/store";
import { Dialog, inputClass } from "../ui/dialog";
import { Button } from "../ui/button";
import { cn } from "@/lib/utils";
import { addProfile, removeProfile, switchProfile } from "@/lib/prefs";

/** Profiles: separate settings and Git identity, switched from the profile button. */
export function ProfilesPanel() {
  const s = useStore(useShallow((st) => ({ activeProfile: st.activeProfile, profiles: st.profiles })));
  const [name, setName] = useState("");
  const [gitName, setGitName] = useState("");
  const [email, setEmail] = useState("");
  return (
    <>
      {s.profiles.map((p) => (
        <div key={p.id} className="flex items-center gap-2 h-[34px] border-b border-white/5 text-[13px]">
          <span className={cn("flex-1 truncate", p.id === s.activeProfile ? "text-white font-semibold" : "text-fg")}>
            {p.name}
            {p.gitEmail && <span className="text-fg3 font-normal"> · {p.gitName} &lt;{p.gitEmail}&gt;</span>}
          </span>
          {p.id === s.activeProfile ? (
            <span className="text-[11.5px] text-good">current</span>
          ) : (
            <>
              <Button size="sm" onClick={() => switchProfile(p.id)}>
                Switch
              </Button>
              <Button size="sm" variant="danger" onClick={() => removeProfile(p.id)}>
                Delete
              </Button>
            </>
          )}
        </div>
      ))}
      <div className="text-[12px] font-semibold tracking-wider text-fg2 mt-4 mb-2">NEW PROFILE</div>
      <div className="flex gap-2 mb-2">
        <input className={cn(inputClass, "flex-1")} placeholder="Profile name (e.g. Work)" value={name} onChange={(e) => setName(e.target.value)} />
      </div>
      <div className="flex gap-2 mb-2">
        <input className={cn(inputClass, "flex-1")} placeholder="Git name" value={gitName} onChange={(e) => setGitName(e.target.value)} />
        <input className={cn(inputClass, "flex-1")} placeholder="Git email" value={email} onChange={(e) => setEmail(e.target.value)} />
      </div>
      <Button
        variant="primary"
        disabled={!name.trim()}
        onClick={() => {
          addProfile(name, gitName, email);
          setName("");
          setGitName("");
          setEmail("");
        }}
      >
        Add Profile
      </Button>
      <div className="text-[12px] text-fg3 mt-3">A new profile starts with a copy of the current settings. Switching sets your global Git name and email to the profile's.</div>
    </>
  );
}

export function ProfilesDialog({ onClose }: { onClose: () => void }) {
  return (
    <Dialog open onClose={onClose} title="Profiles" width={520}>
      <ProfilesPanel />
    </Dialog>
  );
}
