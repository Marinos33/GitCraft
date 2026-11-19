# Renderer content policy

The content security policy is set in `tauri.conf.json` (`app.security.csp`) and
applies to every build, including releases. It was previously an opt-in
configuration (`tauri.csp.conf.json`); it is now the default.

Source inventory (2026-11-17):

- Vite bundles all renderer scripts, dynamic chunks, fonts and editor language grammars locally.
- Monaco creates a local editor worker through Vite's worker import. Blob workers are allowed for webview implementations that wrap worker URLs.
- React, Monaco and resizing/layout controls set inline styles. The policy allows inline styles; executable inline scripts require Tauri's generated hashes.
- Author images request HTTPS Gravatar URLs and may use Gravatar subdomains. Image previews are native-generated data URLs. No arbitrary image host is required.
- Renderer commands and events use Tauri IPC (`ipc:` and the IPC localhost endpoint). Git hosting, AI, downloads and updater HTTP requests execute natively, so they do not require renderer network permission.
- Links launch through native commands. Embedded frames, plugins, forms and injected base URLs are unnecessary.
- No eval permission is included. Development-server HMR is outside this packaged policy.

Why it matters: the renderer can call powerful native commands (terminal, external
tools, file saves). Without a policy, any injected markup would be one step away
from running programs on the user's machine.

Validation: the Windows release executable passes IPC, editor/JSON workers, data
images, the HTTPS avatar source, preferences and native updater configuration with
zero recorded CSP violations or renderer exceptions (see the
[Windows harness](../TESTING.md#windows-release-webview-regression-checks)).
Installed macOS/Linux builds should still be smoke-tested: cold launch, editors and
language workers, image previews and avatars, dialogs, account requests, update
discovery and restart. Record any CSP violation reported in the webview console.
