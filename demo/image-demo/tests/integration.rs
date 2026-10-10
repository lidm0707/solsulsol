use image_demo::background::BackgroundKind;
use image_demo::character::{draw_character, Accessory, HairStyle};
use image_demo::config::{preset_night, preset_ocean, CharacterConfig};
use image_demo::expression::Expression;
use image_demo::geometry::{Canvas, DrawCmd, CANVAS_SIZE};
use image_demo::pose::Pose;
use image_demo::render::{RasterCanvas, SvgCanvas, DEFAULT_PNG_SIZE};

struct CountCanvas {
    count: usize,
}

impl Canvas for CountCanvas {
    fn add(&mut self, _cmd: &DrawCmd) {
        self.count += 1;
    }
}

fn full_config() -> CharacterConfig {
    CharacterConfig {
        palette: preset_ocean(),
        expression: Expression::Surprised,
        pose: Pose::HandsOnHips,
        background: BackgroundKind::Night,
        hair: HairStyle::Spiky,
        accessory: Accessory::Glasses,
    }
}

#[test]
fn renders_png_file_from_geometry() {
    let cfg = full_config();
    let mut canvas = RasterCanvas::new(256).expect("canvas");
    draw_character(
        &mut canvas,
        &cfg.palette,
        cfg.expression,
        cfg.pose,
        cfg.background,
        cfg.hair,
        cfg.accessory,
    );
    let dir = std::env::temp_dir().join("image_demo_test_png");
    std::fs::create_dir_all(&dir).expect("tmp dir");
    let path = dir.join("char.png");
    canvas.save_png(&path).expect("save");
    let bytes = std::fs::read(&path).expect("read back");
    assert!(
        bytes.len() > 2_000,
        "png suspiciously small: {}",
        bytes.len()
    );
    assert_eq!(&bytes[1..4], b"PNG");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn every_variant_combination_renders() {
    let palettes = [preset_ocean(), preset_night()];
    for p in palettes {
        for expr in image_demo::expression::EXPRESSIONS {
            for pose in poses_all() {
                for bg in backgrounds_all() {
                    for hair in image_demo::character::HAIR_STYLES {
                        for acc in image_demo::character::ACCESSORIES {
                            let mut cc = CountCanvas { count: 0 };
                            draw_character(&mut cc, &p, expr, pose, bg, hair, acc);
                            assert!(cc.count > 10);
                        }
                    }
                }
            }
        }
    }
}

fn poses_all() -> [Pose; 3] {
    [Pose::Stand, Pose::Wave, Pose::HandsOnHips]
}

fn backgrounds_all() -> [BackgroundKind; 4] {
    [
        BackgroundKind::None,
        BackgroundKind::Gradient,
        BackgroundKind::Rays,
        BackgroundKind::Night,
    ]
}

#[cfg(feature = "svg")]
#[test]
fn svg_output_parses_with_usvg() {
    let cfg = full_config();
    let mut svg = SvgCanvas::new(512);
    draw_character(
        &mut svg,
        &cfg.palette,
        cfg.expression,
        cfg.pose,
        cfg.background,
        cfg.hair,
        cfg.accessory,
    );
    let markup = svg.finish();
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(&markup, &opts).expect("svg must parse");
    assert!(tree.size().width() > 0.0);
}

#[test]
fn canvas_size_constant_is_square_basis() {
    assert!((CANVAS_SIZE - 512.0).abs() < f32::EPSILON);
    assert_eq!(DEFAULT_PNG_SIZE, 1024);
}
