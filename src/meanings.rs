//! Per-card meanings — upright and reversed.
//!
//! Authored here (askthedeck has no meaning text; it leans on the LLM). These
//! concise Rider-Waite-Smith readings feed both the static card pages and the
//! deterministic offline reading, so the app always says something real with no
//! key and no network. Pure data, keyed by the deck's internal card id.

/// `(upright, reversed)` meaning for a card id, or `None` if unknown.
pub fn meaning(id: &str) -> Option<(&'static str, &'static str)> {
    let m = match id {
        "00-TheFool" => (
            "New beginnings, leaps of faith, open-road innocence.",
            "Recklessness, hesitation at the edge, a foolish risk.",
        ),
        "01-TheMagician" => (
            "Will focused into action; you have all the tools.",
            "Manipulation, scattered power, talent left untapped.",
        ),
        "02-TheHighPriestess" => (
            "Intuition, the unseen, secrets kept and known.",
            "Secrets withheld, the inner voice ignored, surface noise.",
        ),
        "03-TheEmpress" => (
            "Abundance, nurturing, creative fertility.",
            "Smothering, a creative block, neglected self-care.",
        ),
        "04-TheEmperor" => (
            "Structure, authority, steady command.",
            "Rigidity, domination, control that has curdled.",
        ),
        "05-TheHierophant" => (
            "Tradition, shared belief, the trusted teacher.",
            "Dogma questioned; your own path over the institution.",
        ),
        "06-TheLovers" => (
            "Union, aligned values, a meaningful choice.",
            "Misalignment, broken trust, a choice avoided.",
        ),
        "07-TheChariot" => (
            "Willpower steering opposing forces to victory.",
            "Loss of control, scattered drive, stalled momentum.",
        ),
        "08-Strength" => (
            "Quiet courage; patience that tames the beast.",
            "Self-doubt, raw force, depleted resolve.",
        ),
        "09-TheHermit" => (
            "Solitude, inner search, a lantern in the dark.",
            "Isolation, withdrawal, good counsel refused.",
        ),
        "10-WheelOfFortune" => (
            "Turning luck, cycles, a pivotal change.",
            "Resistance to change, a downturn, bad timing.",
        ),
        "11-Justice" => (
            "Fairness, truth, cause and consequence.",
            "Imbalance, evasion, accountability dodged.",
        ),
        "12-TheHangedMan" => (
            "Surrender; a new angle seen from stillness.",
            "Stalling, pointless martyrdom, a stuck perspective.",
        ),
        "13-Death" => (
            "Endings that clear the ground for renewal.",
            "Clinging to the dead; resisting a needed end.",
        ),
        "14-Temperance" => (
            "Balance, patient blending, the middle way.",
            "Excess, imbalance, impatience throwing things off.",
        ),
        "15-TheDevil" => (
            "Bondage to desire; the cage you can leave.",
            "Breaking chains, facing the shadow, release.",
        ),
        "16-TheTower" => (
            "Sudden upheaval; false structures fall.",
            "Disaster delayed or survived; fearful clinging.",
        ),
        "17-TheStar" => (
            "Hope, healing, calm faith after the storm.",
            "Despair, lost faith, disconnection from hope.",
        ),
        "18-TheMoon" => (
            "Illusion, dreams, the half-lit unknown.",
            "Confusion lifting, secrets surfacing, fear faced.",
        ),
        "19-TheSun" => (
            "Joy, clarity, vitality, success in the open.",
            "Dimmed joy, delay, a cloud over a good thing.",
        ),
        "20-Judgement" => (
            "Reckoning, awakening, a clear call to rise.",
            "Self-doubt, a calling ignored, harsh self-judgement.",
        ),
        "21-TheWorld" => (
            "Completion, wholeness, a cycle fulfilled.",
            "Loose ends, near-done, closure withheld.",
        ),
        "Cups01" => (
            "An overflowing heart; love and feeling begin.",
            "Blocked emotion, emptiness, love held back.",
        ),
        "Cups02" => (
            "Partnership, mutual attraction, a true accord.",
            "Discord, imbalance, a connection fraying.",
        ),
        "Cups03" => (
            "Celebration, friendship, community joy.",
            "Overindulgence, gossip, a third-wheel strain.",
        ),
        "Cups04" => (
            "Apathy, contemplation, a gift unnoticed.",
            "Waking from boredom; new openness, acceptance.",
        ),
        "Cups05" => (
            "Grief over what spilled; two cups still stand.",
            "Acceptance, recovery, moving on from loss.",
        ),
        "Cups06" => (
            "Nostalgia, innocence, kindness from the past.",
            "Stuck in the past, leaving home, naivete.",
        ),
        "Cups07" => (
            "Many tempting options, illusion, daydreams.",
            "Clarity, a decisive choice, the fog clearing.",
        ),
        "Cups08" => (
            "Walking away to seek something deeper.",
            "Aimlessness, fear of leaving, drifting back.",
        ),
        "Cups09" => (
            "Contentment, a wish granted, emotional ease.",
            "Smugness, unmet wishes, hollow indulgence.",
        ),
        "Cups10" => (
            "Lasting joy, harmony, the fulfilled home.",
            "Broken harmony, misaligned values, strain.",
        ),
        "Cups11" => (
            "A tender message, creative invitation, wonder.",
            "Emotional immaturity, moodiness, a blocked muse.",
        ),
        "Cups12" => (
            "Romance offered; following the heart.",
            "Moodiness, unrealistic ideals, broken promises.",
        ),
        "Cups13" => (
            "Compassion, emotional depth, intuitive care.",
            "Over-giving, martyrdom, emotional overwhelm.",
        ),
        "Cups14" => (
            "Mastery of feeling; calm, diplomatic warmth.",
            "Moodiness turned cold, manipulation, volatility.",
        ),
        "Pentacles01" => (
            "A tangible opportunity; prosperity takes root.",
            "A missed chance, scarcity thinking, poor planning.",
        ),
        "Pentacles02" => (
            "Juggling priorities with nimble balance.",
            "Overwhelm, dropped balls, poor money flow.",
        ),
        "Pentacles03" => (
            "Skilled collaboration; craft and recognition.",
            "Discord, sloppy work, mismatched effort.",
        ),
        "Pentacles04" => (
            "Holding tight; security guarded too closely.",
            "Loosening the grip, generosity, or reckless spending.",
        ),
        "Pentacles05" => (
            "Hardship, want, feeling left out in the cold.",
            "Recovery, help found, the worst now passing.",
        ),
        "Pentacles06" => (
            "Generosity, fair exchange, give and take.",
            "Strings attached, debt, lopsided giving.",
        ),
        "Pentacles07" => (
            "Patience; assessing a slow-growing investment.",
            "Impatience, poor return, effort wasted.",
        ),
        "Pentacles08" => (
            "Diligence; mastery through steady practice.",
            "Perfectionism, dull repetition, corners cut.",
        ),
        "Pentacles09" => (
            "Earned comfort, self-sufficiency, refinement.",
            "Over-reliance, hollow luxury, setbacks.",
        ),
        "Pentacles10" => (
            "Legacy, lasting wealth, a family foundation.",
            "Financial instability, broken legacy, conflict.",
        ),
        "Pentacles11" => (
            "A student of the practical; promising news.",
            "Distraction, lessons missed, plans unrealized.",
        ),
        "Pentacles12" => (
            "Reliable, methodical, the long steady haul.",
            "Stagnation, dullness, work without progress.",
        ),
        "Pentacles13" => (
            "Grounded nurture; a thriving home and means.",
            "Smothering, work-life imbalance, neglect.",
        ),
        "Pentacles14" => (
            "Abundant master of the material; stable success.",
            "Greed, stubbornness, status over substance.",
        ),
        "Swords01" => (
            "A breakthrough of clarity; truth cuts clean.",
            "Confusion, misused force, clouded judgment.",
        ),
        "Swords02" => (
            "Stalemate; a hard choice avoided, eyes closed.",
            "Indecision breaking; hidden facts revealed.",
        ),
        "Swords03" => (
            "Heartbreak, painful truth, grief that clears.",
            "Recovery, releasing hurt, forgiveness.",
        ),
        "Swords04" => (
            "Rest, retreat, recovery before the next round.",
            "Restlessness, burnout, needed rest refused.",
        ),
        "Swords05" => (
            "Conflict won at a cost; a hollow victory.",
            "Reconciliation, releasing a grudge, regret.",
        ),
        "Swords06" => (
            "Passage to calmer water; trouble left behind.",
            "Stuck, unable to move on, a delayed crossing.",
        ),
        "Swords07" => (
            "Strategy, stealth, getting away with something.",
            "Confession, getting caught, a change of heart.",
        ),
        "Swords08" => (
            "A self-made prison; trapped by your own fears.",
            "Freeing yourself, new perspective, release.",
        ),
        "Swords09" => (
            "Anxiety, sleepless dread, fears magnified at night.",
            "Hope returning, fears faced, despair easing.",
        ),
        "Swords10" => (
            "Rock bottom; a painful but final ending.",
            "Recovery; the only way left is up, survival.",
        ),
        "Swords11" => (
            "Curiosity, sharp questions, vigilant new ideas.",
            "Spite, scattered thoughts, all talk.",
        ),
        "Swords12" => (
            "Charging ahead with fierce, fast conviction.",
            "Recklessness, blunt force, burning out.",
        ),
        "Swords13" => (
            "Clear-eyed honesty; independent, perceptive.",
            "Coldness, harsh words, bitter isolation.",
        ),
        "Swords14" => (
            "Authority of intellect; ethical, decisive truth.",
            "The tyranny of logic, cold judgment, misused power.",
        ),
        "Wands01" => (
            "A spark of inspiration; raw creative drive.",
            "False starts, delays, a spark that fizzles.",
        ),
        "Wands02" => (
            "Planning the future; the world in your hand.",
            "Fear of the unknown, playing it too safe.",
        ),
        "Wands03" => (
            "Expansion, foresight, ships coming in.",
            "Delays, narrow vision, plans stalling.",
        ),
        "Wands04" => (
            "Celebration, homecoming, a stable milestone.",
            "Transition, disharmony, a shaky foundation.",
        ),
        "Wands05" => (
            "Friction, competition, scrappy disagreement.",
            "Conflict avoided, tension easing, or inner strife.",
        ),
        "Wands06" => (
            "Victory, recognition, riding high.",
            "Fallen pride, delayed success, credit withheld.",
        ),
        "Wands07" => (
            "Standing your ground; defending your position.",
            "Overwhelm, the high ground given up, exhaustion.",
        ),
        "Wands08" => (
            "Swift motion; news and events moving fast.",
            "Delays, scattered energy, frustration.",
        ),
        "Wands09" => (
            "Resilience; battered but still standing guard.",
            "Exhaustion, paranoia, defenses overdone.",
        ),
        "Wands10" => (
            "Burden; carrying too much toward the goal.",
            "Letting go of the load, delegating, or collapse.",
        ),
        "Wands11" => (
            "Enthusiasm, discovery, a bold free spirit.",
            "Aimless energy, dramatics, a stalled start.",
        ),
        "Wands12" => (
            "Adventure, passion, charging toward the new.",
            "Impulsiveness, hot temper, a project abandoned.",
        ),
        "Wands13" => (
            "Confident warmth; magnetic, determined vitality.",
            "Insecurity, jealousy, a demanding intensity.",
        ),
        "Wands14" => (
            "Visionary leadership; bold, charismatic command.",
            "Impulsive tyranny, overbearing, reckless vision.",
        ),
        _ => return None,
    };
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deck::all_cards;

    #[test]
    fn every_card_has_a_nonempty_meaning() {
        for c in all_cards() {
            let (up, rev) = meaning(c.id).unwrap_or_else(|| panic!("no meaning for {}", c.id));
            assert!(
                !up.is_empty() && !rev.is_empty(),
                "empty meaning for {}",
                c.id
            );
        }
    }

    #[test]
    fn known_meanings() {
        assert_eq!(
            meaning("00-TheFool").unwrap().0,
            "New beginnings, leaps of faith, open-road innocence."
        );
        assert!(meaning("Swords10").unwrap().0.contains("Rock bottom"));
        assert!(meaning("nope").is_none());
    }
}
