//! Generates an original anime mascot and writes `output.png`.
//!
//! Run: `cargo run --example generate`
//! Tweak `CONFIG` below (or add presets in `src/config.rs`) to change the look.

use image_demo::background::BackgroundKind;
use image_demo::character::{draw_character, Accessory, HairStyle};
use image_demo::config::{
    preset_earthtone, preset_mint, preset_night, preset_ocean, preset_sakura, CharacterConfig,
};
use image_demo::expression::Expression;
use image_demo::pose::Pose;
use image_demo::render::{RasterCanvas, DEFAULT_PNG_SIZE};

const OUTPUT_PATH: &str = "output.png";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CharacterConfig {
        palette: preset_earthtone(),
        expression: Expression::Happy,
        pose: Pose::Stand,
        background: BackgroundKind::Gradient,
        hair: HairStyle::Bob,
        accessory: Accessory::StarPin,
    };
    let mut canvas = RasterCanvas::new(DEFAULT_PNG_SIZE).ok_or("failed to create raster canvas")?;
    draw_character(
        &mut canvas,
        &config.palette,
        config.expression,
        config.pose,
        config.background,
        config.hair,
        config.accessory,
    );
    canvas.save_png(std::path::Path::new(OUTPUT_PATH))?;
    println!(
        "wrote {OUTPUT_PATH} ({}x{})",
        DEFAULT_PNG_SIZE, DEFAULT_PNG_SIZE
    );

    // Unused presets kept here as agent-editable alternatives.
    let _ = (
        preset_sakura(),
        preset_ocean(),
        preset_mint(),
        preset_night(),
    );
    Ok(())
}
