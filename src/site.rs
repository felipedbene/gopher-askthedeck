//! The static tree builder — pure: (config, cosmic) in, a list of tree files out.
//!
//! Everything here is static and emitted once by the `build` subcommand:
//! the root menu, the about + caps + cosmic pages, the 78-card browse menu, and
//! one text page per card (its ASCII frame + upright/reversed meanings). The
//! only dynamic surface is `draw.dcgi`, which is dropped into the tree by the
//! build/deploy step, not generated here.
//!
//! Menus are serialized with `gopher-core`'s `.gph` serializer. Links use the
//! daemon's own host/port placeholders (no baked address), so the tree serves
//! correctly on any host/port; only the *selector* carries an optional base
//! prefix for the case where this hole shares a docroot with a sibling.

use crate::cosmic::Cosmic;
use crate::deck::{all_cards, Card, CardKind, Suit};
use crate::meanings::meaning;
use crate::{frame, reading};
use gopher_core::{info, link, render_menu_index, Entry, ItemKind};

/// Build config: where in the selector namespace this hole lives.
pub struct SiteConfig<'a> {
    /// Selector base prefix, e.g. "" (own root) or "/tarot" (shared docroot).
    /// No trailing slash.
    pub base: &'a str,
}

/// A built tree file: relative path + bytes. (Same shape as `gopher_core::TreeFile`.)
pub type File = (String, Vec<u8>);

/// Render the full static tree.
pub fn build_tree(cfg: &SiteConfig, cosmic: &Cosmic) -> Vec<File> {
    let mut files: Vec<File> = vec![
        ("index.gph".into(), root_menu(cfg).into_bytes()),
        ("about.txt".into(), about_page().into_bytes()),
        ("caps.txt".into(), caps_page().into_bytes()),
        ("cosmic.txt".into(), cosmic_page(cosmic).into_bytes()),
        ("cards/index.gph".into(), cards_menu(cfg).into_bytes()),
    ];

    for card in all_cards() {
        let path = format!("cards/{}.txt", card.page_slug());
        files.push((path, card_page(&card).into_bytes()));
    }

    files
}

/// Selector for `path` under a base prefix (`""` → own root, `"/tarot"` →
/// shared docroot). Shared with the dcgi layer so static and dynamic links agree.
pub fn selector(base: &str, path: &str) -> String {
    if base.is_empty() {
        format!("/{path}")
    } else {
        format!("{}/{path}", base.trim_end_matches('/'))
    }
}

/// Selector for a path under this hole's base prefix.
fn sel(cfg: &SiteConfig, path: &str) -> String {
    selector(cfg.base, path)
}

fn root_menu(cfg: &SiteConfig) -> String {
    let entries = vec![
        info("=============================================================="),
        info("                 A S K   T H E   D E C K"),
        info("        a three-card tarot reading, drawn live over gopher"),
        info("=============================================================="),
        info(""),
        info("  Three cards: Current State, Focus for Growth, Potential in"),
        info("  7 Days -- each read against the real sky overhead right now."),
        info(""),
        link(
            ItemKind::Search,
            "Ask the deck  (type your question)",
            sel(cfg, "draw.dcgi"),
        ),
        info(""),
        link(
            ItemKind::Text,
            "About this deck and the spread",
            sel(cfg, "about.txt"),
        ),
        link(
            ItemKind::Text,
            "Today's cosmic weather",
            sel(cfg, "cosmic.txt"),
        ),
        link(ItemKind::Menu, "Browse all 78 cards", sel(cfg, "cards/")),
        link(
            ItemKind::Text,
            "Server capabilities (caps.txt)",
            sel(cfg, "caps.txt"),
        ),
        info(""),
        info("  No accounts, no cookies, no tracking. Your question seeds the"),
        info("  draw and nothing about you ever reaches the reading."),
    ];
    render_menu_index(&entries)
}

