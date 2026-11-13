# Release Process

- **Making a release (RTM-72):** releases are made on demand, when a user-visible change is worth
  shipping, from release branches `releases/vYY.M` cut from `main`. Each release is a Jira
  epic (see "Release epics" below). After the cut, changes reach a release branch only through
  squash-merged PRs against it, which need the same checks as on `main` (the ruleset covers both,
  RTM-78). `main` always carries the next release's version.
  - **Release epics (RTM-154):** each release is tracked as a Jira epic `Release X.Y.Z`, not
    as a task; the epic's component is Packaging:
    - it depends on the epics whose features it ships (for a patch, the fixes' issues); the
      changelog summary is written from them;
    - its sub-issues are the release steps: cutting the branch and setting the next version (a
      minor release only), the changelog and the release run, and the website post with the
      announcement; each step's PR carries that sub-issue's key;
    - when the release is done, create the next release's epic, so the epics finished in the
      meantime can be linked to it as they land.
  - **Minor release `YY.M.0`:**
    1. Cut `releases/vYY.M` from `main` (already at `YY.M.0`) and push it. The ruleset allows
       creating it without checks (`do_not_enforce_on_create`): `releases/v26.1` was pushed with
       the ruleset unchanged (RTM-98).
    2. On `main`, a PR `build(release): set version to <next> -- RTM-NN`
       (`scripts/set-version.sh`, then `cargo update --workspace` to refresh `Cargo.lock`): the
       next minor, `YY.(M+1).0`, or the next year's first version (`27.1.0`) when no further
       minor release is wanted this year.
    3. On the branch, a PR `docs(release): changelog for YY.M.0 -- RTM-NN` with the `CHANGELOG.md`
       written by `scripts/changelog.sh YY.M.0`, its TODO line replaced by the summary (see
       "`CHANGELOG.md`" below).
    4. Run the Release workflow on `releases/vYY.M`.
    5. Once the release is published, open the website post PR (see "Website post" below); once
       the post is live, announce the release (see "Announcement" below).
  - **Minor release with betas, `YY.M.0-beta.N` then `YY.M.0` (RTM-139):** betas exist only for
    minor releases, on their release branch, before the release.
    1. Cut `releases/vYY.M` from `main` and move `main` to the next version, as steps 1 and 2 of
       a minor release.
    2. On the branch, a PR `build(release): set version to YY.M.0-beta.1 -- RTM-NN`:
       `set-version`, `cargo update --workspace` and `scripts/changelog.sh YY.M.0-beta.1` (a new
       `CHANGELOG.md` since the cut), with the summary in place of the TODO line.
    3. Run the Release workflow on `releases/vYY.M`. The draft is a prerelease and never Latest;
       publish it, then open the website post PR (`channel = "beta"`).
    4. Each further beta: the same PR for `YY.M.0-beta.N` (its section covers the commits since
       beta.(N-1), whose draft must be published first), run, publish, post.
    5. The release: a PR `build(release): set version to YY.M.0 -- RTM-NN` (its section covers
       everything since the cut, above the betas'). Before running the release, dry-run the PR's
       branch with `-f previous=v<last stable release>`: the release run's upgrade tests start
       from the last beta, so this covers the most common upgrade. Then run, publish and post as
       for any release.

    The first beta cycle confirms what no run before a published beta can test:
    - the beta.2 and `YY.M.0` version PRs' Release runs pass: their upgrade tests start from the
      published beta.1 and last beta, and fail if Windows ends with two products or `apt` or
      `dnf` with the old version;
    - beta.1's draft shows "Set as a pre-release" ticked and "Set as the latest release"
      unticked;
    - a run for a beta after its release exists (`YY.M.0` published) is refused.
  - **Patch release `YY.M.P`:**
    1. Fixes land on `main` like all development and are cherry-picked to `releases/vYY.M` by PRs
       titled like the original PR (not the squash commit's subject, which ends in `(#NN)`).
       Several fixes can go in before a release.
    2. On the branch, a PR `build(release): set version to YY.M.P -- RTM-NN`: `set-version`,
       `cargo update --workspace` and `scripts/changelog.sh YY.M.P`, which needs the previous
       patch's tag, so its draft must be published first; the summary replaces the TODO line.
    3. Run the Release workflow on `releases/vYY.M`.
    4. Once the release is published, open the website post PR (see "Website post" below); once
       the post is live, announce the release (see "Announcement" below).
  - **The release run:** a `workflow_dispatch` of Release on a release branch builds and tests the
    packages and publishes a draft release tagged `vX.Y.Z` on that commit, with the version's
    `CHANGELOG.md` section as its body. It first checks that the branch matches the version
    (`releases/v26.1` releases only `26.1.*`) and that no release, draft included, or tag exists
    for it (RTM-73), and that `CHANGELOG.md` has entries for it (RTM-74). A patch on an older line
    (26.1.2 after 26.2.0) is created with `make_latest: false`; when publishing it, make sure "Set
    as the latest release" is unticked. Download and try the draft, then publish it by hand.
    Publishing creates the `vX.Y.Z` tag, which starts no run: the workflow has no tag trigger
    (RTM-102). A beta (RTM-139) is created with `prerelease: true` and `make_latest: false`, and
    is refused once its release exists; releases are not prereleases.
    Keep the `build(release): set version` and `docs(release): changelog` subjects exact when
    squash-merging: the changelog leaves those commits out.
  - **The MCP eval (RTM-159):** when a release changes `rtmmcp/`, `todotxt/src/` or the `mcp`
    entry points, run `nix develop -c scripts/mcp-eval.sh` on the release branch before the
    release run and read its table (see `rtmmcp/AGENTS.md`). It runs locally on the Claude Code
    and Grok Build logins; the Release workflow does not run it.
  - **Website post (RTM-91):** each published release gets a post on the project website, and the
    website's download links are updated by hand in the same change; the site has no JavaScript
    and no automatic links. A PR against `main`, `docs(site): website post for X.Y.Z -- RTM-NN` (the
    release epic's website-post sub-issue), that
    - adds `site/content/news/X.Y.Z.md` with the version's `CHANGELOG.md` section (from the release
      branch, or the release's body), the summary and then `## Changes` (headings one level up),
      and a one-sentence `description` in its front matter;
    - for a patch on an older line (published with "Set as the latest release" unticked) sets
      `older_line = true` under `[extra]` in the post's front matter;
    - for a beta, `A.B.0-beta.N.md`, sets `channel = "beta"` under `[extra]`;
    - for a stable release, adds the `[extra.announce]` text (see "Announcement" below);
    - passes `scripts/site/check.sh --external`.

    The post also updates `releases.json`, the release metadata the programs' update check reads
    (RTM-130); nothing else is edited for it. The download links follow the newest stable post
    without `older_line`: they point at
    `https://github.com/brcha/rtm/releases/download/vX.Y.Z/<asset>`, with X.Y.Z from the post's
    file name (names as under "Assets"; `platforms` in `site/templates/macros.html`). See
    `site/AGENTS.md`.
  - **Announcement (RTM-154):** each stable release, minor or patch (a patch on an older line
    too), is announced on X through Buffer, linking to its website post, once the post is live, so
    the link works. Betas are not announced.
    - The text is written in the website post PR, in the post's front matter:
      ```toml
      [extra.announce]
      x = "..."
      ```
      The post's URL is added after it. It stays within 280 characters, the link counting as 23.
      Like the summary, it stays neutral about other software.
    - Buffer's free plan holds the X account (RTM-168, RTM-169). The Announce workflow
      (`announce.yml`, RTM-171) runs `scripts/announce.sh`:
      - on the website post PR, it checks the text and prints the post as it will appear;
      - **merging the website post PR publishes the announcement:** after the merge it waits until
        the deployed `releases.json` lists the version (up to 30 minutes), then posts right away.
        Review the text in the PR as the final post.
    - **Retry:** if posting failed, or the post took longer to deploy, run Announce by hand
      (`gh workflow run Announce -f version=X.Y.Z -f dry-run=false`). A manual run is a dry run
      unless `dry-run` is false. If Buffer is unavailable, post the same text in Buffer by hand.
    - Facebook is not announced: the project has no Facebook Page, and Meta allows no automated
      posts to personal profiles (RTM-170). LinkedIn is not announced for now (RTM-172); it would
      fit one of the free plan's two remaining channels.
  - **`CHANGELOG.md`** lives on the release branches only, never on `main`, and is written by
    `scripts/changelog.sh X.Y.Z` (git-cliff with `cliff.toml`, in the Nix dev shell; bash only).
    A patch release's section covers the commits since the line's previous tag, prepended; a minor
    release's the commits since the previous release branch was cut (the first CalVer release:
    since `v0.1.3`), in a new file. With betas: beta.1 as a minor release, in a new file; beta.N
    since beta.(N-1), prepended; the release everything since the cut in one section (the beta
    tags ignored), prepended above the betas'. If the release branch moves while the changelog or version PR
    is open, update the PR from the branch and run the script again. Release branches are never
    deleted: the next minor release's range starts at the previous one's cut.

    From 26.2 on, each section is `## [X.Y.Z] - <date>`, a summary, then `### Changes` with
    git-cliff's groups (`#### Features`, ...); the script uses the release branch's own
    `cliff.toml`, so an older line keeps its branch's layout. The script writes a TODO line where the summary goes; replace it before
    committing (a forgotten one shows in the draft release's body, which is checked before
    publishing). The summary is written for users, not copied from issue titles:
    - its epics are the release epic's "depends on" links in Jira (see "Release epics");
    - an opening sentence, then a short paragraph or bullet per epic saying what users can now do;
    - a patch's summary names its fixes in a sentence or two; a beta's covers the epics it has so
      far; a release after betas covers everything since the cut;
    - it stays neutral about other software: it never names or compares with other todo.txt apps or
      sync and file-sharing tools.
  - **The first CalVer release** was 26.1.0 (RTM-98): its release issue first set `main` from
    0.1.4, which was never released, to 26.1.0 by PR, then followed the minor-release steps.
- **Workflow:** `.github/workflows/release.yml` (named `Release`) — triggers on manual
  `workflow_dispatch` (on a `releases/**` branch a release, see "Making a release"; on any other
  branch a dry run: builds and uploads to the Actions run, publishes nothing) and pull requests
  against `main` or `releases/**` (`pr-title.yml` checks those too); on those it builds and tests the packages (publishes nothing)
  only when they touch the crates (`rtmsync` included since RTM-21, `rtmupdate` since RTM-136, `rtmmcp` since RTM-115), `Cargo.toml`/`Cargo.lock`,
  `LICENSE`, `scripts/build-appimage.sh`, `scripts/build-macos.sh`,
  `scripts/check-macos-quit.sh`, `scripts/check-mcp.sh` or the workflow, Markdown
  excluded (its `changes` job, see "Which changes start which build" in [CI](ci.md)). It has no
  `v*` tag trigger (RTM-102): publishing a draft creates its tag, and a run started by that tag
  would rebuild the packages and publish them over the release, which could turn it back into a
  draft;
  releasing by pushing a tag by hand is not supported. Windows and Linux are x64
  only — no ARM64, no 32-bit; macOS is one universal (arm64 + x86_64) build (RTM-11).
- **Assets:** `rtmapp_<v>_x64_en-US.msi`, `rtmcli-<v>-x64.exe`, `rtmapp_<v>_amd64.deb`,
  `rtmapp_<v>_amd64.AppImage`, `rtmcli_<v>_amd64.deb`, `rtmapp-<v>-1.x86_64.rpm`,
  `rtmcli-<v>-1.x86_64.rpm`, `rtmapp_<v>_universal.dmg`, `rtmcli-<v>-macos-universal.tar.gz`. Since
  RTM-38 they are built without Tauri: cargo-wix, cargo-deb, cargo-generate-rpm (RTM-9), appimagetool,
  and (RTM-11) `scripts/build-macos.sh`.
- **Previous release (RTM-49):** the `version` job also outputs `previous`, the newest published
  (non-draft, betas included) release below the version being built, for the upgrade tests. A
  beta sorts below its release, so beta.1 upgrades from the last stable release, beta.N from
  beta.(N-1) and `YY.M.0` from its last beta (RTM-139): each step of the chain is tested once.
  A manual run can name another one with the `previous` input (`gh workflow run Release --ref <branch>
  -f previous=v0.1.3`, or a beta's tag), which also lifts the Windows v0.1.3 skip.
- **Job structure:** `version` (resolves and verifies the release version once, on `ubuntu-latest`;
  outputs the run's `mode`, `release`, `dry-run` or `check`, which `publish` acts on, `latest`,
  `notes`, the draft's body, and for betas `beta`, `package` and `msi`, see "Versioning")
  → `windows`, `linux` and `macos` (build in parallel, each `needs: version`, each uploading its own
  `dist/` as a plain build artifact — `dist-windows` / `dist-linux` / `dist-macos`; Windows and macOS
  also test there) → `linux-deb`, `linux-deb-install` and `linux-rpm` (test the Linux packages from
  `dist-linux`, see "Linux (RTM-8)") → `publish` (`needs: [version, windows, linux, linux-deb,
  linux-deb-install, linux-rpm, macos]`, skipped on pull requests, downloads all artifact sets and
  performs the dry-run upload or draft-release publish exactly once). The `release` gate needs the
  same jobs. The platform jobs deliberately do **not** call `softprops/action-gh-release` themselves —
  independent jobs calling it concurrently would race to create/update the same tagged release.
- **Versioning:** `major.minor.patch`, all numeric, or a beta of a minor release,
  `major.minor.0-beta.N` (RTM-139). The MSI `ProductVersion` allows only numbers, and each package
  format orders versions its own way, so a beta takes a different form in each, all sorting below
  the release:

  | Where | Beta | Release |
  |---|---|---|
  | Cargo, tags, asset file names, `CHANGELOG.md` | `26.3.0-beta.1` | `26.3.0` |
  | deb and RPM version (`~` sorts first) | `26.3.0~beta.1` | `26.3.0` |
  | MSI `ProductVersion` (`DisplayVersion`) | `26.3.0.1` | `26.3.0` |
  | macOS `CFBundleShortVersionString` | `26.3.0` | `26.3.0` |
  | macOS `CFBundleVersion` | `26.3.0.1` | `26.3.0` |

  cargo-deb and cargo-wix make their forms themselves; the workflow sets the RPM's
  (`--set-metadata`), and `scripts/build-macos.sh` the bundle's. The MSI compares only the first
  three fields, so beta.2 over beta.1 and the release over a beta are same-version upgrades, which
  `AllowDowngrades='yes'` in `wix/main.wxs` allows. The install and upgrade checks compare each
  package with its own form (the `version` job's `package` and `msi` outputs). The Windows exe's
  numeric version is `26.3.0.0` for a beta (winresource ignores the pre-release part); its text
  `ProductVersion` is the Cargo version. The
  0.x alpha ended with 26.1.0, the first CalVer release (`YY.minor.patch`); 0.1.4 was never
  released. `minor` counts the releases within a year and starts at 1 (the first release of 2027
  is `27.1.0`, the next `27.2.0`); `patch` fixes a release (`27.1.1`). Every `YY.*` is above `0.1.x`, so the MSI and apt upgrades keep working. The
  `[package]` version of `rtmapp/Cargo.toml` is the single source of truth — the release workflow reads
  it (only that table: other tables have their own `version` keys) and never writes to it. On a release
  branch the branch name is required to match it (see "Making a release"); a mismatch fails the
  build loudly instead of shipping packages whose internal version disagrees with their file names.
- **`scripts/set-version.ps1`** (PowerShell) and **`scripts/set-version.sh`** (bash) are two ports of
  one manual pre-release tool, not something CI invokes; keep them in step. Run either on `main`
  after a cut, or on a release branch for a patch release (see "Making a release"), to bump the
  version across `rtmapp/Cargo.toml`, `rtmcli/Cargo.toml`, `rtmsync/Cargo.toml`,
  `rtmmcp/Cargo.toml`, `rtmupdate/Cargo.toml` and `todotxt/Cargo.toml` in lockstep. The Nix packages read their
  versions from those files, so a bump needs no Nix change (RTM-75). It scopes its match to the
  `[package]` table specifically — `todotxt/Cargo.toml` also has a `[dependencies.uuid]` table
  with its own `version` key. A new release must have a higher version than the last one on its line
  (`releases/vYY.M`), or neither the MSI nor apt replaces the installed copy.
- **Identity:** the binary is `rtmapp` everywhere (`default.nix` `mainProgram`, the desktop
  entry's `Exec=rtmapp`, `Icon=rtmapp` and `StartupWMClass=rtmapp`, the deb package name); the display
  name is "Rusty Todo.txt Manager", also the macOS bundle's name (`Rusty Todo.txt Manager.app`, whose
  executable is `rtmapp`); the reverse-DNS identifier (Tauri's `identifier`) is `com.brcha.rtm`. It is
  the macOS `CFBundleIdentifier` (RTM-11) and the Windows AppUserModelID, set by the app at startup and
  on both MSI shortcuts (RTM-48).
- **Merges are squashes.** One PR becomes exactly one commit on its target branch, so PR titles are
  the changelog, and `.github/workflows/pr-title.yml` forces them to be conventional, scoped (the
  scopes are listed under "Commit subjects" in `AGENTS.md`) and RTM-tagged.
  git-cliff turns them into `CHANGELOG.md` on the release branches (RTM-74), the release body,
  below the hand-written summary (RTM-154).
- **No raw platform binaries are published redundantly with their installer.** `rtmapp.exe` is not
  staged on Windows — the MSI installs the identical binary. `rtmcli.exe` **is** staged, because
  neither the MSI nor (on Linux) the `rtmapp` deb packages the CLI; it is the only channel for
  `rtmcli` on Windows. The same holds for the macOS `rtmcli` tarball: the dmg has only the app. This
  asymmetry is deliberate, not an oversight — do not "fix" it for symmetry.
- **Signing:** the Windows and Linux packages are unsigned; the macOS ones are signed ad hoc only (arm64
  code must carry a signature to run), so Gatekeeper asks on first launch. Developer ID signing and
  notarization: RTM-54.
- **Other channels:** Homebrew (RTM-52) and Developer ID signing and notarization (RTM-54) are not
  required to end the alpha; each is done when wanted. The Nix flake is not a
  release channel: users pin a tag (`github:brcha/rtm/vX.Y.Z`) or follow a release branch
  (`github:brcha/rtm/releases/vYY.M`); the install guide shows both.

## Windows

- **MSI source:** `rtmapp/wix/main.wxs` (WiX v3), built by `cargo wix -p rtmapp` (cargo-wix 0.3.9,
  `cargo install --locked`; WiX 3.14.1 is on the runner image, installed with choco if a future image
  drops it). Its header lists what it keeps from the Tauri MSI; `rtmapp/AGENTS.md` has the details.
- **`UpgradeCode` `E0E423D1-1101-50D4-BAEE-4A27FA127509` must never change.** It is the Tauri MSI's
  (Tauri derived it from the product name), and with it `MajorUpgrade` replaces an installed version
  instead of installing beside it. Per-machine install, downgrades allowed (the build's only ICE
  warning, ICE61, comes from that).
- **Exe resources:** `rtmapp/build.rs` embeds `icon.ico` and the product name in `rtmapp.exe` with
  `winresource`, checking `CARGO_CFG_TARGET_OS` (not `#[cfg]`, which tests the host).
- **Tests in the job:** a silent fresh install (files, both shortcuts and their AppUserModelID, the
  exe's version information, a smoke launch, the launch action's check, the MCP check, uninstall),
  then an upgrade from the v0.1.3 Tauri MSI
  installed into a custom folder (one product afterwards, at the new version, still in that folder),
  then an upgrade from the previous release's MSI in the default folder (skipped with a notice when that
  is v0.1.3, already covered, or when it has no MSI). All read the product from the uninstall entries in
  the registry: the Windows Installer COM API failed with `DISP_E_TYPEMISMATCH` from PowerShell. **The
  v0.1.3 upgrade test downloads that release's asset, so that release must stay.** The smoke launch
  starts the installed `rtmapp.exe` with a sample `todo.txt` (config in `%APPDATA%\rtm`) and expects it
  running after 5 s, then stops it by its PID; it only warns on failure, since the runner's desktop
  session and adapter (WARP) are not under our control. Its stderr (`RUST_LOG=info`) is in the
  `msi-logs` artifact. Then the launch check (RTM-216), for the finish screen's "Launch" action,
  which a silent install never runs:
  - It reads `LaunchApplication` from the built MSI through `msi.dll` and requires a type 34
    action from `APPLICATIONFOLDER` that runs `"[WindowsFolder]explorer.exe"`.
  - It runs that command from a launcher that enforces the RedirectionTrust mitigation on itself,
    as the client msiexec has it, and reads the policy of the rtmapp that starts. It must be 0.
  - The control, rtmapp started directly by the same launcher, must have 1.
  - It skips with a notice on a Windows without the mitigation, and with a warning when the control
    is not 1 or no rtmapp starts through Explorer within 15 s.
  - It reads the policy rather than syncing through a junction because the runner has no Scoop,
    and junctions its administrator makes are trusted anyway.
  - The launcher is started with `Start-Process`, not `&`: a directly started rtmapp inherits the
    step's output pipe, and `&` would wait for it to exit.

  Then the MCP check (RTM-116), which fails the job: a PowerShell
  `System.Diagnostics.Process` with stdin and stdout on pipes, as an MCP client starts a server,
  runs the installed `rtmapp.exe mcp` on that config (this proves stdio works in the `windows`
  subsystem build) and the staged `rtmcli.exe -f <todo.txt> mcp`, one after the other. Each must
  answer `initialize`, `tools/list` and a `list_files` call within 30 s (named `rtm`, a `list_items`
  tool, the sample file's path) and exit once its stdin closes.
- **Manual check of the "Launch" check box** (RTM-216), on a release candidate's MSI, on a Windows
  machine with git from Scoop (`git` on `PATH` is `scoop\shims\git.exe`) and rtmapp configured with
  a synced todo.txt:
  1. Install the MSI interactively and leave "Launch Rusty Todo.txt Manager" checked.
  2. In the rtmapp the installer starts, the header shows no "Not syncing" or "Not synced", and
     Sync now works.
  3. The window groups with the Start Menu shortcut on the taskbar.

  Do it once over an installed version and once after an uninstall. Start the MSI by
  double-clicking it or from a normal terminal. Started from a shell that runs inside a job object
  (some automation and agent shells), the launch action was seen to end with 1631 and start
  nothing, which says nothing about the MSI.

## Linux (RTM-8)

- **Build floor: an `almalinux:9` job container (glibc 2.34), RTM-9.** The `linux` job builds both
  binaries once there and packages all five Linux assets from them, so every package needs glibc 2.34
  at most: RHEL 9 and its clones (2.34), Ubuntu 22.04 (2.35), Debian bookworm (2.36) and newer. Built
  on jammy, `rtmapp` needed libm's `hypot@GLIBC_2.35`, which RHEL 9 lacks; built on noble (24.04),
  the debs would not install on bookworm or older Ubuntu LTS releases. The packages are tested in
  `linux-deb` (on `ubuntu-22.04`, the oldest Ubuntu they target), `linux-deb-install` and
  `linux-rpm`.
- **debs:** both come from cargo-deb, described in `[package.metadata.deb]` of `rtmapp/Cargo.toml`
  and `rtmcli/Cargo.toml` (package name, maintainer, assets, dependencies). `depends` is written by
  hand: cargo-deb's `$auto` needs `dpkg-shlibdeps`, which AlmaLinux lacks, so both list
  `libc6 (>= 2.34), libgcc-s1` — the build image's glibc, so the floor changes with that image
  (`linux-deb` fails if a binary needs a newer glibc than its deb requires). `rtmapp`'s also lists the
  libraries it loads with `dlopen`; see `rtmapp/AGENTS.md`.
- **RPMs (RTM-9):** both come from cargo-generate-rpm 0.21.1, described in
  `[package.metadata.generate-rpm]` of the same files (release `1`; the licences go to
  `/usr/share/licenses/<name>/`). `rtmapp`'s `requires` names its `dlopen` libraries by soname
  (`libwayland-client.so.0()(64bit)`, ...) and the portal packages by the names the distributions
  share, so one package installs on Fedora, RHEL 9 and 10 (and clones) and openSUSE, whose package
  names differ; the deb's alternatives are rich dependencies (`(a or b)`). glibc comes from auto-req
  (`--auto-req builtin`, based on `ldd`), versioned by symbol, so a too-old glibc fails at install
  instead of at start. The deb's `suggests` are RPM `Suggests`. Unsigned: zypper needs
  `--allow-unsigned-rpm`.
- **AppImage:** `scripts/build-appimage.sh` lays out an AppDir by hand (binary, desktop entry, hicolor
  icons, licences; no bundled libraries) and packs it with appimagetool 1.9.1 and the type2 runtime
  20251108, downloaded from fixed release URLs and checked against their SHA-256 (upstream also
  publishes moving "continuous" builds). appimagetool runs with `--appimage-extract-and-run`, so no
  FUSE is needed. The downloads are a network dependency in the release path, so the step has its own
  `timeout-minutes`.
- **Tests (`linux-deb`, `ubuntu-22.04`):** `apt-get install` of both debs from `dist-linux` (every
  `Depends` must resolve on jammy), checks that each binary's newest glibc symbol is within its deb's
  `libc6` floor, that the desktop entry and all icon sizes are installed and that a portal backend
  with a file chooser came with `rtmapp`; then two smoke launches of the installed `rtmapp` (the binary
  in every Linux package) with a sample `todo.txt` (via `XDG_CONFIG_HOME`), each expected running
  after 5 s and stopped by its PID, logging with `RUST_LOG=info`: on X11 under `xvfb-run` with Mesa
  software rendering (lavapipe, llvmpipe; blocking, and its window must be mapped), and on Wayland
  under a headless Weston (non-blocking, a warning on failure). Then both debs are purged, the
  previous release's debs installed and the new ones over them (`dpkg-query` must report the new
  version; skipped with a notice when the release lacks a deb). v0.1.3 is not skipped here: it is the
  Tauri deb, also named `rtmapp`, and pulls webkit2gtk. Then the AppImage is extracted and checked,
  and its `AppRun mcp` passes the MCP check. Before the smoke launches, the MCP check (below) runs
  on the installed binaries.
- **The MCP check (RTM-116)** runs in `linux-deb`, `linux-deb-install`, each `linux-rpm` container and
  `macos`: `scripts/check-mcp.sh <expected path> <command…>` (bash 3.2, for macOS; no python, jq or
  `timeout`, which some containers lack) sends `initialize`, `notifications/initialized`,
  `tools/list` and a `list_files` call, keeps stdin open until the last reply, and checks the
  replies for the name `rtm`, a `list_items` tool and the expected path, which proves the file came
  from `-f` or from rtmapp's config in the platform's place. A watchdog kills a server that does
  not exit within 30 s. It runs `rtmcli -f <todo.txt> mcp`, then `rtmapp mcp` with a scratch config
  (`XDG_CONFIG_HOME`, or `HOME` on macOS), never both at once: servers with the same parent count
  as one client. The Linux test jobs check out no sources, so the `linux` job uploads the script as
  the `check-mcp` artifact (one day's retention; `publish` takes only `dist-*`).
- **Which systems are tested:** the two ends of each package family, since an install test catches a
  package missing on the oldest release or renamed or dropped on the newest, and the releases in
  between follow from the ends. debs: Ubuntu 22.04 (`linux-deb`'s runner, the oldest) and the
  `ubuntu:latest` image, the newest LTS (Ubuntu shares Debian's package names and an LTS is cut from
  Debian testing, so it is ahead of Debian stable and stands for both). RPMs: `almalinux:9`
  (the oldest; bump it when RHEL 9 ends), `fedora:latest`, and openSUSE Leap 16.0 for openSUSE's own
  repositories and zypper. The moving tags need no bumps. Fedora 43 and 44, AlmaLinux 10,
  Tumbleweed, Debian 12, 13 and testing and Ubuntu 24.04 and 26.10 also passed once (RTM-9).
- **Tests (`linux-deb-install`):** an `ubuntu:latest` job container installs both debs with
  `apt-get`, resolving every `Depends` from the default repositories, with the same file checks and
  the MCP check.
- **Tests (`linux-rpm`):** a matrix of job containers (the three RPM images above;
  `fail-fast: false`) installs both RPMs with `dnf` or `zypper`, resolving every `Requires` from the
  default repositories, with the same file checks, a check for the versioned glibc requirement and
  the MCP check.
  On `fedora:latest` the previous release's RPMs are installed and the new ones over them (skipped
  with a notice when the release has none, true until the second release with RPMs).
- **Apt hardening is reused, not reinvented.** `linux-deb` pins the `archive.ubuntu.com` mirror and
  passes `Acquire::Retries`/timeout options (see "Linux CI apt hardening (RTM-28)" in
  [CI](ci.md)). It builds nothing, but the install test resolves the debs' dependencies from the index.
- **No raw Linux binaries are published.** Only the `.deb` and `.rpm` packages and the `.AppImage`.
  Distributions that take neither debs nor RPMs have no release artifact for `rtmcli` as a result.
- **Prebuilt-binary install actions are a trap on this job (RTM-29).** `cargo-deb` was originally
  installed via `taiki-e/install-action`, which fetched upstream's only published asset — built by
  upstream CI on `ubuntu-latest`, which has meant **noble** (24.04, glibc 2.39) since GitHub's runner
  migration. The job then ran on jammy (22.04, glibc 2.35), so the binary failed immediately:
  `` GLIBC_2.39' not found ``. Fixed by compiling with `cargo install cargo-deb --locked --version
  3.7.0` instead, which links against the build host's own glibc by construction. The AlmaLinux 9
  container (glibc 2.34) has the same mismatch. **This generalizes:** prefer `cargo install --locked`
  for Rust tooling on this job (cargo-generate-rpm too) unless a tool's release process is confirmed
  to target this floor specifically.

## macOS (RTM-11)

- **Runner:** `macos-latest` (arm64). Both targets (`aarch64-apple-darwin`, `x86_64-apple-darwin`) are
  built there, with `MACOSX_DEPLOYMENT_TARGET=11.0` for the job (also `LSMinimumSystemVersion` in
  `rtmapp/assets/Info.plist`; keep the two equal, the job checks it), and joined with `lipo` into universal binaries. Each
  release build takes about 5 minutes per target.
- **dmg:** `scripts/build-macos.sh <binary> <version> <output.dmg>` lays out `Rusty Todo.txt
  Manager.app` by hand (binary, `Info.plist` with `@VERSION@` and `@BUNDLE_VERSION@` filled in,
  see "Versioning", `icon.icns` as
  `Resources/rtmapp.icns`, both licences in `Resources/`), signs it ad hoc and packs it with an
  `/Applications` link using `hdiutil` (retried: it can fail with "Resource busy" on runners). Only
  tools that ship with macOS, no downloads. The rtmcli tarball (binary and `LICENSE`) is made in the
  workflow; its `rtmcli` is signed ad hoc there.
- **Tests in the job:** the dmg is mounted and the app copied to `/Applications`; then the Info.plist
  values, the resources, `codesign --verify --strict` on the app and `rtmcli`, both slices and their
  `minos` (`vtool`), `rtmcli --help` natively and under Rosetta (skipped with a notice without it),
  and the MCP check on the tarball's `rtmcli` and the bundle's `Contents/MacOS/rtmapp`, also under
  Rosetta when it is installed. (`rtmapp mcp` returns before eframe starts, so it never registers
  as an application.)
  Then a smoke launch through Launch Services (`open -a`) with a sample `todo.txt` in the runner's own
  config path, expected running after 5 s. It proves only that the process stays up: there is no window
  check (`osascript` would need Accessibility permission), and `open --stderr` captured no log in the
  first run.
- **Quit check (RTM-140):** `scripts/check-macos-quit.sh` then proves that quitting the app through
  NSApplication `terminate:`, the path of Cmd+Q and the app menu's Quit, runs eframe's `on_exit`
  and with it the exit sync. In a scratch directory it makes a bare remote and a clone with a tracked
  `todo.txt`, starts the app with `open -n -a … --env HOME=<scratch>/home` (its config and git's
  global config are then scratch files, `sync_interval_minutes = 0`), waits for the startup sync to
  pull a commit pushed from a second clone (which also shows `HOME` reached the app), commits a marker
  in the clone outside the app, sends the quit Apple event (`tell application id "com.brcha.rtm" to
  quit`) and passes only if the app exits within 15 s and the marker reached the remote. Exit status
  1 is a failure, 2 inconclusive (the startup sync pushed the marker before the quit); either fails
  the job. The runner lets `osascript` send the event without a permission prompt (first run in the
  pull request of RTM-140).
  - **By hand on a Mac:** quit any running rtmapp, install the app, then run
    `scripts/check-macos-quit.sh [<app>]` (default `/Applications/Rusty Todo.txt Manager.app`). The
    first run may ask to let the terminal control the app (Automation). `--keystroke` sends a real
    Cmd+Q through System Events instead of the Apple event, the exact menu path; it needs
    Accessibility permission for the terminal. The user's real config, git config and todo.txt are
    never touched.
- **No upgrade test:** replacing a `.app` carries no installer state, unlike the MSI and the debs.
- `rust.yml` also runs the test suite on `macos-latest`.
