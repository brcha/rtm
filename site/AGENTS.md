# AGENTS.md — rtm website (`site/`)

## What This Is

The project website, meant for `https://rtm.brcha.com`: one static landing page that says what rtm
is, shows the screenshots and features, and links to the downloads on the GitHub releases. Built
with [Zola](https://www.getzola.org). No server-side features. Design and decisions: RTM-80
(epic RTM-79).

---

## Structure

```
site/
├── config.toml           ← base URL, title, description
├── content/
│   ├── _index.md         ← the landing page's text: tagline (front matter), intro and features
│   ├── install.md        ← the install guide, a copy of README.md's "Installing" section
│   └── news/             ← one post per release (`X.Y.Z.md`), listed newest first
├── templates/
│   ├── base.html         ← skeleton: head, header (Install, Download, News, GitHub), footer
│   ├── index.html        ← the landing page: hero (with the latest release notice), screenshots
│   │                       and their viewer, features, downloads
│   ├── macros.html       ← the download links and the latest release's version
│   ├── news-post.html    ← the release posts, dated, opening with that release's downloads
│   ├── page.html         ← plain pages (the install guide)
│   └── section.html      ← the news list
└── static/style.css      ← the only stylesheet; light and dark through prefers-color-scheme
```

`scripts/site/prepare.sh` copies the images into `site/static/` before a build: the three
screenshots from the repository root (`screenshot_*.png`, RTM-51) and `rtmapp.svg` and
`rtmapp-small.svg` (the favicon) from `rtmapp/assets/icons/`. Zola reads static files only from
inside the site, and copying keeps one source per image. The copies and the output (`site/public/`)
are git-ignored; never commit them.

---

## Build and preview

In the dev shell (`zola` and `lychee` come from the nixpkgs pinned in `flake.lock`, see
[Nix Packaging](../doc/nix-packaging.md)):

```sh
scripts/site/prepare.sh
zola --root site serve              # http://127.0.0.1:1111, reloads on change
scripts/site/check.sh               # prepare, zola check and build, lychee offline (what CI runs)
scripts/site/check.sh --external    # the same, then external links over the network
```

The built pages use absolute `https://rtm.brcha.com/` URLs (`base_url`), so open them through
`zola serve`, not from disk. `check.sh` maps that base URL to the built files for lychee, reading
it from `config.toml`. Build site URLs in the templates with `get_url` (absolute), not as
root-relative `/path` links, which lychee cannot resolve offline.

CI: `.github/workflows/site.yml` runs `check.sh` for site changes and `--external` weekly; not a
required check (RTM-89, see [CI](../doc/ci.md)).

---

## Publishing (Vercel)

Vercel's Git integration builds and deploys the site (RTM-90): production from `main` at
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
  doubt. Its list of inputs is `site.yml`'s `paths` without the workflow file itself; change both
  together.
- **Zola version:** `vercel.json` pins the same Zola as `flake.lock`. After a nixpkgs bump, run
  `scripts/site/sync-zola-version.sh`, which rewrites the version and checksum in `vercel.json`.

---

## Conventions

- Text in `content/_index.md`, layout in the templates; no third-party theme.
- Advertise only features that exist in the code (subtasks, for example, exist only in the data
  model and are not listed).
- The install guide is copied from `README.md` ("Installing") into `content/install.md`, not only
  linked (RTM-93); keep the two in step when either changes. The copy is the section's body with
  its `###` headings raised to `##`; to compare, from the repository root (no output means equal):

  ```sh
  diff <(sed -n '/^## Installing/,/^## Syncing/p' README.md | sed '1,4d;$d' | sed '$d' | sed 's/^###/##/') \
       <(sed '1,/^<!--/d' site/content/install.md | sed '1d')
  ```
- Release changes appear in each release's post (`content/news/X.Y.Z.md`, its `CHANGELOG.md`
  section, headings one level up). The post opens with the downloads of its own release, taken from
  its file name (`news-post.html`, set as `page_template` in `content/news/_index.md`); the landing
  page's downloads link to the latest release's files.
  The notice at the top of the landing page's hero links to the newest post on its own (Zola's
  `get_section`, RTM-103); the downloads follow the posts too (below).
- The only JavaScript is the screenshot viewer (RTM-99), inline at the end of
  `templates/index.html`: it wraps each screenshot in a button that opens it enlarged in a native
  `<dialog>`, so without JavaScript the screenshots stay plain images. Keep it dependency-free and
  add no other scripts.
- The download links are the `platforms` macro in `templates/macros.html`, given a version. The
  landing page shows the latest release: `latest_version`, the file name of the newest post in
  `content/news/` that does not set `older_line = true` under `[extra]` (a patch on an older line
  sets it). Adding the post in each release's website PR therefore moves the links; there is no
  version to edit ("Website post" in [Release Process](../doc/release-process.md), RTM-91, RTM-104).
- Screenshots and the icon stay where they are; update them there, not in `site/static/`.
- Website-only changes start no Rust or Nix build (`paths-ignore` on push, RTM-87; see
  [CI](../doc/ci.md)).

---

## Future Plans

- Serving the site from perseus as a container image (epic RTM-94; cluster side PER-52).
