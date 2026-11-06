#!/usr/bin/env bash
# Build Reaper from this clone and install the `reaper` command (to ~/.cargo/bin).
# `reaper --update` later pulls this clone and re-runs the same install step,
# so keep the clone where it is.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

command -v cargo >/dev/null || { echo "Rust is required. On Arch: sudo pacman -S rust"; exit 1; }
command -v git   >/dev/null || { echo "git is required. On Arch: sudo pacman -S git"; exit 1; }

cargo install --path crates/reaper-tui --locked --force

BIN="${CARGO_HOME:-$HOME/.cargo}/bin"

# Append a line to a file once (idempotent), creating the file/dir if needed.
add_line() { # file line
  mkdir -p "$(dirname "$1")"
  if grep -qsxF "$2" "$1"; then return 0; fi
  printf '\n# Added by Reaper installer\n%s\n' "$2" >> "$1"
  echo "Added $BIN to PATH in $1"
}

# Already usable in the current environment? Then nothing to do.
case ":$PATH:" in *":$BIN:"*) echo "Done. Run: reaper"; exit 0 ;; esac

# This script runs in bash, so detect the user's login shell instead of $0.
shell="$(basename "${SHELL:-}")"
case "$shell" in
  fish) add_line "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/reaper.fish" "fish_add_path \"$BIN\"" ;;
  zsh)  add_line "${ZDOTDIR:-$HOME}/.zshrc" "export PATH=\"$BIN:\$PATH\"" ;;
  bash) add_line "$HOME/.bashrc" "export PATH=\"$BIN:\$PATH\"" ;;
  *)    echo "Unknown shell '${shell:-?}'. Add $BIN to your PATH manually."; exit 0 ;;
esac
echo "Done. Open a new terminal (or restart your shell), then run: reaper"
[ "$shell" = fish ] && echo "In the current fish session you can also run: fish_add_path $BIN"
exit 0
