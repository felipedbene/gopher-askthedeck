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

use std::path::Path;

use crate::cosmic::{self, CivilTime, Cosmic};
use crate::deck::{self, DrawnCard};
use crate::reading::{build_prompt, local_reading, render_llm_reading};
use crate::site::selector;
use crate::{cache, ratelimit};
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

// ---- the orchestrated path (cache + cap + rate limit + LLM) ----------------

/// The reading generator the dcgi calls: a prompt in, the model's text out (or
/// `None` on any failure). Injected so the cost/abuse path is testable without a
/// network; production passes a closure wrapping the DeepSeek call.
pub type Llm<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Abuse + cost limits.
pub struct Limits {
    /// Max LLM calls per UTC day before everything falls back to local.
    pub daily_call_cap: u32,
    /// Token-bucket capacity per client (burst size).
    pub rate_capacity: f64,
    /// Token refill rate per second.
    pub rate_refill_per_sec: f64,
}

/// Per-request context for [`handle`]. All IO is rooted at `state_dir`.
pub struct Ctx<'a> {
    /// Writable dir for the cache, rate-limit buckets, and the daily counter.
    pub state_dir: &'a Path,
    /// Hash of the client IP (hashed at the IO edge; the raw IP never arrives).
    pub ip_hash: u64,
    pub now_unix: i64,
    pub base: &'a str,
    pub limits: Limits,
}

/// The full dynamic path with all controls. `llm` is the (optional) reading
/// generator: `Some(f)` when a key is configured, `None` for pure offline. It's
/// injected so the cost/abuse logic is testable without a network. Order:
/// rate-limit -> cache -> daily cap -> LLM (or local) -> cache.
pub fn handle(args: &DcgiArgs, ctx: &Ctx, llm: Option<Llm>) -> String {
    let q = args.question();
    if q.is_empty() {
        return render_menu_index(&prompt_entries(ctx.base));
    }

    // Per-IP throttle first — cheapest rejection, and it guards the LLM path.
    if !ratelimit::allow(
        ctx.state_dir,
        ctx.ip_hash,
        ctx.now_unix,
        ctx.limits.rate_capacity,
        ctx.limits.rate_refill_per_sec,
    ) {
        return render_menu_index(&throttle_entries(ctx.base));
    }

    let window = time_window(ctx.now_unix);
    let seed = deck::seed_hash(&format!("{q}__{window}"));
    let spread = deck::draw(seed);
    let sky = cosmic::compute(CivilTime::from_unix(ctx.now_unix));
    let key = format!("{seed:016x}");

    // Cache hit => zero LLM calls.
    let body = if let Some(cached) = cache::get(ctx.state_dir, &key, ctx.now_unix) {
        cached
    } else {
        let body = produce(q, &spread, &sky, ctx, &window, llm);
        let _ = cache::put(ctx.state_dir, &key, ctx.now_unix, &body);
        body
    };

    render_menu_index(&reading_entries(&body, &spread, ctx.base))
}

/// Produce a fresh reading: try the LLM (if available and under the day's cap),
/// else the deterministic local reading. Reserving the cap slot before the call
/// means a transient outage degrades to local for the day rather than hammering
/// a paid, failing API.
fn produce(
    q: &str,
    spread: &[DrawnCard; 3],
    sky: &Cosmic,
    ctx: &Ctx,
    window: &str,
    llm: Option<Llm>,
) -> String {
    if let Some(llm) = llm {
        if ratelimit::try_acquire_call(ctx.state_dir, window, ctx.limits.daily_call_cap) {
            // Standardized prompt: cards + cosmic only. The typed `q` shuffled
            // the draw but is deliberately NOT passed to the LLM.
            let prompt = build_prompt(spread, sky);
            if let Some(text) = llm(&prompt) {
                return render_llm_reading(q, spread, sky, &text);
            }
        }
    }
    local_reading(q, spread, sky)
}

/// The polite over-rate response — a text item, not an error.
fn throttle_entries(base: &str) -> Vec<Entry> {
    vec![
        info("=============================================================="),
        info("  EASY THERE"),
        info("=============================================================="),
        info(""),
        info("  The deck needs a moment between readings -- you've drawn a"),
        info("  few in quick succession. Sit with the last one; the cards"),
        info("  don't like to be rushed. Try again shortly."),
        info(""),
        link(
            ItemKind::Menu,
            "Browse the 78 cards meanwhile",
            selector(base, "cards/"),
        ),
        link(
            ItemKind::Text,
            "About this deck",
            selector(base, "about.txt"),
        ),
    ]
}

