//! Package-manager backends. Each one shells out to the native tool and parses its output.

pub mod pacman;

use anyhow::Result;
use reaper_core::{Edge, Package};

/// Everything a backend knows about the system.
#[derive(Debug, Default)]
pub struct Inventory {
    pub packages: Vec<Package>,
    pub edges: Vec<Edge>,
}

pub trait Backend {
    fn name(&self) -> &'static str;
    /// Is this package manager present on the system?
    fn detect(&self) -> bool;
    fn scan(&self) -> Result<Inventory>;
}

/// All known backends, in display order.
pub fn all() -> Vec<Box<dyn Backend>> {
    vec![Box::new(pacman::Pacman)]
}

pub(crate) fn run(cmd: &str, args: &[&str]) -> Result<String> {
    use anyhow::{bail, Context};
    let out = std::process::Command::new(cmd)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .with_context(|| format!("failed to run {cmd}"))?;
    if !out.status.success() {
        bail!(
            "{cmd} {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
