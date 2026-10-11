use std::path::PathBuf;

use crate::phoneme::Phoneme;

pub const DEFAULT_SAMPLE_RATE: u32 = 44_100;
pub const MIN_SAMPLE_RATE: u32 = 8_000;
pub const MAX_SAMPLE_RATE: u32 = 192_000;
pub const DEFAULT_TEMPO: f32 = 100.0;
pub const MIN_TEMPO: f32 = 30.0;
pub const MAX_TEMPO: f32 = 300.0;
pub const DEFAULT_OUTPUT: &str = "output.wav";
pub const DEFAULT_VIBRATO_RATE: f32 = 5.5;
pub const DEFAULT_VIBRATO_DEPTH: f32 = 40.0;
pub const DEFAULT_VIBRATO_DELAY: f32 = 0.3;
pub const DEFAULT_PORTAMENTO_MS: f32 = 60.0;
pub const DEFAULT_BREATHINESS: f32 = 0.08;
pub const MAX_BREATHINESS: f32 = 1.0;
pub const DEFAULT_VOICE: VoiceCharacter = VoiceCharacter::Neutral;

const ANIME_FORMANT_SCALE: f32 = 1.18;
const NEUTRAL_FORMANT_SCALE: f32 = 1.0;
const NEUTRAL_TILT_CUTOFF_RATIO: f32 = 0.04;
const ANIME_TILT_CUTOFF_RATIO: f32 = 0.08;
const ANIME_BREATHINESS: f32 = 0.14;
const ANIME_VIBRATO_RATE: f32 = 6.5;
const ANIME_VIBRATO_DEPTH: f32 = 55.0;
const ANIME_VIBRATO_DELAY: f32 = 0.4;
const NEUTRAL_CONSONANT_WEIGHT: f32 = 0.15;
const ANIME_CONSONANT_WEIGHT: f32 = 0.08;
pub const DEFAULT_CONTOUR_CENTS: f32 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCharacter {
    Neutral,
    AnimeGirl,
}

impl VoiceCharacter {
    pub const CHOICES: &'static str = "neutral | anime";

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "neutral" => Some(Self::Neutral),
            "anime" => Some(Self::AnimeGirl),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::AnimeGirl => "anime",
        }
    }

    pub fn formant_scale(self) -> f32 {
        match self {
            Self::Neutral => NEUTRAL_FORMANT_SCALE,
            Self::AnimeGirl => ANIME_FORMANT_SCALE,
        }
    }

    pub fn tilt_cutoff_ratio(self) -> f32 {
        match self {
            Self::Neutral => NEUTRAL_TILT_CUTOFF_RATIO,
            Self::AnimeGirl => ANIME_TILT_CUTOFF_RATIO,
        }
    }

    pub fn breathiness(self) -> f32 {
        match self {
            Self::Neutral => DEFAULT_BREATHINESS,
            Self::AnimeGirl => ANIME_BREATHINESS,
        }
    }

    pub fn vibrato_rate(self) -> f32 {
        match self {
            Self::Neutral => DEFAULT_VIBRATO_RATE,
            Self::AnimeGirl => ANIME_VIBRATO_RATE,
        }
    }

    pub fn vibrato_depth_cents(self) -> f32 {
        match self {
            Self::Neutral => DEFAULT_VIBRATO_DEPTH,
            Self::AnimeGirl => ANIME_VIBRATO_DEPTH,
        }
    }

    pub fn vibrato_delay(self) -> f32 {
        match self {
            Self::Neutral => DEFAULT_VIBRATO_DELAY,
            Self::AnimeGirl => ANIME_VIBRATO_DELAY,
        }
    }

    pub fn consonant_weight(self) -> f32 {
        match self {
            Self::Neutral => NEUTRAL_CONSONANT_WEIGHT,
            Self::AnimeGirl => ANIME_CONSONANT_WEIGHT,
        }
    }
}
pub const TAIL_SECONDS: f32 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSettings {
    pub character: VoiceCharacter,
    pub vibrato_rate: f32,
    pub vibrato_depth_cents: f32,
    pub vibrato_delay: f32,
    pub portamento_seconds: f32,
    pub breathiness: f32,
    pub speech_contour_cents: f32,
}

