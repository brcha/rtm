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
│   └── install.md        ← the install guide, a copy of README.md's "Installing" section
├── templates/
│   ├── base.html         ← skeleton: head, header (Install, Download, GitHub), footer
│   ├── index.html        ← the landing page: hero, screenshots, features, downloads
│   └── page.html         ← plain pages (the install guide)
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
  section); the landing page links to the GitHub releases.
- No JavaScript. The download links are updated by hand in each release's website PR, a step of
  the release process ("Website post" in [Release Process](../doc/release-process.md), RTM-91).
  Until the first CalVer release they point at the releases page.
- Screenshots and the icon stay where they are; update them there, not in `site/static/`.
- Website-only changes start no Rust or Nix build (`paths-ignore` on push, RTM-87; see
  [CI](../doc/ci.md)).

---

## Future Plans

- Publishing for perseus (RTM-90, PER-1).
- The news section and the real download links, with the first CalVer release's website post.
