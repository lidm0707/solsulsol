use crate::config::VoiceSettings;
use crate::phoneme::{Phoneme, Vowel};

const FORMANT_GAINS: [f32; 3] = [1.0, 0.5, 0.25];
const VOWEL_BANDWIDTH: f32 = 90.0;
const CONSONANT_BANDWIDTH: f32 = 600.0;
const NASAL_BANDWIDTH: f32 = 150.0;
const STOP_SILENCE_RATIO: f32 = 0.4;
const STOP_DECAY_SECONDS: f32 = 0.05;
const GLIDE_MIN_FREQ: f32 = 50.0;
const VIBRATO_RAMP_SECONDS: f32 = 0.15;
const ATTACK_SECONDS: f32 = 0.02;
const RELEASE_SECONDS: f32 = 0.06;
const NASAL_F1: f32 = 250.0;
const NASAL_F2: f32 = 1_000.0;
const LIQUID_F1: f32 = 400.0;
const LIQUID_F2: f32 = 1_300.0;

#[derive(Clone, Copy, Default)]
pub struct Resonator {
    a1: f32,
    a2: f32,
    b0: f32,
    y1: f32,
    y2: f32,
}

impl Resonator {
    pub fn tune(&mut self, center_hz: f32, bandwidth: f32, sample_rate: f32) {
        let r = (-std::f32::consts::PI * bandwidth / sample_rate).exp();
        let theta = std::f32::consts::TAU * center_hz / sample_rate;
        self.a1 = 2.0 * r * theta.cos();
        self.a2 = -r * r;
        self.b0 = 1.0 - self.a1 - self.a2;
    }

