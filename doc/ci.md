# CI

- **Linux CI apt hardening (RTM-28):** GitHub-hosted Ubuntu runners resolve apt through a
  mirrorlist that tries `azure.archive.ubuntu.com` before falling back to `archive.ubuntu.com`.
  When the Azure-local mirror degrades — dropping connections rather than refusing them — apt
  retries it per index file across roughly twenty files, which can stall a job for hours with no
  bound. Any job that installs apt packages (`release.yml`'s `linux-deb` job, and `rust.yml`'s Linux
  job for pijul's libraries since RTM-18) overwrites `/etc/apt/apt-mirrors.txt` with the canonical
  archive before updating, and passes explicit `Acquire::Retries`/timeout options to both
  `apt-get update` and `apt-get install` as defence in depth. **Do not revert this for a marginally
  faster "good day" run** — it trades a small, predictable slowdown for an unbounded hang becoming
  a fast, loud failure instead.
  - **When Ubuntu's DNS fails (RTM-213):** on 2026-11-11 Canonical's nameservers for `ubuntu.com`
    were unreachable from Azure's resolver, which the runners use, so `archive.ubuntu.com` and
    `security.ubuntu.com` did not resolve. Each apt step therefore first looks up
    `archive.ubuntu.com` (`getent hosts`). When the name does not resolve, it pins
    `http://mirrors.edge.kernel.org/ubuntu/` instead, with a warning: a full Ubuntu mirror, with
    `-security`, on DNS that does not go through Canonical's. On the runners every suite goes
    through the mirror list, so the list is the only thing that changes; `linux-deb-install`'s
    `ubuntu:latest` container names both hosts directly, so there the step rewrites its
    `ubuntu.sources`. A normal day is unchanged.
  - The fallback is a lookup, not a second entry in the mirror list: apt would try the archive
    first for every file, and each failed lookup takes 20 s (about 80 s with the retries), far past
    the step timeouts for the dozens of packages `linux-deb` installs.
  - `packages.microsoft.com`, which still resolved, is no alternative: it carries only Microsoft's
    own packages, none of the Ubuntu ones these jobs install.
- **Which changes start which build (RTM-76):** Markdown alone (`**/*.md`) starts no build.
  - The ruleset "Run tests in the PR" covers `main` and `releases/**` (RTM-78). It requires the
    checks `validate` (`pr-title.yml`, which requires a scope from the list under "Commit
    subjects" in `AGENTS.md`) and `rust`, `nix` and `release`: one gate job per
    workflow, which passes when its builds passed or were skipped and fails when any failed or was
    cancelled.
  - So `rust.yml`, `nix.yml` and `release.yml` run on every pull request, never filtered by
    `paths` on the trigger: a workflow skipped by a path filter never reports its checks, and the
    required check would block the merge. Their `changes` job reads the pull request's diff and
    skips the build jobs when none of the workflow's inputs changed, Markdown always excluded
    (RTM-82):
    - Rust: the crates, `Cargo.toml`/`Cargo.lock`, the workflow.
    - Nix: the crates, `Cargo.toml`/`Cargo.lock`, `flake.nix`, `flake.lock`, `default.nix`, the
      workflow. Not `shell.nix`: CI does not build the dev shell.
    - Release: its list of package inputs (the crates, `Cargo.toml`/`Cargo.lock`, `LICENSE`, the
      build scripts, the workflow).

    Keep each list in step with what its build reads (a tracked `rust-toolchain*` or `.cargo/`
    would go into Rust's and Nix's). The website (`site/`, `scripts/site/`, `site.yml`,
    `vercel.json`; RTM-79) is in none of them.
  - Pushes to `main` start Rust and Nix unless every changed file is Markdown or part of the
    website (RTM-87): `site/**`, `scripts/site/**`, `.github/workflows/site.yml`, `vercel.json`
    (`paths-ignore`; no checks are required there). Release also runs by manual dispatch, always
    building, and never on tags (RTM-102, see [Release Process](release-process.md)).
  - The four workflows behind the required checks run on every pull request against `main` or
    `releases/**`. The required checks are not enforced when a branch is created
    (`do_not_enforce_on_create`), so a release branch can be cut by pushing a `main` commit,
    which has no PR checks. Besides the checks, the
    ruleset forbids deleting and force-pushing those branches and allows only squash-merged PRs.

  A new required check needs the same shape: a gate job that always runs, not a path filter.
- **Website (RTM-89):** `site.yml` builds the site and checks its links with
  `scripts/site/check.sh`: Zola's check and build, then lychee over the built pages offline, with
  the absolute `base_url` mapped to the built files, so broken templates, links, anchors and asset
  references fail. It is not a required check, so its trigger filters by `paths` (the site, its
  scripts, the workflow, `flake.lock`, and the screenshots and icons it copies); pull requests
  without site changes never see it. External links are checked only weekly and on manual
  dispatch (`check.sh --external`, with `GITHUB_TOKEN`), so a remote outage never blocks a merge.
  Zola and lychee come from the pinned nixpkgs through `nix shell`, not the full dev shell.
  Publishing is not CI's job: Vercel's Git integration builds and deploys the site from
  `vercel.json` (RTM-90, see `site/AGENTS.md`); its preview status on pull requests is not a
  required check.
- **Actions security (RTM-145):** every `uses:` names a full commit SHA, with the release it
  stands for as a comment (`actions/checkout@<sha> # v7.0.1`), so a moved or replaced tag cannot
  change what runs. The repository setting "Require actions to be pinned to a full-length commit
  SHA" enforces it: a workflow with a tag or branch reference fails.
  - Take each SHA from the action's own repository, never from a fork.
    `gh api repos/<owner>/<repo>/commits/<tag> --jq .sha` resolves an annotated tag to its commit.
  - To update an action, look up the new release's SHA and change both the SHA and the comment.
  - `dtolnay/rust-toolchain` has no releases, only branches named after toolchains. It is pinned
    to a commit of its `stable` branch. That branch's `toolchain` input defaults to `stable`; the
    workflows still pass `toolchain: stable`, so the choice shows next to the SHA.
  - Workflows from fork pull requests wait for the maintainer's approval ("Require approval for
    all external contributors"), not only those from first-time contributors.
