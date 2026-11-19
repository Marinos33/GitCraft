# Releasing GitCraft

Each push to `main` starts the **Release** workflow unless the push only changes `CHANGELOG.md`. It builds the tip of that push and adds the workflow run number to the patch component of the base version in `tauri.conf.json`. For example, base `0.1.0` plus run 1 yields `0.1.1`. The workflow updates the version in its checkout only. It creates a version tag at the source commit when it publishes, so no version bump commits or manually pushed tags are needed.

The workflow builds Windows x64 (NSIS and MSI), macOS (universal DMG), and Linux x64 (AppImage, DEB, and RPM). It signs the updater bundles, checks that all expected assets are present, and publishes them together to [GitHub Releases](https://github.com/MystifiedSky/GitCraft/releases) as a **pre-release**. It then rewrites `latest.json` on the rolling `beta` release, so copies on the beta channel are offered that build. Until that succeeds, installed copies continue to see the previous build.

In a fork, a push to `main` only runs the tests. To publish a build from a fork, open **Actions → Release → Run workflow** on `main`. Forks have no update signing key, so the workflow builds the installers without update packages and publishes them as a pre-release without `latest.json`: install them by hand, GitCraft cannot update them in the app. Clear **Build the macOS installer** to leave macOS out when no macOS runner is available. **Promote to stable** works on these pre-releases too. The main repository keeps refusing to build without its signing key and to promote a release without `latest.json`.

The test workflow runs on Windows and Linux for every build, and also on macOS for pull requests and manual runs: GitHub's macOS runners can stay queued for a long time and would otherwise hold up every build of `main`.

## Promoting a build to stable

Pushes to `main` never reach the stable channel on their own. To release a build to everyone, open **Actions → Promote to stable → Run workflow** and enter its version (for example `0.1.42`). The workflow checks that the pre-release finished publishing and is newer than the current stable release, writes release notes covering every change since the previous stable release, then clears the pre-release flag and marks it as the latest release. Copies on the stable channel read `latest.json` from the latest release, so they are offered it at their next check.

Use **Re-run failed jobs** in Actions to retry a failed release with the same version. If a higher version has already published, the retry skips publication. Use **Run workflow** on `main` if you need a fresh build. To change the base version, set it with `npm --prefix ui run version:set -- <version>` and commit the changed manifests and lockfiles. Keep the next calculated version higher than the current release.

## Changelog

Write new entries under `## [Unreleased]` in `CHANGELOG.md`. At publication, the workflow reads `CHANGELOG.md` from the release commit and the previous published tag. It posts only new entries, followed by a link to the full comparison. When there are no new entries, the release notes show just the comparison link. This also works when several releases queue up before the changelog is archived.

Archive dated sections in a changelog pull request, oldest release first. Save `CHANGELOG.md` from the released tag and its previous published tag as UTF-8 files, then run `node scripts/changelog-release.mjs record <version> MystifiedSky/GitCraft <released-changelog> --previous <previous-changelog> --date <publication-date>`. Entries added after the released commit stay under Unreleased. Leave already published bullets as written until they are archived so the workflow can recognize them in the next release. Describe distinct changes with specific wording so two different changes do not look like duplicate entries.

## Updating installed copies

GitCraft checks GitHub Releases at startup and every hour while it’s open, on the channel chosen in **Preferences → General → Update channel**: **Stable** reads the latest full release and **Beta** reads the rolling `beta` release. When it finds a newer version, it downloads the update and verifies the signature using the public key built into the app. Choose **Update Ready (Restart GitCraft)** in the bottom right to install it. Turn off **Automatically download GitCraft updates** in **Preferences → General** to stop background checks and downloads. **File → Check for Update** still works with the setting off.

Install a published release once if your current build predates the signing key. Linux AppImage builds can update in place; DEB and RPM installations should be upgraded using the matching package from GitHub.

Update signing is separate from Windows Authenticode signing and Apple notarization. First-time installations may still show platform security prompts.

## Signing key

The repository already has the public key in `tauri.conf.json` and the corresponding `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` Actions secrets. Keep a backup of that same private key and its password in a secure location. Replacing the key would require users to install a new build manually before receiving more updates.

For a new repository or restoration, run `npm --prefix ui ci` followed by `powershell -ExecutionPolicy Bypass -File scripts/setup-updates.ps1` from the app checkout. Use the existing key and its password when restoring this repository.
