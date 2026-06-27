//! gopher-askthedeck CLI.
//!
//! Two shapes share one binary:
//!   gopher-askthedeck build [--out <dir>] [--base-prefix <p>] [--keep <n>]
//!       one-shot: render the static tree (deck pages, menus, about, caps,
//!       cosmic) and atomically publish it under <out>/current.
//!   gopher-askthedeck draw [args...]
//!       the dynamic dcgi entry: one tarot reading per invocation (slice 5).
//!
//! The system clock is read here (the IO edge); the pure core takes a
//! `CivilTime` so the math and rendering stay deterministic and testable.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use gopher_askthedeck::cosmic::{self, CivilTime};
use gopher_askthedeck::{dcgi, site};

const DEFAULT_OUT: &str = "public";
const DEFAULT_KEEP: usize = 3;
/// Hub cross-links advertised in the root menu (the hub topology shared with
/// cta/blog). Override with `--cta-link` / `--phlog-link`; `none` disables one.
const DEFAULT_CTA_LINK: &str = "gopher://gopher.debene.dev:70";
const DEFAULT_PHLOG_LINK: &str = "gopher://gopher.debene.dev:7071";

/// An owned reading generator (the boxed counterpart of `dcgi::Llm`).
type BoxedLlm = Box<dyn Fn(&str) -> Option<String>>;

fn main() -> ExitCode {
    // Load a local .env if present; a real exported env var always wins.
    let _ = dotenvy::dotenv();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("build") => match run_build(&args[2..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("build failed: {e}");
                ExitCode::FAILURE
            }
        },
        Some("draw") => {
            run_draw(&args[2..]);
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: gopher-askthedeck <build|draw> [args...]");
            ExitCode::from(2)
        }
    }
}

fn run_build(flags: &[String]) -> std::io::Result<()> {
    let mut out = DEFAULT_OUT.to_string();
    let mut base = String::new();
    let mut keep = DEFAULT_KEEP;
    // Hub cross-links to the sibling holes (the hub topology). `none` disables one.
    let mut cta_raw = DEFAULT_CTA_LINK.to_string();
    let mut phlog_raw = DEFAULT_PHLOG_LINK.to_string();

    let mut it = flags.iter();
    while let Some(f) = it.next() {
        match f.as_str() {
            "--out" => out = next_val(&mut it, "--out")?,
            "--base-prefix" => base = next_val(&mut it, "--base-prefix")?,
            "--cta-link" => cta_raw = next_val(&mut it, "--cta-link")?,
            "--phlog-link" => phlog_raw = next_val(&mut it, "--phlog-link")?,
            "--keep" => {
                keep = next_val(&mut it, "--keep")?
                    .parse()
                    .map_err(|_| std::io::Error::other("--keep expects a number"))?
            }
            other => return Err(std::io::Error::other(format!("unknown flag {other}"))),
        }
    }

    // Assemble the hub list (label, host, port), skipping any disabled link.
    let cta = parse_gopher_link(&cta_raw).map_err(std::io::Error::other)?;
    let phlog = parse_gopher_link(&phlog_raw).map_err(std::io::Error::other)?;
    let mut hubs: Vec<site::Hub> = Vec::new();
    if let Some((h, p)) = &cta {
        hubs.push(("Live CTA trains (gopher-cta)", h, *p));
    }
    if let Some((h, p)) = &phlog {
        hubs.push(("Phlog -- the blog (gopher-blog)", h, *p));
    }

    let now = CivilTime::from_unix(unix_now());
    let cosmic = cosmic::compute(now);
    let cfg = site::SiteConfig {
        base: &base,
        hubs: &hubs,
    };
    let files = site::build_tree(&cfg, &cosmic);

    let snap = gopher_core::publish(Path::new(&out), &files, keep)?;
    eprintln!(
        "published {} files to {} ({} in {})",
        files.len(),
        snap.display(),
        cosmic.human_line(),
        out,
    );
    Ok(())
}

fn next_val<'a>(it: &mut impl Iterator<Item = &'a String>, flag: &str) -> std::io::Result<String> {
    it.next()
        .cloned()
        .ok_or_else(|| std::io::Error::other(format!("{flag} expects a value")))
}

/// Parse a hub link `gopher://host[:port]` into `(host, port)` (default port 70),
/// or `None` for `none`/empty. Mirrors gopher-cta's `parse_phlog_link`.
fn parse_gopher_link(raw: &str) -> Result<Option<(String, u16)>, String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let rest = raw
        .strip_prefix("gopher://")
        .ok_or_else(|| format!("hub link must start with gopher:// (got {raw})"))?;
    let rest = rest.trim_end_matches('/');
    let (host, port) = match rest.split_once(':') {
        Some((h, p)) => (
            h,
            p.parse::<u16>()
                .map_err(|_| format!("bad port in hub link: {p}"))?,
        ),
        None => (rest, 70),
    };
    if host.is_empty() {
        return Err(format!("hub link has no host: {raw}"));
    }
    Ok(Some((host.to_string(), port)))
}

/// The dcgi entry: geomyidae calls `gopher-askthedeck draw $search $arguments
/// $host $port $traversal $selector`. We parse argv, read the clock + the
/// client IP (from $REMOTE_ADDR, hashed immediately), and print a gophermap
/// through the full cache + cap + rate-limit path.
fn run_draw(rest: &[String]) {
    let args = dcgi::DcgiArgs::from_argv(rest);
    let env_base = std::env::var("ATD_BASE").unwrap_or_default();
    let base = dcgi::base_prefix(&args.selector, &env_base);

    // Client IP -> hash at once; the raw address goes no further.
    let ip_hash =
        gopher_askthedeck::deck::seed_hash(&std::env::var("REMOTE_ADDR").unwrap_or_default());

    let state_dir = std::env::var("ATD_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("gopher-askthedeck"));
    // Shareable snapshots live in the docroot (served at /r/<id>.txt). Must
    // differ from state_dir (both name files <id>.txt).
    let share_dir = std::env::var("ATD_SHARE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_dir.join("r"));

    let ctx = dcgi::Ctx {
        state_dir: &state_dir,
        share_dir: &share_dir,
        ip_hash,
        now_unix: unix_now(),
        entropy: draw_entropy(),
        base: &base,
        limits: dcgi::Limits {
            daily_call_cap: env_u32("ATD_DAILY_CAP", 500),
            rate_capacity: env_f64("ATD_RATE_CAPACITY", 5.0),
            rate_refill_per_sec: env_f64("ATD_RATE_REFILL", 0.05),
        },
    };

    print!("{}", dcgi::handle(&args, &ctx, llm().as_deref()));
}

/// The reading generator handed to the dcgi: `Some` when a DeepSeek key is
/// configured and the `net` feature is built in, else `None` (pure offline).
#[cfg(feature = "net")]
fn llm() -> Option<BoxedLlm> {
    let key = std::env::var("DEEPSEEK_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())?;
    let timeout = env_u32("ATD_LLM_TIMEOUT", 12) as u64;
    Some(Box::new(move |prompt: &str| {
        gopher_askthedeck::deepseek::ask(&key, prompt, timeout)
    }))
}

#[cfg(not(feature = "net"))]
fn llm() -> Option<BoxedLlm> {
    None
}

fn env_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Current Unix time in seconds (UTC). The single clock read in the build path.
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Per-request entropy that seeds the shuffle: high-resolution clock mixed with
/// the pid, so each draw is fresh (the draw is random, like the web's tap-to-draw).
fn draw_entropy() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (std::process::id() as u64).rotate_left(32)
}
