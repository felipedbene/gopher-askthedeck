//! The dynamic `draw.dcgi` entry point — the only non-static surface.
//!
//! Like the askthedeck web app, a reading is a **shuffle**, not a typed question:
//! the menu item is a plain type-1 link, so selecting it just fetches
//! `draw.dcgi` (no input box) and the deck deals three random cards. geomyidae
//! executes the dcgi as `script $search $arguments $host $port $traversal
//! $selector` and interprets its stdout as a gophermap (`.gph`); we ignore the
//! (empty) search/arguments and emit the reading as a gophermap.
//!
//! IO boundary: argv + injected clock/entropy come in, a gophermap goes out, so
//! the whole thing is testable without a process or a wall clock. The shuffle is
//! seeded by [`Ctx::entropy`] (a high-resolution clock read at the IO edge).
//!
//! Ethical invariant: argv carries the client/server host and port and the
//! selector. The server host/port appear only in the share permalink (display
//! only); none of argv reaches the reading interpretation or the LLM.

use std::path::Path;

use crate::cosmic::{self, CivilTime, Cosmic};
use crate::deck::{self, DrawnCard};
use crate::site::selector;
use crate::{cache, ratelimit, reading, share};
use gopher_core::{info, link, render_menu_index, Entry, ItemKind};

/// The arguments geomyidae hands a dcgi, in its documented order.
#[derive(Debug, Clone, Default)]
pub struct DcgiArgs {
    /// argv[1] — the type-7 search term. Unused: the draw is a shuffle, not a
    /// query (kept so the struct mirrors the real calling convention).
    pub search: String,
    /// argv[2] — query string after `?` in the selector. Unused (see `search`).
    pub arguments: String,
    /// argv[3] / argv[4] — the SERVER's host/port. Used only to build the share
    /// permalink (display); never reaches the reading or the LLM.
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

/// The coarse time window folded into the reading id / cache key: the UTC
/// calendar date. Mirrors askthedeck's same-day, card-keyed cache.
fn time_window(now_unix: i64) -> String {
    let t = CivilTime::from_unix(now_unix);
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

/// Render a reading (a gophermap string) from a draw `seed`, with no cost
/// controls and no persistence. Pure: no IO. `base` is the selector prefix for
/// navigation links. Like the web app, the draw is a shuffle — there is no typed
/// question — so the seed is supplied by the caller (entropy at the IO edge).
pub fn render(base: &str, seed: u64, now_unix: i64) -> String {
    let spread = deck::draw(seed);
    let sky = cosmic::compute(CivilTime::from_unix(now_unix));
    let body = reading::local_reading(None, &spread, &sky);

    // The no-controls path persists nothing, so it offers no share permalink.
    render_menu_index(&reading_entries(&body, &spread, base, None))
}

// ---- the orchestrated path (cache + cap + rate limit + LLM) ----------------

/// The reading generator the dcgi calls: a prompt in, the model's text out (or
/// `None` on any failure). Injected so the cost/abuse path is testable without a
/// network; production passes a closure wrapping the DeepSeek call.
pub type Llm<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Abuse + cost limits.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Max LLM calls per UTC day before everything falls back to local.
    pub daily_call_cap: u32,
    /// Token-bucket capacity per client (burst size).
    pub rate_capacity: f64,
    /// Token refill rate per second.
    pub rate_refill_per_sec: f64,
}

/// Per-request context for [`handle`]. All IO is rooted at these dirs.
pub struct Ctx<'a> {
    /// Writable dir for the cache, rate-limit buckets, and the daily counter.
    pub state_dir: &'a Path,
    /// Writable dir for shareable reading snapshots (served at `/r/<id>.txt`).
    pub share_dir: &'a Path,
    /// Hash of the client IP (hashed at the IO edge; the raw IP never arrives).
    pub ip_hash: u64,
    pub now_unix: i64,
    /// Per-request entropy that seeds the shuffle (the draw is random, like the
    /// web's tap-to-draw — there is no typed question). Supplied at the IO edge
    /// from a high-resolution clock; a fixed value in tests makes draws reproducible.
    pub entropy: u64,
    pub base: &'a str,
    pub limits: Limits,
}

