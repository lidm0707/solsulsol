//! 64×64 pixel-art waifu: mono-color line art on an amber CRT,
//! upscaled nearest-neighbor to 1024. Run: `cargo run --example pixel_waifu`.

use image_demo::character::{draw_character, Accessory, HairStyle};
use image_demo::config::preset_earthtone;
use image_demo::expression::Expression;
use image_demo::palette::Rgba;
use image_demo::pose::Pose;
use image_demo::render::{RasterCanvas, DEFAULT_PNG_SIZE};
use image_demo::scene::{
    draw_crt_backdrop, draw_crt_overlay, phosphorize, MonoCanvas, AMBER, AMBER_DIM, SCREEN_BOTTOM,
    SCREEN_TOP, TITLE_BAR_COLOR, WINDOW_COLOR,
};

const PX_SIZE: u32 = 64;
const OUTPUT_PATH: &str = "pixel_waifu.png";

const PIXEL_PALETTE: [Rgba; 7] = [
    Rgba::opaque(0x8a, 0xb0, 0x6a),
    WINDOW_COLOR,
    TITLE_BAR_COLOR,
    SCREEN_TOP,
    SCREEN_BOTTOM,
    AMBER,
    AMBER_DIM,
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let palette = phosphorize(&preset_earthtone());
    let mut small = RasterCanvas::new(PX_SIZE).ok_or("failed to create 64px canvas")?;

    draw_crt_backdrop(&mut small);

    {
        let mut mono = MonoCanvas::new(&mut small, AMBER, SCREEN_BOTTOM);
        draw_character(
            &mut mono,
            &palette,
            Expression::Happy,
            Pose::Stand,
            image_demo::background::BackgroundKind::None,
            HairStyle::Bob,
            Accessory::StarPin,
        );
    }

    draw_crt_overlay(&mut small);

    let quantized = small.quantize(&PIXEL_PALETTE).ok_or("quantize failed")?;
    let out = quantized
        .upscale_nearest(DEFAULT_PNG_SIZE)
        .ok_or("upscale requires a multiple of the source size")?;
    out.save_png(std::path::Path::new(OUTPUT_PATH))?;
    println!("wrote {OUTPUT_PATH} ({PX_SIZE}x{PX_SIZE} -> {DEFAULT_PNG_SIZE}x{DEFAULT_PNG_SIZE})");
    Ok(())
}