/// The empty-input prompt: explain, and offer the type-7 item again.
fn prompt_entries(base: &str) -> Vec<Entry> {
    vec![
        info("=============================================================="),
        info("  DRAW THREE CARDS"),
        info("=============================================================="),
        info(""),
        info("  You didn't type anything. Pick \"Draw three cards\" and type"),
        info("  a word, an intention, a worry -- anything: it shuffles the"),
        info("  deck and seeds your draw. Three cards are then read in their"),
        info("  positions against the sky overhead right now."),
        info(""),
        link(
            ItemKind::Search,
            "Draw three cards",
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
        "Draw three more cards",
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
        assert!(out.contains("DRAW THREE CARDS"));
        assert!(out.to_lowercase().contains("type"));
        // a type-7 item to draw
        assert!(out.contains("[7|Draw three cards|/draw.dcgi|server|port]"));
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
        assert!(out.contains("[7|Draw three more cards|/draw.dcgi|server|port]"));
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

    use std::sync::atomic::{AtomicUsize, Ordering};

    fn state_dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("atd-handle-test-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn loose_limits() -> Limits {
        Limits {
            daily_call_cap: 1000,
            rate_capacity: 100.0,
            rate_refill_per_sec: 1.0,
        }
    }

    fn ctx<'a>(dir: &'a Path, limits: Limits) -> Ctx<'a> {
        Ctx {
            state_dir: dir,
            ip_hash: 12345,
            now_unix: NOW,
            base: "",
            limits,
        }
    }

    #[test]
    fn cache_hit_does_not_call_the_llm() {
        let d = state_dir("cache");
        let calls = AtomicUsize::new(0);
        let llm = |_p: &str| -> Option<String> {
            calls.fetch_add(1, Ordering::SeqCst);
            Some("## A Reading\n\nThe model speaks plainly here.".to_string())
        };
        let a = args_with("same question");
        let c = ctx(&d, loose_limits());

        let first = handle(&a, &c, Some(&llm));
        let second = handle(&a, &c, Some(&llm));
        assert_eq!(first, second, "same seed -> identical output");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second hit served from cache"
        );
        assert!(first.contains("The model speaks plainly here."));
    }

    #[test]
    fn falls_back_to_local_when_llm_unavailable() {
        let d = state_dir("fallback");
        let c = ctx(&d, loose_limits());
        let out = handle(&args_with("guidance please"), &c, None);
        assert!(out.contains("YOUR READING"), "deterministic local reading");
        assert!(out.contains("guidance please"));
    }

    #[test]
    fn falls_back_to_local_when_llm_errors() {
        let d = state_dir("llmerr");
        let llm = |_p: &str| -> Option<String> { None }; // simulates timeout/down
        let c = ctx(&d, loose_limits());
        let out = handle(&args_with("anything"), &c, Some(&llm));
        assert!(out.contains("YOUR READING"));
    }

    #[test]
    fn over_daily_cap_falls_back_to_local() {
        let d = state_dir("cap");
        let calls = AtomicUsize::new(0);
        let llm = |_p: &str| -> Option<String> {
            calls.fetch_add(1, Ordering::SeqCst);
            Some("## R\n\nmodel text".to_string())
        };
        let limits = Limits {
            daily_call_cap: 1,
            rate_capacity: 100.0,
            rate_refill_per_sec: 1.0,
        };
        let c = ctx(&d, limits);
        // first distinct question: uses the one cap slot -> LLM
        let a = handle(&args_with("q-one"), &c, Some(&llm));
        assert!(a.contains("model text"));
        // second distinct question: cap exhausted -> local
        let b = handle(&args_with("q-two"), &c, Some(&llm));
        assert!(b.contains("YOUR READING"));
        assert!(!b.contains("model text"));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "only one paid call");
    }

    #[test]
    fn rate_limit_throttles_a_burst_from_one_ip() {
        let d = state_dir("rl");
        let limits = Limits {
            daily_call_cap: 1000,
            rate_capacity: 2.0,
            rate_refill_per_sec: 0.001, // effectively no refill in-test
        };
        let c = ctx(&d, limits);
        // distinct questions so cache never absorbs the hit
        assert!(handle(&args_with("a"), &c, None).contains("YOUR READING"));
        assert!(handle(&args_with("b"), &c, None).contains("YOUR READING"));
        let third = handle(&args_with("c"), &c, None);
        assert!(
            third.contains("EASY THERE"),
            "burst beyond capacity is throttled"
        );
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