impl VoiceSettings {
    fn from_character(character: VoiceCharacter) -> Self {
        Self {
            character,
            vibrato_rate: character.vibrato_rate(),
            vibrato_depth_cents: character.vibrato_depth_cents(),
            vibrato_delay: character.vibrato_delay(),
            portamento_seconds: DEFAULT_PORTAMENTO_MS / 1000.0,
            breathiness: character.breathiness(),
            speech_contour_cents: DEFAULT_CONTOUR_CENTS,
        }
    }
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self::from_character(DEFAULT_VOICE)
    }
}

impl VoiceSettings {}

impl From<VoiceCharacter> for VoiceSettings {
    fn from(character: VoiceCharacter) -> Self {
        Self::from_character(character)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderConfig {
    pub lyrics: Vec<String>,
    pub notes: Vec<NoteSpec>,
    pub tempo_bpm: f32,
    pub sample_rate: u32,
    pub output: PathBuf,
    pub voice: VoiceSettings,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteSpec {
    pub midi: u8,
    pub beats: f32,
}

impl RenderConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.lyrics.is_empty() {
            return Err(ConfigError::EmptyLyrics);
        }
        if self.lyrics.len() != self.notes.len() {
            return Err(ConfigError::WordNoteMismatch {
                words: self.lyrics.len(),
                notes: self.notes.len(),
            });
        }
        if !(MIN_TEMPO..=MAX_TEMPO).contains(&self.tempo_bpm) {
            return Err(ConfigError::TempoOutOfRange {
                tempo: self.tempo_bpm,
                min: MIN_TEMPO,
                max: MAX_TEMPO,
            });
        }
        if !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&self.sample_rate) {
            return Err(ConfigError::SampleRateOutOfRange {
                rate: self.sample_rate,
                min: MIN_SAMPLE_RATE,
                max: MAX_SAMPLE_RATE,
            });
        }
        if !(0.0..=MAX_BREATHINESS).contains(&self.voice.breathiness) {
            return Err(ConfigError::BreathinessOutOfRange {
                max: MAX_BREATHINESS,
            });
        }
        for note in &self.notes {
            if note.beats <= 0.0 {
                return Err(ConfigError::NonPositiveDuration);
            }
        }
        Ok(())
    }

    pub fn total_seconds(&self) -> f32 {
        let beat_seconds = SECONDS_PER_MINUTE / self.tempo_bpm;
        self.notes.iter().map(|n| n.beats).sum::<f32>() * beat_seconds + TAIL_SECONDS
    }
}

pub const AUTO_MELODY_SCALE: [u8; 5] = [60, 62, 64, 67, 69];
pub const DEFAULT_BEATS: f32 = 1.0;

pub fn auto_melody(word_count: usize) -> Vec<NoteSpec> {
    (0..word_count)
        .map(|i| {
            let midi = AUTO_MELODY_SCALE[i % AUTO_MELODY_SCALE.len()];
            NoteSpec {
                midi,
                beats: DEFAULT_BEATS,
            }
        })
        .collect()
}

pub fn note_phonemes(config: &RenderConfig) -> Vec<Vec<Phoneme>> {
    config
        .lyrics
        .iter()
        .map(|word| crate::phoneme::g2p(word))
        .collect()
}

