# Reaper
Reaper is a linux app made to show you each installed apps, dependencies and programs.

## Status

Early development — see [PLAN.md](PLAN.md). Phases 0–1 done: workspace, core graph/analysis, pacman (+AUR) backend.

```bash
cargo run -p reaper-tui -- list            # packages, largest first
cargo run -p reaper-tui -- list --orphans  # unneeded dependencies
cargo run -p reaper-tui -- why <package>   # what breaks / what is freed if removed
```

Licensed under GPL-3.0.
