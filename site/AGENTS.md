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
├── content/_index.md     ← the page's text: tagline (front matter), intro and features (Markdown)
├── templates/
│   ├── base.html         ← skeleton: head, header, footer
│   └── index.html        ← the landing page: hero, screenshots, features, downloads
└── static/style.css      ← the only stylesheet; light and dark through prefers-color-scheme
```

`scripts/site/prepare.sh` copies the images into `site/static/` before a build: the three
screenshots from the repository root (`screenshot_*.png`, RTM-51) and `rtmapp.svg` and
`rtmapp-small.svg` (the favicon) from `rtmapp/assets/icons/`. Zola reads static files only from
inside the site, and copying keeps one source per image. The copies and the output (`site/public/`)
are git-ignored; never commit them.

---

## Build and preview

In the dev shell (`zola` comes from the nixpkgs pinned in `flake.lock`, see
[Nix Packaging](../doc/nix-packaging.md)):

```sh
scripts/site/prepare.sh
zola --root site serve                        # http://127.0.0.1:1111, reloads on change
zola --root site check --skip-external-links  # templates and internal links
zola --root site build                        # into site/public/
```

The built pages use absolute `https://rtm.brcha.com/` URLs (`base_url`), so open them through
`zola serve`, not from disk.

---

## Conventions

- Text in `content/_index.md`, layout in the templates; no third-party theme.
- Advertise only features that exist in the code (subtasks, for example, exist only in the data
  model and are not listed).
- Do not copy the install guide or the changelog: link to `README.md#installing` and to the GitHub
  releases, whose bodies are the changelog (RTM-74).
- Screenshots and the icon stay where they are; update them there, not in `site/static/`.
- Website-only changes start no Rust or Nix build (`paths-ignore` on push, RTM-87; see
  [CI](../doc/ci.md)).

---

## Future Plans

- CI build and link check (RTM-89), publishing for perseus (RTM-90, PER-1), download links that
  follow the newest release (RTM-91).