pub const SECONDS_PER_MINUTE: f32 = 60.0;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ConfigError {
    #[error("lyrics are empty")]
    EmptyLyrics,

    #[error("{words} words but {notes} notes; provide one note per word")]
    WordNoteMismatch { words: usize, notes: usize },

    #[error("tempo {tempo} out of range [{min}, {max}]")]
    TempoOutOfRange { tempo: f32, min: f32, max: f32 },

    #[error("sample rate {rate} out of range [{min}, {max}]")]
    SampleRateOutOfRange { rate: u32, min: u32, max: u32 },

    #[error("breathiness out of range [0.0, {max}]")]
    BreathinessOutOfRange { max: f32 },

    #[error("note duration must be positive")]
    NonPositiveDuration,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(beats: &[f32]) -> RenderConfig {
        RenderConfig {
            lyrics: vec!["la".into(); beats.len()],
            notes: beats
                .iter()
                .map(|&b| NoteSpec { midi: 60, beats: b })
                .collect(),
            tempo_bpm: 120.0,
            sample_rate: DEFAULT_SAMPLE_RATE,
            output: DEFAULT_OUTPUT.into(),
            voice: VoiceSettings::default(),
        }
    }

    #[test]
    fn beat_timing_follows_tempo() {
        assert!((config(&[2.0]).total_seconds() - 1.0 - TAIL_SECONDS).abs() < 1e-5);
        assert!((config(&[4.0, 4.0]).total_seconds() - 4.0 - TAIL_SECONDS).abs() < 1e-5);
    }

    #[test]
    fn rejects_word_note_mismatch() {
        let cfg = RenderConfig {
            notes: vec![NoteSpec {
                midi: 60,
                beats: 1.0,
            }],
            ..config(&[1.0, 1.0])
        };
        assert_eq!(
            cfg.validate(),
            Err(ConfigError::WordNoteMismatch { words: 2, notes: 1 })
        );
    }

    #[test]
    fn rejects_tempo_out_of_range() {
        let mut cfg = config(&[1.0]);
        cfg.tempo_bpm = 500.0;
        assert!(matches!(
            cfg.validate(),
            Err(ConfigError::TempoOutOfRange { .. })
        ));
    }

    #[test]
    fn rejects_zero_duration_note() {
        let cfg = config(&[0.0]);
        assert_eq!(cfg.validate(), Err(ConfigError::NonPositiveDuration));
    }

    #[test]
    fn accepts_valid_config() {
        assert_eq!(config(&[1.0, 2.0]).validate(), Ok(()));
    }

    #[test]
    fn auto_melody_matches_word_count() {
        for count in 1..=250 {
            assert_eq!(auto_melody(count).len(), count);
        }
    }

    #[test]
    fn auto_melody_is_deterministic() {
        assert_eq!(auto_melody(12), auto_melody(12));
    }

    #[test]
    fn auto_melody_cycles_scale_at_default_beats() {
        let notes = auto_melody(AUTO_MELODY_SCALE.len() + 2);
        assert_eq!(notes[0].midi, AUTO_MELODY_SCALE[0]);
        assert_eq!(notes[AUTO_MELODY_SCALE.len()].midi, AUTO_MELODY_SCALE[0]);
        assert!(notes.iter().all(|n| n.beats == DEFAULT_BEATS));
    }

    #[test]
    fn contour_cents_rise_then_fall() {
        const DEPTH: f32 = 120.0;
        assert_eq!(crate::synth::contour_cents_at(0.0, DEPTH), 0.0);
        let mid = crate::synth::contour_cents_at(0.5, DEPTH);
        assert!(mid > 0.0 && mid < DEPTH, "mid {mid}");
        let end = crate::synth::contour_cents_at(1.0, DEPTH);
        assert!((end + DEPTH).abs() < 1e-4, "end {end}");
    }

    #[test]
    fn voice_character_parse_round_trip() {
        for name in ["neutral", "anime"] {
            let c = VoiceCharacter::parse(name).expect("valid choice");
            assert_eq!(c.as_str(), name);
        }
        assert_eq!(VoiceCharacter::parse("robot"), None);
    }

    #[test]
    fn anime_formants_higher_than_neutral() {
        assert!(
            VoiceCharacter::AnimeGirl.formant_scale() > VoiceCharacter::Neutral.formant_scale()
        );
        assert!(
            VoiceCharacter::AnimeGirl.tilt_cutoff_ratio()
                > VoiceCharacter::Neutral.tilt_cutoff_ratio()
        );
        assert!(VoiceCharacter::AnimeGirl.breathiness() > VoiceCharacter::Neutral.breathiness());
    }

    #[test]
    fn anime_preset_applies_character() {
        let anime = VoiceSettings::from(VoiceCharacter::AnimeGirl);
        assert_eq!(anime.character, VoiceCharacter::AnimeGirl);
        assert_eq!(anime.vibrato_rate, VoiceCharacter::AnimeGirl.vibrato_rate());
        assert_eq!(anime.breathiness, VoiceCharacter::AnimeGirl.breathiness());
    }
}
