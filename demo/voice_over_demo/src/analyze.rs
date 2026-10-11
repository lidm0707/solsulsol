use std::path::Path;

use crate::error::WavError;

const FFT_SIZE: usize = 8_192;
const F0_MIN_HZ: f32 = 150.0;
const F0_MAX_HZ: f32 = 500.0;
const FULL_SCALE: f32 = 0x7FFF as f32;
const WAV_HEADER_BYTES: usize = 44;

pub struct AudioStats {
    pub centroid_hz: f32,
    pub dominant_hz: f32,
    pub f0_hz: f32,
    pub peak: f32,
    pub rms_dbfs: f32,
}

const AUDIBLE_MIN_PEAK: f32 = 0.001;
const AUDIBLE_MIN_RMS_DBFS: f32 = -60.0;
const DBFS_REF: f32 = 20.0;

impl AudioStats {
    pub fn is_audible(&self) -> bool {
        self.peak >= AUDIBLE_MIN_PEAK && self.rms_dbfs >= AUDIBLE_MIN_RMS_DBFS
    }
}

impl std::fmt::Display for AudioStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "centroid={:.0}Hz dominant={:.0}Hz f0={:.1}Hz peak={:.2} rms={:.1}dBFS audible={}",
            self.centroid_hz,
            self.dominant_hz,
            self.f0_hz,
            self.peak,
            self.rms_dbfs,
            if self.is_audible() { "yes" } else { "NO" }
        )
    }
}

pub fn stats(samples: &[f32], sample_rate: u32) -> AudioStats {
    let window = hann(FFT_SIZE);
    let hop = FFT_SIZE / 2;
    let mut acc = vec![0.0_f32; FFT_SIZE / 2];
    let mut frames = 0;
    let mut start = 0;
    while start + FFT_SIZE <= samples.len() {
        let frame: Vec<f32> = samples[start..start + FFT_SIZE]
            .iter()
            .zip(&window)
            .map(|(&s, &w)| s * w)
            .collect();
        for (a, m) in acc.iter_mut().zip(magnitude(&frame)) {
            *a += m;
        }
        frames += 1;
        start += hop;
    }
    if frames == 0 {
        let frame: Vec<f32> = samples
            .iter()
            .chain(std::iter::repeat_n(
                &0.0,
                FFT_SIZE - samples.len().min(FFT_SIZE),
            ))
            .take(FFT_SIZE)
            .zip(&window)
            .map(|(&s, &w)| s * w)
            .collect();
        for (a, m) in acc.iter_mut().zip(magnitude(&frame)) {
            *a += m;
        }
        frames = 1;
    }
    for a in &mut acc {
        *a /= frames as f32;
    }
    let freqs: Vec<f32> = (0..acc.len())
        .map(|k| k as f32 * sample_rate as f32 / FFT_SIZE as f32)
        .collect();
    let total: f32 = acc.iter().sum();
    let centroid = acc.iter().zip(&freqs).map(|(&m, &f)| m * f).sum::<f32>() / total.max(1e-12);
    let (dominant_idx, _) =
        acc.iter().enumerate().fold(
            (0, 0.0_f32),
            |(bi, bm), (i, &m)| {
                if m > bm {
                    (i, m)
                } else {
                    (bi, bm)
                }
            },
        );
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32;
    let rms_dbfs = DBFS_REF * mean_square.max(1e-12).log10();
    AudioStats {
        centroid_hz: centroid,
        dominant_hz: freqs[dominant_idx],
        f0_hz: autocorr_f0(samples, sample_rate),
        peak: samples.iter().fold(0.0_f32, |m, &s| m.max(s.abs())),
        rms_dbfs,
    }
}

pub fn read_wav(path: &Path) -> Result<(Vec<f32>, u32), WavError> {
    let bytes =
        std::fs::read(path).map_err(|source| WavError::OpenFailed(path.to_path_buf(), source))?;
    if bytes.len() < WAV_HEADER_BYTES || &bytes[0..4] != b"RIFF" {
        return Err(WavError::OpenFailed(
            path.to_path_buf(),
            std::io::Error::new(std::io::ErrorKind::InvalidData, "not a PCM WAV file"),
        ));
    }
    let sample_rate = u32::from_le_bytes(bytes[24..28].try_into().expect("le"));
    let data = &bytes[WAV_HEADER_BYTES..];
    let samples: Vec<f32> = data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / FULL_SCALE)
        .collect();
    Ok((samples, sample_rate))
}

fn hann(size: usize) -> Vec<f32> {
    (0..size)
        .map(|n| {
            let t = std::f32::consts::TAU * n as f32 / size as f32;
            0.5 - 0.5 * t.cos()
        })
        .collect()
}

