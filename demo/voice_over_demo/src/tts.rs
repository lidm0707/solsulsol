//! Piper TTS: espeak-ng phonemes → phoneme ids → ONNX (VITS) → f32 samples.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::error::TtsError;

const ESPEAK_BIN: &str = "/opt/homebrew/bin/espeak-ng";
const ESPEAK_VOICE: &str = "en-us";
const PHONEME_BOS: char = '^';
const PHONEME_EOS: char = '$';
const PHONEME_PAD: char = '_';
const TOKEN_PAD_ID: i64 = 0;

#[derive(Debug, Deserialize)]
pub struct VoiceConfig {
    pub phoneme_id_map: BTreeMap<String, Vec<i64>>,
    pub inference: InferenceScales,
    pub audio: AudioConfig,
}

#[derive(Debug, Deserialize)]
pub struct InferenceScales {
    pub noise_scale: f32,
    pub length_scale: f32,
    pub noise_w: f32,
}

#[derive(Debug, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
}

/// `^` + each phoneme id padded with `TOKEN_PAD_ID` + `$`, as the model expects.
pub fn phoneme_ids(text: &str, map: &BTreeMap<String, Vec<i64>>) -> Vec<i64> {
    let mut ids = vec![map_id(&PHONEME_BOS.to_string(), map)];
    for ch in text.chars() {
        let key = ch.to_string();
        if let Some(found) = map.get(&key) {
            ids.extend_from_slice(found);
            if ch != PHONEME_PAD {
                ids.push(TOKEN_PAD_ID);
            }
        }
    }
    ids.push(map_id(&PHONEME_EOS.to_string(), map));
    ids
}

fn map_id(key: &str, map: &BTreeMap<String, Vec<i64>>) -> i64 {
    map.get(key)
        .and_then(|v| v.first().copied())
        .unwrap_or(TOKEN_PAD_ID)
}

/// IPA phonemes via the espeak-ng CLI (text-to-ipa front end).
pub fn phonemize(text: &str) -> Result<String, TtsError> {
    let out = Command::new(ESPEAK_BIN)
        .args(["--ipa", "-q", "-v", ESPEAK_VOICE, text])
        .output()
        .map_err(TtsError::EspeakSpawn)?;
    if !out.status.success() {
        return Err(TtsError::EspeakFailed(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    let ipa = String::from_utf8_lossy(&out.stdout);
    // espeak splits clauses onto separate lines and drops punctuation; both
    // must survive — the model needs them for sentence pauses and intonation.
    let ipa = ipa.lines().collect::<Vec<_>>().join(" ");
    Ok(restore_punctuation(&ipa, text))
}

const SENTENCE_END: [char; 3] = ['.', '!', '?'];
const CLAUSE_BREAK: [char; 3] = [',', ';', ':'];

/// Re-attaches sentence ends as terminal pauses; clause commas become short
/// breaks (espeak's pad phoneme, id 0, reads as a tiny gap).
fn restore_punctuation(ipa: &str, text: &str) -> String {
    let sentences = count_punct(text, &SENTENCE_END);
    let clauses = count_punct(text, &CLAUSE_BREAK);
    let pad = PHONEME_PAD.to_string();
    let mut out = ipa.to_owned();
    for _ in 0..clauses {
        if let Some(pos) = out.rfind(" ") {
            out.replace_range(pos..pos + 1, &pad);
        }
    }
    for _ in 0..sentences {
        out.push_str(&pad.repeat(8));
    }
    out
}

fn count_punct(text: &str, marks: &[char]) -> usize {
    text.chars().filter(|c| marks.contains(c)).count()
}

/// Runs the VITS model and returns interleaved mono f32 samples.
pub fn synthesize(
    model_path: &Path,
    cfg: &VoiceConfig,
    text: &str,
) -> Result<(Vec<f32>, u32), TtsError> {
    let ipa = phonemize(text)?;
    let ids = phoneme_ids(&ipa, &cfg.phoneme_id_map);
    let len = ids.len() as i64;

    let mut session = ort::session::Session::builder()
        .map_err(tts_err)?
        .commit_from_file(model_path)
        .map_err(tts_err)?;
    let input = ort::value::Tensor::from_array(([1usize, ids.len()], ids)).map_err(tts_err)?;
    let lengths = ort::value::Value::from_array(([1usize], vec![len])).map_err(tts_err)?;
    let scales = ort::value::Tensor::from_array((
        [3usize],
        vec![
            cfg.inference.noise_scale,
            cfg.inference.length_scale,
            cfg.inference.noise_w,
        ],
    ))
    .map_err(tts_err)?;

    let outputs = session
        .run(ort::inputs![
            "input" => input,
            "input_lengths" => lengths,
            "scales" => scales,
        ])
        .map_err(tts_err)?;
    let (_, audio) = outputs["output"]
        .try_extract_tensor::<f32>()
        .map_err(tts_err)?;
    Ok((audio.to_vec(), cfg.audio.sample_rate))
}

fn tts_err<E: std::fmt::Display>(err: E) -> TtsError {
    TtsError::Inference(err.to_string())
}

pub fn parse_config(path: &Path) -> Result<VoiceConfig, TtsError> {
    let raw =
        std::fs::read_to_string(path).map_err(|e| TtsError::ConfigOpen(path.to_path_buf(), e))?;
    serde_json::from_str(&raw).map_err(TtsError::ConfigParse)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_map() -> BTreeMap<String, Vec<i64>> {
        BTreeMap::from([
            ("_".to_owned(), vec![0]),
            ("^".to_owned(), vec![1]),
            ("$".to_owned(), vec![2]),
            (" ".to_owned(), vec![3]),
            ("a".to_owned(), vec![10]),
            ("b".to_owned(), vec![11]),
        ])
    }

    #[test]
    fn wraps_bos_eos_and_pads_each_phoneme() {
        let ids = phoneme_ids("aba", &test_map());
        assert_eq!(ids, vec![1, 10, 0, 11, 0, 10, 0, 2]);
    }

    #[test]
    fn pads_space_phoneme_once() {
        let ids = phoneme_ids("a a", &test_map());
        assert_eq!(ids, vec![1, 10, 0, 3, 0, 10, 0, 2]);
    }

    #[test]
    fn unknown_chars_are_dropped() {
        let ids = phoneme_ids("a?", &test_map());
        assert_eq!(ids, vec![1, 10, 0, 2]);
    }

    #[test]
    fn parses_real_config_shape() {
        let raw = r#"{
            "phoneme_id_map": {"_": [0], "^": [1]},
            "inference": {"noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.8},
            "audio": {"sample_rate": 22050}
        }"#;
        let cfg: VoiceConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.audio.sample_rate, 22050);
        assert_eq!(cfg.inference.noise_scale, 0.667);
    }
}
