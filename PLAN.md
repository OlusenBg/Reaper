# Reaper — Project Plan

> A fast, discreet, read-only-first TUI that tells you what's installed on your Linux box,
> what you actually use, what depends on what, and how much space you'd get back by cleaning up.

## 1. Goals

- **Unified inventory** across every package source (pacman, AUR helpers, apt/dpkg, flatpak, dnf/rpm, zypper, nix, snap, cargo/pip/npm globals).
- **Usage insight**: most-used, rarely-used and never-used apps.
- **Dependency insight**: what depends on a package, what it depends on, "what breaks if I remove X".
- **Savings insight**: "you can free N GB by removing these unused/orphaned packages".
- **Discreet**: no telemetry, no network, near-zero RAM/CPU, daemon strictly opt-in.
- **v1 is read-only.** Removal comes later (see Phase 6).

## 2. Stack

- **Rust** + **ratatui** + **crossterm**
- `clap` (CLI), `serde` + `toml` (config), `rusqlite` (bundled SQLite for usage DB / cache)
- `rayon` for parallel backend scans, `petgraph` for the dependency graph, `anyhow`/`thiserror` for errors
- Shell out to native tools (`pacman`, `dpkg-query`, `flatpak`, ...) and parse their output; no libalpm/libapt bindings (portable, no C deps).

## 3. Architecture

```
reaper/
├─ crates/
│  ├─ reaper-core/      # Package model, dependency graph, analysis (pure logic, heavily tested)
│  ├─ reaper-backends/  # One module per package manager, implements `Backend` trait
│  ├─ reaper-usage/     # Usage sources + SQLite store
│  ├─ reaper-daemon/    # Optional tracker (systemd user service)
│  └─ reaper-tui/       # ratatui UI + `reaper` binary
```

### Core model

```rust
struct Package { id, name, version, source: Source, size_installed, install_date,
                 explicit: bool, description, binaries: Vec<PathBuf>, desktop_entries: Vec<..> }
struct Edge    { from: PkgId, to: PkgId, kind: Hard | Optional | Recommends | Runtime }

trait Backend {
    fn detect(&self) -> bool;                       // is this package manager present?
    fn list(&self) -> Result<Vec<Package>>;
    fn edges(&self) -> Result<Vec<Edge>>;
    fn files(&self, p: &Package) -> Result<Vec<PathBuf>>;   // for usage mapping
    fn remove_command(&self, pkgs: &[Package]) -> Command;  // Phase 6 only; never auto-run in v1
}
```

All backends feed one **unified dependency graph**. Cross-source edges are rare, so it is mostly
disjoint per source, but the UI treats it as one.

### Backend data sources

| Backend | List / size | Deps | Explicit vs dep | Notes |
|---|---|---|---|---|
| pacman | `pacman -Qi` | `pacman -Qi` (Depends On / Optional / Required By) | `-Qe` / `-Qd` | AUR = `-Qm` (foreign); works with yay/paru |
| apt/dpkg | `dpkg-query -W` | `apt-cache depends/rdepends` | `apt-mark showmanual` | |
| flatpak | `flatpak list --columns=...` | runtime relations | apps vs runtimes | Detect unused runtimes |
| dnf/rpm | `rpm -qa --qf` | `rpm -q --requires/--whatrequires` | `dnf repoquery --userinstalled` | |
| zypper | `rpm` + `zypper` | same as rpm | `zypper se -i` | |
| nix | `nix-store -q` | `--references` / `--referrers` | profile vs closure | |
| snap | `snap list` | base/content snaps | | |
| cargo/pip/npm | `cargo install --list`, `pip list`, `npm ls -g` | flat | all explicit | Shown as "language-level globals" |

## 4. Usage tracking (opt-in, minimal footprint)

Linux doesn't record "last launched", so Reaper builds that history itself. Tiered, cheapest first:

