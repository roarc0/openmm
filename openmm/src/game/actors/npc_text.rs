//! NPC dialogue text substitution — expands %01..%XX placeholders in greeting
//! and dialogue strings from npcbtb/proftext.
//!
//! MM6 substitution codes (from npcbtb.txt):
//!   %01 = NPC first name
//!   %02 = Player/party leader name
//!   %03 = Possessive pronoun (his/her)
//!   %04 = Gold amount (bribe cost)
//!   %05 = Time-of-day word (morning/day/evening/night)
//!   %06 = Gendered address (sir/madam)
//!   %07 = Gendered honorific (Lord/Lady)
//!   %08 = Reputation deed text
//!   %09 = Article (a/an)
//!   %10 = Gendered informal (lord/lady, lowercase)
//!   %11 = Reputation value
//!   %12 = Reputation threshold
//!   %13 = Party title/demonym
//!   %14 = Title prefix (Father/Sister etc.)
//!   %15 = Formal address
//!   %16 = Specific NPC reference
//!   %17 = Percentage value

use openmm_data::utils::time;

/// Context needed for text substitution.
pub struct SubstitutionContext {
    /// NPC first name (%01).
    pub npc_name: String,
    /// Current hour 0–23, used to derive time-of-day word (%05).
    pub hour: u32,
}

/// Expand %XX placeholders in NPC dialogue text.
///
/// Unrecognised codes are left in place so they're visible during development
/// — makes it obvious which substitutions still need implementing.
pub fn substitute_npc_text(text: &str, ctx: &SubstitutionContext) -> String {
    let mut result = text.to_string();

    // %01 — NPC first name
    if result.contains("%01") {
        result = result.replace("%01", &ctx.npc_name);
    }

    // %05 — time-of-day word (from openmm_data::utils::time)
    if result.contains("%05") {
        result = result.replace("%05", time::time_of_day_word(ctx.hour));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use openmm_data::utils::time;

    #[test]
    fn substitute_npc_name() {
        let ctx = SubstitutionContext {
            npc_name: "Alice".into(),
            hour: 10,
        };
        assert_eq!(
            substitute_npc_text("I'm %01. Pleased to meet you!", &ctx),
            "I'm Alice. Pleased to meet you!"
        );
    }

    #[test]
    fn substitute_time_of_day() {
        let ctx = SubstitutionContext {
            npc_name: "Bob".into(),
            hour: 9,
        };
        assert_eq!(substitute_npc_text("Good %05!", &ctx), "Good morning!");

        let evening_ctx = SubstitutionContext {
            npc_name: "Bob".into(),
            hour: 19,
        };
        assert_eq!(
            substitute_npc_text("Nice %05, isn't it?", &evening_ctx),
            "Nice evening, isn't it?"
        );
    }

    #[test]
    fn substitute_combined() {
        let ctx = SubstitutionContext {
            npc_name: "Carol".into(),
            hour: 14,
        };
        assert_eq!(substitute_npc_text("Good %05! I'm %01.", &ctx), "Good day! I'm Carol.");
    }

    #[test]
    fn unknown_codes_preserved() {
        let ctx = SubstitutionContext {
            npc_name: "Dan".into(),
            hour: 10,
        };
        // %02 (player name) not yet implemented — should remain in output.
        let result = substitute_npc_text("%02, eh? I'll remember that.", &ctx);
        assert!(result.contains("%02"));
    }

    #[test]
    fn time_of_day_boundaries() {
        assert_eq!(time::time_of_day_word(0), "night");
        assert_eq!(time::time_of_day_word(4), "night");
        assert_eq!(time::time_of_day_word(5), "morning");
        assert_eq!(time::time_of_day_word(11), "morning");
        assert_eq!(time::time_of_day_word(12), "day");
        assert_eq!(time::time_of_day_word(17), "day");
        assert_eq!(time::time_of_day_word(18), "evening");
        assert_eq!(time::time_of_day_word(21), "evening");
        assert_eq!(time::time_of_day_word(22), "night");
        assert_eq!(time::time_of_day_word(23), "night");
    }
}
