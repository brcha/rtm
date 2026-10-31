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
    checks `validate` (`pr-title.yml`) and `rust`, `nix` and `release`: one gate job per
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
    would go into Rust's and Nix's). The website (`site/`, `scripts/site/`, `site.yml`; RTM-79) is
    in none of them.
  - Pushes to `main` start Rust and Nix unless every changed file is Markdown or part of the
    website (RTM-87): `site/**`, `scripts/site/**`, `.github/workflows/site.yml` (`paths-ignore`; no
    checks are required there). Release also runs on `v*` tags and by manual dispatch, always
    building.
  - All four workflows run on every pull request against `main` or `releases/**`. The required
    checks are not enforced when a branch is created (`do_not_enforce_on_create`), so a release
    branch can be cut by pushing a `main` commit, which has no PR checks. Besides the checks, the
    ruleset forbids deleting and force-pushing those branches and allows only squash-merged PRs.

  A new required check needs the same shape: a gate job that always runs, not a path filter.
