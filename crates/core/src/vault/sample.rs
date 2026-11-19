//! The built-in sample graph, shown while no folder is chosen.
//!
//! SKELETON: returns an empty graph until the sample is designed.

use super::graph::Graph;

/// A deterministic sample graph; `now_ms` places its edit times so the recency glow
/// shows.
pub fn sample_graph(now_ms: f64) -> Graph {
    let _ = now_ms;
    Graph::default()
}
