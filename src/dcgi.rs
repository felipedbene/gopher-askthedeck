//! The dynamic `draw.dcgi` entry point — the only non-static surface.
//!
//! geomyidae executes a dcgi as `script $search $arguments $host $port
//! $traversal $selector` and interprets its stdout as a gophermap (`.gph`).
//! So the seeker's question arrives as **argv[1] (the type-7 search term)**,
//! and we emit a gophermap, which is exactly how a gopher client renders the
//! response to a type-7 selection.
//!
//! IO boundary: argv comes in, a gophermap string goes out. The clock read is
//! injected ([`render`] takes `now_unix`) so the whole thing is testable without
//! a process or a wall clock. The DeepSeek call, cache, cap, and rate limit are
//! layered in front of this in slice 6; here the reading is always the
//! deterministic offline one.
//!
//! Ethical invariant: argv carries the client/server host and port and the
//! selector. None of them are ever passed to the reading. We use the host/port
//! not at all (geomyidae substitutes the `server`/`port` link tokens itself) and
//! the selector only to discover our own base prefix for navigation links.

use crate::cosmic::{self, CivilTime};
use crate::deck::{self, DrawnCard};
use crate::reading::local_reading;
use crate::site::selector;
use gopher_core::{info, link, render_menu_index, Entry, ItemKind};

/// The arguments geomyidae hands a dcgi, in its documented order.
#[derive(Debug, Clone, Default)]
pub struct DcgiArgs {
    /// argv[1] — the type-7 search term (the seeker's question).
    pub search: String,
    /// argv[2] — query string after `?` in the selector.
    pub arguments: String,
    /// argv[3] / argv[4] — server host/port. Deliberately unused (see module
    /// note); kept only so the struct mirrors the real calling convention.
    pub host: String,
    pub port: String,
    /// argv[6] — the full request selector, used only to find our base prefix.
    pub selector: String,
}

impl DcgiArgs {
    /// Parse argv *excluding* the program name (i.e. `std::env::args[1..]`).
    pub fn from_argv(rest: &[String]) -> DcgiArgs {
        let get = |i: usize| rest.get(i).cloned().unwrap_or_default();
        DcgiArgs {
            search: get(0),
            arguments: get(1),
            host: get(2),
            port: get(3),
            // argv[4] is traversal; argv[5] is the selector.
            selector: get(5),
        }
    }

    /// The seeker's question: the type-7 search term, falling back to the `?`
    /// arguments. Never the host, port, or selector.
    pub fn question(&self) -> &str {
        let q = self.search.trim();
        if q.is_empty() {
            self.arguments.trim()
        } else {
            q
        }
    }
}

/// The base selector prefix this dcgi is mounted under, derived from its own
/// selector (`/tarot/draw.dcgi?...` → `/tarot`, `/draw.dcgi` → ``). Falls back
/// to the `ATD_BASE` value the caller passes (from the environment).
pub fn base_prefix(selector: &str, env_base: &str) -> String {
    // strip any query (`?...`) or search (`\t...`) suffix
    let path = selector.split(['?', '\t']).next().unwrap_or(selector);
    if let Some(idx) = path.rfind('/') {
        let dir = &path[..idx];
        return dir.to_string();
    }
    env_base.trim_end_matches('/').to_string()
}

