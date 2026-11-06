use crate::model::{DepKind, Edge, Package, PkgId};
use std::collections::{HashMap, HashSet, VecDeque};

/// Unified dependency graph over every package source.
#[derive(Debug, Default)]
pub struct Graph {
    packages: HashMap<PkgId, Package>,
    deps: HashMap<PkgId, Vec<(PkgId, DepKind)>>,
    rdeps: HashMap<PkgId, Vec<(PkgId, DepKind)>>,
}

impl Graph {
    pub fn new(packages: Vec<Package>, edges: Vec<Edge>) -> Self {
        let mut g = Graph::default();
        for p in packages {
            g.packages.insert(p.id.clone(), p);
        }
        for e in edges {
            // Ignore edges to/from packages we don't know about.
            if !g.packages.contains_key(&e.from)
                || !g.packages.contains_key(&e.to)
                || e.from == e.to
            {
                continue;
            }
            g.deps
                .entry(e.from.clone())
                .or_default()
                .push((e.to.clone(), e.kind));
            g.rdeps.entry(e.to).or_default().push((e.from, e.kind));
        }
        g
    }

    pub fn get(&self, id: &PkgId) -> Option<&Package> {
        self.packages.get(id)
    }

    pub fn packages(&self) -> impl Iterator<Item = &Package> {
        self.packages.values()
    }

    pub fn len(&self) -> usize {
        self.packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    pub fn deps(&self, id: &PkgId) -> &[(PkgId, DepKind)] {
        self.deps.get(id).map_or(&[], Vec::as_slice)
    }

    pub fn rdeps(&self, id: &PkgId) -> &[(PkgId, DepKind)] {
        self.rdeps.get(id).map_or(&[], Vec::as_slice)
    }

    /// Packages that nothing depends on (hard) and that were not explicitly installed.
    pub fn orphans(&self) -> Vec<&Package> {
        let mut v: Vec<_> = self
            .packages
            .values()
            .filter(|p| !p.explicit && !self.rdeps(&p.id).iter().any(|(_, k)| *k == DepKind::Hard))
            .collect();
        v.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));
        v
    }

    /// Everything that would break if `id` were removed (transitive hard reverse deps).
    pub fn broken_by_removing(&self, id: &PkgId) -> Vec<&Package> {
        let mut seen: HashSet<&PkgId> = HashSet::new();
        let mut queue: VecDeque<&PkgId> = VecDeque::new();
        queue.push_back(id);
        while let Some(cur) = queue.pop_front() {
            for (r, kind) in self.rdeps(cur) {
                if *kind == DepKind::Hard && r != id && seen.insert(r) {
                    queue.push_back(r);
                }
            }
        }
        let mut v: Vec<_> = seen
            .into_iter()
            .filter_map(|i| self.packages.get(i))
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    /// Dependencies that would become orphaned (and could be dropped too) if `id` were removed.
    /// Only non-explicit packages are considered, matching `pacman -Rs` semantics.
    pub fn freed_by_removing(&self, id: &PkgId) -> Vec<&Package> {
        let mut removed: HashSet<PkgId> = HashSet::new();
        removed.insert(id.clone());
        let mut queue = VecDeque::from([id.clone()]);
        while let Some(cur) = queue.pop_front() {
            for (d, kind) in self.deps(&cur) {
                if *kind != DepKind::Hard || removed.contains(d) {
                    continue;
                }
                let Some(pkg) = self.packages.get(d) else {
                    continue;
                };
                if pkg.explicit {
                    continue;
                }
                let still_needed = self
                    .rdeps(d)
                    .iter()
                    .any(|(r, k)| *k == DepKind::Hard && !removed.contains(r));
                if !still_needed {
                    removed.insert(d.clone());
                    queue.push_back(d.clone());
                }
            }
        }
        removed.remove(id);
        let mut v: Vec<_> = removed
            .iter()
            .filter_map(|i| self.packages.get(i))
            .collect();
        v.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id)));
        v
    }

    /// Total bytes freed by removing `id` plus everything that becomes orphaned.
    pub fn savings_for(&self, id: &PkgId) -> u64 {
        self.get(id).map_or(0, |p| p.size)
            + self
                .freed_by_removing(id)
                .iter()
                .map(|p| p.size)
                .sum::<u64>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Source;

    fn pkg(name: &str, size: u64, explicit: bool) -> Package {
        Package {
            id: PkgId::new(Source::Pacman, name),
            version: "1".into(),
            description: String::new(),
            size,
            explicit,
        }
    }
    fn edge(a: &str, b: &str, kind: DepKind) -> Edge {
        Edge {
            from: PkgId::new(Source::Pacman, a),
            to: PkgId::new(Source::Pacman, b),
            kind,
        }
    }
    fn id(n: &str) -> PkgId {
        PkgId::new(Source::Pacman, n)
    }

    // app -> libA -> libB ; app -> libC (optional) ; tool(explicit) -> libB ; lonely(dep, nobody needs)
    fn sample() -> Graph {
        Graph::new(
            vec![
                pkg("app", 100, true),
                pkg("liba", 10, false),
                pkg("libb", 20, false),
                pkg("libc", 5, false),
                pkg("tool", 50, true),
                pkg("lonely", 7, false),
            ],
            vec![
                edge("app", "liba", DepKind::Hard),
                edge("liba", "libb", DepKind::Hard),
                edge("app", "libc", DepKind::Optional),
                edge("tool", "libb", DepKind::Hard),
            ],
        )
    }

    #[test]
    fn orphans_are_unneeded_non_explicit() {
        let g = sample();
        let names: Vec<_> = g.orphans().iter().map(|p| p.id.name.clone()).collect();
        // libc is only optionally needed, lonely not at all
        assert_eq!(names, vec!["lonely", "libc"]);
    }

    #[test]
    fn removal_breaks_transitive_dependents() {
        let g = sample();
        let broken: Vec<_> = g
            .broken_by_removing(&id("libb"))
            .iter()
            .map(|p| p.id.name.clone())
            .collect();
        assert_eq!(broken, vec!["app", "liba", "tool"]);
        assert!(g.broken_by_removing(&id("app")).is_empty());
    }

    #[test]
    fn removing_frees_unshared_deps_only() {
        let g = sample();
        // libb is still needed by `tool`, so only liba is freed
        let freed: Vec<_> = g
            .freed_by_removing(&id("app"))
            .iter()
            .map(|p| p.id.name.clone())
            .collect();
        assert_eq!(freed, vec!["liba"]);
        assert_eq!(g.savings_for(&id("app")), 110);
    }

    #[test]
    fn unknown_edges_ignored() {
        let g = Graph::new(
            vec![pkg("a", 1, true)],
            vec![edge("a", "ghost", DepKind::Hard)],
        );
        assert!(g.deps(&id("a")).is_empty());
    }
}
