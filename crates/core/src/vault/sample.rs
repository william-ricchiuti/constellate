//! The built-in sample graph, shown while no folder is chosen.
//!
//! A small made-up notes vault: five top-level folders, a few hub notes, six tags, some
//! dangling links (ghosts) and edit times spread from minutes to months before `now_ms`,
//! so the recency glow has something to show. Everything comes from the static tables
//! below and a fixed integer mixer: the same `now_ms` always gives the same graph.

use std::collections::BTreeMap;

use rustc_hash::FxHashSet as HashSet;

use super::graph::{Graph, Link, Node};

const MINUTE: f64 = 60_000.0;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;

/// Notes at the vault root. The first is the home hub.
const ROOT: &[&str] = &["Start here", "Inbox", "Weekly review", "Someday maybe"];

/// A top-level folder: its name, its main tag, and its notes. The first note is the
/// folder's hub and links to every other note in it.
struct Folder {
    name: &'static str,
    tag: &'static str,
    notes: &'static [&'static str],
}

const FOLDERS: &[Folder] = &[
    Folder {
        name: "Projects",
        tag: "project",
        notes: &[
            "Projects overview",
            "Website redesign",
            "Garden plan",
            "Kitchen renovation",
            "Photo archive cleanup",
            "Conference talk draft",
            "Newsletter relaunch",
            "Home server setup",
            "Budget spreadsheet",
            "Moving checklist",
            "Language course",
            "Podcast pilot",
            "Bike tour route",
            "Book club reading order",
            "Family recipe collection",
            "Portfolio update",
            "Backup strategy",
            "Weekend workshop",
            "Grant application",
            "Volunteer schedule",
            "Short story draft",
            "Board game prototype",
            "Sourdough experiments",
            "Running plan",
            "Desk setup",
            "Coast trip",
            "Tool library",
            "Quarterly goals",
        ],
    },
    Folder {
        name: "Areas",
        tag: "area",
        notes: &[
            "Areas overview",
            "Health",
            "Finances",
            "Household",
            "Career",
            "Learning",
            "Friends and family",
            "Writing",
            "Fitness",
            "Sleep",
            "Cooking",
            "Garden",
            "Travel",
            "Reading habits",
            "Tools and workflows",
            "Meetings",
            "Mentoring",
            "Volunteering",
            "Music practice",
            "Photography",
            "Admin and paperwork",
            "Side projects",
        ],
    },
    Folder {
        name: "Reading",
        tag: "reading",
        notes: &[
            "Reading list",
            "Notes on habit formation",
            "Notes on deep focus",
            "Notes on systems thinking",
            "Notes on the history of maps",
            "Notes on urban walking",
            "Notes on old letters",
            "Notes on note-taking",
            "Notes on attention",
            "Notes on slow cooking",
            "Notes on distance running",
            "Notes on making decisions",
            "Notes on memory",
            "Notes on small gardens",
            "Notes on writing clearly",
            "Notes on negotiation",
            "Notes on design basics",
            "Notes on sleep",
            "Notes on personal finance",
            "Notes on teaching",
            "Article on checklists",
            "Article on lists",
            "Paper on the spacing effect",
            "Talk on working in public",
            "Essay on boredom",
            "Essay on craft",
        ],
    },
    Folder {
        name: "Ideas",
        tag: "idea",
        notes: &[
            "Ideas index",
            "Spaced repetition",
            "Deep work",
            "Atomic notes",
            "Progressive summarization",
            "Second-order effects",
            "Feedback loops",
            "Compounding",
            "Friction and defaults",
            "Constraints help creativity",
            "Small bets",
            "Writing to think",
            "Make it easy to start",
            "Energy over time",
            "Batching chores",
            "Inversion",
            "Good enough is fine",
            "Seasonal rhythm",
            "Learning in public",
            "Tiny experiments",
            "Mental models",
            "Rest is part of work",
            "Ask better questions",
            "Systems over goals",
            "The map is not the territory",
            "Shared vocabulary",
            "A garden, not a stream",
            "The 80/20 rule",
        ],
    },
];