/// Content-addressed id for a reading: the cards (sorted, with orientation) and
/// the UTC day — NOT the typed text. Identical draws collapse to one permalink,
/// matching askthedeck's card-keyed cache. Used as both the cache key and the
/// share-file name.
fn reading_key(spread: &[DrawnCard; 3], now_unix: i64) -> String {
    let mut parts: Vec<String> = spread
        .iter()
        .map(|d| format!("{}:{}", d.card.id, d.reversed as u8))
        .collect();
    parts.sort();
    let material = format!("{}__{}", parts.join(","), time_window(now_unix));
    format!("{:016x}", deck::seed_hash(&material))
}

/// The full dynamic path with all controls. `llm` is the (optional) reading
/// generator: `Some(f)` when a key is configured, `None` for pure offline. It's
/// injected so the cost/abuse logic is testable without a network. Order:
/// rate-limit -> cache -> daily cap -> LLM (or local) -> cache + persist share.
pub fn handle(args: &DcgiArgs, ctx: &Ctx, llm: Option<Llm>) -> String {
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

    // The draw is a random shuffle (no typed question, like the web app).
    let spread = deck::draw(ctx.entropy);
    let sky = cosmic::compute(CivilTime::from_unix(ctx.now_unix));
    let id = reading_key(&spread, ctx.now_unix);

    // Cache the reading CORE keyed by cards+day. Cache hit => zero LLM calls.
    let core = if let Some(cached) = cache::get(ctx.state_dir, &id, ctx.now_unix) {
        cached
    } else {
        let c = produce_core(&spread, &sky, ctx, llm);
        let _ = cache::put(ctx.state_dir, &id, ctx.now_unix, &c);
        c
    };

    // The reading body == the shareable snapshot: there is no typed text to
    // echo, so the live view and the persisted permalink are identical.
    let body = format!("{}{}", reading::render_header(None, &sky), core);
    let _ = share::store(ctx.share_dir, &id, &body);
    let share_selector = selector(ctx.base, &format!("r/{id}.txt"));
    let permalink = share::permalink(&args.host, &args.port, &share_selector);

    render_menu_index(&reading_entries(
        &body,
        &spread,
        ctx.base,
        Some((&share_selector, &permalink)),
    ))
}

