use clap::Parser;

use crate::config::{VoiceSettings, DEFAULT_OUTPUT, DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO};
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

    /// Vibrato rate in Hz
    #[arg(long, default_value_t = VoiceSettings::default().vibrato_rate)]
    pub vibrato_rate: f32,

    /// Vibrato depth in cents
    #[arg(long, default_value_t = VoiceSettings::default().vibrato_depth_cents)]
    pub vibrato_depth: f32,

    /// Delay before vibrato starts, as fraction of note length
    #[arg(long, default_value_t = VoiceSettings::default().vibrato_delay)]
    pub vibrato_delay: f32,

    /// Glide into each note, in milliseconds
    #[arg(long, default_value_t = 60.0)]
    pub portamento_ms: f32,

    /// Breath noise amount [0.0, 1.0]
    #[arg(long, default_value_t = 0.08)]
    pub breathiness: f32,
}

fn require(flag: &str, value: Option<String>) -> Result<String, AppError> {
    value.ok_or_else(|| AppError::MissingArg(flag.to_owned()))
}

impl Cli {
    pub fn into_config(self) -> Result<crate::config::RenderConfig, AppError> {
        let lyrics_raw = require("--lyrics", self.lyrics)?;
        let lyrics: Vec<String> = lyrics_raw.split_whitespace().map(str::to_owned).collect();
        let notes = crate::score::parse_notes(self.notes.as_deref().unwrap_or_default())?;
        Ok(crate::config::RenderConfig {
            lyrics,
            notes,
            tempo_bpm: self.tempo,
            sample_rate: self.sample_rate,
            output: self.output.into(),
            voice: VoiceSettings {
                vibrato_rate: self.vibrato_rate,
                vibrato_depth_cents: self.vibrato_depth,
                vibrato_delay: self.vibrato_delay,
                portamento_seconds: self.portamento_ms / 1000.0,
                breathiness: self.breathiness,
            },
        })
    }
}
