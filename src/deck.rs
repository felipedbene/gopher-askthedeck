//! The tarot deck — pure data + a deterministic seeded draw.
//!
//! Ported from askthedeck's `src/lib/deck/cards.ts`: the same 78-card universe
//! (22 Major Arcana + 56 Minor, four suits of 14), the same canonical English
//! names, the same internal id slugs (`00-TheFool`, `Cups07`, …). What is *new*
//! here is determinism: a gopher draw is seeded from the seeker's question so it
//! is reproducible (same question + same time window → same three cards), which
//! is what makes the seed double as the reading cache key.
//!
//! Pure: no IO, no clock, no network. The seed is supplied by the caller.

/// The four Minor Arcana suits, in the deck's canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suit {
    Cups,
    Pentacles,
    Swords,
    Wands,
}

impl Suit {
    /// Canonical English suit name.
    pub fn name(self) -> &'static str {
        match self {
            Suit::Cups => "Cups",
            Suit::Pentacles => "Pentacles",
            Suit::Swords => "Swords",
            Suit::Wands => "Wands",
        }
    }
}

/// What kind of card this is — the structural identity used by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// A Major Arcana trump, numbered 0..=21.
    Major { number: u8 },
    /// A Minor Arcana pip or court, `rank` 1..=14 (1 Ace … 10, 11 Page,
    /// 12 Knight, 13 Queen, 14 King).
    Minor { suit: Suit, rank: u8 },
}

/// A single card: its internal id slug, canonical English name, and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card {
    /// Internal id, matching askthedeck (`00-TheFool`, `Cups07`).
    pub id: &'static str,
    /// Canonical English display name (`The Fool`, `Seven of Cups`).
    pub name: &'static str,
    pub kind: CardKind,
}

impl Card {
    /// Filesystem-/selector-safe slug for the card's static page: a lowercase
    /// kebab of the English name (`The Fool` → `the-fool`, `Seven of Cups` →
    /// `seven-of-cups`). Unique across the deck.
    pub fn page_slug(&self) -> String {
        let mut s = String::with_capacity(self.name.len());
        let mut prev_dash = false;
        for ch in self.name.chars() {
            if ch.is_ascii_alphanumeric() {
                s.push(ch.to_ascii_lowercase());
                prev_dash = false;
            } else if !prev_dash {
                s.push('-');
                prev_dash = true;
            }
        }
        s.trim_matches('-').to_string()
    }

    /// True for the 16 court cards (Page/Knight/Queen/King).
    pub fn is_court(&self) -> bool {
        matches!(self.kind, CardKind::Minor { rank, .. } if rank >= 11)
    }
}

/// A drawn card: the card plus whether it landed reversed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawnCard {
    pub card: Card,
    pub reversed: bool,
}

/// The three spread positions, in order. Mirrors askthedeck's
/// `POSITION_LABELS` (Current State / Focus for Growth / Potential in 7 Days).
pub const POSITIONS: [&str; 3] = ["Current State", "Focus for Growth", "Potential in 7 Days"];

/// Whether reversals are enabled. Off would make every card upright. Build/config
/// flag per the brief — not exposed to the gopher client.
pub const REVERSALS_ENABLED: bool = true;

// ---- the 78 cards ---------------------------------------------------------

const MAJOR_NAMES: [&str; 22] = [
    "The Fool",
    "The Magician",
    "The High Priestess",
    "The Empress",
    "The Emperor",
    "The Hierophant",
    "The Lovers",
    "The Chariot",
    "Strength",
    "The Hermit",
    "Wheel of Fortune",
    "Justice",
    "The Hanged Man",
    "Death",
    "Temperance",
    "The Devil",
    "The Tower",
    "The Star",
    "The Moon",
    "The Sun",
    "Judgement",
    "The World",
];

const MAJOR_IDS: [&str; 22] = [
    "00-TheFool",
    "01-TheMagician",
    "02-TheHighPriestess",
    "03-TheEmpress",
    "04-TheEmperor",
    "05-TheHierophant",
    "06-TheLovers",
    "07-TheChariot",
    "08-Strength",
    "09-TheHermit",
    "10-WheelOfFortune",
    "11-Justice",
    "12-TheHangedMan",
    "13-Death",
    "14-Temperance",
    "15-TheDevil",
    "16-TheTower",
    "17-TheStar",
    "18-TheMoon",
    "19-TheSun",
    "20-Judgement",
    "21-TheWorld",
];

const SUITS: [Suit; 4] = [Suit::Cups, Suit::Pentacles, Suit::Swords, Suit::Wands];

/// Build the full ordered deck: 22 majors, then each suit's 14 cards. The ids
/// and order match askthedeck exactly so a card always maps to the same image
/// slug and the same draw index.
pub fn all_cards() -> Vec<Card> {
    let mut cards = Vec::with_capacity(78);
    for (i, (id, name)) in MAJOR_IDS.iter().zip(MAJOR_NAMES.iter()).enumerate() {
        cards.push(Card {
            id,
            name,
            kind: CardKind::Major { number: i as u8 },
        });
    }
    for &suit in &SUITS {
        for rank in 1..=14u8 {
            // ids are the suit name + zero-padded rank: `Cups07`, `Wands14`.
            let id: &'static str = suit_rank_id(suit, rank);
            let name: &'static str = suit_rank_name(suit, rank);
            cards.push(Card {
                id,
                name,
                kind: CardKind::Minor { suit, rank },
            });
        }
    }
    cards
}

