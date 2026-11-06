//! TUI state and key handling (no drawing here, so it is easy to test).

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;
use reaper_core::{DepKind, Graph, Package, PkgId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Size,
    Name,
}

/// Expensive-to-compute info about the selected package, cached per selection.
pub struct Detail {
    pub id: PkgId,
    pub deps: Vec<(String, DepKind)>,
    pub rdeps: Vec<(String, DepKind)>,
    pub broken: usize,
    pub freed: usize,
    pub reclaim: u64,
}

pub struct App {
    pub graph: Graph,
    pub rows: Vec<PkgId>,
    pub table: TableState,
    pub filter: String,
    pub searching: bool,
    pub orphans_only: bool,
    pub sort: Sort,
    pub show_help: bool,
    pub quit: bool,
    pub detail: Option<Detail>,
    pub total_size: u64,
    pub orphan_size: u64,
}

impl App {
    pub fn new(graph: Graph) -> Self {
        let total_size = graph.packages().map(|p| p.size).sum();
        let orphan_size = graph.orphans().iter().map(|p| p.size).sum();
        let mut app = App {
            graph,
            rows: Vec::new(),
            table: TableState::default(),
            filter: String::new(),
            searching: false,
            orphans_only: false,
            sort: Sort::Size,
            show_help: false,
            quit: false,
            detail: None,
            total_size,
            orphan_size,
        };
        app.refilter();
        app
    }

    pub fn selected(&self) -> Option<&Package> {
        self.table
            .selected()
            .and_then(|i| self.rows.get(i))
            .and_then(|id| self.graph.get(id))
    }

    fn refilter(&mut self) {
        let keep = self.selected().map(|p| p.id.clone());
        let needle = self.filter.to_lowercase();
        let mut pkgs: Vec<&Package> = if self.orphans_only {
            self.graph.orphans()
        } else {
            self.graph.packages().collect()
        };
        pkgs.retain(|p| needle.is_empty() || p.id.name.to_lowercase().contains(&needle));
        match self.sort {
            Sort::Size => pkgs.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.id.cmp(&b.id))),
            Sort::Name => pkgs.sort_by(|a, b| a.id.cmp(&b.id)),
        }
        self.rows = pkgs.into_iter().map(|p| p.id.clone()).collect();
        let idx = keep.and_then(|k| self.rows.iter().position(|r| *r == k));
        self.table
            .select(idx.or(if self.rows.is_empty() { None } else { Some(0) }));
        self.update_detail();
    }

    fn move_by(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let cur = self.table.selected().unwrap_or(0) as isize;
        let next = (cur + delta).clamp(0, self.rows.len() as isize - 1);
        self.table.select(Some(next as usize));
        self.update_detail();
    }

    fn update_detail(&mut self) {
        let Some(p) = self.selected() else {
            self.detail = None;
            return;
        };
        let id = p.id.clone();
        if self.detail.as_ref().is_some_and(|d| d.id == id) {
            return;
        }
        let names = |v: &[(PkgId, DepKind)]| {
            let mut n: Vec<_> = v.iter().map(|(i, k)| (i.name.clone(), *k)).collect();
            n.sort();
            n
        };
        self.detail = Some(Detail {
            deps: names(self.graph.deps(&id)),
            rdeps: names(self.graph.rdeps(&id)),
            broken: self.graph.broken_by_removing(&id).len(),
            freed: self.graph.freed_by_removing(&id).len(),
            reclaim: self.graph.savings_for(&id),
            id,
        });
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.searching {
            match key.code {
                KeyCode::Enter => self.searching = false,
                KeyCode::Esc => {
                    self.searching = false;
                    self.filter.clear();
                    self.refilter();
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.refilter();
                }
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.refilter();
                }
                _ => {}
            }
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Esc if !self.filter.is_empty() => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::PageDown => self.move_by(10),
            KeyCode::PageUp => self.move_by(-10),
            KeyCode::Char('g') | KeyCode::Home => self.move_by(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.move_by(isize::MAX / 2),
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('o') => {
                self.orphans_only = !self.orphans_only;
                self.refilter();
            }
            KeyCode::Char('s') => {
                self.sort = if self.sort == Sort::Size {
                    Sort::Name
                } else {
                    Sort::Size
                };
                self.refilter();
            }
            KeyCode::Char('?') => self.show_help = true,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reaper_core::{Edge, Source};

    fn pkg(n: &str, size: u64, explicit: bool) -> Package {
        Package {
            id: PkgId::new(Source::Pacman, n),
            version: "1".into(),
            description: String::new(),
            size,
            explicit,
        }
    }

    fn app() -> App {
        let g = Graph::new(
            vec![
                pkg("app", 100, true),
                pkg("lib", 10, false),
                pkg("junk", 50, false),
            ],
            vec![Edge {
                from: PkgId::new(Source::Pacman, "app"),
                to: PkgId::new(Source::Pacman, "lib"),
                kind: DepKind::Hard,
            }],
        );
        App::new(g)
    }

    fn press(a: &mut App, c: KeyCode) {
        a.on_key(KeyEvent::new(c, KeyModifiers::NONE));
    }

    #[test]
    fn sorted_by_size_and_selected_first() {
        let a = app();
        let names: Vec<_> = a.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["app", "junk", "lib"]);
        assert_eq!(a.selected().unwrap().id.name, "app");
        assert_eq!(a.detail.as_ref().unwrap().reclaim, 110);
    }

    #[test]
    fn navigation_is_clamped() {
        let mut a = app();
        press(&mut a, KeyCode::Char('k'));
        assert_eq!(a.table.selected(), Some(0));
        press(&mut a, KeyCode::Char('G'));
        assert_eq!(a.table.selected(), Some(2));
        press(&mut a, KeyCode::Down);
        assert_eq!(a.table.selected(), Some(2));
        press(&mut a, KeyCode::Char('g'));
        assert_eq!(a.table.selected(), Some(0));
    }

    #[test]
    fn search_filters_and_esc_clears() {
        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        for c in "ju".chars() {
            press(&mut a, KeyCode::Char(c));
        }
        assert_eq!(a.rows.len(), 1);
        press(&mut a, KeyCode::Enter);
        assert!(!a.searching);
        press(&mut a, KeyCode::Esc);
        assert!(a.filter.is_empty());
        assert_eq!(a.rows.len(), 3);
        assert!(!a.quit);
        press(&mut a, KeyCode::Esc);
        assert!(a.quit);
    }

    #[test]
    fn orphan_filter_and_sort_toggle() {
        let mut a = app();
        press(&mut a, KeyCode::Char('o'));
        let names: Vec<_> = a.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["junk"]);
        press(&mut a, KeyCode::Char('o'));
        press(&mut a, KeyCode::Char('s'));
        let names: Vec<_> = a.rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["app", "junk", "lib"]);
        assert_eq!(a.sort, Sort::Name);
    }

    #[test]
    fn q_quits_but_not_while_searching() {
        let mut a = app();
        press(&mut a, KeyCode::Char('/'));
        press(&mut a, KeyCode::Char('q'));
        assert!(!a.quit);
        assert_eq!(a.filter, "q");
    }
}
