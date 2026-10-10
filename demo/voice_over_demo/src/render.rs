use crate::config::RenderConfig;
use crate::synth::Synth;

pub const NORMALIZE_PEAK: f32 = 0.8;

pub fn render(cfg: &RenderConfig) -> (Vec<f32>, u32) {
    let phonemes_per_note = crate::config::note_phonemes(cfg);
    let mut synth = Synth::new(cfg.sample_rate, cfg.voice.clone());
    let mut out = Vec::with_capacity((cfg.total_seconds() * cfg.sample_rate as f32) as usize);

    for (note, phonemes) in cfg.notes.iter().zip(phonemes_per_note) {
        let duration = note.beats * crate::config::SECONDS_PER_MINUTE / cfg.tempo_bpm;
        out.extend(synth.render_note(note.midi, duration, &phonemes));
    }
    normalize(&mut out, NORMALIZE_PEAK);
    (out, cfg.sample_rate)
}

fn normalize(samples: &mut [f32], target: f32) {
    let peak = samples.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
    if peak > 1e-6 {
        let gain = target / peak;
        for s in samples.iter_mut() {
            *s *= gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NoteSpec, VoiceSettings};
    use crate::synth::midi_freq;

    fn cfg() -> RenderConfig {
        RenderConfig {
            lyrics: vec!["la".into(), "la".into()],
            notes: vec![
                NoteSpec {
                    midi: 60,
                    beats: 1.0,
                },
                NoteSpec {
                    midi: 64,
                    beats: 1.0,
                },
            ],
            tempo_bpm: 120.0,
            sample_rate: 22_050,
            output: "output.wav".into(),
            voice: VoiceSettings::default(),
        }
    }

    #[test]
    fn render_length_matches_score_timing() {
        let (samples, rate) = render(&cfg());
        assert_eq!(rate, 22_050);
        let expected = (1.0 * rate as f32) as usize;
        assert_eq!(samples.len(), expected);
    }

    #[test]
    fn rendered_audio_is_normalized_and_audible() {
        let (samples, _) = render(&cfg());
        let peak = samples.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
        assert!(peak > 0.1);
        assert!(peak <= NORMALIZE_PEAK + 1e-3);
    }

    #[test]
    fn pitch_rises_between_notes() {
        let (samples, rate) = render(&cfg());
        let half = samples.len() / 2;
        assert!(avg_zero_crossings(&samples[..half], rate) > 0.0);
        assert!(
            avg_zero_crossings(&samples[half..], rate)
                >= avg_zero_crossings(&samples[..half], rate)
        );
    }

    fn avg_zero_crossings(buf: &[f32], rate: u32) -> f32 {
        let crossings = buf.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        crossings as f32 / (buf.len() as f32 / rate as f32)
    }

    #[test]
    fn midi_freq_matches_synth() {
        assert!((midi_freq(60) - 261.6256).abs() < 1e-3);
    }
}
