//! pacman (+ AUR packages installed through yay/paru, which pacman sees as "foreign").

use crate::{run, Backend, Inventory};
use anyhow::Result;
use reaper_core::{DepKind, Edge, Package, PkgId, Source};
use std::collections::{HashMap, HashSet};

pub struct Pacman;

impl Backend for Pacman {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn detect(&self) -> bool {
        std::process::Command::new("pacman")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn scan(&self) -> Result<Inventory> {
        let qi = run("pacman", &["-Qi"])?;
        // Foreign = not in any sync db = AUR / manual builds.
        let foreign = run("pacman", &["-Qmq"]).unwrap_or_default();
        Ok(build_inventory(
            &qi,
            foreign.lines().map(str::to_owned).collect(),
        ))
    }
}

/// One `pacman -Qi` record.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Record {
    pub name: String,
    pub version: String,
    pub description: String,
    pub size: u64,
    pub explicit: bool,
    pub depends: Vec<String>,
    pub optional: Vec<String>,
    pub provides: Vec<String>,
}

/// Parse the full output of `pacman -Qi`.
pub(crate) fn parse_qi(text: &str) -> Vec<Record> {
    let mut records = Vec::new();
    for block in text.split("\n\n") {
        let fields = parse_block(block);
        let Some(name) = fields.get("Name") else {
            continue;
        };
        let get = |k: &str| fields.get(k).map(String::as_str).unwrap_or("");
        records.push(Record {
            name: name.clone(),
            version: get("Version").to_owned(),
            description: get("Description").to_owned(),
            size: parse_size(get("Installed Size")),
            explicit: get("Install Reason").starts_with("Explicitly"),
            depends: split_list(get("Depends On")),
            optional: get("Optional Deps")
                .lines()
                .filter(|l| !l.trim().is_empty() && l.trim() != "None")
                .map(|l| {
                    // "pkg: reason [installed]"
                    l.split(':').next().unwrap_or("").trim().to_owned()
                })
                .collect(),
            provides: split_list(get("Provides")),
        });
    }
    records
}

/// Fields are `Key  : value`; continuation lines are indented and belong to the previous key.
fn parse_block(block: &str) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut last: Option<String> = None;
    for line in block.lines() {
        if let Some((k, v)) = line.split_once(" : ").filter(|(k, _)| !k.starts_with(' ')) {
            let key = k.trim().to_owned();
            map.insert(key.clone(), v.trim().to_owned());
            last = Some(key);
        } else if line.starts_with(' ') {
            if let Some(v) = last.as_ref().and_then(|k| map.get_mut(k)) {
                // Optional Deps entries are newline-separated; everything else is space-separated.
                let sep = if last.as_deref() == Some("Optional Deps") {
                    "\n"
                } else {
                    " "
                };
                v.push_str(sep);
                v.push_str(line.trim());
            }
        } else if let Some((k, v)) = line.split_once(':') {
            // "Name:" with an empty value (e.g. no space before colon)
            let key = k.trim().to_owned();
            map.insert(key.clone(), v.trim().to_owned());
            last = Some(key);
        }
    }
    map
}

fn split_list(s: &str) -> Vec<String> {
    if s.trim() == "None" {
        return Vec::new();
    }
    s.split_whitespace().map(str::to_owned).collect()
}

/// "1.50 MiB" -> bytes.
pub(crate) fn parse_size(s: &str) -> u64 {
    let mut it = s.split_whitespace();
    let (Some(n), Some(unit)) = (it.next(), it.next()) else {
        return 0;
    };
    let Ok(n) = n.replace(',', ".").parse::<f64>() else {
        return 0;
    };
    let mult: f64 = match unit {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0_f64.powi(4),
        _ => return 0,
    };
    (n * mult) as u64
}

/// Strip version constraint and description: "foo>=1.2" -> "foo", "bar: reason" -> "bar".
fn base_name(dep: &str) -> &str {
    let dep = dep.split(':').next().unwrap_or(dep);
    dep.split(['<', '>', '=']).next().unwrap_or(dep)
}

pub(crate) fn build_inventory(qi: &str, foreign: HashSet<String>) -> Inventory {
    let records = parse_qi(qi);
    let source_of = |n: &str| {
        if foreign.contains(n) {
            Source::Aur
        } else {
            Source::Pacman
        }
    };

    // Dependencies may name a virtual package ("sh", "libfoo.so"); resolve through Provides.
    let mut providers: HashMap<&str, Vec<&str>> = HashMap::new();
    for r in &records {
        providers
            .entry(r.name.as_str())
            .or_default()
            .push(r.name.as_str());
        for p in &r.provides {
            providers
                .entry(base_name(p))
                .or_default()
                .push(r.name.as_str());
        }
    }

    let mut edges = Vec::new();
    for r in &records {
        let from = PkgId::new(source_of(&r.name), &r.name);
        for (list, kind) in [
            (&r.depends, DepKind::Hard),
            (&r.optional, DepKind::Optional),
        ] {
            for dep in list {
                let Some(targets) = providers.get(base_name(dep)) else {
                    continue;
                };
                // Prefer a real package of that name; else the first provider.
                let target = if targets.contains(&base_name(dep)) {
                    base_name(dep)
                } else {
                    targets[0]
                };
                edges.push(Edge {
                    from: from.clone(),
                    to: PkgId::new(source_of(target), target),
                    kind,
                });
            }
        }
    }

    let packages = records
        .into_iter()
        .map(|r| Package {
            id: PkgId::new(source_of(&r.name), &r.name),
            version: r.version,
            description: r.description,
            size: r.size,
            explicit: r.explicit,
        })
        .collect();
    Inventory { packages, edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reaper_core::Graph;

    const FIXTURE: &str = include_str!("../tests/fixtures/pacman-Qi.txt");

    #[test]
    fn sizes() {
        assert_eq!(parse_size("1.50 MiB"), 1_572_864);
        assert_eq!(parse_size("12.00 KiB"), 12_288);
        assert_eq!(parse_size("garbage"), 0);
    }

    #[test]
    fn parses_records() {
        let recs = parse_qi(FIXTURE);
        assert_eq!(recs.len(), 5);
        let fw = recs.iter().find(|r| r.name == "firefox").unwrap();
        assert!(fw.explicit);
        assert_eq!(fw.depends, vec!["gtk3", "libpulse", "sh", "nss>=3.1"]);
        assert_eq!(fw.optional, vec!["hunspell-en_US"]);
        let gtk = recs.iter().find(|r| r.name == "gtk3").unwrap();
        assert!(!gtk.explicit);
        assert_eq!(gtk.size, parse_size("21.00 MiB"));
    }

    #[test]
    fn builds_graph_with_provides_and_aur() {
        let inv = build_inventory(FIXTURE, HashSet::from(["paru-bin".to_owned()]));
        let g = Graph::new(inv.packages, inv.edges);
        assert_eq!(g.len(), 5);
        // `sh` is provided by bash
        let ff = PkgId::new(Source::Pacman, "firefox");
        assert!(g.deps(&ff).iter().any(|(t, _)| t.name == "bash"));
        // the version constraint on nss is stripped
        assert!(g.deps(&ff).iter().any(|(t, _)| t.name == "nss"));
        assert!(g.get(&PkgId::new(Source::Aur, "paru-bin")).is_some());
        // gtk3 is only needed via firefox -> removing firefox frees it
        let freed = g.freed_by_removing(&ff);
        assert!(freed.iter().any(|p| p.id.name == "gtk3"));
    }
}
