import { launchOptions } from "@/lib/launch";
import { PreferencesScreen } from "./prefs/Preferences";
import { RepoManagementDialog } from "./RepoManagement";
import { AddRemoteDialog } from "./AddRemoteDialog";
import { useStore } from "@/lib/store";
import { Dialog } from "./ui/dialog";
import { Button } from "./ui/button";
import { AppMark } from "./Icons";
import { PromptDialog } from "./dialogs/PromptDialog";
import { OpenDialog } from "./dialogs/OpenDialog";
import { CloneDialog } from "./dialogs/CloneDialog";
import { InitDialog } from "./dialogs/InitDialog";
import { LogDialog } from "./dialogs/LogDialog";
import { ShortcutsDialog } from "./dialogs/ShortcutsDialog";
import { PlanDialog } from "./dialogs/PlanDialog";
import { FormDialog } from "./dialogs/FormDialog";
import { CleanupDialog } from "./dialogs/CleanupDialog";
import { MessageDialog } from "./dialogs/MessageDialog";
import { FileFinder } from "./dialogs/FileFinder";
import { SparseDialog } from "./dialogs/SparseDialog";
import { AddProfileDialog } from "./dialogs/AddProfileDialog";
import { ProfilesDialog } from "./dialogs/ProfilesDialog";
import { WorkspacesDialog } from "./dialogs/WorkspacesDialog";

export function Dialogs() {
  const d = useStore((s) => s.dialog);
  const close = () => useStore.getState().setDialog(null);
  if (!d) return null;
  switch (d.kind) {
    case "prompt":
      return <PromptDialog key={d.title} {...d} onClose={close} />;
    case "confirm":
      return (
        <Dialog
          open
          onClose={close}
          title={d.title}
          footer={
            <>
              <Button data-dialog-initial-focus onClick={close}>Cancel</Button>
              <Button
                variant={d.danger ? "danger" : "primary"}
                onClick={() => {
                  close();
                  d.onConfirm();
                }}
              >
                {d.confirm}
              </Button>
            </>
          }
        >
          <p className="text-[13px] text-fg leading-relaxed">{d.text}</p>
        </Dialog>
      );
    case "open":
      return <OpenDialog onClose={close} />;
    case "clone":
      return <CloneDialog onClose={close} />;
    case "addRemote":
      return <AddRemoteDialog key={d.path} path={d.path} onClose={close} />;
    case "init":
      return <InitDialog onClose={close} />;
    case "log":
      return <LogDialog onClose={close} />;
    case "shortcuts":
      return <ShortcutsDialog onClose={close} />;
    case "about":
      return (
        <Dialog open onClose={close} title="About GitCraft" width={380}>
          <div className="flex flex-col items-center gap-2 py-2">
            <AppMark size={48} />
            <div className="text-[18px] font-bold text-white">GitCraft</div>
            <div className="text-fg2 text-[12.5px]">Version {launchOptions().version} · Stage 6</div>
            <div className="text-fg3 text-[12px] text-center mt-2">A graphical Git client built with Tauri, Rust and React. Every action runs the real Git command line.</div>
          </div>
        </Dialog>
      );
    case "plan":
      return <PlanDialog onClose={close} />;
    case "prefs":
      return <PreferencesScreen onClose={close} section={d.section} />;
    case "choice":
      return (
        <Dialog
          open
          onClose={close}
          title={d.title}
          footer={
            <>
              <Button onClick={close}>Cancel</Button>
              {d.choices.map((c) => (
                <Button
                  key={c.label}
                  variant={c.variant ?? "default"}
                  onClick={() => {
                    close();
                    c.onSelect();
                  }}
                >
                  {c.label}
                </Button>
              ))}
            </>
          }
        >
          <p className="text-[13px] text-fg leading-relaxed whitespace-pre-wrap">{d.text}</p>
        </Dialog>
      );
    case "repoManagement":
      return <RepoManagementDialog onClose={close} />;
    case "info":
      return (
        <Dialog open onClose={close} title={d.title} width={560} footer={<Button variant="primary" onClick={close}>Close</Button>}>
          <div className="text-[13px] text-fg leading-relaxed whitespace-pre-wrap select-text max-h-[60vh] overflow-auto">{d.text}</div>
          <div className="text-[11.5px] text-fg3 mt-3">Generated using your selected AI connection. Review the result before using it.</div>
        </Dialog>
      );
    case "message":
      return <MessageDialog key={`${d.title}:${d.value}`} {...d} onClose={close} />;
    case "form":
      return <FormDialog key={d.title} {...d} onClose={close} />;
    case "cleanup":
      return <CleanupDialog onClose={close} />;
    case "sparse":
      return <SparseDialog onClose={close} />;
    case "accounts":
      return <PreferencesScreen onClose={close} section="integrations" />;
    case "profiles":
      return <ProfilesDialog onClose={close} />;
    case "addProfile":
      return <AddProfileDialog onClose={close} />;
    case "workspaces":
      return <WorkspacesDialog onClose={close} />;
    case "fileFinder":
      return <FileFinder onClose={close} />;
  }
}
