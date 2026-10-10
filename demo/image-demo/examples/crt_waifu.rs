//! Waifu inside a retro CRT terminal (Cool Retro Term style).
//! Run: `cargo run --example crt_waifu` — writes `crt_waifu.png`.

use image_demo::character::{draw_character, Accessory, HairStyle};
use image_demo::config::preset_earthtone;
use image_demo::expression::Expression;
use image_demo::pose::Pose;
use image_demo::render::{RasterCanvas, DEFAULT_PNG_SIZE};
use image_demo::scene::{draw_crt_backdrop, draw_crt_overlay, phosphorize};

const OUTPUT_PATH: &str = "crt_waifu.png";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let palette = phosphorize(&preset_earthtone());
    let mut canvas = RasterCanvas::new(DEFAULT_PNG_SIZE).ok_or("failed to create raster canvas")?;
    draw_crt_backdrop(&mut canvas);
    draw_character(
        &mut canvas,
        &palette,
        Expression::Happy,
        Pose::Stand,
        image_demo::background::BackgroundKind::None,
        HairStyle::Bob,
        Accessory::StarPin,
    );
    draw_crt_overlay(&mut canvas);
    canvas.save_png(std::path::Path::new(OUTPUT_PATH))?;
    println!(
        "wrote {OUTPUT_PATH} ({}x{})",
        DEFAULT_PNG_SIZE, DEFAULT_PNG_SIZE
    );
    Ok(())
}
