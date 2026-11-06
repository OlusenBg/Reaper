# Reaper
Reaper is a linux app made to show you each installed apps, dependencies and programs.

## Install (Arch)

```bash
sudo pacman -S git rust
git clone https://github.com/OlusenBg/Reaper && cd Reaper
./install.sh
```

`install.sh` detects your login shell (fish, zsh or bash) and adds `~/.cargo/bin` to your PATH if needed
(fish: `~/.config/fish/conf.d/reaper.fish`, using `fish_add_path`). Open a new terminal, then just run:

```bash
reaper
```

Keep the cloned folder: `reaper --update` pulls it and rebuilds from it.

## Usage

| Command | What it does |
|---|---|
| `reaper` | Open the TUI |
| `reaper -h` | List all commands |
| `reaper -update` | Pull the repo and reinstall (`--update` works too) |
| `reaper list [--orphans]` | Plain-text package list |
| `reaper why <pkg>` | What breaks / what is freed if you remove it |

On launch Reaper checks (quietly, 5s timeout, silent when offline) for a new version and asks
`Update now? [y/N]`. Disable it with `check_updates = false` in `~/.config/reaper/config.toml`
or by setting `REAPER_NO_UPDATE_CHECK=1`. The prompt only appears in an interactive terminal.

TUI keys: `j/k` move · `/` search · `o` unneeded dependencies only · `s` sort · `?` help · `q` quit.

## Status

Early development, see [PLAN.md](PLAN.md). Done: core graph/analysis, pacman (+AUR) backend, basic TUI, self-update.
Reaper is read-only: it never removes anything.

Licensed under GPL-3.0.
