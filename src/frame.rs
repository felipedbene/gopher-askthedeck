//! ASCII line-art card frames — a pure, unit-tested renderer.
//!
//! One reusable boxed frame, parameterized by the card. The motif inside is
//! generated, not hand-drawn per card:
//!   - Minor pips: the suit glyph repeated by rank, laid out in a small grid
//!     (Five of Cups → five chalices).
//!   - Minor courts: a figure + the court rank word + the suit glyph.
//!   - Majors: one of 22 distinct emblem motifs from a table.
//!
//! Reversed cards flip the motif rows top-to-bottom and are marked `[REVERSED]`.
//!
//! Everything is ASCII (the deck browses fine in any gopher client) and pure —
//! a given (card, reversed) renders to an exact string, which the tests pin.

use crate::deck::{Card, CardKind, Suit};

/// Inner width of the frame (between the side borders).
const INNER: usize = 30;
/// Height of the motif region, in lines.
const MOTIF_H: usize = 3;

/// Render the full boxed frame for a card in the given orientation.
pub fn render_frame(card: &Card, reversed: bool) -> String {
    let mut motif = motif(card);
    if reversed {
        motif.reverse();
    }

    let orientation = if reversed {
        "[ REVERSED ]"
    } else {
        "[ UPRIGHT ]"
    };
    let mut lines: Vec<String> = vec![
        border(),
        blank(),
        boxed(&card.name.to_uppercase()),
        boxed(subtitle(card)),
        blank(),
    ];
    lines.extend(motif.iter().map(|m| boxed(m)));
    lines.push(blank());
    lines.push(boxed(orientation));
    lines.push(blank());
    lines.push(border());
    lines.join("\n")
}

fn subtitle(card: &Card) -> &'static str {
    match card.kind {
        CardKind::Major { .. } => "Major Arcana",
        CardKind::Minor { suit, .. } => match suit {
            Suit::Cups => "Cups - Minor Arcana",
            Suit::Pentacles => "Pentacles - Minor Arcana",
            Suit::Swords => "Swords - Minor Arcana",
            Suit::Wands => "Wands - Minor Arcana",
        },
    }
}

fn border() -> String {
    format!(".{}.", "-".repeat(INNER))
}

fn blank() -> String {
    format!("|{}|", " ".repeat(INNER))
}

/// Wrap a centered string in side borders, padded to the inner width. Content
/// longer than the inner width is truncated (no card name comes close).
fn boxed(s: &str) -> String {
    format!("|{}|", center(s, INNER))
}

/// Center `s` within `width` ASCII columns (extra space biased to the right).
fn center(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.chars().take(width).collect();
    }
    let pad = width - len;
    let left = pad / 2;
    let right = pad - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

// ---- motifs ----------------------------------------------------------------

/// The 3-line motif region for a card.
fn motif(card: &Card) -> Vec<String> {
    match card.kind {
        CardKind::Major { number } => MAJOR_MOTIFS[number as usize]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        CardKind::Minor { suit, rank } if rank <= 10 => pip_grid(suit, rank),
        CardKind::Minor { suit, rank } => court_motif(suit, rank),
    }
}

/// The 3-char suit glyph repeated to build pips.
fn suit_glyph(suit: Suit) -> &'static str {
    match suit {
        Suit::Cups => "\\_/",     // a chalice
        Suit::Pentacles => "(o)", // a coin
        Suit::Swords => "}=>",    // a blade
        Suit::Wands => "==|",     // a staff
    }
}

/// Lay `rank` (1..=10) suit glyphs into the motif region. Up to 5 per row; for
/// 6..=10 the glyphs split across two rows (6→3+3, 7→4+3, 8→4+4, 9→5+4,
/// 10→5+5). Vertically centered in the 3-line region.
fn pip_grid(suit: Suit, rank: u8) -> Vec<String> {
    let g = suit_glyph(suit);
    let n = rank as usize;
    let row = |count: usize| -> String {
        let mut parts = Vec::with_capacity(count);
        for _ in 0..count {
            parts.push(g);
        }
        parts.join(" ")
    };

    let rows: Vec<String> = if n <= 5 {
        vec![row(n)]
    } else {
        let top = n.div_ceil(2);
        vec![row(top), row(n - top)]
    };

    // Center the (1 or 2) rows vertically within MOTIF_H lines.
    let mut out = vec![String::new(); MOTIF_H];
    let start = (MOTIF_H - rows.len()) / 2;
    for (i, r) in rows.into_iter().enumerate() {
        out[start + i] = r;
    }
    out
}

/// Court motif: the rank word, a small figure, and the suit glyph.
fn court_motif(suit: Suit, rank: u8) -> Vec<String> {
    let word = match rank {
        11 => "PAGE",
        12 => "KNIGHT",
        13 => "QUEEN",
        14 => "KING",
        _ => "COURT",
    };
    vec![
        format!("[ {word} ]"),
        "\\o/".to_string(),
        suit_glyph(suit).to_string(),
    ]
}