fn magnitude(frame: &[f32]) -> Vec<f32> {
    let n = frame.len();
    let mut re: Vec<f32> = frame.to_vec();
    let mut im = vec![0.0_f32; n];
    bit_reverse(&mut re, &mut im);
    let mut len = 2;
    while len <= n {
        let angle = -std::f32::consts::TAU / len as f32;
        let (w_re, w_im) = (angle.cos(), angle.sin());
        for chunk in (0..n).step_by(len) {
            let mut cur_re = 1.0_f32;
            let mut cur_im = 0.0_f32;
            for j in 0..len / 2 {
                let a = chunk + j;
                let b = a + len / 2;
                let t_re = cur_re * re[b] - cur_im * im[b];
                let t_im = cur_re * im[b] + cur_im * re[b];
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                let next_re = cur_re * w_re - cur_im * w_im;
                cur_im = cur_re * w_im + cur_im * w_re;
                cur_re = next_re;
            }
        }
        len *= 2;
    }
    (0..n / 2)
        .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt())
        .collect()
}

fn bit_reverse(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 0..n - 1 {
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
        let mut m = n / 2;
        while m >= 1 && j & m != 0 {
            j ^= m;
            m /= 2;
        }
        j |= m;
    }
}

fn autocorr_f0(samples: &[f32], sample_rate: u32) -> f32 {
    let len = (sample_rate as usize / 4).min(samples.len());
    let seg = &samples[samples.len().saturating_sub(len)..];
    let mean = seg.iter().sum::<f32>() / seg.len() as f32;
    let seg: Vec<f32> = seg.iter().map(|&s| s - mean).collect();
    let lag_lo = (sample_rate as f32 / F0_MAX_HZ) as usize;
    let lag_hi = ((sample_rate as f32 / F0_MIN_HZ) as usize).min(len - 1);
    let energy: f32 = seg.iter().map(|s| s * s).sum();
    let mut best_lag = 0;
    let mut best_score = 0.0_f32;
    for lag in lag_lo..=lag_hi {
        let score: f32 = (0..len - lag).map(|i| seg[i] * seg[i + lag]).sum::<f32>() / energy;
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    if best_lag == 0 {
        0.0
    } else {
        sample_rate as f32 / best_lag as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_peaks_at_expected_bin() {
        let n = FFT_SIZE;
        let tone: Vec<f32> = (0..n)
            .map(|i| (std::f32::consts::TAU * 100.0 * i as f32 / 44_100.0).sin())
            .collect();
        let mag = magnitude(&tone);
        let peak_bin = mag
            .iter()
            .enumerate()
            .fold(
                (0, 0.0_f32),
                |(bi, bm), (i, &m)| {
                    if m > bm {
                        (i, m)
                    } else {
                        (bi, bm)
                    }
                },
            )
            .0;
        let peak_hz = peak_bin as f32 * 44_100.0 / n as f32;
        assert!((peak_hz - 100.0).abs() < 44_100.0 / n as f32);
    }

    #[test]
    fn f0_detected_for_sine() {
        let sr = 44_100_u32;
        let tone: Vec<f32> = (0..sr as usize)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / sr as f32).sin())
            .collect();
        let f0 = autocorr_f0(&tone, sr);
        assert!((f0 - 220.0).abs() < 5.0, "f0 {f0}");
    }

    #[test]
    fn centroid_rises_with_brighter_tone() {
        let sr = 44_100_u32;
        let sine = |hz: f32| {
            (0..FFT_SIZE)
                .map(|i| (std::f32::consts::TAU * hz * i as f32 / sr as f32).sin())
                .collect::<Vec<f32>>()
        };
        let low = stats(&sine(300.0), sr).centroid_hz;
        let high = stats(&sine(3000.0), sr).centroid_hz;
        assert!(high > low);
    }

    #[test]
    fn audible_sine_passes_silence_fails() {
        let sr = 44_100_u32;
        let tone: Vec<f32> = (0..sr as usize)
            .map(|i| 0.5 * (std::f32::consts::TAU * 220.0 * i as f32 / sr as f32).sin())
            .collect();
        assert!(stats(&tone, sr).is_audible());
        assert!(!stats(&[0.0; 44_100], sr).is_audible());
    }

    #[test]
    fn read_wav_round_trip() {
        let path = std::env::temp_dir().join("voice_over_demo_read.wav");
        let src: Vec<f32> = (0..1_000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
        crate::wav::write_wav(&path, 44_100, &src).expect("write");
        let (samples, rate) = read_wav(&path).expect("read");
        assert_eq!(rate, 44_100);
        assert_eq!(samples.len(), 1_000);
        std::fs::remove_file(&path).expect("cleanup");
    }
}