/// The coarse time window used for the seed (and, in slice 6, the cache key):
/// the UTC calendar date. Identical questions on the same UTC day draw the same
/// spread — mirrors askthedeck's same-day cache.
fn time_window(now_unix: i64) -> String {
    let t = CivilTime::from_unix(now_unix);
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

/// Render the full dcgi response (a gophermap string) for the given args and
/// clock. Pure: no IO. `base` is the selector prefix for navigation links.
pub fn render(args: &DcgiArgs, base: &str, now_unix: i64) -> String {
    let q = args.question();
    if q.is_empty() {
        return render_menu_index(&prompt_entries(base));
    }

    let seed = deck::seed_hash(&format!("{q}__{}", time_window(now_unix)));
    let spread = deck::draw(seed);
    let sky = cosmic::compute(CivilTime::from_unix(now_unix));
    let body = local_reading(q, &spread, &sky);

    render_menu_index(&reading_entries(&body, &spread, base))
}

/// The empty-question prompt: explain, and offer the type-7 item again.
fn prompt_entries(base: &str) -> Vec<Entry> {
    vec![
        info("=============================================================="),
        info("  ASK THE DECK"),
        info("=============================================================="),
        info(""),
        info("  You didn't type a question. That's the whole interaction:"),
        info("  pick \"Ask the deck\" and type what's on your mind -- a"),
        info("  question, a worry, a single word. The deck answers in three"),
        info("  cards read against the sky overhead right now."),
        info(""),
        link(
            ItemKind::Search,
            "Ask the deck  (type your question)",
            selector(base, "draw.dcgi"),
        ),
        info(""),
        link(
            ItemKind::Menu,
            "Browse the 78 cards instead",
            selector(base, "cards/"),
        ),
        link(
            ItemKind::Text,
            "About this deck",
            selector(base, "about.txt"),
        ),
    ]
}

/// Wrap the reading body (multi-line text) as gophermap info lines, then append
/// real navigation links to each drawn card's page, asking again, and browsing.
fn reading_entries(body: &str, spread: &[DrawnCard; 3], base: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = body.lines().map(info).collect();
    entries.push(info(""));
    entries.push(info(
        "--------------------------------------------------------------",
    ));
    for d in spread {
        entries.push(link(
            ItemKind::Text,
            format!("The {} -- full card", d.card.name),
            selector(base, &format!("cards/{}.txt", d.card.page_slug())),
        ));
    }
    entries.push(link(
        ItemKind::Search,
        "Ask the deck again",
        selector(base, "draw.dcgi"),
    ));
    entries.push(link(
        ItemKind::Menu,
        "Browse all 78 cards",
        selector(base, "cards/"),
    ));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-06-27T12:00:00Z
    const NOW: i64 = 1_782_561_600;

    fn args_with(search: &str) -> DcgiArgs {
        DcgiArgs {
            search: search.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn argv_maps_to_search_arguments_host_port_selector() {
        let argv: Vec<String> = [
            "the question",
            "args",
            "h.example",
            "70",
            "0",
            "/tarot/draw.dcgi",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let a = DcgiArgs::from_argv(&argv);
        assert_eq!(a.search, "the question");
        assert_eq!(a.arguments, "args");
        assert_eq!(a.host, "h.example");
        assert_eq!(a.port, "70");
        assert_eq!(a.selector, "/tarot/draw.dcgi");
        assert_eq!(a.question(), "the question");
    }

    #[test]
    fn question_falls_back_to_arguments() {
        let a = DcgiArgs {
            search: "  ".into(),
            arguments: "from-args".into(),
            ..Default::default()
        };
        assert_eq!(a.question(), "from-args");
    }

    #[test]
    fn empty_query_renders_a_prompt() {
        let out = render(&args_with(""), "", NOW);
        assert!(out.contains("ASK THE DECK"));
        assert!(out.to_lowercase().contains("type"));
        // a type-7 item to ask
        assert!(out.contains("[7|Ask the deck  (type your question)|/draw.dcgi|server|port]"));
        // no reading content
        assert!(!out.contains("YOUR READING"));
    }

    #[test]
    fn non_empty_query_renders_a_reading() {
        let out = render(&args_with("should I move cities?"), "", NOW);
        assert!(out.contains("YOUR READING"));
        assert!(out.contains("should I move cities?"));
        // three card frames present
        assert_eq!(out.matches(".------------------------------.").count(), 6);
        // real navigation links appended
        assert!(out.contains("[7|Ask the deck again|/draw.dcgi|server|port]"));
        assert!(out.contains("[1|Browse all 78 cards|/cards/|server|port]"));
    }

    #[test]
    fn output_is_a_valid_gophermap_no_tabs() {
        let out = render(&args_with("anything at all"), "", NOW);
        assert!(!out.contains('\t'), "gophermap lines must not contain tabs");
        // every non-empty line is either an info line or a [..] link line
        for line in out.lines() {
            if line.starts_with('[') {
                assert!(line.ends_with(']'), "malformed link line: {line}");
            }
        }
    }

    #[test]
    fn deterministic_for_same_question_and_day() {
        let a = render(&args_with("steady?"), "", NOW);
        let b = render(&args_with("steady?"), "", NOW);
        assert_eq!(a, b);
    }

    #[test]
    fn base_prefix_is_applied_to_links() {
        let out = render(&args_with("hi"), "/tarot", NOW);
        assert!(out.contains("|/tarot/draw.dcgi|"));
        assert!(out.contains("|/tarot/cards/"));
    }

    #[test]
    fn base_prefix_derived_from_selector() {
        assert_eq!(base_prefix("/tarot/draw.dcgi", ""), "/tarot");
        assert_eq!(base_prefix("/draw.dcgi", ""), "");
        assert_eq!(base_prefix("/a/b/draw.dcgi?q\tterm", ""), "/a/b");
    }

    #[test]
    fn host_port_selector_never_reach_the_reading() {
        // Even when geomyidae hands us a host/port/selector, the rendered
        // reading body must not contain them.
        let a = DcgiArgs {
            search: "what now?".into(),
            host: "client-9.example".into(),
            port: "54321".into(),
            selector: "/draw.dcgi?secret".into(),
            ..Default::default()
        };
        let out = render(&a, "", NOW);
        assert!(!out.contains("client-9.example"));
        assert!(!out.contains("54321"));
        assert!(!out.contains("secret"));
    }
}
