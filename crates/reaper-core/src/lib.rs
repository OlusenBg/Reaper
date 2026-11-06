//! Reaper core: package model, dependency graph and analysis. Pure logic, no I/O.

pub mod config;
pub mod graph;
pub mod model;

pub use graph::Graph;
pub use model::{DepKind, Edge, Package, PkgId, Source};
