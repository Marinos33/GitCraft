# Testing GitCraft across desktop platforms

Use automated tests on each OS, then test the installed app in a real desktop session. The release workflow builds packages for all three platforms; the test workflow runs the suites on each OS. Neither replaces interactive validation.

## Ubuntu on a Windows development machine

For quick Linux development, use a separate Ubuntu 24.04 WSL 2 distribution with WSLg. From an administrator PowerShell terminal:

```powershell
wsl --install -d Ubuntu-24.04
```

Complete Ubuntu's first-run account setup. Follow Microsoft's [WSL GUI setup](https://learn.microsoft.com/en-us/windows/wsl/tutorials/gui-apps) if Linux windows do not open. Keep the checkout inside the Linux filesystem, such as `~/src/GitCraft`, rather than `/mnt/c`, for Linux filesystem behavior and file-watch performance. Install tools within Ubuntu; Windows installations of Rust and Node do not replace Linux installations.

In Ubuntu:

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl git libssl-dev \
  libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf
mkdir -p ~/src
cd ~/src
git clone https://github.com/MystifiedSky/GitCraft.git
cd GitCraft
```

Install stable Rust using the [official Rust installer](https://www.rust-lang.org/tools/install) and Node.js 22.12 or later using the [Node.js installation instructions](https://nodejs.org/en/download). Reopen the terminal, verify `cargo --version`, `node --version`, and `git --version`, then run the commands below from the cloned repository root.

WSLg is useful for everyday development. Also test packaged releases in a full Ubuntu Desktop VM or physical Linux desktop to check menus, file dialogs, notifications, credential storage, terminal behavior, and desktop installation. Use a disposable VM with a snapshot for package and updater testing. CI currently uses Ubuntu 22.04 as the Linux release build baseline; testing 24.04 additionally checks a newer desktop environment.

## macOS

Use a Mac, a remote Mac you can access interactively, or a macOS VM hosted on Apple hardware. GitHub's macOS runners provide automated build/test feedback; an interactive Mac session is still needed for desktop checks.

Install Apple's command-line developer tools:

```bash
xcode-select --install
```

Install stable Rust and Node.js 22.12 or later, clone the repository, and use the same build/test commands below. See [Tauri's platform prerequisites](https://v2.tauri.app/start/prerequisites/) for current setup requirements. Verify Apple silicon and Intel separately before declaring both fully validated. The release pipeline compiles a universal app, while the test job runs on the runner's native architecture.

## Build and run the checks

From the directory containing `Cargo.toml` on each OS:

```bash
npm --prefix ui ci
npm --prefix ui run build
npm --prefix ui test
node --test scripts/changelog-release.test.mjs
cargo test --locked -- --test-threads=1
cargo run --locked
```

The Rust suite uses disposable repositories. Run it serially because some tests change process-wide runtime preferences. Live AI tests are ignored by default and require explicit local setup. Linux runs the GLib security regression as part of the full native suite.

To package the app locally, run `npm --prefix ui run installer`. Published updater artifacts are built separately by the signed release pipeline; a local build alone does not verify the published update flow.

## Desktop smoke test

Use a disposable repository with a local bare remote. Record the app version, OS version, architecture, package format, and results.

- Install, launch, close, and reopen the packaged app.
- Open, clone, and initialize repositories, including paths with spaces and Unicode.
- Stage a file, hunk, and line; commit; amend; verify the index and working tree.
- Create and switch branches; merge, resolve a conflict, rebase, and exercise guarded undo/redo.
- Stash and restore work; fetch, pull, and push against the local remote.
- Verify graph scrolling, author images, diffs, file watching, and multiple repository tabs.
- Open the terminal, external editor, and native file dialogs. Verify preferences persist.
- Check credential storage, notifications, and optional AI integrations in a real signed-in desktop session.
- Install a previous published version, check for updates, restart into the new version, and verify the displayed version. Test Linux package upgrades separately from AppImage updates.

Report failures with reproduction steps and sanitized logs. Keep platform validation marked as in progress until the packaged desktop flows have been exercised.

## Headless component regression checks

After installing frontend dependencies, run `node scripts/verify-browser.mjs` from the repository root. It uses a locally installed Chrome or Chromium in a temporary profile. Set `CHROME_PATH` if the executable is not in a standard location. No desktop windows or user browser profiles are controlled.

The checks exercise actual React components and Monaco with synthetic backend responses: editor switches, virtual-list scrolling and keyboard focus, delayed commits, upstream submission, graph statistics, remote detection, SSH/signing selection, merge-backup review, LFS failures, bounded repository operations, ranking and session grouping. Reports are written under `target/verification/`. Backend IPC and clipboard writes are intercepted, so real OS/service interactions still require the separate desktop smoke test.

## Windows release-webview regression checks

Build the production renderer, then a release executable (the content policy is part of the default configuration):

```powershell
npm --prefix ui run build
./ui/node_modules/.bin/tauri.cmd build --no-bundle
node scripts/verify-desktop.mjs release csp
```

The first argument selects `release` (default) or `debug` in this checkout's
`target` directory; the former `target/release/gitcraft.exe` and
`target/debug/gitcraft.exe` spellings are also accepted. Arbitrary executable
paths are rejected. The optional report tag is limited to 1–48 ASCII letters,
digits, underscores and hyphens, starting with a letter or digit. Artifact
junctions pointing outside the checkout are refused. Install Git for Windows
under Program Files; the harness uses explicit Git, PowerShell and system tool
paths and gives the app a restricted process search path.

Run `node --test scripts/desktop-harness-paths.test.mjs` to check the executable
allowlist, traversal rejection, junction boundaries and Windows PATH isolation.
This suite also runs in the Windows and Linux CI matrix, and on macOS when the Tests workflow is started by hand.

The Windows harness uses WebView2's loopback debugging endpoint and an explicit
`--screenshot ... --shot-view validation` startup mode. It creates disposable
repositories and separate app/WebView profiles under `target/verification/`,
without loading or saving the regular user session. Close other verification
runs before starting it. It opens Explorer for selection checks and closes only
new fixture windows; it also checks localhost administrative-share UNC selection
when that share is accessible. Reports retain resource samples, timings, policy
violations and renderer exceptions. Fixtures and profiles remain for inspection.

Checks include a 50,000-commit repository, restoration after an actual local
commit, language/editor startup, JSON workers, preferences, image policy, native
updater configuration, concurrent watchers, five terminals and twelve cancelled
Git reads. No live account mutation, update installation or release publishing
occurs. The validation-only inspection commands reject ordinary app sessions.
This verifies a release executable, not installer behavior or signed upgrades;
use the platform smoke-test procedure above for those checks.
