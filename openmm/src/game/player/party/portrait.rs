//! Character portrait and voice system.
//!
//! Each party member has a portrait identity (e.g. `MaleA`, `GirlC`) that determines:
//! - Their character creation thumbnail: `CC{TYPE}` (e.g. `CCMALEA`)
//! - Their facial expression textures: `{type}{01-53}` (e.g. `MaleA01` through `MaleA53`)
//! - Their voice lines: `{type}{01-44}{variant}` (e.g. `MaleA31a`, `MaleA31b`)
//!
//! Portrait types are detected dynamically from the dsounds registry at startup.

use crate::game::sound::SoundManager;

/// A character portrait identity — encodes sex and variant letter (A–H male, A–D female).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortraitId {
    MaleA,
    MaleB,
    MaleC,
    MaleD,
    MaleE,
    MaleF,
    MaleG,
    MaleH,
    GirlA,
    GirlB,
    GirlC,
    GirlD,
}

impl PortraitId {
    /// All known portrait identities.
    pub const ALL_ARR: [PortraitId; 12] = {
        let mut all = [PortraitId::MaleA; 12];
        let mut i = 0;
        while i < Self::MALE.len() {
            all[i] = Self::MALE[i];
            i += 1;
        }
        let mut j = 0;
        while j < Self::FEMALE.len() {
            all[i] = Self::FEMALE[j];
            i += 1;
            j += 1;
        }
        all
    };
    
    pub const ALL: &'static [PortraitId] = &Self::ALL_ARR;

    pub const MALE: &[PortraitId] = &[
        Self::MaleA,
        Self::MaleB,
        Self::MaleC,
        Self::MaleD,
        Self::MaleE,
        Self::MaleF,
        Self::MaleG,
        Self::MaleH,
    ];

    pub const FEMALE: &[PortraitId] = &[Self::GirlA, Self::GirlB, Self::GirlC, Self::GirlD];

    pub fn is_male(self) -> bool {
        matches!(
            self,
            Self::MaleA
                | Self::MaleB
                | Self::MaleC
                | Self::MaleD
                | Self::MaleE
                | Self::MaleF
                | Self::MaleG
                | Self::MaleH
        )
    }

    /// The string prefix used in texture and sound names (e.g. `"MaleA"`, `"GirlC"`).
    pub fn prefix(self) -> &'static str {
        match self {
            Self::MaleA => "MaleA",
            Self::MaleB => "MaleB",
            Self::MaleC => "MaleC",
            Self::MaleD => "MaleD",
            Self::MaleE => "MaleE",
            Self::MaleF => "MaleF",
            Self::MaleG => "MaleG",
            Self::MaleH => "MaleH",
            Self::GirlA => "GirlA",
            Self::GirlB => "GirlB",
            Self::GirlC => "GirlC",
            Self::GirlD => "GirlD",
        }
    }

    /// Character creation thumbnail texture key (e.g. `"icons/CCMALEA"`).
    pub fn creation_texture(self) -> &'static str {
        match self {
            Self::MaleA => "icons/CCMALEA",
            Self::MaleB => "icons/CCMALEB",
            Self::MaleC => "icons/CCMALEC",
            Self::MaleD => "icons/CCMALED",
            Self::MaleE => "icons/CCMALEE",
            Self::MaleF => "icons/CCMALEF",
            Self::MaleG => "icons/CCMALEG",
            Self::MaleH => "icons/CCMALEH",
            Self::GirlA => "icons/CCGIRLA",
            Self::GirlB => "icons/CCGIRLB",
            Self::GirlC => "icons/CCGIRLC",
            Self::GirlD => "icons/CCGIRLD",
        }
    }

    /// Parse from a creation texture key like `"icons/CCMALEA"`.
    pub fn from_creation_texture(tex: &str) -> Option<Self> {
        let key = tex.strip_prefix("icons/")?;
        Self::ALL.iter().find(|p| p.creation_texture().ends_with(key)).copied()
    }

    /// Parse from a prefix string like `"MaleA"` or `"GirlD"` (case-sensitive).
    pub fn from_prefix(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|p| p.prefix() == s).copied()
    }

    /// Get the in-game facial expression texture key for a given expression.
    /// Returns e.g. `"icons/MaleA01"` for `PortraitId::MaleA` + `Expression::Unk01`.
    pub fn expression_texture(self, expr: Expression) -> String {
        format!("icons/{}{:02}", self.prefix(), expr.index())
    }

    /// Build the dsounds name for a voice line: `"{prefix}{nn}{variant}"`.
    /// The name is lowercase as stored in dsounds.bin.
    pub fn voice_name(self, speech: Speech, variant: SoundVariant) -> String {
        format!(
            "{}{:02}{}",
            self.prefix().to_lowercase(),
            speech.index(),
            variant.suffix()
        )
    }

    /// Get all available sound variants for a given speech type by probing dsounds.
    pub fn available_variants(self, speech: Speech, sound_manager: &SoundManager) -> Vec<SoundVariant> {
        let mut found = Vec::new();
        for variant in SoundVariant::ALL {
            let name = self.voice_name(speech, *variant);
            if sound_manager.dsounds.get_by_name(&name).is_some() {
                found.push(*variant);
            }
        }
        found
    }
}

/// Sound variant suffix (a, b, or c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundVariant {
    A,
    B,
    C,
}

impl SoundVariant {
    pub const ALL: &[SoundVariant] = &[Self::A, Self::B, Self::C];

    pub fn suffix(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
        }
    }
}

