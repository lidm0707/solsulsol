# voice_over_demo — English Singing Synthesizer (DSP prototype)

Offline English singing-voice synthesizer in pure Rust: lyrics + MIDI melody +
tempo in, WAV out. Anime-style formant synthesis — **not** neural singing
(see [FEASIBILITY.md](FEASIBILITY.md) for why, and what exists).

```
src/
  main.rs     entry point / exit codes
  cli.rs      clap CLI
  config.rs   RenderConfig + validation + tempo math (named consts)
  score.rs    "C4:1" note-spec parsing → MIDI numbers
  phoneme.rs  rule-based English G2P → vowel formants / consonant specs
  synth.rs    glottal saw source, resonator bank, vibrato, portamento, ADSR
  render.rs   score + lyrics → samples, peak normalization
  wav.rs      16-bit PCM mono WAV writer
  error.rs    typed errors
```

## Usage

```sh
cargo run --release -- \
  --lyrics "hello bright world" \
  --notes "C4:1" "E4:1" "G4:2" \
  --tempo 100 \
  --output output.wav
```

- Each **word maps to one note**, sung over that note's duration.
- Note format `PITCH:BEATS`, e.g. `C4:1`, `F#4:0.5`, `Bb3:2`.
- Notes and words are length-checked against each other.

Options: `--tempo` (30–300 BPM), `--sample-rate` (default 44100),
`--vibrato-rate` (Hz), `--vibrato-depth` (cents), `--vibrato-delay` (fraction
of note), `--portamento-ms`, `--breathiness` (0–1).

## Requirements

- Stable Rust + Cargo. No Python, no cloud, no native libraries, no models —
  fully offline, std + `clap` + `thiserror` only.

## Tests

```sh
cargo test          # MIDI timing, phoneme alignment, G2P, WAV header, DSP bounds
cargo clippy --all-targets
```

## Video + intelligibility verification

Make a lyric video (words on screen, synced to the notes):

```sh
ffmpeg -y -f lavfi -i "color=c=0x14141e:s=1280x720:d=4.6,format=yuv420p" \
  -i sing_test.wav -shortest -c:v libx264 -preset fast -c:a aac -pix_fmt yuv420p sing_test.mp4
```

`sing_video.mp4` in this dir was made that way plus per-word caption frames
(see `.plans/36` for the generator snippet).

Machine check (whisper.cpp, tiny.en + base.en): output is classified as
"*sings*"/music, not transcribed — expected: formant-synth singing is outside
ASR training data, and the control test showed whisper can't even transcribe
pitch-shifted natural speech. Structural checks pass:

- Autocorrelation of the vowel segment: f0 ≈ 220.5 Hz for an A3 note ✓
- Energy envelope of "hello": /h/ 0–0.25s, /e/ 0.25–2.0s, /l/ dip 2.0–2.25s,
  /o/ 2.25–3.95s, release ✓ (see `spec_hello3.png`)
- Spectrogram shows harmonic stack + vibrato + consonant noise bursts ✓

Human check — listen yourself:

```sh
afplay sing_test.wav     # "I can sing this song" on C4-E4-G4-E4-C5
afplay hello_long.wav    # slow "hello"
afplay lalala.wav        # vowel-only scale, easiest to judge pitch/vowels
```

## Honest quality statement

This is a **source-filter formant synthesizer** (sawtooth glottal source into
3 formant resonators, noise-based consonants, ADSR + vibrato + portamento).
Output is intelligible, singing-pitched vowel/consonant audio — useful for
timing/mocking a vocal track in video. It is **not comparable to Vocaloid,
Synthesizer V, or DiffSinger**. Real neural singing in Rust today is blocked
by the absence of verified pretrained English singing models for any
Rust-native inference runtime — details and the trade study are in
[FEASIBILITY.md](FEASIBILITY.md).

Extension point: `Synth::render_note` (per-note samples) and
`phoneme::g2p` (lyrics → phonemes) are the two seams where a real acoustic
model would slot in.
