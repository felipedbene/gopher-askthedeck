//! Reading assembly — pure.
//!
//! Three things live here, none of which touch the clock, the filesystem, or the
//! network:
//!   - [`spread_description`]: the human description of the three positions,
//!     shared by the about page and the reading so they never drift.
//!   - [`build_prompt`]: the LLM prompt, ported from askthedeck's `buildPrompt`
//!     but trimmed to this round (English, no saved history/continuity, no
//!     donations) and fed the cosmic block that carries no locating date. It is
//!     built from exactly three inputs — question, spread, cosmic — so it is
//!     *structurally* incapable of leaking client metadata; the slice-5 guard
//!     test proves it.
//!   - [`local_reading`]: the deterministic offline reading, assembled from the
//!     static card meanings + positions + a cosmic-anchored line. This is what
//!     makes the app always answer with no key, no network, or over budget.

use crate::cosmic::Cosmic;
use crate::deck::{DrawnCard, POSITIONS};
use crate::frame::render_frame;
use crate::meanings::meaning;

/// Human description of the three spread positions, indented for the about page
/// and reused by the reading so the two never drift. Mirrors askthedeck's
/// position semantics (Current State / Focus for Growth / Potential in 7 Days).
pub fn spread_description() -> String {
    let blurbs = [
        "What is actually happening in your life right now -- the\n    texture of the present, not where you came from or what you\n    want.",
        "Where your attention should land in the coming days. The work\n    in front of you -- a verb, not a destination.",
        "A quality of energy that could open within the week IF you\n    engage with the second card -- an atmosphere or invitation,\n    never a guaranteed outcome.",
    ];
    let mut s = String::new();
    for (i, (pos, blurb)) in POSITIONS.iter().zip(blurbs.iter()).enumerate() {
        s.push_str(&format!("  Position {} -- {}\n    {}\n", i + 1, pos, blurb));
        if i + 1 < POSITIONS.len() {
            s.push('\n');
        }
    }
    s
}

/// Display name of a drawn card, with orientation suffix.
fn card_label(d: &DrawnCard) -> String {
    if d.reversed {
        format!("{} (reversed)", d.card.name)
    } else {
        d.card.name.to_string()
    }
}

/// The meaning for a drawn card in its actual orientation.
fn drawn_meaning(d: &DrawnCard) -> &'static str {
    let (up, rev) = meaning(d.card.id).unwrap_or(("", ""));
    if d.reversed {
        rev
    } else {
        up
    }
}

// ---- LLM prompt ------------------------------------------------------------

/// Assemble the DeepSeek prompt. Built ONLY from the question, the spread, and
/// the cosmic block — see the module note and the prompt-guard test.
pub fn build_prompt(question: &str, spread: &[DrawnCard; 3], cosmic: &Cosmic) -> String {
    let cards = spread
        .iter()
        .zip(POSITIONS.iter())
        .map(|(d, pos)| format!("{pos}: {}", card_label(d)))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        r#"You are an experienced tarot reader who speaks plainly and with conviction. The seeker has drawn a three-card spread. Each card sits in a specific position; read each card AS THAT POSITION, not as a generic card meaning.

THE SPREAD POSITIONS

Position 1 -- Current State: what is actually happening in the seeker's life right now. The texture of their present moment. Not where they "are coming from", not what they "want" -- what *is*.

Position 2 -- Focus for Growth: where their attention should land in the coming days. The work in front of them. Not a destination -- a verb.

Position 3 -- Potential in 7 Days: a quality of energy that could become available within the week, IF they engage with Position 2. Frame this as an atmosphere or invitation, NEVER as a guaranteed outcome or specific event.

THE SEEKER'S QUESTION

{question}

THE CARDS

{cards}

THE ASTROLOGICAL WEATHER

{astro}

Use the moon phase and moon sign to colour how Position 2's work will FEEL -- waxing energy builds, waning energy releases; fire signs push, water signs absorb. Use the zodiac season as the broad terrain. The planetary day is a minor accent.

STRUCTURE

Use three subheadings, one per card, in this exact format:

## {{Position label}}: {{Card name}}

Then one or two paragraphs of prose for that card. After the third card, add a short closing paragraph (no header) that names the through-line. No bullet lists.

HOW TO WRITE THIS READING

- Commit to one interpretation per card. Do not hedge.
- When two cards pull in different directions, name the tension.
- Use concrete sensory imagery; avoid abstract spiritual vocabulary.

BANNED PHRASES -- DO NOT USE

"dark night of the soul", "trust the process", "the universe is conspiring", "divine timing", "high vibration", "low vibration", "shadow work", "manifesting", "manifestation", "energetic shift", "in alignment", "ancient wisdom", "the veil is thin", "dear one", "beloved", "sweet soul"

WHAT YOU MUST NOT DO

- Do not promise specific outcomes. The Potential card is an atmosphere, not a forecast.
- Do not address the seeker as "dear one" or any equivalent. Address them as "you", directly.
- Do not begin with throat-clearing ("Ah,", "I see..."). Start with the first card's heading.
- NEVER mention donations, payment, or supporting the project.
- NEVER reference anything you were not given here: not the reader's location, city, country, or timezone; not the time of day or the calendar date; not their device, client, or network. The only context you have is the question, the cards, and the astrological weather above.

Write the entire reading in English. Begin."#,
        question = question,
        cards = cards,
        astro = cosmic.prompt_block(),
    )
}

