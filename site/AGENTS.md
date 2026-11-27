# AGENTS.md — site

The project website (Zola), deployed by Vercel through `vercel.json`; building, previewing and publishing are in [doc/website.md](../doc/website.md).

- Text in `content/`, layout in `templates/`; no third-party theme.
- The only JavaScript is the screenshot viewer in `templates/index.html`: keep it dependency-free and add no other scripts.
- `scripts/site/prepare.sh` copies the images from their sources (the root screenshots, `rtmapp/assets/icons/`); edit them there and never commit the copies or `site/public/`.
- Build site URLs with `get_url`, never root-relative `/path` links, which lychee cannot check offline.
- External links open in a new tab: `target="_blank" rel="noopener"` in the templates.
- Advertise only features the code has.
- `content/install.md` and `content/mcp.md` copy README sections and `content/report.md` follows the issue forms: change them together.
- The download links and the hero notice follow the newest stable post without `older_line`; there is no version to edit.
- `/releases.json` is generated from the posts, never edited; change its `schema` only for an incompatible change.
- `scripts/site/vercel-ignore.sh`'s inputs are `site.yml`'s `paths` without the workflow file: change both together.
- After a nixpkgs bump, run `scripts/site/sync-zola-version.sh` so `vercel.json` pins the Zola of `flake.lock`.