/// Produce a fresh reading CORE (header-free): the LLM core if a generator is
/// available and under the day's cap, else the deterministic local core.
/// Reserving the cap slot before the call means a transient outage degrades to
/// local for the day rather than hammering a paid, failing API.
fn produce_core(spread: &[DrawnCard; 3], sky: &Cosmic, ctx: &Ctx, llm: Option<Llm>) -> String {
    if let Some(llm) = llm {
        if ratelimit::try_acquire_call(
            ctx.state_dir,
            &time_window(ctx.now_unix),
            ctx.limits.daily_call_cap,
        ) {
            // Standardized prompt: cards + cosmic only (the typed text shuffled
            // the draw but is never passed to the LLM).
            let prompt = reading::build_prompt(spread, sky);
            if let Some(text) = llm(&prompt) {
                return reading::llm_core(spread, sky, &text);
            }
        }
    }
    reading::local_core(spread, sky)
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

/// Wrap the reading body (multi-line text) as gophermap info lines, then append
/// the share permalink (if any) and real navigation links to each drawn card's
/// page, drawing again, and browsing. `share` is `(selector, permalink)` for the
/// persisted snapshot, or `None` on the no-controls path.
fn reading_entries(
    body: &str,
    spread: &[DrawnCard; 3],
    base: &str,
    share: Option<(&str, &str)>,
) -> Vec<Entry> {
    let mut entries: Vec<Entry> = body.lines().map(info).collect();
    entries.push(info(""));
    entries.push(info(
        "--------------------------------------------------------------",
    ));
    if let Some((share_sel, permalink)) = share {
        entries.push(info("  Share this reading -- bookmark it to keep it:"));
        entries.push(info(format!("  {permalink}")));
        entries.push(link(
            ItemKind::Text,
            "Open this reading's permalink",
            share_sel,
        ));
        entries.push(info(""));
    }
    for d in spread {
        entries.push(link(
            ItemKind::Text,
            format!("The {} -- full card", d.card.name),
            selector(base, &format!("cards/{}.txt", d.card.page_slug())),
        ));
    }
    entries.push(link(
        ItemKind::Menu,
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

    const SEED: u64 = 0x5151_5151_2323_2323;

    #[test]
    fn renders_a_reading_with_type1_nav() {
        let out = render("", SEED, NOW);
        assert!(out.contains("YOUR READING"));
        // there is no typed question to echo
        assert!(!out.contains("shuffled the deck with"));
        // three card frames present
        assert_eq!(out.matches(".------------------------------.").count(), 6);
        // the draw/redraw items are plain type-1 links (no input box)
        assert!(out.contains("[1|Draw three more cards|/draw.dcgi|server|port]"));
        assert!(out.contains("[1|Browse all 78 cards|/cards/|server|port]"));
        // and no type-7 search item anywhere
        assert!(!out.contains("[7|"));
    }

    #[test]
    fn output_is_a_valid_gophermap_no_tabs() {
        let out = render("", SEED, NOW);
        assert!(!out.contains('\t'), "gophermap lines must not contain tabs");
        for line in out.lines() {
            if line.starts_with('[') {
                assert!(line.ends_with(']'), "malformed link line: {line}");
            }
        }
    }

    #[test]
    fn deterministic_for_same_seed() {
        assert_eq!(render("", SEED, NOW), render("", SEED, NOW));
    }

    #[test]
    fn different_seeds_usually_differ() {
        assert_ne!(render("", 1, NOW), render("", 999_999, NOW));
    }

    #[test]
    fn base_prefix_is_applied_to_links() {
        let out = render("/tarot", SEED, NOW);
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

    /// A fresh (state_dir, share_dir) pair — they must differ, since both name
    /// files `<id>.txt` keyed by the same reading id.
    fn dirs(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let base = std::env::temp_dir().join(format!("atd-handle-test-{name}"));
        let _ = std::fs::remove_dir_all(&base);
        (base.join("state"), base.join("share"))
    }

    fn loose_limits() -> Limits {
        Limits {
            daily_call_cap: 1000,
            rate_capacity: 100.0,
            rate_refill_per_sec: 1.0,
        }
    }

    fn ctx<'a>(state: &'a Path, share: &'a Path, entropy: u64, limits: Limits) -> Ctx<'a> {
        Ctx {
            state_dir: state,
            share_dir: share,
            ip_hash: 12345,
            now_unix: NOW,
            entropy,
            base: "",
            limits,
        }
    }

    #[test]
    fn cache_hit_does_not_call_the_llm() {
        let (s, sh) = dirs("cache");
        let calls = AtomicUsize::new(0);
        let llm = |_p: &str| -> Option<String> {
            calls.fetch_add(1, Ordering::SeqCst);
            Some("## A Reading\n\nThe model speaks plainly here.".to_string())
        };
        let a = args_with("");
        let c = ctx(&s, &sh, SEED, loose_limits());

        // same ctx => same entropy => same draw => second is a cache hit
        let first = handle(&a, &c, Some(&llm));
        let second = handle(&a, &c, Some(&llm));
        assert_eq!(first, second, "same draw -> identical output");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second hit served from cache"
        );
        assert!(first.contains("The model speaks plainly here."));
        // the reading offers a share permalink
        assert!(first.contains("Share this reading"));
        assert!(first.contains("/r/"));
    }

    #[test]
    fn falls_back_to_local_when_llm_unavailable() {
        let (s, sh) = dirs("fallback");
        let c = ctx(&s, &sh, SEED, loose_limits());
        let out = handle(&args_with(""), &c, None);
        assert!(out.contains("YOUR READING"), "deterministic local reading");
    }

    #[test]
    fn falls_back_to_local_when_llm_errors() {
        let (s, sh) = dirs("llmerr");
        let llm = |_p: &str| -> Option<String> { None }; // simulates timeout/down
        let c = ctx(&s, &sh, SEED, loose_limits());
        let out = handle(&args_with(""), &c, Some(&llm));
        assert!(out.contains("YOUR READING"));
    }

    #[test]
    fn over_daily_cap_falls_back_to_local() {
        let (s, sh) = dirs("cap");
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
        // Two different draws (distinct entropy) so they don't collapse to one
        // cached reading and each takes the produce path.
        let c1 = ctx(&s, &sh, 1, limits);
        let c2 = ctx(&s, &sh, 2, limits);
        // first draw: uses the one cap slot -> LLM
        let a = handle(&args_with(""), &c1, Some(&llm));
        assert!(a.contains("model text"));
        // second, different draw: cap exhausted -> local
        let b = handle(&args_with(""), &c2, Some(&llm));
        assert!(b.contains("YOUR READING"));
        assert!(!b.contains("model text"));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "only one paid call");
    }

    #[test]
    fn rate_limit_throttles_a_burst_from_one_ip() {
        let (s, sh) = dirs("rl");
        let limits = Limits {
            daily_call_cap: 1000,
            rate_capacity: 2.0,
            rate_refill_per_sec: 0.001, // effectively no refill in-test
        };
        let c = ctx(&s, &sh, SEED, limits);
        // the rate check runs before cache, so a burst is throttled regardless
        assert!(handle(&args_with(""), &c, None).contains("YOUR READING"));
        assert!(handle(&args_with(""), &c, None).contains("YOUR READING"));
        let third = handle(&args_with(""), &c, None);
        assert!(
            third.contains("EASY THERE"),
            "burst beyond capacity is throttled"
        );
    }

    #[test]
    fn share_snapshot_is_persisted_without_the_typed_text() {
        let (s, sh) = dirs("share");
        let a = DcgiArgs {
            host: "gopher.debene.dev".into(),
            port: "7072".into(),
            ..Default::default()
        };
        let c = ctx(&s, &sh, SEED, loose_limits());
        let out = handle(&a, &c, None);

        // the live response carries a permalink built from the server host/port
        assert!(out.contains("gopher://gopher.debene.dev:7072/0/r/"));

        // exactly one snapshot was written; it's a valid reading
        let files: Vec<_> = std::fs::read_dir(&sh).unwrap().flatten().collect();
        assert_eq!(files.len(), 1, "one shared snapshot persisted");
        let body = std::fs::read_to_string(files[0].path()).unwrap();
        assert!(body.contains("YOUR READING"));
        // the snapshot equals the live body minus the appended gophermap nav
        assert!(!body.contains("shuffled the deck with"));
    }

    #[test]
    fn argv_selector_never_reaches_the_reading() {
        // geomyidae hands us the raw selector (with any query); it must not
        // appear in the reading. (Server host/port legitimately appear only in
        // the permalink.)
        let (s, sh) = dirs("argvleak");
        let a = DcgiArgs {
            host: "gopher.debene.dev".into(),
            port: "7072".into(),
            selector: "/draw.dcgi?super-secret".into(),
            ..Default::default()
        };
        let c = ctx(&s, &sh, SEED, loose_limits());
        let out = handle(&a, &c, None);
        assert!(!out.contains("super-secret"));
    }

    #[test]
    fn no_controls_render_is_self_contained() {
        // The no-controls render path takes no argv and emits no permalink, so
        // it carries only host/port placeholder tokens, never a concrete address.
        let out = render("", SEED, NOW);
        assert!(out.contains("YOUR READING"));
        assert!(
            out.contains("|server|port]"),
            "links use placeholder tokens"
        );
    }
}
