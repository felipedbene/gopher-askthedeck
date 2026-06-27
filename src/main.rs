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

use std::path::Path;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use gopher_askthedeck::cosmic::{self, CivilTime};
use gopher_askthedeck::{deck, site};

const DEFAULT_OUT: &str = "public";
const DEFAULT_KEEP: usize = 3;

fn main() -> ExitCode {
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
            // Placeholder until slice 5 wires the dcgi argv/stdout.
            let seed = deck::seed_hash("placeholder");
            for (pos, drawn) in deck::POSITIONS.iter().zip(deck::draw(seed).iter()) {
                let rev = if drawn.reversed { " (reversed)" } else { "" };
                println!("{pos}: {}{rev}", drawn.card.name);
            }
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

    let mut it = flags.iter();
    while let Some(f) = it.next() {
        match f.as_str() {
            "--out" => out = next_val(&mut it, "--out")?,
            "--base-prefix" => base = next_val(&mut it, "--base-prefix")?,
            "--keep" => {
                keep = next_val(&mut it, "--keep")?
                    .parse()
                    .map_err(|_| std::io::Error::other("--keep expects a number"))?
            }
            other => return Err(std::io::Error::other(format!("unknown flag {other}"))),
        }
    }

    let now = CivilTime::from_unix(unix_now());
    let cosmic = cosmic::compute(now);
    let cfg = site::SiteConfig { base: &base };
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

/// Current Unix time in seconds (UTC). The single clock read in the build path.
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
