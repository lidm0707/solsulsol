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
pub const TAIL_SECONDS: f32 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSettings {
    pub vibrato_rate: f32,
    pub vibrato_depth_cents: f32,
    pub vibrato_delay: f32,
    pub portamento_seconds: f32,
    pub breathiness: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            vibrato_rate: DEFAULT_VIBRATO_RATE,
            vibrato_depth_cents: DEFAULT_VIBRATO_DEPTH,
            vibrato_delay: DEFAULT_VIBRATO_DELAY,
            portamento_seconds: DEFAULT_PORTAMENTO_MS / 1000.0,
            breathiness: DEFAULT_BREATHINESS,
        }
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
}