/// Leak-free `&'static str` ids/names for the 56 minors via a compile-time table.
fn suit_rank_id(suit: Suit, rank: u8) -> &'static str {
    MINOR_IDS[suit_index(suit) * 14 + (rank as usize - 1)]
}
fn suit_rank_name(suit: Suit, rank: u8) -> &'static str {
    MINOR_NAMES[suit_index(suit) * 14 + (rank as usize - 1)]
}
fn suit_index(suit: Suit) -> usize {
    match suit {
        Suit::Cups => 0,
        Suit::Pentacles => 1,
        Suit::Swords => 2,
        Suit::Wands => 3,
    }
}

// The minor ids/names are spelled out as `&'static` tables (rather than
// formatted at runtime) so `Card` can stay `Copy` with `&'static str` fields.
include!("minors.rs");

// ---- deterministic draw ---------------------------------------------------

/// FNV-1a 64-bit hash of the seed material. Std-only, stable across builds and
/// platforms — important, because this hash seeds both the draw and the cache
/// key, so two identical questions must hash identically everywhere.
pub fn seed_hash(material: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in material.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// SplitMix64 — a tiny, well-distributed PRNG seeded by the question hash. We
/// only need a handful of draws, so this is plenty and keeps us std-only.
struct SplitMix64 {
    state: u64,
}
impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// Uniform-ish integer in `0..n` (n small, so modulo bias is negligible).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Draw three distinct cards (plus a reversal bit each) deterministically from a
/// seed. The same seed always yields the same spread — that is what lets the
/// seed serve as the reading cache key. No card repeats within a spread.
pub fn draw(seed: u64) -> [DrawnCard; 3] {
    let cards = all_cards();
    let mut idx: Vec<usize> = (0..cards.len()).collect();
    let mut rng = SplitMix64::new(seed);

    // Partial Fisher–Yates: pick 3 distinct positions from the front.
    let mut picked = [0usize; 3];
    for (slot, p) in picked.iter_mut().enumerate() {
        let j = slot + rng.below(idx.len() - slot);
        idx.swap(slot, j);
        *p = idx[slot];
    }

    let mut out = [DrawnCard {
        card: cards[picked[0]],
        reversed: false,
    }; 3];
    for (slot, drawn) in out.iter_mut().enumerate() {
        let reversed = REVERSALS_ENABLED && (rng.next_u64() & 1 == 1);
        *drawn = DrawnCard {
            card: cards[picked[slot]],
            reversed,
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn deck_has_78_cards() {
        assert_eq!(all_cards().len(), 78);
    }

    #[test]
    fn deck_has_22_majors_and_56_minors() {
        let cards = all_cards();
        let majors = cards
            .iter()
            .filter(|c| matches!(c.kind, CardKind::Major { .. }))
            .count();
        let minors = cards
            .iter()
            .filter(|c| matches!(c.kind, CardKind::Minor { .. }))
            .count();
        assert_eq!(majors, 22);
        assert_eq!(minors, 56);
    }

    #[test]
    fn ids_are_unique() {
        let ids: HashSet<_> = all_cards().iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 78);
    }

    #[test]
    fn page_slugs_are_unique() {
        let slugs: HashSet<_> = all_cards().iter().map(|c| c.page_slug()).collect();
        assert_eq!(slugs.len(), 78, "every card needs a distinct page slug");
    }

    #[test]
    fn known_names_match_askthedeck() {
        let cards = all_cards();
        assert_eq!(cards[0].name, "The Fool");
        assert_eq!(cards[0].id, "00-TheFool");
        // first minor after the 22 majors is Ace of Cups (id Cups01)
        assert_eq!(cards[22].name, "Ace of Cups");
        assert_eq!(cards[22].id, "Cups01");
        // courts
        let king_wands = cards.iter().find(|c| c.id == "Wands14").unwrap();
        assert_eq!(king_wands.name, "King of Wands");
        assert!(king_wands.is_court());
    }

    #[test]
    fn page_slug_examples() {
        let cards = all_cards();
        let fool = cards.iter().find(|c| c.id == "00-TheFool").unwrap();
        assert_eq!(fool.page_slug(), "the-fool");
        let wheel = cards.iter().find(|c| c.id == "10-WheelOfFortune").unwrap();
        assert_eq!(wheel.page_slug(), "wheel-of-fortune");
        let seven_cups = cards.iter().find(|c| c.id == "Cups07").unwrap();
        assert_eq!(seven_cups.page_slug(), "seven-of-cups");
    }

    #[test]
    fn draw_is_deterministic() {
        let s = seed_hash("what should I focus on?__2026-06-27");
        let a = draw(s);
        let b = draw(s);
        assert_eq!(a, b, "same seed must yield the same spread");
    }

    #[test]
    fn draw_has_no_duplicate_cards() {
        for q in [
            "love",
            "career path",
            "",
            "a much longer question about my life",
        ] {
            let spread = draw(seed_hash(q));
            let ids: HashSet<_> = spread.iter().map(|d| d.card.id).collect();
            assert_eq!(ids.len(), 3, "no card may repeat within a spread ({q})");
        }
    }

    #[test]
    fn different_questions_usually_differ() {
        let a = draw(seed_hash("question one"));
        let b = draw(seed_hash("a completely different question"));
        // Not a guarantee, but with 78^3 space these should differ.
        assert_ne!(
            a.iter().map(|d| d.card.id).collect::<Vec<_>>(),
            b.iter().map(|d| d.card.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn reversal_distribution_is_sane() {
        // Over many draws, reversed cards should be roughly half — not 0%, not
        // 100%. Guards against a stuck reversal bit.
        let mut reversed = 0usize;
        let mut total = 0usize;
        for i in 0..2000u64 {
            for d in draw(seed_hash(&format!("q{i}"))) {
                total += 1;
                if d.reversed {
                    reversed += 1;
                }
            }
        }
        let frac = reversed as f64 / total as f64;
        assert!(
            (0.40..=0.60).contains(&frac),
            "reversal fraction {frac} out of sane range"
        );
    }
}
