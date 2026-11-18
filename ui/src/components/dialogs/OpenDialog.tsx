import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { useStore } from "@/lib/store";
import { openRepoPath } from "@/lib/repo";
import { Dialog } from "../ui/dialog";
import { Button } from "../ui/button";
import { FolderField } from "./fields";

export function OpenDialog({ onClose }: { onClose: () => void }) {
  const [p, setP] = useState("");
  const go = async () => {
    if (!p.trim()) return;
    onClose();
    await openRepoPath(p.trim());
  };
  useEffect(() => {
    // Go straight to the system folder picker.
    api.pickFolder("Open a Git repository").then(async (r) => {
      if (r) {
        onClose();
        await openRepoPath(r);
      }
    }).catch((error) => useStore.getState().toast(false, String(error)));
  }, []);
  return (
    <Dialog
      open
      onClose={onClose}
      title="Open a repository"
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" onClick={go}>
            Open
          </Button>
        </>
      }
    >
      <FolderField label="Repository folder" value={p} onChange={setP} title="Open a Git repository" />
    </Dialog>
  );
}
