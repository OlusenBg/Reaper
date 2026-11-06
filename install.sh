#!/usr/bin/env bash
# Build Reaper from this clone and install the `reaper` command (to ~/.cargo/bin).
# `reaper --update` later pulls this clone and re-runs the same install step,
# so keep the clone where it is.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

command -v cargo >/dev/null || { echo "Rust is required. On Arch: sudo pacman -S rust"; exit 1; }
command -v git   >/dev/null || { echo "git is required. On Arch: sudo pacman -S git"; exit 1; }

cargo install --path crates/reaper-tui --locked --force

case ":$PATH:" in
  *":$HOME/.cargo/bin:"*) echo "Done. Run: reaper" ;;
  *) echo "Done, but ~/.cargo/bin is not in your PATH. Add this to your shell config:"
     echo '  export PATH="$HOME/.cargo/bin:$PATH"' ;;
esac
