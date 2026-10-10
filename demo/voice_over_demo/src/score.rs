use crate::config::NoteSpec;
use crate::error::ParseError;

pub const NOTE_LETTERS: [char; 7] = ['c', 'd', 'e', 'f', 'g', 'a', 'b'];
const LETTER_SEMITONES: [i8; 7] = [0, 2, 4, 5, 7, 9, 11];
const SHARP: char = '#';
const FLAT: char = 'b';
const MIDI_C4: i16 = 60;
const REFERENCE_OCTAVE: i16 = 4;

pub fn parse_note(spec: &str) -> Result<NoteSpec, ParseError> {
    let (name, beats) = spec
        .split_once(':')
        .ok_or_else(|| ParseError::MissingDuration(spec.to_owned()))?;
    let midi = parse_pitch(name)?;
    let beats: f32 = beats
        .parse()
        .map_err(|_| ParseError::InvalidBeats(spec.to_owned()))?;
    Ok(NoteSpec { midi, beats })
}

pub fn parse_notes(specs: &[String]) -> Result<Vec<NoteSpec>, ParseError> {
    specs.iter().map(|s| parse_note(s)).collect()
}

pub fn parse_pitch(name: &str) -> Result<u8, ParseError> {
    let lowered = name.to_lowercase();
    let mut chars = lowered.chars();
    let letter = chars
        .next()
        .ok_or_else(|| ParseError::InvalidNoteName(name.to_owned()))?;
    let letter_idx = NOTE_LETTERS
        .iter()
        .position(|&l| l == letter)
        .ok_or_else(|| ParseError::InvalidNoteName(name.to_owned()))?;
    let mut semitone = i16::from(LETTER_SEMITONES[letter_idx]);
    let mut accidental = 0;
    for c in chars {
        match c {
            SHARP => accidental += 1,
            FLAT => accidental -= 1,
            digit => {
                let octave = digit
                    .to_digit(10)
                    .ok_or_else(|| ParseError::InvalidNoteName(name.to_owned()))?
                    as i16;
                semitone += accidental;
                let midi = MIDI_C4 + (octave - REFERENCE_OCTAVE) * 12 + semitone;
                return u8::try_from(midi)
                    .map_err(|_| ParseError::NoteOutOfRange(name.to_owned(), midi));
            }
        }
    }
    Err(ParseError::MissingOctave(name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c4_is_midi_60() {
        assert_eq!(parse_pitch("C4"), Ok(60));
    }

    #[test]
    fn a4_is_midi_69() {
        assert_eq!(parse_pitch("A4"), Ok(69));
    }

    #[test]
    fn accidentals_shift_semitone() {
        assert_eq!(parse_pitch("C#4"), Ok(61));
        assert_eq!(parse_pitch("Bb3"), Ok(58));
    }

    #[test]
    fn octave_span_is_twelve() {
        assert_eq!(parse_pitch("C5"), Ok(72));
        assert_eq!(parse_pitch("c3"), Ok(48));
    }

    #[test]
    fn rejects_bad_names() {
        assert!(matches!(
            parse_pitch("H4"),
            Err(ParseError::InvalidNoteName(_))
        ));
        assert!(matches!(
            parse_pitch("C"),
            Err(ParseError::MissingOctave(_))
        ));
        assert!(matches!(
            parse_pitch("C#"),
            Err(ParseError::MissingOctave(_))
        ));
    }

    #[test]
    fn note_spec_parses_with_duration() {
        let n = parse_note("E4:0.5").expect("valid spec");
        assert_eq!(n.midi, 64);
        assert!((n.beats - 0.5).abs() < 1e-6);
    }

    #[test]
    fn note_spec_requires_colon() {
        assert!(matches!(
            parse_note("E4"),
            Err(ParseError::MissingDuration(_))
        ));
    }

    #[test]
    fn note_list_parses() {
        let specs: Vec<String> = ["C4:1", "E4:2"].iter().map(|s| s.to_string()).collect();
        let notes = parse_notes(&specs).expect("valid list");
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[1].midi, 64);
    }
}