/// 22 distinct emblem motifs, indexed by Major Arcana number (0..=21).
const MAJOR_MOTIFS: [[&str; 3]; 22] = [
    ["\\o/", " | ", "/ \\"], // 0  The Fool
    ["\\|/", "(+)", "/|\\"], // 1  The Magician
    [")|(", "|O|", "_|_"],   // 2  The High Priestess
    [" * ", "\\V/", "_|_"],  // 3  The Empress
    ["_#_", "|#|", "/_\\"],  // 4  The Emperor
    ["_+_", "/|\\", "_|_"],  // 5  The Hierophant
    ["o o", "\\^/", " | "],  // 6  The Lovers
    ["[#]", "/|\\", "O O"],  // 7  The Chariot
    ["oo ", "(~)", " ^ "],   // 8  Strength
    [" * ", "/O\\", " | "],  // 9  The Hermit
    ["_-_", "(O)", "-_-"],   // 10 Wheel of Fortune
    ["_|_", "-+-", "/ \\"],  // 11 Justice
    ["_|_", " O ", "/^\\"],  // 12 The Hanged Man
    ["_#_", "(X)", "/|\\"],  // 13 Death
    ["\\_/", " | ", "/_\\"], // 14 Temperance
    ["\\v/", "(#)", "/^\\"], // 15 The Devil
    ["/\\ ", "|!|", "*|*"],  // 16 The Tower
    [" * ", "*.*", " | "],   // 17 The Star
    ["(  ", " ))", "_|_"],   // 18 The Moon
    ["\\|/", "-O-", "/|\\"], // 19 The Sun
    ["\\o/", "_|_", "/_\\"], // 20 Judgement
    ["(O)", " | ", "/_\\"],  // 21 The World
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::all_cards;

    fn card(id: &str) -> Card {
        *all_cards().iter().find(|c| c.id == id).unwrap()
    }

    #[test]
    fn frame_is_rectangular() {
        // Every line of every card's frame is the same width, both orientations.
        for c in all_cards() {
            for rev in [false, true] {
                let f = render_frame(&c, rev);
                let widths: Vec<usize> = f.lines().map(|l| l.chars().count()).collect();
                assert!(
                    widths.iter().all(|&w| w == INNER + 2),
                    "ragged frame for {} (rev={rev}): {widths:?}",
                    c.id
                );
            }
        }
    }

    #[test]
    fn pip_count_matches_rank() {
        // Three of Cups shows exactly three chalices in the motif region.
        let f = render_frame(&card("Cups03"), false);
        let chalices = f.matches("\\_/").count();
        assert_eq!(chalices, 3, "Three of Cups should show 3 cups");
        // Ten of Wands shows ten staves.
        let f10 = render_frame(&card("Wands10"), false);
        assert_eq!(f10.matches("==|").count(), 10, "Ten of Wands → 10 staves");
    }

    #[test]
    fn ace_shows_one_glyph() {
        let f = render_frame(&card("Pentacles01"), false);
        assert_eq!(f.matches("(o)").count(), 1);
    }

    #[test]
    fn court_shows_rank_word_and_suit() {
        let f = render_frame(&card("Swords13"), false);
        assert!(f.contains("QUEEN"), "court rank word present");
        assert!(f.contains("}=>"), "court suit glyph present");
        assert!(f.contains("SWORDS"), "name uppercased in frame");
    }

    #[test]
    fn orientation_marker_and_flip() {
        let up = render_frame(&card("00-TheFool"), false);
        let rev = render_frame(&card("00-TheFool"), true);
        assert!(up.contains("[ UPRIGHT ]"));
        assert!(rev.contains("[ REVERSED ]"));
        assert_ne!(up, rev, "reversed frame must differ (motif flipped)");
        // The reversal flips motif row order: Fool's first motif row \o/ should
        // now appear lower than its last row / \.
        let up_lines: Vec<&str> = up.lines().collect();
        let rev_lines: Vec<&str> = rev.lines().collect();
        assert_ne!(up_lines, rev_lines);
    }

    #[test]
    fn major_motifs_are_distinct() {
        use std::collections::HashSet;
        let set: HashSet<_> = MAJOR_MOTIFS.iter().map(|m| m.concat()).collect();
        assert_eq!(set.len(), 22, "each major needs a distinct motif");
    }

    #[test]
    fn name_appears_uppercased() {
        let f = render_frame(&card("02-TheHighPriestess"), false);
        assert!(f.contains("THE HIGH PRIESTESS"));
        assert!(f.contains("Major Arcana"));
    }
}