fn cards_menu(cfg: &SiteConfig) -> String {
    let mut entries: Vec<Entry> = vec![
        info("--------------------------------------------------------------"),
        info("  THE 78 CARDS"),
        info("--------------------------------------------------------------"),
        info(""),
    ];

    let mut last_section: Option<String> = None;
    for card in all_cards() {
        let section = section_label(&card);
        if last_section.as_deref() != Some(section.as_str()) {
            if last_section.is_some() {
                entries.push(info(""));
            }
            entries.push(info(format!("  {section}")));
            last_section = Some(section);
        }
        entries.push(link(
            ItemKind::Text,
            card.name,
            sel(cfg, &format!("cards/{}.txt", card.page_slug())),
        ));
    }

    entries.push(info(""));
    entries.push(link(ItemKind::Menu, "<- Back to the deck", sel(cfg, "")));
    render_menu_index(&entries)
}

fn section_label(card: &Card) -> String {
    match card.kind {
        CardKind::Major { .. } => "Major Arcana".to_string(),
        CardKind::Minor { suit, .. } => format!("Minor Arcana - {}", suit_name(suit)),
    }
}

fn suit_name(suit: Suit) -> &'static str {
    suit.name()
}

fn card_page(card: &Card) -> String {
    let (upright, reversed) = meaning(card.id).unwrap_or(("", ""));
    let mut s = String::new();
    s.push_str(&frame::render_frame(card, false));
    s.push_str("\n\n");
    s.push_str(&format!("  {}\n", card.name));
    s.push_str(&format!("  {}\n\n", subtitle_line(card)));
    s.push_str(&format!("  Upright   {upright}\n\n"));
    s.push_str(&format!("  Reversed  {reversed}\n\n"));
    s.push_str("--------------------------------------------------------------\n");
    s.push_str("  Browse the deck:  selector /cards/\n");
    s.push_str("  Ask the deck:     the root menu's \"Ask the deck\" item\n");
    s
}

fn subtitle_line(card: &Card) -> String {
    match card.kind {
        CardKind::Major { number } => format!("Major Arcana - {number}"),
        CardKind::Minor { suit, rank } => format!("{} - rank {rank}", suit_name(suit)),
    }
}

fn about_page() -> String {
    // Reuse the reading module's spread description so the about page and the
    // reading stay in lockstep.
    format!(
        "\
==============================================================
  ABOUT -- ASK THE DECK
==============================================================

  A three-card tarot reading, drawn the moment you ask and
  read against the real sky overhead. You type a question; the
  deck answers in three positions.

  THE SPREAD

{positions}

  THE SKY

  Every reading is anchored to the live cosmic weather computed
  from the server clock: the current moon phase and moon sign,
  the zodiac season, and the planetary day. The moon's light
  colours how the work of the middle card will feel; the season
  is the broad terrain.

  THE ETHIC -- NO AMBIENT METADATA

  A gopher server can see more about a visitor than a reading
  has any right to use. So we don't. The interpretation is built
  from exactly three things: your question, the three cards you
  drew, and the sky. Your IP, hostname, port, the path you came
  in on, your client software, your location, and the wall-clock
  time are NEVER part of the reading -- not shown to it, not
  hinted at, not laundered in. This is enforced by a test that
  fails the build if any of them could reach the interpreter.

  No accounts. No cookies. No saved history. No tracking. The
  reading is the reading.

--------------------------------------------------------------
  Browse all 78 cards:  selector /cards/
  Cosmic weather:       selector /cosmic.txt
",
        positions = reading::spread_description(),
    )
}

fn cosmic_page(c: &Cosmic) -> String {
    format!(
        "\
==============================================================
  TODAY'S COSMIC WEATHER
==============================================================

  {line}

  Moon phase    {phase}
  Moon sign     {moon_sign}
  Zodiac season {sun_sign}
  Planetary day {day}

  This is the sky every reading drawn today is anchored to. The
  moon's phase and sign colour how the middle card's work will
  feel; the zodiac season is the broad terrain; the planetary
  day a minor accent.

--------------------------------------------------------------
  Ask the deck:  the root menu's \"Ask the deck\" item
  Browse cards:  selector /cards/
",
        line = c.human_line(),
        phase = c.moon_phase,
        moon_sign = c.moon_sign,
        sun_sign = c.sun_sign,
        day = c.planetary_day,
    )
}