    pub fn step(&mut self, input: f32) -> f32 {
        let y = self.b0 * input + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct Synth {
    sample_rate: f32,
    voice: VoiceSettings,
    phase: f32,
    prev_freq: f32,
    rng: u32,
    tilt_state: f32,
    vowel: [Resonator; 3],
    nasal: [Resonator; 2],
    liquid: [Resonator; 2],
    fricative: Resonator,
}

impl Synth {
    pub fn new(sample_rate: u32, voice: VoiceSettings) -> Self {
        Self {
            sample_rate: sample_rate as f32,
            voice,
            phase: 0.0,
            prev_freq: GLIDE_MIN_FREQ,
            rng: 0x9E37_79B9,
            tilt_state: 0.0,
            vowel: [Resonator::default(); 3],
            nasal: [Resonator::default(); 2],
            liquid: [Resonator::default(); 2],
            fricative: Resonator::default(),
        }
    }

    pub fn render_note(&mut self, midi: u8, duration: f32, phonemes: &[Phoneme]) -> Vec<f32> {
        let target = midi_freq(midi);
        let glide = (self.voice.portamento_seconds * self.sample_rate) as usize;
        let total = self.samples_for(duration);
        let segments = split_segments(phonemes, total);
        let mut out = vec![0.0; total];

        for (phoneme, (start, len)) in segments {
            for j in 0..len {
                let n = start + j;
                let freq = self.instantaneous_freq(target, glide.saturating_sub(start), j);
                let progress = j as f32 / len as f32;
                out[n] += self.sample(phoneme, freq, n, progress)
                    * boundary_ramp(j, len, self.sample_rate);
            }
            self.prev_freq = target;
        }
        apply_envelope(&mut out, self.sample_rate);
        out
    }

    fn instantaneous_freq(&self, target: f32, glide_len: usize, j: usize) -> f32 {
        if glide_len == 0 || j >= glide_len {
            return target;
        }
        let from = self.prev_freq.max(GLIDE_MIN_FREQ);
        from + (target - from) * (j as f32 / glide_len as f32)
    }

    fn sample(&mut self, phoneme: Phoneme, freq: f32, n: usize, progress: f32) -> f32 {
        let f0 = freq * self.vibrato_scale(n);
        let voiced = self.voiced_source(f0);
        match phoneme {
            Phoneme::Vowel(v) => self.vowel_sample(v, voiced),
            Phoneme::Nasal => band_sample(
                &mut self.nasal,
                voiced,
                NASAL_BANDWIDTH,
                [NASAL_F1, NASAL_F2],
                self.sample_rate,
            ),
            Phoneme::Liquid => band_sample(
                &mut self.liquid,
                voiced,
                NASAL_BANDWIDTH,
                [LIQUID_F1, LIQUID_F2],
                self.sample_rate,
            ),
            Phoneme::Fricative {
                center_hz,
                voiced: is_voiced,
            } => {
                let gain = if is_voiced { 0.4 } else { 0.9 };
                self.fricative
                    .tune(center_hz, CONSONANT_BANDWIDTH, self.sample_rate);
                let excitation = self.noise() + 0.2 * voiced;
                gain * self.fricative.step(excitation)
            }
            Phoneme::Stop { burst_hz, .. } => self.stop_sample(burst_hz, progress),
        }
    }

    fn vowel_sample(&mut self, v: Vowel, voiced: f32) -> f32 {
        let formants = [v.f1, v.f2, v.f3];
        let mut sum = self.voice.breathiness * self.noise();
        for (filter, (&f, gain)) in self
            .vowel
            .iter_mut()
            .zip(formants.iter().zip(FORMANT_GAINS))
        {
            filter.tune(f, VOWEL_BANDWIDTH, self.sample_rate);
            sum += gain * filter.step(voiced);
        }
        sum
    }

    fn stop_sample(&mut self, burst_hz: f32, progress: f32) -> f32 {
        if progress < STOP_SILENCE_RATIO {
            return 0.0;
        }
        let decay = -(progress - STOP_SILENCE_RATIO) / STOP_DECAY_SECONDS;
        self.fricative
            .tune(burst_hz, CONSONANT_BANDWIDTH, self.sample_rate);
        let excitation = self.noise();
        decay.exp() * self.fricative.step(excitation)
    }

    fn vibrato_scale(&self, n: usize) -> f32 {
        let t = n as f32 / self.sample_rate;
        let delay = self.voice.vibrato_delay.clamp(0.0, 0.9);
        let active = t - delay;
        if active <= 0.0 {
            return 1.0;
        }
        let ramp = (active / VIBRATO_RAMP_SECONDS).min(1.0);
        let depth = self.voice.vibrato_depth_cents / 1200.0;
        let cents = depth * ramp * (std::f32::consts::TAU * self.voice.vibrato_rate * active).sin();
        2.0_f32.powf(cents)
    }

    fn voiced_source(&mut self, f0: f32) -> f32 {
        let step = (f0 / self.sample_rate).min(0.5);
        self.phase = (self.phase + step) % 1.0;
        let saw = 2.0 * self.phase - 1.0;
        self.tilt(saw)
    }

    fn tilt(&mut self, input: f32) -> f32 {
        const CUTOFF_RATIO: f32 = 0.15;
        let alpha = (std::f32::consts::TAU * CUTOFF_RATIO).min(1.0);
        self.tilt_state += alpha * (input - self.tilt_state);
        self.tilt_state
    }

    fn noise(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        let scaled = self.rng as i32 as f32 / i32::MAX as f32;
        scaled.clamp(-1.0, 1.0)
    }

    fn samples_for(&self, duration: f32) -> usize {
        (duration * self.sample_rate).max(1.0) as usize
    }
}

fn band_sample(
    bank: &mut [Resonator; 2],
    voiced: f32,
    bandwidth: f32,
    centers: [f32; 2],
    sample_rate: f32,
) -> f32 {
    let mut sum = 0.0;
    for (filter, (&center, gain)) in bank.iter_mut().zip(centers.iter().zip([1.0_f32, 0.5])) {
        filter.tune(center, bandwidth, sample_rate);
        sum += gain * filter.step(voiced);
    }
    sum
}

const VOWEL_WEIGHT: f32 = 1.0;
const CONSONANT_WEIGHT: f32 = 0.15;

const BOUNDARY_RAMP_SECONDS: f32 = 0.005;

fn boundary_ramp(j: usize, len: usize, sample_rate: f32) -> f32 {
    let ramp = (BOUNDARY_RAMP_SECONDS * sample_rate) as usize;
    let head = (j.min(ramp)) as f32 / ramp as f32;
    let tail = ((len - j).min(ramp)) as f32 / ramp as f32;
    head.min(tail)
}

fn split_segments(phonemes: &[Phoneme], total: usize) -> Vec<(Phoneme, (usize, usize))> {
    if phonemes.is_empty() {
        return vec![(Phoneme::Vowel(crate::phoneme::VOWEL_SCHWA), (0, total))];
    }
    let weights: Vec<f32> = phonemes
        .iter()
        .map(|p| {
            if is_vowel(p) {
                VOWEL_WEIGHT
            } else {
                CONSONANT_WEIGHT
            }
        })
        .collect();
    let weight_sum: f32 = weights.iter().sum();
    let mut bounds = Vec::with_capacity(phonemes.len() + 1);
    let mut acc = 0.0_f32;
    for &w in &weights {
        bounds.push(acc as usize);
        acc += total as f32 * (w / weight_sum);
    }
    bounds.push(total);
    (0..phonemes.len())
        .filter_map(|i| {
            let start = bounds[i].min(total.saturating_sub(1));
            let end = bounds[i + 1].max(start + 1).min(total);
            if end <= start {
                None
            } else {
                Some((phonemes[i], (start, end - start)))
            }
        })
        .collect()
}

fn is_vowel(p: &Phoneme) -> bool {
    matches!(p, Phoneme::Vowel(_))
}

fn apply_envelope(samples: &mut [f32], sample_rate: f32) {
    let attack = (ATTACK_SECONDS * sample_rate) as usize;
    let release = (RELEASE_SECONDS * sample_rate) as usize;
    let len = samples.len();
    for (i, s) in samples.iter_mut().enumerate() {
        if i < attack {
            *s *= i as f32 / attack as f32;
        }
        let tail = len - i.saturating_sub(1) - 1;
        if tail < release {
            *s *= tail as f32 / release as f32;
        }
    }
}

pub fn midi_freq(midi: u8) -> f32 {
    const A4_MIDI: f32 = 69.0;
    const A4_HZ: f32 = 440.0;
    A4_HZ * 2.0_f32.powf((f32::from(midi) - A4_MIDI) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phoneme::{g2p, VOWEL_A};

    const SR: u32 = 22_050;

    #[test]
    fn midi_freq_follows_equal_temperament() {
        assert!((midi_freq(69) - 440.0).abs() < 1e-4);
        assert!((midi_freq(81) - 880.0).abs() < 1e-4);
        assert!((midi_freq(57) - 220.0).abs() < 1e-4);
    }

    #[test]
    fn note_length_matches_duration() {
        let mut synth = Synth::new(SR, VoiceSettings::default());
        let out = synth.render_note(60, 0.5, &[Phoneme::Vowel(VOWEL_A)]);
        assert_eq!(out.len(), (0.5 * SR as f32) as usize);
    }

    #[test]
    fn vowel_note_is_audible() {
        let mut synth = Synth::new(SR, VoiceSettings::default());
        let out = synth.render_note(69, 0.3, &[Phoneme::Vowel(VOWEL_A)]);
        let peak = out.iter().fold(0.0_f32, |m, &s| m.max(s.abs()));
        assert!(peak > 0.01, "silent output");
    }

    #[test]
    fn word_phonemes_render_without_panic() {
        let mut synth = Synth::new(SR, VoiceSettings::default());
        let out = synth.render_note(64, 0.4, &g2p("hello"));
        assert!(!out.is_empty());
    }

    #[test]
    fn segments_cover_whole_note() {
        let segs = split_segments(&g2p("sing"), 1_000);
        assert_eq!(segs.first().unwrap().1 .0, 0);
        let last = segs.last().unwrap();
        assert_eq!(last.1 .0 + last.1 .1, 1_000);
    }

    #[test]
    fn vowels_dominate_note_duration() {
        let segs = split_segments(&g2p("hello"), 44_100);
        let vowel_samples: usize = segs
            .iter()
            .filter(|(p, _)| is_vowel(p))
            .map(|(_, (_, len))| len)
            .sum();
        let consonant_samples: usize = segs
            .iter()
            .filter(|(p, _)| !is_vowel(p))
            .map(|(_, (_, len))| len)
            .sum();
        assert!(vowel_samples > 3 * consonant_samples);
    }

    #[test]
    fn vibrato_is_silent_before_delay() {
        let voice = VoiceSettings::default();
        let synth = Synth::new(SR, voice);
        let early = (0.1 * SR as f32) as usize;
        assert!((synth.vibrato_scale(early) - 1.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod segment_probe {
    use super::*;
    use crate::phoneme::Phoneme;

    #[test]
    fn probe_segment_energies() {
        let mut synth = Synth::new(44_100, VoiceSettings::default());
        let out = synth.render_note(
            57,
            2.0,
            &[
                Phoneme::Vowel(crate::phoneme::VOWEL_E),
                Phoneme::Liquid,
                Phoneme::Vowel(crate::phoneme::VOWEL_O),
            ],
        );
        for (name, range) in [
            ("e", 0..30_000),
            ("l", 30_000..40_000),
            ("o", 40_000..80_000),
        ] {
            let rms = (out[range].iter().map(|s| s * s).sum::<f32>() / 30_000.0).sqrt();
            println!("{name} rms = {rms}");
        }
    }
}
