#!/usr/bin/env bash
# Copies the images the website uses into site/static/: the screenshots from the repository root
# and the app icon from rtmapp/assets/icons/. Zola reads static files only from inside the site,
# and copying keeps a single source for each image. The copies are git-ignored. Run it before
# `zola --root site build` or `zola --root site serve`; see site/AGENTS.md.
#
# Usage: scripts/site/prepare.sh

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

cp screenshot_gui.png screenshot_gui_dark.png screenshot_cli.png \
    rtmapp/assets/icons/rtmapp.svg rtmapp/assets/icons/rtmapp-small.svg \
    site/static/