/// Character facial expression (01–53). Maps to portrait texture index.
/// Names are based on OpenEnroth's SpeechId where known; unknown ones use `Unk{NN}`.
/// The expression texture shows the character's face in that emotional state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Expression {
    Unk1 = 1,
    Unk2 = 2,
    Unk3 = 3,
    Unk4 = 4,
    Unk5 = 5,
    Unk6 = 6,
    Unk7 = 7,
    Unk8 = 8,
    Unk9 = 9,
    Unk10 = 10,
    Unk11 = 11,
    Unk12 = 12,
    Unk13 = 13,
    Unk14 = 14,
    Unk15 = 15,
    Unk16 = 16,
    Unk17 = 17,
    Unk18 = 18,
    Unk19 = 19,
    Unk20 = 20,
    Unk21 = 21,
    Unk22 = 22,
    Unk23 = 23,
    Unk24 = 24,
    Unk25 = 25,
    Unk26 = 26,
    Unk27 = 27,
    Unk28 = 28,
    Unk29 = 29,
    Unk30 = 30,
    Unk31 = 31,
    Unk32 = 32,
    Unk33 = 33,
    Unk34 = 34,
    Unk35 = 35,
    Unk36 = 36,
    Unk37 = 37,
    Unk38 = 38,
    Unk39 = 39,
    Unk40 = 40,
    Unk41 = 41,
    /// "Pick me!" — character creation selection
    PickMe = 42,
    Unk43 = 43,
    Unk44 = 44,
    Unk45 = 45,
    Unk46 = 46,
    Unk47 = 47,
    Unk48 = 48,
    Unk49 = 49,
    Unk50 = 50,
    Unk51 = 51,
    Unk52 = 52,
    Unk53 = 53,
}

impl Expression {
    pub const COUNT: usize = 53;

    /// Numeric index (1-based, matches filename suffix).
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Self> {
        if (1..=53).contains(&i) {
            // Safety: repr(u8) enum with contiguous values 1..=53
            Some(unsafe { std::mem::transmute(i) })
        } else {
            None
        }
    }
}

/// Character voice/speech type (01–44). Maps to sound file index.
/// Names are based on OpenEnroth's SpeechId mapping where known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Speech {
    Yes = 1,         // Yes
    No = 2,          // No
    Success = 3,     // It Worked
    CallOut = 4,     // Hey you!
    Tired = 5,       // Ahh tired
    Thinking = 6,    // Hmmm
    Aha = 7,         // Ha!
    Humph = 8,       // Humph!
    Closed = 9,      // Closed!
    GotIt = 10,      // Got it!
    UhOh = 11,       // Oh oh!
    UhOhStrong = 12, // Oh oh!!! stronger
    Sigh = 13,       // Ahhh
    Groan = 14,      // Uuuuh!
    Moan = 15,       // OOOOohhhuu
    Unk16 = 16,
    Unk17 = 17,
    Unk18 = 18,
    Unk19 = 19,
    Unk20 = 20,
    Unk21 = 21,
    Unk22 = 22,
    Unk23 = 23,
    Unk24 = 24,
    Unk25 = 25,
    Unk26 = 26,
    Unk27 = 27,
    Unk28 = 28,
    Unk29 = 29,
    Unk30 = 30,
    Unk31 = 31,
    Unk32 = 32,
    Unk33 = 33,
    Unk34 = 34,
    Unk35 = 35,
    Unk36 = 36,
    Unk37 = 37,
    Unk38 = 38,
    Unk39 = 39,
    Unk40 = 40,
    Unk41 = 41,
    PickMe = 42, // "Pick me!" — character creation selection
    Unk43 = 43,
    Unk44 = 44,
}

impl Speech {
    pub const COUNT: usize = 44;

    /// Numeric index (1-based, matches sound filename number).
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Option<Self> {
        if (1..=44).contains(&i) {
            // Safety: repr(u8) enum with contiguous values 1..=44
            Some(unsafe { std::mem::transmute(i) })
        } else {
            None
        }
    }
}

/// Cached voice availability for a portrait — which speech+variant combos exist in dsounds.
/// Built once at startup by probing the sound archive.
#[derive(Debug, Clone)]
pub struct PortraitVoices {
    pub portrait: PortraitId,
    /// For each speech type, which variants are available.
    /// Index: `speech.index() - 1`
    variants: [[bool; 3]; Speech::COUNT],
}

impl PortraitVoices {
    /// Probe dsounds to discover which voice lines exist for this portrait.
    pub fn discover(portrait: PortraitId, sound_manager: &SoundManager) -> Self {
        let mut variants = [[false; 3]; Speech::COUNT];
        let prefix = portrait.prefix().to_lowercase();

        for speech_idx in 1..=Speech::COUNT as u8 {
            for (vi, variant) in SoundVariant::ALL.iter().enumerate() {
                let name = format!("{}{:02}{}", prefix, speech_idx, variant.suffix());
                if sound_manager.dsounds.get_by_name(&name).is_some() {
                    variants[(speech_idx - 1) as usize][vi] = true;
                }
            }
        }

        Self { portrait, variants }
    }

    /// Get available variants for a speech type.
    pub fn available_variants(&self, speech: Speech) -> Vec<SoundVariant> {
        let idx = (speech.index() - 1) as usize;
        SoundVariant::ALL
            .iter()
            .enumerate()
            .filter(|(vi, _)| self.variants[idx][*vi])
            .map(|(_, v)| *v)
            .collect()
    }

    /// Get the dsounds name for a specific speech + variant combo.
    pub fn voice_name(&self, speech: Speech, variant: SoundVariant) -> String {
        self.portrait.voice_name(speech, variant)
    }

    /// Check if any variant exists for this speech type.
    pub fn has_speech(&self, speech: Speech) -> bool {
        let idx = (speech.index() - 1) as usize;
        self.variants[idx].iter().any(|&v| v)
    }
}
