#!/bin/bash
# Renders every app state (both themes, split and folded, dialogs, empty and
# install pages) to target/ui-shots/*.png for review against the mockup.
# Needs a display; GTK draws the window itself, so display scaling doesn't
# matter.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f $HOME/.cargo/env ]] && source "$HOME/.cargo/env"
cargo run --quiet --example screenshots -- "${1:-target/ui-shots}"
