# image-demo — procedural anime mascot generator

Original anime-style character illustrations generated entirely from code:
geometry (Bézier paths) → tiny-skia raster (PNG) and hand-built SVG (validated
with usvg, rasterized with resvg). No embedded images anywhere — every pixel
comes from generated paths.

## Build & run

```
cargo run --example generate                 # writes output.png (1024x1024)
cargo run --example generate_svg --features svg   # writes output.svg + rasterized output.svg.png
cargo test  --all-features                   # unit + integration tests
cargo clippy --all-features --all-targets
```

## Modules

| Module        | Responsibility                                                        |
|---------------|-----------------------------------------------------------------------|
| `palette`     | `Rgba` (hex parsing), 14-slot `Palette`, color math (darken/lighten)  |
| `geometry`    | `PathBuilder` (move/line/quad/cubic/close), `ellipse_path`, `Paint`, `Stroke`, `Canvas` trait |
| `render`      | `RasterCanvas` (tiny-skia → PNG), `SvgCanvas` (SVG markup), `rasterize_svg` (resvg, feature `svg`) |
| `expression`  | Eyes / brows / nose / mouth / blush variants: `Neutral, Happy, Wink, Surprised` |
| `pose`        | Neck, torso, collar, arms: `Stand, Wave, HandsOnHips`                 |
| `background`  | `None` (transparent), `Gradient`, `Rays`, `Night`                     |
| `character`   | Layer orchestration + hair styles (`Bob, TwinTails, Spiky`) + accessories (`None, StarPin, Glasses, Ribbon`) |
| `config`      | `CharacterConfig` + palettes: `preset_sakura/ocean/mint/night`        |

Draw order (layers): background → hair back → ears → neck → head → body/arms →
face (brows, eyes, nose, mouth, blush) → hair front → accessory.

## Changing the character (for agents)

Edit `examples/generate.rs`:

- swap `PRESET_PALETTE` to `preset_ocean` / `preset_mint` / `preset_night`
  (or add a new preset in `src/config.rs` — 14 hex slots),
- change `expression`, `pose`, `background`, `hair`, `accessory` — all enums
  defined in the modules above with `name()` methods and `pub const ...ALL`-style
  arrays (`EXPRESSIONS`, `POSES`, `BACKGROUNDS`, `HAIR_STYLES`, `ACCESSORIES`)
  for exhaustive iteration.

Every combination of expression × pose × background × hair × accessory renders
(tests enforce this). `BackgroundKind::None` produces a transparent PNG,
useful as a mascot overlay for video/social media.

## Adding new content

- New expression: add a variant to `Expression`, cover it in `draw_eyes` /
  `draw_mouth` / `draw_eyebrows` matches, add it to `EXPRESSIONS`.
- New hair style: add a variant + bangs path fn in `character.rs`, match it in
  `draw_hair_back` / `draw_hair_front`, add to `HAIR_STYLES`.
- New palette slot: extend `Palette`, update `from_hexes`, presets, and the
  drawing fns that should use it.

## Extending to video / effects

- Transparent mascot: `background: BackgroundKind::None` → PNG with alpha.
- Different resolutions: `RasterCanvas::new(size)` (any square size; drawing
  space is a fixed 512×512 coordinate system, scaled automatically).
- SVG output scales losslessly for social media posts.
