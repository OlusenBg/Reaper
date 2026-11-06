mod app;
mod ui;
mod update;

use anyhow::Result;
use clap::{Parser, Subcommand};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use reaper_core::{config::Config, model::human_size, Graph, Package};
use std::io::{IsTerminal, Write};

#[derive(Parser)]
#[command(
    name = "reaper",
    version,
    about = "See what's installed, what depends on what, and what you can safely reclaim.\n\nRun without arguments to open the TUI."
)]
struct Cli {
    /// Update Reaper: pull the repo and reinstall (also accepted as `-update`)
    #[arg(long)]
    update: bool,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Open the TUI (same as running `reaper` alone)
    Tui,
    /// Print installed packages, largest first
    List {
        /// Only show unneeded dependencies (not explicitly installed, nothing requires them)
        #[arg(long)]
        orphans: bool,
    },
    /// Show what depends on a package and what removing it would break/free
    Why { name: String },
}

fn config() -> Result<Config> {
    Ok(match Config::default_path() {
        Some(p) => Config::load(&p)?,
        None => Config::default(),
    })
}

fn load(cfg: &Config) -> Result<Graph> {
    let (mut packages, mut edges) = (Vec::new(), Vec::new());
    for b in reaper_backends::all() {
        if cfg.disabled_backends.iter().any(|d| d == b.name()) || !b.detect() {
            continue;
        }
        let inv = b.scan()?;
        packages.extend(inv.packages);
        edges.extend(inv.edges);
    }
    Ok(Graph::new(packages, edges))
}

fn print_pkgs(pkgs: &[&Package]) {
    for p in pkgs {
        println!(
            "{:<8} {:<40} {:>10}  {}",
            p.id.source,
            p.id.name,
            human_size(p.size),
            if p.explicit { "explicit" } else { "dep" }
        );
    }
}

fn confirm(question: &str) -> bool {
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).is_ok()
        && matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
}

/// `reaper --update`: always explicit, never silent.
fn run_update() -> Result<()> {
    let Some(repo) = update::Repo::locate() else {
        anyhow::bail!(
            "can't find the Reaper repository this binary was built from, so I can't update it.\n\
             Re-clone the repo and run ./install.sh"
        );
    };
    match repo.check()? {
        None => println!("Reaper is already up to date."),
        Some(info) => {
            println!(
                "{} new commit(s) on {}. Updating...",
                info.behind, info.upstream
            );
            repo.update()?;
            println!("Updated. Run `reaper` again to use the new version.");
        }
    }
    Ok(())
}

/// Launch-time check. Silent when offline, up to date, disabled, or not interactive.
/// Returns true if an update was installed (the running binary is now outdated).
fn auto_update(cfg: &Config) -> bool {
    if !cfg.check_updates
        || std::env::var_os("REAPER_NO_UPDATE_CHECK").is_some()
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        return false;
    }
    let Some(repo) = update::Repo::locate() else {
        return false;
    };
    let Ok(Some(info)) = repo.check() else {
        return false;
    };
    println!(
        "A Reaper update is available ({} new commit(s) on {}):",
        info.behind, info.upstream
    );
    for s in &info.summary {
        println!("  - {s}");
    }
    if !confirm("Update now?") {
        println!();
        return false;
    }
    match repo.update() {
        Ok(()) => {
            println!("Updated. Run `reaper` again to use the new version.");
            true
        }
        Err(e) => {
            eprintln!("Update failed: {e:#}\nContinuing with the current version.\n");
            false
        }
    }
}

fn run_tui(cfg: &Config) -> Result<()> {
    if !std::io::stdout().is_terminal() {
        anyhow::bail!("the TUI needs a terminal; use `reaper list` for plain output");
    }
    if auto_update(cfg) {
        return Ok(());
    }
    eprintln!("Scanning installed packages...");
    let graph = load(cfg)?;
    if graph.is_empty() {
        anyhow::bail!("no supported package manager found (only pacman is supported so far)");
    }
    let mut app = app::App::new(graph);
    let mut terminal = ratatui::init();
    let res = (|| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui::draw(f, &mut app))?;
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    app.on_key(k);
                }
            }
        }
        Ok(())
    })();
    ratatui::restore();
    res
}

fn main() -> Result<()> {
    // Accept the single-dash `-update` the way people type it.
    let args = std::env::args_os().map(|a| if a == "-update" { "--update".into() } else { a });
    let cli = Cli::parse_from(args);

    if cli.update {
        return run_update();
    }
    let cfg = config()?;
    match cli.cmd.unwrap_or(Cmd::Tui) {
        Cmd::Tui => run_tui(&cfg)?,
        Cmd::List { orphans } => {
            let g = load(&cfg)?;
            let mut v: Vec<&Package> = if orphans {
                g.orphans()
            } else {
                g.packages().collect()
            };
            v.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));
            print_pkgs(&v);
            let total: u64 = v.iter().map(|p| p.size).sum();
            println!("\n{} packages, {}", v.len(), human_size(total));
        }
        Cmd::Why { name } => {
            let g = load(&cfg)?;
            let Some(p) = g.packages().find(|p| p.id.name == name) else {
                anyhow::bail!("package '{name}' not found");
            };
            let id = p.id.clone();
            println!("{id} {} ({})", p.version, human_size(p.size));
            let broken = g.broken_by_removing(&id);
            println!("\nRemoving it would break {} package(s):", broken.len());
            print_pkgs(&broken);
            let freed = g.freed_by_removing(&id);
            println!("\nAlso freeable (orphaned afterwards): {}", freed.len());
            print_pkgs(&freed);
            println!("\nTotal reclaimed: {}", human_size(g.savings_for(&id)));
        }
    }
    Ok(())
}
