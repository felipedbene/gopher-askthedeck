//! gopher-askthedeck CLI.
//!
//! Two shapes share one binary:
//!   gopher-askthedeck build [--out <dir>] [--host H] [--port N]
//!       one-shot: render the static tree (deck pages, menus, about, caps).
//!   gopher-askthedeck draw [args...]
//!       the dynamic dcgi entry: one tarot reading per invocation.
//!
//! Subcommands beyond `build` arrive in later slices; for now `build` is the
//! only wired path and `draw` is a placeholder.

use gopher_askthedeck::deck;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("draw") => {
            // Placeholder until slice 5 wires the dcgi.
            let seed = deck::seed_hash("placeholder");
            let spread = deck::draw(seed);
            for (pos, drawn) in deck::POSITIONS.iter().zip(spread.iter()) {
                let rev = if drawn.reversed { " (reversed)" } else { "" };
                println!("{pos}: {}{rev}", drawn.card.name);
            }
            ExitCode::SUCCESS
        }
        Some("build") => {
            eprintln!("build: not yet implemented (slice 3)");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: gopher-askthedeck <build|draw> [args...]");
            ExitCode::from(2)
        }
    }
}