fn caps_page() -> String {
    // Mirrors gopher-cta's caps.txt policy file.
    "\
CAPS

# caps.txt -- capability + server info policy file
# Served at gopher selector: caps.txt

CapsVersion=1
ExpireCapsAfter=3600

# --- Path section: POSIX defaults ---
PathDelimeter=/
PathIdentity=.
PathParent=..
PathParentDouble=FALSE
PathEscapeCharacter=\\
PathKeepPreDelimeter=FALSE

# --- Server section ---
ServerSoftware=geomyidae
ServerSoftwareVersion=0.99
ServerArchitecture=Linux/x86_64
ServerDescription=Ask the Deck -- an interactive tarot reading over Gopher
ServerAdmin=gopher@debene.dev
DefaultEncoding=utf-8
"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmic::{compute, CivilTime};

    fn sample_cosmic() -> Cosmic {
        compute(CivilTime {
            year: 2026,
            month: 6,
            day: 27,
            hour: 12,
            minute: 0,
            second: 0,
        })
    }

    fn cfg() -> SiteConfig<'static> {
        SiteConfig { base: "" }
    }

    #[test]
    fn tree_has_root_about_caps_cosmic_and_78_card_pages() {
        let files = build_tree(&cfg(), &sample_cosmic());
        let names: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
        assert!(names.contains(&"index.gph"));
        assert!(names.contains(&"about.txt"));
        assert!(names.contains(&"caps.txt"));
        assert!(names.contains(&"cosmic.txt"));
        assert!(names.contains(&"cards/index.gph"));
        let card_pages = names
            .iter()
            .filter(|p| p.starts_with("cards/") && p.ends_with(".txt"))
            .count();
        assert_eq!(card_pages, 78, "one text page per card");
    }

    #[test]
    fn root_menu_has_type_7_search_item() {
        let gph = root_menu(&cfg());
        assert!(
            gph.contains("[7|Ask the deck  (type your question)|/draw.dcgi|server|port]"),
            "root must carry the type-7 draw item:\n{gph}"
        );
    }

    #[test]
    fn base_prefix_is_applied_to_selectors() {
        let c = SiteConfig { base: "/tarot" };
        let gph = root_menu(&c);
        assert!(gph.contains("|/tarot/draw.dcgi|"));
        assert!(gph.contains("|/tarot/about.txt|"));
    }

    #[test]
    fn card_page_carries_frame_and_both_meanings() {
        let files = build_tree(&cfg(), &sample_cosmic());
        let (_, bytes) = files
            .iter()
            .find(|(p, _)| p == "cards/the-fool.txt")
            .expect("the fool has a page");
        let page = String::from_utf8(bytes.clone()).unwrap();
        assert!(page.contains("THE FOOL"));
        assert!(page.contains("Upright"));
        assert!(page.contains("Reversed"));
        assert!(page.contains("New beginnings"));
    }

    #[test]
    fn every_card_menu_link_points_at_an_existing_page() {
        let files = build_tree(&cfg(), &sample_cosmic());
        let paths: std::collections::HashSet<&str> =
            files.iter().map(|(p, _)| p.as_str()).collect();
        for card in all_cards() {
            let p = format!("cards/{}.txt", card.page_slug());
            assert!(paths.contains(p.as_str()), "missing page {p}");
        }
    }

    #[test]
    fn cosmic_page_shows_weather() {
        let page = cosmic_page(&sample_cosmic());
        assert!(page.contains("Moon phase"));
        assert!(page.contains("Zodiac season"));
    }
}
