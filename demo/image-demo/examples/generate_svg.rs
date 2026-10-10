//! SVG export example. Run with: `cargo run --example generate_svg --features svg`

use image_demo::background::BackgroundKind;
use image_demo::character::{draw_character, Accessory, HairStyle};
use image_demo::config::{preset_night, CharacterConfig};
use image_demo::expression::Expression;
use image_demo::pose::Pose;
use image_demo::render::{rasterize_svg, SvgCanvas, DEFAULT_PNG_SIZE};

const OUTPUT_SVG: &str = "output.svg";
const OUTPUT_PNG: &str = "output.svg.png";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CharacterConfig {
        palette: preset_night(),
        expression: Expression::Neutral,
        pose: Pose::HandsOnHips,
        background: BackgroundKind::Night,
        hair: HairStyle::Spiky,
        accessory: Accessory::Glasses,
    };
    let mut svg = SvgCanvas::new(DEFAULT_PNG_SIZE);
    draw_character(
        &mut svg,
        &config.palette,
        config.expression,
        config.pose,
        config.background,
        config.hair,
        config.accessory,
    );
    let markup = svg.finish();
    std::fs::write(OUTPUT_SVG, &markup)?;

    if let Some(png) = rasterize_svg(&markup, DEFAULT_PNG_SIZE) {
        std::fs::write(OUTPUT_PNG, png)?;
        println!("wrote {OUTPUT_SVG} and {OUTPUT_PNG}");
    } else {
        println!("wrote {OUTPUT_SVG} (rasterization check skipped)");
    }
    Ok(())
}