1. **Passive, no daemon (always available):**
   - Shell history (bash/zsh/fish) → CLI command counts, mapped to owning package
   - `.desktop` launch hints, recent-files data, flatpak/snap per-app data dir mtimes
   - Binary `atime` (only when the mount isn't `noatime`; flagged low-confidence)
2. **Optional daemon (`reaperd`, systemd *user* service, off by default):**
   - Toggled with `reaper daemon enable|disable|status`
   - **Lightweight mode:** sample `/proc/*/exe` on a slow interval (default ~5s) and record newly seen exec paths. Tiny RAM, no root, but can miss very short-lived commands.
   - **Accurate mode (separate opt-in, needs privileges):** `fanotify` `FAN_OPEN_EXEC` or the proc connector to catch every exec.
   - Stores aggregated rows only: `(package, day, launch_count, last_seen)`, so the DB stays tiny.
   - Hard budgets: target < 5 MB RSS, ~0% idle CPU; `reaper daemon status` reports its real usage.
   - **Local only. No network, no telemetry.**
3. **Exec path → package mapping:** each backend's `files()` builds an index (`/usr/bin/foo` → `foo-pkg`), cached in SQLite and invalidated when the package DB changes.

Usage always carries a **confidence indicator** ("tracked 42 days" vs "estimated from atime").

## 5. TUI design

- **Tabs:** Overview · Packages · Usage · Dependencies · Cleanup
- **Overview:** totals per source, disk used, top 10 most used, "potential savings" headline.
- **Packages:** sortable/filterable table (name, source, size, last used, uses/30d, explicit/dep), fuzzy search.
- **Detail pane:** description, files, deps, reverse deps, last used.
- **Dependencies:** tree both directions + "removal impact" (everything that would be removed or broken).
- **Cleanup:** candidates ranked by size × unused-ness: orphans, explicit-but-never-used, unused flatpak runtimes, caches/old versions; running total "select these → free X GB".
- **Keys:** vim-style + arrows, `/` search, `s` sort, `f` filter, `Tab` switch pane, `?` help.
- **Non-TUI:** `reaper report --json|--md` shares the same core for scripting.

## 6. Roadmap

| Phase | Deliverable |
|---|---|
| **0 – Scaffolding** | Cargo workspace, CI (fmt/clippy/test), README, config loader |
| **1 – Core + pacman** | Package model, graph, pacman/AUR backend, `reaper list` CLI, fixture tests |
| **2 – TUI MVP** | Packages table, detail pane, deps/rdeps view, sizes |
| **3 – More backends** | apt, flatpak, dnf/rpm, zypper, nix, snap, cargo/pip/npm (fixture tests each) |
| **4 – Usage** | Passive sources, SQLite store, exec→package mapping, Usage tab |
| **5 – Daemon** | `reaperd` user service, enable/disable/status, resource budgets, unit file |
| **6 – Cleanup & removal** | Cleanup tab, "what breaks" simulation, then opt-in removal: show exact native command, dry-run first, explicit confirmation, protected packages |
| **7 – Release** | AUR package, `.deb`, cargo install, man page, shell completions |

You asked for all package managers at once; the plan keeps them all in scope but lands them in
Phase 3 behind a common trait, so pacman (your main machine) works end-to-end first and the rest
plug in without rewriting the UI.

## 7. Testing

- Parsers tested against **recorded real outputs** per package manager (no need for every distro in CI).
- Graph/analysis logic unit-tested (removal impact, orphan detection, savings math).
- Container integration tests (Arch, Debian, Fedora) for backend smoke tests.
- Daemon RSS/CPU benchmarked in CI to enforce the "discreet" promise.

## 8. Safety principles

- Read-only until Phase 6; nothing that modifies the system runs silently.
- Protected list (kernel, glibc, systemd, bootloader, the package manager itself) flagged "do not remove".
- Unknown usage is shown as "unknown", never "unused".
- No network, no telemetry; data lives in `~/.config/reaper` and `~/.local/share/reaper`.

## 9. Open questions

1. **Daemon accuracy vs privilege:** is the rootless `/proc` sampler enough for v1, with root `fanotify` as a later opt-in?
2. **License:** repo has GPL-3.0; keep it?
3. **Binary name:** `reaper` clashes with the Reaper DAW on some distros. Keep it, or use something like `reaper-tui`?
4. **Distros to test first:** Arch + Debian assumed; add Fedora early?
5. **Config format:** TOML fine?
