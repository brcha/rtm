# The website

Reference for the project website in `site/`, built with [Zola](https://www.getzola.org) and
served at `https://rtm.brcha.com`. Its conventions are in [`site/AGENTS.md`](../site/AGENTS.md);
the steps of a release's website post are "Website post" and "Announcement" in
[Release Process](release-process.md).

## Build and preview

In the dev shell (`zola`, `lychee` and `jq` come from the nixpkgs pinned in `flake.lock`, see
[Nix packaging](nix-packaging.md)):

```sh
scripts/site/prepare.sh
zola --root site serve              # http://127.0.0.1:1111, reloads on change
scripts/site/check.sh               # prepare, zola check and build, releases.json, lychee offline (what CI runs)
scripts/site/check.sh --external    # the same, then external links over the network
```

`prepare.sh` copies the images into `site/static/`: the three screenshots from the repository root
(`screenshot_*.png`) and `rtmapp.svg` and `rtmapp-small.svg` (the favicon) from
`rtmapp/assets/icons/`. Zola reads static files only from inside the site, and copying keeps one
source per image. The copies and the output (`site/public/`) are git-ignored.

The built pages use absolute `https://rtm.brcha.com/` URLs (`base_url`), so open them through
`zola serve`, not from disk. `check.sh` maps that base URL to the built files for lychee, reading
it from `config.toml`.

CI: `.github/workflows/site.yml` runs `check.sh` for site changes and `--external` weekly; it is not
a required check, and website-only changes start no Rust or Nix build (see [CI](ci.md)).

## Publishing (Vercel)

Vercel's Git integration builds and deploys the site: production from `main` at
`https://rtm.brcha.com`, a preview for other branches and pull requests. The project stays at the
repository root (a Root Directory would cut off the screenshots and icons); everything it runs is
in `vercel.json`:

- **Install:** `scripts/site/install-zola.sh <version> <sha256>` downloads Zola's musl binary and
  checks it (Vercel's own Zola preset fails on current Zola).
- **Build:** `scripts/site/vercel-build.sh`: `prepare.sh`, then `zola build`; previews get their
  own `*.vercel.app` address as the base URL (`VERCEL_ENV`, `VERCEL_URL`), so they load their own
  stylesheet and images.
- **Ignore:** `scripts/site/vercel-ignore.sh` skips a deployment when no website input changed
  since the branch's last successful deployment (`VERCEL_GIT_PREVIOUS_SHA`), and builds when in
  doubt. Its list of inputs is `site.yml`'s `paths` without the workflow file itself.
- **Zola version:** `vercel.json` pins the same Zola as `flake.lock`. After a nixpkgs bump, run
  `scripts/site/sync-zola-version.sh`, which rewrites the version and checksum in `vercel.json`.

## Pages copied from the README

`content/install.md` is README.md's "Installing" section, its `###` headings raised to `##`. To
compare, from the repository root (no output means equal):

```sh
diff <(awk '/^## /{p=/^## Installing$/} p' README.md | sed '1,4d;$d' | sed 's/^###/##/') \
     <(sed '1,/^<!--/d' site/content/install.md | sed '1d')
```

The awk takes the section up to the next `##` heading, whatever it is. The line pointing to the AI
assistants page comes before the copy marker, outside the comparison.

`content/mcp.md` is README.md's "Using rtm with AI assistants (MCP)" section, its `###` headings
raised to `##` and its links adapted to the site (`@/install.md`, the README's Syncing section on
GitHub). `content/report.md` is written for the site; README's "Reporting problems and requests"
links to it, and it lists what the issue forms in `.github/ISSUE_TEMPLATE/` ask for.

## Releases on the site

- Each release has a post, `content/news/X.Y.Z.md`, rendered with `news-post.html`, which opens
  with the downloads of that release, taken from the file name.
- The download links are the `platforms` macro in `templates/macros.html`, given a version. The
  landing page and the install guide show `latest_version`: the file name of the newest post that
  sets neither `older_line = true` (a patch on an older line) nor `channel = "beta"` under
  `[extra]`. The install guide's block also shows that release's Nix references, its tag and its
  release branch (`releases/vYY.M`), in `page.html`.
- The notice at the top of the landing page's hero links to the same newest stable post (Zola's
  `get_section`); `check.sh` checks that both name the same release.
- A beta post, `A.B.0-beta.N.md` with `channel = "beta"`, carries a Beta label in the news list and
  on its page and its own downloads, but never moves the landing page, the hero notice or the
  install guide.
- A stable post's `[extra.announce]` text is not rendered; the Announce workflow posts it.

## `/releases.json`

The release metadata that `rtmcli check-update` and rtmapp's update notice read (through
`rtmupdate`, which holds the URL). Zola renders it as a feed (`generate_feeds` and `feed_filenames`
in `config.toml`) from `templates/releases.json`, at the site root, from the release posts only:

```json
{"schema": 1, "releases": [{"version": "…", "channel": "…", "date": "…", "url": "…"}]}
```

Newest first by date, every post (betas and older-line patches too). `version` is the post's file
name, `channel` its `[extra] channel` (`stable` when unset), `date` as `YYYY-MM-DD`, `url` the
post. Every value goes through `json_encode`. Clients ignore fields and channels they do not know.
`check.sh` checks its shape with `jq`, that the landing page's release is one of its stable
entries, and its URLs (lychee skips `.json` files when it scans a directory, so the file is passed
explicitly).