// ---- deterministic offline reading -----------------------------------------

/// The four elements and how each colours the focus-card's work.
fn element_tone(sign: &str) -> &'static str {
    match sign {
        "Aries" | "Leo" | "Sagittarius" => "it wants to be acted on, out in the open",
        "Taurus" | "Virgo" | "Capricorn" => "it wants something concrete and patient",
        "Gemini" | "Libra" | "Aquarius" => "it moves through thought and conversation",
        "Cancer" | "Scorpio" | "Pisces" => "it works underneath, in feeling",
        _ => "it asks for steady attention",
    }
}

/// Whether the moon is gathering or shedding light.
fn moon_motion(phase: &str) -> &'static str {
    match phase {
        "New Moon" | "Waxing Crescent" | "First Quarter" | "Waxing Gibbous" => {
            "this is building energy -- start it, grow it"
        }
        _ => "this is releasing energy -- finish, shed, let go",
    }
}

/// Templated narrative for one position, woven from the card's meaning, its
/// orientation, the position's intent, and (for the focus card) the sky.
fn local_narrative(index: usize, d: &DrawnCard, cosmic: &Cosmic) -> String {
    let m = drawn_meaning(d);
    match index {
        0 => {
            let mut s = format!("Where you actually are: {m}");
            if d.reversed {
                s.push_str(
                    " It comes through turned -- read it as the held-back or shadow form, not the bright one.",
                );
            }
            s
        }
        1 => format!(
            "Put your attention here: {m} The {phase} in {sign} colours the work: {motion}, and {tone}.",
            phase = cosmic.moon_phase,
            sign = cosmic.moon_sign,
            motion = moon_motion(cosmic.moon_phase),
            tone = element_tone(cosmic.moon_sign),
        ),
        _ => format!(
            "What could open within the week: {m} Hold it lightly -- an atmosphere that becomes available if you do the work of the second card, never a promise."
        ),
    }
}

/// One rendered section: the ASCII frame, a position header, and a narrative.
fn render_section(position: &str, d: &DrawnCard, narrative: &str) -> String {
    let mut s = String::new();
    s.push_str("--------------------------------------------------------------\n");
    s.push_str(&format!(
        "  {}  --  {}\n",
        position.to_uppercase(),
        card_label(d)
    ));
    s.push_str("--------------------------------------------------------------\n\n");
    s.push_str(&render_frame(&d.card, d.reversed));
    s.push_str("\n\n");
    // wrap narrative at ~60 cols with a two-space indent
    s.push_str(&wrap_indented(narrative, 60, "  "));
    s.push('\n');
    s.push_str(&format!(
        "\n  Full card: selector /cards/{}.txt\n\n",
        d.card.page_slug()
    ));
    s
}

/// The reading header: the question echoed back and the cosmic weather (the
/// human-facing line is fine here; this text goes to the seeker, not the LLM).
pub fn render_header(question: &str, cosmic: &Cosmic) -> String {
    format!(
        "\
==============================================================
  ASK THE DECK -- YOUR READING
==============================================================

  You asked:
    \"{q}\"

  The sky right now: {phase} in {moon}, {sun} Season, {day}'s day.

",
        q = question,
        phase = cosmic.moon_phase,
        moon = cosmic.moon_sign,
        sun = cosmic.sun_sign,
        day = cosmic.planetary_day,
    )
}

/// The deterministic offline reading: a complete, well-formed gopher text page
/// (header + three framed sections + a through-line), with no key and no
/// network. Same (question, spread, cosmic) always yields the same text.
pub fn local_reading(question: &str, spread: &[DrawnCard; 3], cosmic: &Cosmic) -> String {
    let mut s = render_header(question, cosmic);
    for (i, (d, pos)) in spread.iter().zip(POSITIONS.iter()).enumerate() {
        let narrative = local_narrative(i, d, cosmic);
        s.push_str(&render_section(pos, d, &narrative));
    }
    s.push_str("--------------------------------------------------------------\n");
    s.push_str("  THE THREAD\n");
    s.push_str("--------------------------------------------------------------\n\n");
    s.push_str(&wrap_indented(
        &format!(
            "The thread runs from {} through {} toward {}. Read them as one motion, not three verdicts.",
            spread[0].card.name, spread[1].card.name, spread[2].card.name
        ),
        60,
        "  ",
    ));
    s.push('\n');
    s
}

