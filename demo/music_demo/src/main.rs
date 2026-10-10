//! Synthesizes the solsulsol demo soundtrack: a soft chord pad, bass and a
//! gentle arpeggio over a four-bar progression, rendered to a stereo WAV.
use fundsp::hacker32::sine_hz;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::f32::consts::PI;

const SAMPLE_RATE: f32 = 44_100.0;
const BAR_SECONDS: f32 = 2.75;
const BAR_SAMPLES: usize = (BAR_SECONDS * SAMPLE_RATE) as usize;
/// Transition overlap between chord pads so the progression breathes.
const PAD_FADE: f32 = 0.4;
const PAD_GAIN: f32 = 0.10;
const BASS_GAIN: f32 = 0.14;
const ARP_GAIN: f32 = 0.09;
const MASTER_PEAK: f32 = 0.70;

/// Note frequency from semitone distance below A4 = 440 Hz.
fn note(semitones_below_a4: i32) -> f32 {
    440.0 * 2f32.powf(-semitones_below_a4 as f32 / 12.0)
}

struct Voice {
    freq: f32,
    gain: f32,
    detune: f32,
}

fn pad_voice(out: &mut [f32], voice: &Voice) {
    let mut osc = sine_hz(voice.freq);
    let mut detuned = sine_hz(voice.freq * (1.0 + voice.detune));
    for sample in out.iter_mut() {
        let (l, r) = osc.get_stereo();
        let (dl, dr) = detuned.get_stereo();
        *sample += ((l + dl) * 0.5 + (r + dr) * 0.5) * 0.5 * voice.gain;
    }
}

fn render_pad(bars: &[Vec<Voice>]) -> Vec<f32> {
    let total = bars.len() * BAR_SAMPLES;
    let mut out = vec![0.0; total];
    for (bar, voices) in bars.iter().enumerate() {
        let start = bar * BAR_SAMPLES;
        for voice in voices {
            let mut mono = vec![0.0; BAR_SAMPLES * 2];
            pad_voice(&mut mono, voice);
            for i in 0..BAR_SAMPLES {
                let t = i as f32 / SAMPLE_RATE;
                let fade_in = (t / PAD_FADE).min(1.0);
                let fade_out = ((BAR_SECONDS - t) / PAD_FADE).min(1.0);
                out[start + i] += mono[i * 2] * fade_in.min(fade_out);
            }
        }
    }
    out
}

fn render_bass(roots: &[f32]) -> Vec<f32> {
    let total = roots.len() * BAR_SAMPLES;
    let mut out = vec![0.0; total];
    for (bar, root) in roots.iter().enumerate() {
        let mut osc = sine_hz(*root / 2.0);
        let start = bar * BAR_SAMPLES;
        for i in 0..BAR_SAMPLES {
            let t = i as f32 / SAMPLE_RATE;
            let envelope = (t / 0.05).min(1.0) * (((BAR_SECONDS - t) / 0.3).min(1.0).max(0.0));
            let (l, r) = osc.get_stereo();
            out[start + i] += (l + r) * 0.5 * BASS_GAIN * envelope;
        }
    }
    out
}

/// Eighth-note plucks with an exponential decay, alternating stereo sides.
fn render_arp(chord_arps: &[Vec<f32>]) -> Vec<f32> {
    let pluck = (BAR_SECONDS * SAMPLE_RATE as f32 / 4.0) as usize;
    let total = chord_arps.len() * BAR_SAMPLES;
    let mut out = vec![0.0; total];
    for (bar, notes) in chord_arps.iter().enumerate() {
        for (step, freq) in notes.iter().enumerate() {
            let mut osc = sine_hz(*freq);
            let start = bar * BAR_SAMPLES + step * pluck;
            for i in 0..pluck {
                if start + i >= total {
                    break;
                }
                let t = i as f32 / SAMPLE_RATE;
                let decay = (-t * 6.0).exp();
                let (l, r) = osc.get_stereo();
                let mono = (l + r) * 0.5;
                let side = if step % 2 == 0 { 0.8 } else { 0.45 };
                out[start + i] += mono * ARP_GAIN * decay * side;
            }
        }
    }
    out
}

/// Soft-knee tanh normalization to MASTER_PEAK.
fn master(out: &mut [f32]) {
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak <= f32::EPSILON {
        return;
    }
    for sample in out.iter_mut() {
        *sample = (*sample / peak * MASTER_PEAK).tanh() * MASTER_PEAK;
    }
}

fn write_wav(path: &str, left: &[f32], right: &[f32]) -> hound::Result<()> {
    let spec = WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE as u32,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)?;
    for (l, r) in left.iter().zip(right) {
        writer.write_sample((l * i16::MAX as f32) as i16)?;
        writer.write_sample((r * i16::MAX as f32) as i16)?;
    }
    writer.finalize()
}

/// Am9 - Fmaj7 - Cmaj7 - G6, voiced below A4 (negative = below A4).
fn progression() -> (Vec<Vec<Voice>>, Vec<f32>, Vec<Vec<f32>>) {
    let pad = [
        [12, 15, 19, 23, 26],
        [8, 12, 16, 19, 24],
        [3, 7, 12, 16, 19],
        [10, 14, 17, 19, 24],
    ];
    let arps = [
        [12, 19, 23, 26, 31, 26, 23, 19],
        [8, 16, 19, 24, 28, 24, 19, 16],
        [3, 16, 19, 22, 27, 22, 19, 16],
        [10, 17, 19, 26, 29, 26, 19, 17],
    ];
    let pad_voices: Vec<Vec<Voice>> = pad
        .iter()
        .map(|bar| {
            bar.iter()
                .enumerate()
                .map(|(i, &s)| Voice {
                    freq: note(s),
                    gain: PAD_GAIN * if i == 0 { 1.0 } else { 0.8 },
                    detune: 0.0012,
                })
                .collect()
        })
        .collect();
    let roots: Vec<f32> = pad.iter().map(|bar| note(bar[0])).collect();
    let arp_notes: Vec<Vec<f32>> = arps
        .iter()
        .map(|bar| bar.iter().map(|&s| note(s)).collect())
        .collect();
    (pad_voices, roots, arp_notes)
}

fn main() {
    let (pad_voices, roots, arps) = progression();
    let pad = render_pad(&pad_voices);
    let bass = render_bass(&roots);
    let arp = render_arp(&arps);

    let mut left = vec![0.0; pad.len()];
    let mut right = vec![0.0; pad.len()];
    for i in 0..pad.len() {
        let wobble = 1.0 + 0.03 * (i as f32 / SAMPLE_RATE * 2.0 * PI).sin();
        left[i] = (pad[i] + bass[i] + arp[i] * 1.1) * wobble;
        right[i] = (pad[i] + bass[i] + arp[i] * 0.9) / wobble;
    }
    master(&mut left);
    master(&mut right);

    write_wav("music.wav", &left, &right).expect("write music.wav");
    println!("wrote music.wav ({} s)", left.len() as f32 / SAMPLE_RATE);
}
