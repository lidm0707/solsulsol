use crate::error::G2pError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phoneme {
    Vowel(Vowel),
    Fricative { center_hz: f32, voiced: bool },
    Nasal,
    Liquid,
    Stop { voiced: bool, burst_hz: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vowel {
    pub f1: f32,
    pub f2: f32,
    pub f3: f32,
}

pub const VOWEL_A: Vowel = Vowel {
    f1: 730.0,
    f2: 1090.0,
    f3: 2440.0,
};
pub const VOWEL_E: Vowel = Vowel {
    f1: 530.0,
    f2: 1840.0,
    f3: 2480.0,
};
pub const VOWEL_I: Vowel = Vowel {
    f1: 270.0,
    f2: 2290.0,
    f3: 3010.0,
};
pub const VOWEL_O: Vowel = Vowel {
    f1: 570.0,
    f2: 840.0,
    f3: 2410.0,
};
pub const VOWEL_U: Vowel = Vowel {
    f1: 300.0,
    f2: 870.0,
    f3: 2240.0,
};
pub const VOWEL_SCHWA: Vowel = Vowel {
    f1: 500.0,
    f2: 1500.0,
    f3: 2500.0,
};

const BURST_T: f32 = 4_000.0;

pub const FRIC_S: f32 = 5_500.0;
pub const FRIC_F: f32 = 5_000.0;
pub const FRIC_TH: f32 = 6_000.0;
pub const FRIC_SH: f32 = 2_500.0;
pub const FRIC_H: f32 = 1_500.0;

const BURST_P: f32 = 800.0;
const BURST_K: f32 = 2_000.0;

fn fricative(center_hz: f32, voiced: bool) -> Phoneme {
    Phoneme::Fricative { center_hz, voiced }
}

fn stop(voiced: bool, burst_hz: f32) -> Phoneme {
    Phoneme::Stop { voiced, burst_hz }
}

fn single_letter(c: char) -> Option<Phoneme> {
    Some(match c {
        'a' => Phoneme::Vowel(VOWEL_A),
        'e' | 'y' => Phoneme::Vowel(VOWEL_E),
        'i' => Phoneme::Vowel(VOWEL_I),
        'o' => Phoneme::Vowel(VOWEL_O),
        'u' | 'w' => Phoneme::Vowel(VOWEL_U),
        'h' => fricative(FRIC_H, false),
        's' => fricative(FRIC_S, false),
        'z' => fricative(FRIC_S, true),
        'f' => fricative(FRIC_F, false),
        'v' => fricative(FRIC_F, true),
        'j' => fricative(FRIC_SH, true),
        'm' | 'n' => Phoneme::Nasal,
        'l' | 'r' => Phoneme::Liquid,
        't' => stop(false, BURST_T),
        'p' => stop(false, BURST_P),
        'k' | 'q' => stop(false, BURST_K),
        'd' => stop(true, BURST_T),
        'b' => stop(true, BURST_P),
        'g' => stop(true, BURST_K),
        'c' | 'x' => stop(false, BURST_K),
        _ => return None,
    })
}

fn digraph(pair: [char; 2]) -> Option<Phoneme> {
    Some(match pair {
        ['s', 'h'] => fricative(FRIC_SH, false),
        ['c', 'h'] => stop(false, BURST_T),
        ['t', 'h'] => fricative(FRIC_TH, false),
        ['p', 'h'] => fricative(FRIC_F, false),
        ['w', 'h'] => Phoneme::Vowel(VOWEL_U),
        ['c', 'k'] | ['k', 'n'] => stop(false, BURST_K),
        ['n', 'g'] => Phoneme::Nasal,
        ['o', 'o'] | ['o', 'u'] | ['o', 'w'] => Phoneme::Vowel(VOWEL_O),
        ['e', 'e'] | ['e', 'a'] | ['i', 'e'] => Phoneme::Vowel(VOWEL_I),
        ['a', 'i'] | ['a', 'y'] | ['e', 'y'] | ['a', 'e'] => Phoneme::Vowel(VOWEL_A),
        ['o', 'i'] | ['o', 'y'] => Phoneme::Vowel(VOWEL_O),
        ['q', 'u'] => stop(false, BURST_K),
        _ => return None,
    })
}

pub fn g2p(word: &str) -> Vec<Phoneme> {
    let lowered: Vec<char> = word
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    let trimmed = silent_final_e(&lowered);
    let mut out = Vec::with_capacity(trimmed.len());
    let mut i = 0;
    while i < trimmed.len() {
        if i + 1 < trimmed.len()
            && let Some(p) = digraph([trimmed[i], trimmed[i + 1]])
        {
            out.push(p);
            i += 2;
            continue;
        }
        if let Some(p) = single_letter(trimmed[i]) {
            out.push(p);
        }
        i += 1;
    }
    collapse_repeats(out)
}

fn silent_final_e(chars: &[char]) -> &[char] {
    if chars.len() > 2 && chars.last() == Some(&'e') {
        &chars[..chars.len() - 1]
    } else {
        chars
    }
}

fn collapse_repeats(phonemes: Vec<Phoneme>) -> Vec<Phoneme> {
    let mut out: Vec<Phoneme> = Vec::with_capacity(phonemes.len());
    for p in phonemes {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    if out.is_empty() {
        out.push(Phoneme::Vowel(VOWEL_SCHWA));
    }
    out
}

pub fn validate_word(word: &str) -> Result<(), G2pError> {
    if word.trim().is_empty() {
        return Err(G2pError::EmptyWord);
    }
    if !word.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(G2pError::NoLetters(word.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_maps_to_phonemes() {
        let p = g2p("Hello");
        assert_eq!(
            p.first(),
            Some(&Phoneme::Fricative {
                center_hz: FRIC_H,
                voiced: false
            })
        );
        assert!(p.contains(&Phoneme::Vowel(VOWEL_E)));
        assert!(p.contains(&Phoneme::Vowel(VOWEL_O)));
        assert!(p.len() >= 3);
    }

    #[test]
    fn silent_final_e_dropped() {
        assert!(!g2p("like").contains(&Phoneme::Vowel(VOWEL_E)));
    }

    #[test]
    fn digraphs_take_priority() {
        assert_eq!(
            g2p("she")[0],
            Phoneme::Fricative {
                center_hz: FRIC_SH,
                voiced: false
            }
        );
        assert_eq!(g2p("too")[1], Phoneme::Vowel(VOWEL_O));
    }

    #[test]
    fn unknown_letters_skipped_nonempty_result() {
        assert_eq!(g2p("!!!"), vec![Phoneme::Vowel(VOWEL_SCHWA)]);
    }

    #[test]
    fn phoneme_lists_are_never_empty() {
        for word in ["a", "world", "sing", "zzz"] {
            assert!(!g2p(word).is_empty());
        }
    }
}
