use crate::config::{VoiceCharacter, VoiceSettings};
use crate::phoneme::{Phoneme, Vowel};

const FORMANT_GAINS: [f32; 4] = [1.0, 0.5, 0.25, 0.12];
const NEUTRAL_F4_HZ: f32 = 3_600.0;
const ANIME_F4_SCALE: f32 = 1.15;
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
const OPEN_QUOTIENT: f32 = 0.6;
const RISE_FRACTION: f32 = 0.7;
// mean of glottal_pulse over one period; subtracted to avoid DC offset
const PULSE_DC: f32 = OPEN_QUOTIENT * (2.0 - RISE_FRACTION) / 3.0;
const JITTER_CENTS: f32 = 25.0;
const JITTER_SMOOTH: f32 = 0.05;
const SHIMMER: f32 = 0.06;

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
    jitter_state: f32,
    tilt_state: f32,
    vowel: [Resonator; 4],
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
            jitter_state: 0.0,
            tilt_state: 0.0,
            vowel: [Resonator::default(); 4],
            nasal: [Resonator::default(); 2],
            liquid: [Resonator::default(); 2],
            fricative: Resonator::default(),
        }
    }

    pub fn render_note(&mut self, midi: u8, duration: f32, phonemes: &[Phoneme]) -> Vec<f32> {
        let target = midi_freq(midi);
        let glide = (self.voice.portamento_seconds * self.sample_rate) as usize;
        let total = self.samples_for(duration);
        let segments = split_segments(phonemes, total, self.voice.character.consonant_weight());
        let mut out = vec![0.0; total];

        for (i, (phoneme, (start, len))) in segments.iter().enumerate() {
            let head_s = glide_seconds(&segments, i.wrapping_sub(1));
            let tail_s = glide_seconds(&segments, i + 1);
            for j in 0..*len {
                let n = start + j;
                let freq = self.instantaneous_freq(target, glide.saturating_sub(*start), j)
                    * contour_scale(n as f32 / total as f32, self.voice.speech_contour_cents);
                let progress = j as f32 / *len as f32;
                out[n] += self.sample(*phoneme, freq, n, progress)
                    * boundary_ramp(j, *len, self.sample_rate, head_s, tail_s);
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
        let formants = self.vowel_formants(v);
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

    fn vowel_formants(&self, v: Vowel) -> [f32; 4] {
        let scale = self.voice.character.formant_scale();
        let f4 = NEUTRAL_F4_HZ
            * if self.voice.character == VoiceCharacter::AnimeGirl {
                ANIME_F4_SCALE
            } else {
                1.0
            };
        [v.f1 * scale, v.f2 * scale, v.f3 * scale, f4]
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
        self.jitter_state += JITTER_SMOOTH * (self.noise() - self.jitter_state);
        let jitter = 2.0_f32.powf(JITTER_CENTS * self.jitter_state / 1200.0);
        let step = (f0 * jitter / self.sample_rate).min(0.5);
        self.phase = (self.phase + step) % 1.0;
        let shimmer = 1.0 + SHIMMER * self.noise();
        let pulse = glottal_pulse(self.phase) - PULSE_DC;
        self.tilt(pulse * shimmer)
    }

    fn tilt(&mut self, input: f32) -> f32 {
        let alpha = (std::f32::consts::TAU * self.voice.character.tilt_cutoff_ratio()).min(1.0);
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

const BOUNDARY_RAMP_SECONDS: f32 = 0.005;
const VOWEL_GLIDE_SECONDS: f32 = 0.035;

fn boundary_ramp(j: usize, len: usize, sample_rate: f32, head_s: f32, tail_s: f32) -> f32 {
    let head = (j as f32 / (head_s * sample_rate)).min(1.0);
    let tail = ((len - j) as f32 / (tail_s * sample_rate)).min(1.0);
    head.min(tail)
}

fn split_segments(
    phonemes: &[Phoneme],
    total: usize,
    consonant_weight: f32,
) -> Vec<(Phoneme, (usize, usize))> {
    if phonemes.is_empty() {
        return vec![(Phoneme::Vowel(crate::phoneme::VOWEL_SCHWA), (0, total))];
    }
    let weights: Vec<f32> = phonemes
        .iter()
        .map(|p| {
            if is_vowel(p) {
                VOWEL_WEIGHT
            } else {
                consonant_weight
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

fn glide_seconds(segments: &[(Phoneme, (usize, usize))], neighbor: usize) -> f32 {
    match segments.get(neighbor) {
        Some((p, _)) if is_vowel(p) => VOWEL_GLIDE_SECONDS,
        _ => BOUNDARY_RAMP_SECONDS,
    }
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

pub fn contour_cents_at(t: f32, depth_cents: f32) -> f32 {
    depth_cents * (std::f32::consts::PI * t).sin() - depth_cents * t
}

fn contour_scale(t: f32, depth_cents: f32) -> f32 {
    if depth_cents == 0.0 {
        return 1.0;
    }
    2.0_f32.powf(contour_cents_at(t, depth_cents) / 1200.0)
}

fn glottal_pulse(phase: f32) -> f32 {
    if phase >= OPEN_QUOTIENT {
        return 0.0;
    }
    let rise_len = RISE_FRACTION * OPEN_QUOTIENT;
    if phase < rise_len {
        (phase / rise_len).powi(2)
    } else {
        let fall = (phase - rise_len) / (OPEN_QUOTIENT - rise_len);
        1.0 - fall * fall
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
    fn glottal_pulse_is_continuous_and_bounded() {
        let at_open = glottal_pulse(OPEN_QUOTIENT - 1e-4);
        let at_close = glottal_pulse(OPEN_QUOTIENT + 1e-4);
        assert!((at_open - at_close).abs() < 0.01, "{at_open} vs {at_close}");
        assert_eq!(glottal_pulse(0.0), 0.0);
        assert!((glottal_pulse(OPEN_QUOTIENT * RISE_FRACTION) - 1.0).abs() < 1e-4);
        for p in [0.0_f32, 0.3, 0.5, 0.7, 0.95] {
            assert!((0.0..=1.0).contains(&glottal_pulse(p)));
        }
    }

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
        let segs = split_segments(&g2p("sing"), 1_000, 0.15);
        assert_eq!(segs.first().unwrap().1 .0, 0);
        let last = segs.last().unwrap();
        assert_eq!(last.1 .0 + last.1 .1, 1_000);
    }

    #[test]
    fn vowels_dominate_note_duration() {
        let segs = split_segments(&g2p("hello"), 44_100, 0.15);
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
