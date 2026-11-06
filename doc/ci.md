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
      build scripts and the macOS quit check, the workflow).

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
  `scripts/site/check.sh`: Zola's check and build, the shape of `releases.json` (`jq`), then
  lychee over the built pages and `releases.json` offline, with the absolute `base_url` mapped to
  the built files, so broken templates, links, anchors and asset references fail. It is not a required check, so its trigger filters by `paths` (the site, its
  scripts, the workflow, `flake.lock`, and the screenshots and icons it copies); pull requests
  without site changes never see it. External links are checked only weekly and on manual
  dispatch (`check.sh --external`, with `GITHUB_TOKEN`), so a remote outage never blocks a merge.
  Zola, lychee and jq come from the pinned nixpkgs through `nix shell`, not the full dev shell.
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
- **Claude review (RTM-147):** `claude-review.yml` runs Anthropic's `claude-code-action` with the
  official `code-review` plugin. It reviews a pull request against `main` or `releases/**` and
  posts the findings as comments from `claude[bot]`.
  - It runs when a pull request is opened, reopened or marked ready for review. It does not run
    on every push, and it skips drafts and pull requests from forks, which get no secrets. The
    plugin also stops when Claude has already commented on the pull request, so reopening does
    not start a second review.
  - It only comments: it never pushes, approves or blocks. It is not a required check, so a
    failed or skipped review never holds up a merge.
  - The plugin checks the changes against `CLAUDE.md` files. The prompt tells it that rtm's are
    named `AGENTS.md`.
  - The plugin marketplace (`anthropics/claude-plugins-official`) is checked out at a pinned
    commit and passed to the action as a local path. The plugin's frontmatter grants the tools it
    may use, so the pin keeps an upstream change from widening them. To update it, change the
    commit SHA and its comment, as for an action.
  - The workflow token is read-only. The comments go out through the Claude GitHub App, which is
    installed on this repository only; the action gets the app's token in exchange for the run's
    OIDC token (`id-token: write`). Claude may only run the `gh` commands the plugin needs, which
    read the pull request and post the review.
  - The action refuses to run while the workflow file differs from the copy on `main` ("Workflow
    validation failed"). So a pull request that changes `claude-review.yml` is not reviewed by
    it.
  - It authenticates with the repository secret `CLAUDE_CODE_OAUTH_TOKEN`, a token from the
    maintainer's Claude subscription. `claude setup-token` creates it, valid for one year. When
    it expires, the review fails until the secret is replaced.
  - To bill the Anthropic API instead, store an API key as `ANTHROPIC_API_KEY` and replace the
    `claude_code_oauth_token` input with `anthropic_api_key`.