/// Render a reading whose narrative came from the LLM: the same header and
/// framed cards as the offline reading, followed by the model's prose (markdown
/// lightly flattened to plain gopher text and wrapped). The LLM's own `##`
/// per-card headers carry the structure.
pub fn render_llm_reading(
    question: &str,
    spread: &[DrawnCard; 3],
    cosmic: &Cosmic,
    prose: &str,
) -> String {
    let mut s = render_header(question, cosmic);
    s.push_str("  YOUR THREE CARDS\n\n");
    for (d, pos) in spread.iter().zip(POSITIONS.iter()) {
        s.push_str(&render_frame(&d.card, d.reversed));
        s.push_str(&format!("\n  {pos}: {}\n\n", card_label(d)));
    }
    s.push_str("--------------------------------------------------------------\n");
    s.push_str("  THE READING\n");
    s.push_str("--------------------------------------------------------------\n\n");
    s.push_str(&flatten_markdown(prose));
    s
}

/// Flatten LLM markdown to plain gopher text: drop heading hashes and bold/italic
/// markers, then word-wrap each paragraph with a two-space indent. Blank lines
/// are preserved as paragraph breaks.
fn flatten_markdown(prose: &str) -> String {
    let mut out = String::new();
    for raw in prose.lines() {
        let line = raw.trim_start_matches('#').trim_start().replace("**", "");
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str(&wrap_indented(&line, 60, "  "));
        }
    }
    out
}

/// Greedy word-wrap with a constant indent on every line.
fn wrap_indented(text: &str, width: usize, indent: &str) -> String {
    let mut out = String::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            out.push_str(indent);
            out.push_str(&line);
            out.push('\n');
            line.clear();
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push_str(indent);
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosmic::{compute, CivilTime};
    use crate::deck::{draw, seed_hash};

    fn sky() -> Cosmic {
        compute(CivilTime {
            year: 2026,
            month: 6,
            day: 27,
            hour: 12,
            minute: 0,
            second: 0,
        })
    }

    #[test]
    fn describes_all_three_positions() {
        let d = spread_description();
        assert!(d.contains("Current State"));
        assert!(d.contains("Focus for Growth"));
        assert!(d.contains("Potential in 7 Days"));
    }

    #[test]
    fn local_reading_is_well_formed() {
        let spread = draw(seed_hash("what should I focus on?"));
        let r = local_reading("what should I focus on?", &spread, &sky());
        assert!(r.contains("YOUR READING"));
        assert!(r.contains("what should I focus on?"));
        for pos in POSITIONS {
            assert!(
                r.to_uppercase().contains(&pos.to_uppercase()),
                "missing {pos}"
            );
        }
        for d in &spread {
            assert!(r.contains(d.card.name), "missing card {}", d.card.name);
        }
        // three frames present
        let frames = r.matches(".------------------------------.").count();
        assert_eq!(frames, 6, "three framed cards = 6 border lines");
        assert!(r.contains("THE THREAD"));
        assert!(r.contains("/cards/"), "links to static card pages");
    }

    #[test]
    fn local_reading_is_deterministic() {
        let spread = draw(seed_hash("seed-q"));
        let a = local_reading("seed-q", &spread, &sky());
        let b = local_reading("seed-q", &spread, &sky());
        assert_eq!(a, b);
    }

    #[test]
    fn local_reading_colours_focus_card_with_sky() {
        let spread = draw(seed_hash("anything"));
        let r = local_reading("anything", &spread, &sky());
        // the focus section names the moon phase + sign (Sagittarius on this date)
        assert!(r.contains("Sagittarius"));
    }

    /// RELEASE GATE (the ethical invariant). The assembled LLM prompt must carry
    /// the question, the cards, and the cosmic context -- and NONE of the client
    /// metadata a dcgi can see. `build_prompt` doesn't even take those as inputs,
    /// so this is structurally guaranteed; the test pins it against any future
    /// accidental plumbing and is wired as a CI gate.
    #[test]
    fn prompt_never_contains_client_metadata() {
        let q = "what should I focus on this week?";
        let spread = draw(seed_hash(q));
        let p = build_prompt(q, &spread, &sky());

        // Sentinel values a dcgi could observe but the prompt must never carry.
        let forbidden = [
            "203.0.113.77",        // client IP
            "client.evil.example", // client hostname
            "61234",               // client port
            "/tarot/draw.dcgi",    // selector path
            "Lynx/2.8.9",          // user-agent
            "Chicago, Illinois",   // geolocation
        ];
        for f in forbidden {
            assert!(!p.contains(f), "prompt leaked client metadata: {f}");
        }
        // No locating timestamp (date / year / weekday / clock / zone).
        for t in ["2026", "June", "27,", "Saturday", "12:00", "UTC", "GMT"] {
            assert!(!p.contains(t), "prompt leaked a locating timestamp: {t}");
        }
        // ...while the legitimate inputs ARE present.
        assert!(p.contains(q));
        assert!(p.contains("Zodiac Season"));
        for d in &spread {
            assert!(p.contains(d.card.name));
        }
    }

    #[test]
    fn build_prompt_has_question_cards_and_cosmic() {
        let spread = draw(seed_hash("a question"));
        let p = build_prompt("a question", &spread, &sky());
        assert!(p.contains("a question"));
        assert!(p.contains("Current State"));
        for d in &spread {
            assert!(p.contains(d.card.name));
        }
        assert!(p.contains("Zodiac Season"));
        // the prompt instructs against leaking ambient context
        assert!(p.contains("NEVER reference"));
    }
}
