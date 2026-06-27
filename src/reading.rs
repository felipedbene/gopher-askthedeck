//! Reading assembly — the shared spread description now; the prompt builder and
//! the deterministic offline reading land in slice 4.

use crate::deck::POSITIONS;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_all_three_positions() {
        let d = spread_description();
        assert!(d.contains("Current State"));
        assert!(d.contains("Focus for Growth"));
        assert!(d.contains("Potential in 7 Days"));
    }
}
