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
pub enum AppError {
    #[error("missing required argument: {0}")]
    MissingArg(String),

    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    G2p(#[from] G2pError),

    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),

    #[error(transparent)]
    Wav(#[from] WavError),
}
