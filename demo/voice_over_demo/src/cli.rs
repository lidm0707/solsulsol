use clap::Parser;

use crate::config::{
    auto_melody, VoiceCharacter, VoiceSettings, DEFAULT_OUTPUT, DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO,
};
use crate::error::AppError;

pub const ABOUT: &str =
    "Offline English singing-voice synthesizer prototype (formant DSP, pure Rust)";

#[derive(Debug, Clone, Parser)]
#[command(name = "singer", about = ABOUT)]
pub struct Cli {
    /// English lyrics; words map one-to-one onto --notes
    #[arg(short, long)]
    pub lyrics: Option<String>,

    /// Melody as MIDI note specs "C4:1" (pitch:beats), one per word
    #[arg(short, long, num_args = 1..)]
    pub notes: Option<Vec<String>>,

    /// Tempo in BPM
    #[arg(long, default_value_t = DEFAULT_TEMPO)]
    pub tempo: f32,

    /// Output WAV file
    #[arg(short, long, default_value = DEFAULT_OUTPUT)]
    pub output: String,

    /// WAV sample rate in Hz
    #[arg(long, default_value_t = DEFAULT_SAMPLE_RATE)]
    pub sample_rate: u32,

    /// Voice character preset (neutral | anime)
    #[arg(long, default_value = VoiceCharacter::Neutral.as_str())]
    pub voice: String,

    /// Vibrato rate in Hz (defaults from --voice preset)
    #[arg(long)]
    pub vibrato_rate: Option<f32>,

    /// Vibrato depth in cents (defaults from --voice preset)
    #[arg(long)]
    pub vibrato_depth: Option<f32>,

    /// Delay before vibrato starts, as fraction of note length (defaults from --voice preset)
    #[arg(long)]
    pub vibrato_delay: Option<f32>,

    /// Glide into each note, in milliseconds
    #[arg(long, default_value_t = 60.0)]
    pub portamento_ms: f32,

    /// Breath noise amount [0.0, 1.0] (defaults from --voice preset)
    #[arg(long)]
    pub breathiness: Option<f32>,

    /// Print spectral stats (centroid, dominant, f0, peak) after rendering
    #[arg(long)]
    pub stats: bool,

    /// Analyze an existing WAV file instead of rendering
    #[arg(long)]
    pub stats_file: Option<String>,

    /// Per-word f0 contour depth in cents (rise then fall; 0 = flat)
    #[arg(long, default_value_t = crate::config::DEFAULT_CONTOUR_CENTS)]
    pub contour_cents: f32,

    /// Fetch a model from Hugging Face (owner/repo/path/file.onnx) into the
    /// cache and exit; skips normal rendering
    #[arg(long, conflicts_with_all = ["lyrics", "stats_file"])]
    pub fetch: Option<String>,

    /// Synthesize speech from text using a Piper model instead of singing
    #[arg(long, conflicts_with_all = ["lyrics", "stats_file", "fetch"])]
    pub say: Option<String>,

    /// Piper ONNX model file for --say
    #[arg(long)]
    pub model: Option<String>,

    /// Piper config json for --say (defaults to <model>.json)
    #[arg(long)]
    pub model_config: Option<String>,
}

fn require(flag: &str, value: Option<String>) -> Result<String, AppError> {
    value.ok_or_else(|| AppError::MissingArg(flag.to_owned()))
}

impl Cli {
    pub fn into_config(self) -> Result<crate::config::RenderConfig, AppError> {
        let lyrics_raw = require("--lyrics", self.lyrics)?;
        let lyrics: Vec<String> = lyrics_raw.split_whitespace().map(str::to_owned).collect();
        let notes = match self.notes.as_deref() {
            Some(raw) => crate::score::parse_notes(raw)?,
            None => auto_melody(lyrics.len()),
        };
        let character = VoiceCharacter::parse(&self.voice).ok_or_else(|| {
            AppError::UnknownVoice(format!(
                "{} (choose from: {})",
                self.voice,
                VoiceCharacter::CHOICES
            ))
        })?;
        let preset = VoiceSettings::from(character);
        Ok(crate::config::RenderConfig {
            lyrics,
            notes,
            tempo_bpm: self.tempo,
            sample_rate: self.sample_rate,
            output: self.output.into(),
            voice: VoiceSettings {
                character,
                vibrato_rate: self.vibrato_rate.unwrap_or(preset.vibrato_rate),
                vibrato_depth_cents: self.vibrato_depth.unwrap_or(preset.vibrato_depth_cents),
                vibrato_delay: self.vibrato_delay.unwrap_or(preset.vibrato_delay),
                portamento_seconds: self.portamento_ms / 1000.0,
                breathiness: self.breathiness.unwrap_or(preset.breathiness),
                speech_contour_cents: self.contour_cents,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(notes: Option<Vec<&str>>) -> Cli {
        Cli {
            lyrics: Some("la la la".into()),
            notes: notes.map(|n| n.iter().map(|s| s.to_string()).collect()),
            tempo: DEFAULT_TEMPO,
            output: DEFAULT_OUTPUT.into(),
            sample_rate: DEFAULT_SAMPLE_RATE,
            voice: "neutral".into(),
            vibrato_rate: None,
            vibrato_depth: None,
            vibrato_delay: None,
            portamento_ms: 60.0,
            breathiness: None,
            stats: false,
            stats_file: None,
            fetch: None,
            say: None,
            model: None,
            model_config: None,
            contour_cents: crate::config::DEFAULT_CONTOUR_CENTS,
        }
    }

    #[test]
    fn provided_notes_are_parsed() {
        let cfg = cli(Some(vec!["C4:1", "D4:2", "E4:1"]))
            .into_config()
            .unwrap();
        assert_eq!(cfg.notes.len(), 3);
        assert_eq!(cfg.notes[1].midi, 62);
        assert_eq!(cfg.notes[1].beats, 2.0);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn missing_notes_generate_auto_melody() {
        let cfg = cli(None).into_config().unwrap();
        assert_eq!(cfg.notes.len(), cfg.lyrics.len());
        assert_eq!(cfg.notes, crate::config::auto_melody(3));
        assert!(cfg.validate().is_ok());
    }
}
