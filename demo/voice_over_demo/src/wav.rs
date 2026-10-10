use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::error::WavError;

const RIFF: &[u8; 4] = b"RIFF";
const WAVE: &[u8; 4] = b"WAVE";
const FMT_CHUNK: &[u8; 4] = b"fmt ";
const DATA_CHUNK: &[u8; 4] = b"data";
const PCM_FORMAT: u16 = 1;
const CHANNELS: u16 = 1;
const BITS_PER_SAMPLE: u16 = 16;
const HEADER_BYTES: u32 = 44;
const FULL_SCALE: f32 = 0x7FFF as f32;

pub fn write_wav(path: &Path, sample_rate: u32, samples: &[f32]) -> Result<(), WavError> {
    let file =
        File::create(path).map_err(|source| WavError::OpenFailed(path.to_path_buf(), source))?;
    let mut writer = BufWriter::new(file);

    let data_bytes = (samples.len() as u32) * 2;
    write_le(&mut writer, RIFF)?;
    write_le(&mut writer, &(HEADER_BYTES + data_bytes - 8).to_le_bytes())?;
    write_le(&mut writer, WAVE)?;
    write_le(&mut writer, FMT_CHUNK)?;
    write_le(&mut writer, &16_u32.to_le_bytes())?;
    write_le(&mut writer, &PCM_FORMAT.to_le_bytes())?;
    write_le(&mut writer, &CHANNELS.to_le_bytes())?;
    write_le(&mut writer, &sample_rate.to_le_bytes())?;
    let byte_rate = sample_rate * u32::from(CHANNELS) * u32::from(BITS_PER_SAMPLE) / 8;
    write_le(&mut writer, &byte_rate.to_le_bytes())?;
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    write_le(&mut writer, &block_align.to_le_bytes())?;
    write_le(&mut writer, &BITS_PER_SAMPLE.to_le_bytes())?;
    write_le(&mut writer, DATA_CHUNK)?;
    write_le(&mut writer, &data_bytes.to_le_bytes())?;

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let pcm = (clamped * FULL_SCALE) as i16;
        write_le(&mut writer, &pcm.to_le_bytes())?;
    }
    writer
        .flush()
        .map_err(|source| WavError::WriteFailed(path.to_path_buf(), source))
}

fn write_le(writer: &mut impl Write, bytes: &[u8]) -> Result<(), WavError> {
    writer
        .write_all(bytes)
        .map_err(|source| WavError::WriteFailed(std::path::PathBuf::new(), source))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(name)
    }

    #[test]
    fn writes_valid_mono_wav_header() {
        let path = tmp_path("voice_over_demo_test.wav");
        write_wav(&path, 22_050, &[0.0, 0.5, -0.5, 1.0]).expect("write wav");
        let bytes = std::fs::read(&path).expect("read wav");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[36..40], b"data");
        let data_bytes = u32::from_le_bytes(bytes[40..44].try_into().expect("le"));
        assert_eq!(data_bytes, 8);
        let riff_size = u32::from_le_bytes(bytes[4..8].try_into().expect("le"));
        assert_eq!(riff_size, 36 + 8);
        std::fs::remove_file(&path).expect("cleanup");
    }

    #[test]
    fn clamps_out_of_range_samples() {
        let path = tmp_path("voice_over_demo_clamp.wav");
        write_wav(&path, 8_000, &[2.0, -2.0]).expect("write wav");
        let bytes = std::fs::read(&path).expect("read wav");
        let s0 = i16::from_le_bytes(bytes[44..46].try_into().expect("le"));
        let s1 = i16::from_le_bytes(bytes[46..48].try_into().expect("le"));
        assert_eq!(s0, i16::MAX);
        assert_eq!(s1, -i16::MAX);
        std::fs::remove_file(&path).expect("cleanup");
    }
}