/// Days before `now_ms` of the daily notes in `Journal/`, newest first.
const JOURNAL_DAYS: &[u32] = &[
    0, 1, 2, 3, 4, 6, 7, 9, 11, 14, 16, 19, 22, 26, 30, 35, 41, 47, 54, 62, 70, 79, 88, 98, 109, 121, 134, 148,
];

/// Link targets that have no note yet.
const GHOSTS: &[&str] = &[
    "Meal plan",
    "Car maintenance",
    "Trip budget",
    "Reading goals",
    "Morning routine",
    "Team retrospective",
    "Gift ideas",
];

/// A fixed integer mixer (SplitMix64's finaliser): spreads small inputs over 64 bits.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn mix2(a: u64, b: u64) -> u64 {
    mix(mix(a) ^ b.wrapping_mul(0x2545_F491_4F6C_DD1D))
}

/// `YYYY-MM-DD` (UTC) for a time in ms since the epoch.
fn ymd(ms: f64) -> String {
    // Howard Hinnant's days-to-civil.
    let z = (ms / DAY).floor() as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// How long ago a non-journal note was last edited: mostly weeks to months, some days,
/// a few hours or minutes.
fn age(seed: u64) -> f64 {
    let r = mix(seed);
    let pick = (r % 100) as f64;
    let frac = ((r >> 8) % 1000) as f64 / 1000.0;
    if pick < 6.0 {
        (3.0 + frac * 50.0) * MINUTE
    } else if pick < 14.0 {
        (1.0 + frac * 20.0) * HOUR
    } else if pick < 32.0 {
        (1.0 + frac * 6.0) * DAY
    } else if pick < 60.0 {
        (7.0 + frac * 28.0) * DAY
    } else {
        (35.0 + frac * 200.0) * DAY
    }
}

struct Builder {
    nodes: Vec<Node>,
    links: Vec<Link>,
    seen: HashSet<(usize, usize)>,
}

impl Builder {
    fn note(&mut self, id: String, folder: Option<&str>, tags: Vec<&str>, mtime: f64) -> usize {
        let tags: Vec<String> = tags.into_iter().map(String::from).collect();
        self.nodes.push(Node {
            id,
            label: None,
            tag: tags.first().cloned(),
            tags: (tags.len() > 1).then_some(tags),
            mtime: Some(mtime.round()),
            folder: folder.map(String::from),
            ghost: false,
            weight: None,
        });
        self.nodes.len() - 1
    }

    fn link(&mut self, s: usize, t: usize) {
        if s != t && self.seen.insert((s, t)) {
            self.links.push(Link { source: self.nodes[s].id.clone(), target: self.nodes[t].id.clone() });
        }
    }
}

/// A deterministic sample graph; `now_ms` places its edit times so the recency glow
/// shows.
pub fn sample_graph(now_ms: f64) -> Graph {
    let mut b = Builder { nodes: Vec::with_capacity(160), links: Vec::with_capacity(480), seen: HashSet::default() };

    // Root notes.
    let root: Vec<usize> = ROOT
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let tags = if *name == "Inbox" { vec!["todo"] } else { vec![] };
            // The home note and the inbox were touched today.
            let ago = [40.0 * MINUTE, 7.0 * MINUTE, 2.0 * DAY, 60.0 * DAY][i];
            b.note((*name).to_string(), None, tags, now_ms - ago)
        })
        .collect();
    let (home, inbox, weekly, someday) = (root[0], root[1], root[2], root[3]);

    // Folders: hub first, every hub linked from home and to its notes.
    let mut groups: Vec<Vec<usize>> = Vec::with_capacity(FOLDERS.len());
    for (f, folder) in FOLDERS.iter().enumerate() {
        let mut ids = Vec::with_capacity(folder.notes.len());
        for (i, name) in folder.notes.iter().enumerate() {
            let mut tags = vec![folder.tag];
            match folder.tag {
                "project" if i % 3 == 1 => tags.push("todo"),
                "area" if i % 4 == 3 => tags.clear(),
                "reading" if i % 5 == 2 => tags.push("idea"),
                "idea" if i % 7 == 4 => tags.push("reading"),
                _ => {}
            }
            let ago = if i == 0 { (2 + f) as f64 * DAY } else { age(((f as u64) << 16) | i as u64) };
            ids.push(b.note((*name).to_string(), Some(folder.name), tags, now_ms - ago));
        }
        groups.push(ids);
    }
    for g in &groups {
        b.link(home, g[0]);
        for &n in &g[1..] {
            b.link(g[0], n);
        }
    }
    b.link(home, inbox);
    b.link(home, weekly);
    b.link(home, someday);
    let (projects, areas, reading, ideas) = (&groups[0], &groups[1], &groups[2], &groups[3]);

    // Daily notes: each links to the day before, a project or two and an idea; every
    // weekly review links back to the week's notes.
    let mut journal = Vec::with_capacity(JOURNAL_DAYS.len());
    for (i, &d) in JOURNAL_DAYS.iter().enumerate() {
        let day = now_ms - f64::from(d) * DAY;
        let ago = f64::from(d) * DAY + (12.0 + (mix(900 + i as u64) % 300) as f64) * MINUTE;
        let tags = if i % 5 == 0 { vec!["daily", "todo"] } else { vec!["daily"] };
        journal.push(b.note(ymd(day), Some("Journal"), tags, now_ms - ago));
    }
    for (i, &j) in journal.iter().enumerate() {
        if let Some(&prev) = journal.get(i + 1) {
            b.link(j, prev);
        }
        let r = mix(1000 + i as u64);
        b.link(j, projects[1 + (r % (projects.len() as u64 - 1)) as usize]);
        if r & 0x100 != 0 {
            b.link(j, projects[1 + ((r >> 12) % 6) as usize]);
        }
        b.link(j, ideas[1 + ((r >> 24) % (ideas.len() as u64 - 1)) as usize]);
        if JOURNAL_DAYS[i] < 8 {
            b.link(weekly, j);
        }
    }
    b.link(weekly, inbox);
    b.link(weekly, projects[0]);

    // Cross links: every note in a folder links to one to three others, most often to
    // a handful of popular ideas and areas, which become the graph's secondary hubs.
    let mut seed = 5000u64;
    for g in [projects, areas, reading, ideas] {
        for &n in &g[1..] {
            seed += 1;
            let r = mix(seed);
            for k in 0..1 + r % 3 {
                let r = mix2(seed, k);
                let pool = match r % 4 {
                    0 => ideas,
                    1 => areas,
                    2 => reading,
                    _ => projects,
                };
                // Squaring a uniform pick skews it towards the front of the pool.
                let m = pool.len() as u64 - 1;
                let u = (r >> 8) % m;
                let idx = 1 + (u * u / m) as usize;
                b.link(n, pool[idx]);
            }
        }
    }
    b.link(inbox, ideas[0]);
    b.link(someday, projects[21]);
    b.link(someday, projects[11]);

    // Ghosts: links to notes nobody has written yet.
    let real = b.nodes.len();
    for (g, name) in GHOSTS.iter().enumerate() {
        b.nodes.push(Node { id: (*name).to_string(), ghost: true, ..Node::default() });
        let ghost = b.nodes.len() - 1;
        for k in 0..1 + (g as u64 % 3) {
            let src = 1 + (mix2(7000 + g as u64, k) % (real as u64 - 1)) as usize;
            b.link(src, ghost);
        }
    }

    let mut discovered_tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut tagged = 0;
    for n in &b.nodes {
        let tags: Vec<&String> = match (&n.tags, &n.tag) {
            (Some(all), _) => all.iter().collect(),
            (None, Some(t)) => vec![t],
            (None, None) => continue,
        };
        tagged += 1;
        for t in tags {
            *discovered_tags.entry(t.clone()).or_default() += 1;
        }
    }

    Graph { nodes: b.nodes, links: b.links, tagged, discovered_tags, capped: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: f64 = 1_795_000_000_000.0; // 2026-11-18, 06:13 UTC

    #[test]
    fn deterministic() {
        assert_eq!(sample_graph(NOW), sample_graph(NOW));
    }

    #[test]
    fn shape_looks_like_a_small_vault() {
        let g = sample_graph(NOW);
        let ghosts = g.nodes.iter().filter(|n| n.ghost).count();
        let folders: HashSet<&str> = g.nodes.iter().filter_map(|n| n.folder.as_deref()).collect();
        eprintln!(
            "sample: {} nodes ({} ghosts), {} links, {} tags {:?}, {} folders, {} tagged",
            g.nodes.len(),
            ghosts,
            g.links.len(),
            g.discovered_tags.len(),
            g.discovered_tags,
            folders.len(),
            g.tagged
        );
        assert!((120..=180).contains(&g.nodes.len()), "{} nodes", g.nodes.len());
        assert!((3..=10).contains(&ghosts));
        assert!((5..=7).contains(&g.discovered_tags.len()));
        assert!((4..=6).contains(&folders.len()));
        assert!(!g.capped);
        assert!(g.nodes.iter().any(|n| n.tags.as_ref().is_some_and(|t| t.len() == 2)));

        // A few hubs with many links.
        let mut degree: std::collections::HashMap<&str, usize> = Default::default();
        for l in &g.links {
            *degree.entry(&l.source).or_default() += 1;
            *degree.entry(&l.target).or_default() += 1;
        }
        let mut top: Vec<(&str, usize)> = degree.into_iter().collect();
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        eprintln!("busiest: {:?}", &top[..10]);
        assert!(top.iter().filter(|(_, d)| *d >= 15).count() >= 4);
    }

    #[test]
    fn links_are_valid() {
        let g = sample_graph(NOW);
        let ids: HashSet<&str> = g.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids.len(), g.nodes.len(), "ids are unique");
        let mut pairs = HashSet::default();
        for l in &g.links {
            assert!(ids.contains(l.source.as_str()) && ids.contains(l.target.as_str()), "{l:?}");
            assert_ne!(l.source, l.target);
            assert!(pairs.insert((&l.source, &l.target)), "duplicate {l:?}");
        }
        // Every real note is connected to something.
        let linked: HashSet<&str> = g.links.iter().flat_map(|l| [l.source.as_str(), l.target.as_str()]).collect();
        assert!(g.nodes.iter().all(|n| linked.contains(n.id.as_str())));
    }

    #[test]
    fn tag_counts_match() {
        let g = sample_graph(NOW);
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut tagged = 0;
        for n in &g.nodes {
            let all = n.tags.clone().or_else(|| n.tag.clone().map(|t| vec![t])).unwrap_or_default();
            if let Some(tags) = &n.tags {
                assert!(tags.len() > 1);
                assert_eq!(n.tag.as_ref(), tags.first());
            }
            if !all.is_empty() {
                tagged += 1;
            }
            for t in all {
                *counts.entry(t).or_default() += 1;
            }
        }
        assert_eq!(counts, g.discovered_tags);
        assert_eq!(tagged, g.tagged);
        assert!(g.tagged < g.nodes.iter().filter(|n| !n.ghost).count(), "some notes are untagged");
    }

    #[test]
    fn edit_times_spread_from_minutes_to_months() {
        let g = sample_graph(NOW);
        let ages: Vec<f64> = g.nodes.iter().filter_map(|n| n.mtime).map(|m| NOW - m).collect();
        assert!(ages.iter().all(|&a| a >= 0.0));
        assert!(ages.iter().filter(|&&a| a < DAY).count() >= 4);
        assert!(ages.iter().filter(|&&a| a < 7.0 * DAY).count() >= 12);
        assert!(ages.iter().any(|&a| a > 90.0 * DAY));
        assert!(g.nodes.iter().filter(|n| !n.ghost).all(|n| n.mtime.is_some()));
    }

    #[test]
    fn ghosts_are_bare() {
        let g = sample_graph(NOW);
        let ghosts: Vec<&Node> = g.nodes.iter().filter(|n| n.ghost).collect();
        assert!(!ghosts.is_empty());
        for n in ghosts {
            assert!(n.tag.is_none() && n.tags.is_none() && n.mtime.is_none() && n.folder.is_none());
            assert!(g.links.iter().any(|l| l.target == n.id));
        }
    }

    #[test]
    fn journal_titles_are_dates() {
        assert_eq!(ymd(0.0), "1970-01-01");
        assert_eq!(ymd(1_795_000_000_000.0), "2026-11-18");
        let g = sample_graph(NOW);
        assert!(g.nodes.iter().any(|n| n.id == "2026-11-18"));
    }
}
