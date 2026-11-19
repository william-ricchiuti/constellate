//! Writes a fixture folder for running the app by hand:
//!
//!   cargo run -p vw-core --example make_fixture -- <kind> <dir> [n]
//!
//! `kind` is logseq, dendron, foam, zettlr, org, plain, tree or huge (`n` files,
//! 100,000 by default). Builds into `dir`; refuses a folder that already has files.

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(kind), Some(dir)) = (args.first(), args.get(1)) else {
        eprintln!("usage: make_fixture <{}> <dir> [n]", fixtures::KINDS.join("|"));
        std::process::exit(2);
    };
    let n = args.get(2).map_or(100_000, |s| s.parse().expect("n is a number"));
    let dir = PathBuf::from(dir);
    if std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_some()) {
        eprintln!("{} is not empty; choose a new folder", dir.display());
        std::process::exit(1);
    }
    if let Err(e) = fixtures::build(kind, &dir, n) {
        eprintln!("{e}");
        std::process::exit(2);
    }
    eprintln!("wrote the {kind} fixture to {}", dir.display());
}
