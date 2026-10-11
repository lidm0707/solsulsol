use std::path::PathBuf;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ParseError {
    #[error("note spec missing ':beats' (e.g. C4:1): {0}")]
    MissingDuration(String),

    #[error("invalid note name: {0}")]
    InvalidNoteName(String),

    #[error("note name missing octave (e.g. C4): {0}")]
    MissingOctave(String),

    #[error("note {0} out of MIDI range (midi {1})")]
    NoteOutOfRange(String, i16),

    #[error("invalid beats value: {0}")]
    InvalidBeats(String),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum G2pError {
    #[error("word is empty")]
    EmptyWord,

    #[error("word contains no letters: {0}")]
    NoLetters(String),
}

#[derive(Debug, thiserror::Error)]
pub enum WavError {
    #[error("cannot create output file {0}: {1}")]
    OpenFailed(PathBuf, std::io::Error),

    #[error("failed writing wav data: {0}")]
    WriteFailed(PathBuf, std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum HfError {
    #[error("model spec must be owner/repo/path/file.onnx: {0}")]
    BadSpec(String),

    #[error("HOME is not set; cannot locate model cache")]
    NoHome,

    #[error("download failed for {0}: {1}")]
    Download(String, std::io::Error),

    #[error("http status {1} for {0}")]
    HttpStatus(String, u16),

    #[error("cached file too small (truncated download): {0}")]
    TooSmall(PathBuf),

    #[error("cannot create cache dir/file {}: {}", .0.display(), .1)]
    CacheCreate(PathBuf, std::io::Error),

    #[error("http request failed: {0}")]
    Request(String),
}

#[derive(Debug, thiserror::Error)]
pub enum TtsError {
    #[error("cannot spawn {0:?}")]
    EspeakSpawn(std::io::Error),

    #[error("espeak-ng failed: {0}")]
    EspeakFailed(String),

    #[error("cannot open voice config {}: {}", .0.display(), .1)]
    ConfigOpen(PathBuf, std::io::Error),

    #[error("invalid voice config json: {0}")]
    ConfigParse(#[source] serde_json::Error),

    #[error("inference failed: {0}")]
    Inference(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("missing required argument: {0}")]
    MissingArg(String),

    #[error("unknown voice preset: {0}")]
    UnknownVoice(String),

    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    G2p(#[from] G2pError),

    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    #[error(transparent)]
    Wav(#[from] WavError),

    #[error(transparent)]
    Hf(#[from] HfError),

    #[error(transparent)]
    Tts(#[from] TtsError),
}
