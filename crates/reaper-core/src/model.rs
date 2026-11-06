use std::fmt;

/// Where a package comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Pacman,
    Aur,
    Apt,
    Flatpak,
    Rpm,
    Nix,
    Snap,
    Cargo,
    Pip,
    Npm,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Pacman => "pacman",
            Source::Aur => "aur",
            Source::Apt => "apt",
            Source::Flatpak => "flatpak",
            Source::Rpm => "rpm",
            Source::Nix => "nix",
            Source::Snap => "snap",
            Source::Cargo => "cargo",
            Source::Pip => "pip",
            Source::Npm => "npm",
        })
    }
}

/// Unique id: source + package name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PkgId {
    pub source: Source,
    pub name: String,
}

impl PkgId {
    pub fn new(source: Source, name: impl Into<String>) -> Self {
        Self {
            source,
            name: name.into(),
        }
    }
}

impl fmt::Display for PkgId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.source, self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub id: PkgId,
    pub version: String,
    pub description: String,
    /// Installed size in bytes.
    pub size: u64,
    /// Installed by the user (true) or pulled in as a dependency (false).
    pub explicit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepKind {
    /// Required: removing the target breaks the dependent.
    Hard,
    /// Optional / recommended: removing the target only loses a feature.
    Optional,
}

/// `from` depends on `to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: PkgId,
    pub to: PkgId,
    pub kind: DepKind,
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}
