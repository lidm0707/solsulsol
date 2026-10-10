# Feasibility Report — Pure-Rust English Singing Synthesis (2026-10)

Question: can we ship offline English Vocaloid-style singing in pure Rust today?

## 1. Crates investigated

### voirs-singing 0.1.0-rc.1 (cool-japan/voirs) — Apache-2.0

Verified by adding it to a probe project and compiling, then reading the
published source in `~/.cargo/registry`:

- **Compiles**: pure Rust, no native libs required to build. Default features
  are dependency-heavy (scirs2-core ecosystem) but work.
- **Default synthesis path is placeholder DSP**, not neural:
  - `synthesis/diffsinger.rs`: `predict_noise` is commented
    "placeholder implementation"; the diffusion loop denoises toward
    synthesized noise with no trained weights.
  - `extract_phoneme_features`: phoneme "embeddings" are character sums:
    `(syllable_hash + dim) / 100.0`.
  - `SynthesisModel::load_from_file` is a no-op placeholder.
  - The working `HarmonicSynthesisModel` is a 1/n-decay additive saw — an
    oscillator, not a voice model.
- **Neural path** (`onnx` feature): DiffSinger acoustic + optional vocoder
  through `oxionnx` (a brand-new pure-Rust ONNX runtime). This is
  architecturally what we want, but:
  - **No pretrained English singing voice model is shipped or documented**
    with the crate. The README's `"soprano_voice"` voice bank is a config
    string, not a downloadable model.
  - `oxionnx` operator coverage for DiffSinger graphs (attention, conv,
    einsum-heavy) is unverified and could not be verified without a model.
- Download counts are tiny (≈100 recent); rc maturity, API unstable.

### Everything else

- **Piper / ONNX TTS (speech)**: sings nothing — cannot follow MIDI. Explicitly
  rejected by the task ("do not assume a speech TTS model can sing").
- **Sinsy, NEUTRINO, OpenUtau/DiffSinger voicebanks**: real singing models,
  but C++/Python runtimes or ONNX packs whose graphs have not been verified
  through any Rust runtime. No license-clean, Rust-runnable English pack found.
- No other maintained Rust singing-synthesis crate exists on crates.io
  (keyword `singing` has 2 crates; the second is this ecosystem's SDK).

## 2. Conclusion

**No compatible pretrained neural singing model can currently be verified to
run through a Rust-native inference stack.** Per the task's fallback clause we
implement a **modular Rust DSP prototype**: rule-based English G2P → phoneme
alignment to MIDI notes → formant (source-filter) synthesis with vibrato,
portamento, and breath noise.

**This is prototype DSP audio, NOT natural neural singing.** It produces
clearly intelligible vowel/consonant singing-like output suitable as a
scaffolding/mocking track for video timing — it will not sound like Vocaloid.
If a verified DiffSinger ONNX English voicebank + a Rust runtime that runs it
appear, the `Synth` trait boundary in `src/synth.rs` is the swap point.

## 3. Licenses

- This prototype: same license as the repo it lives in.
- No third-party models or voice data are used. Dependencies: clap, thiserror
  (MIT/Apache-2.0).
