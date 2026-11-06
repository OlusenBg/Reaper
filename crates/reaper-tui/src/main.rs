use anyhow::Result;
use clap::{Parser, Subcommand};
use reaper_core::{config::Config, model::human_size, Graph, Package};

#[derive(Parser)]
#[command(
    name = "reaper",
    version,
    about = "See what's installed, what depends on what, and what you can safely reclaim"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List installed packages, largest first
    List {
        /// Only show packages nobody depends on that weren't installed explicitly
        #[arg(long)]
        orphans: bool,
    },
    /// Show dependents and removal impact for one package
    Why { name: String },
}

fn load() -> Result<Graph> {
    let cfg = match Config::default_path() {
        Some(p) => Config::load(&p)?,
        None => Config::default(),
    };
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

fn main() -> Result<()> {
    let cli = Cli::parse();
    let g = load()?;
    match cli.cmd {
        Cmd::List { orphans } => {
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
