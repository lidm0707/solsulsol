use crate::geometry::{
    circle_path, outline, Canvas, DrawCmd, Paint, PathBuilder, Pt, Stroke, CANVAS_SIZE,
};
use crate::palette::{Palette, Rgba};

pub const CENTER: f32 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundKind {
    None,
    Gradient,
    Rays,
    Night,
    Gears,
}

pub const BACKGROUNDS: [BackgroundKind; 5] = [
    BackgroundKind::None,
    BackgroundKind::Gradient,
    BackgroundKind::Rays,
    BackgroundKind::Night,
    BackgroundKind::Gears,
];

impl BackgroundKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Gradient => "gradient",
            Self::Rays => "rays",
            Self::Night => "night",
            Self::Gears => "gears",
        }
    }
}

const RAY_COUNT: usize = 12;
const STAR_COUNT: usize = 24;

pub fn draw_background(c: &mut dyn Canvas, p: &Palette, kind: BackgroundKind) {
    match kind {
        BackgroundKind::None => {}
        BackgroundKind::Gradient | BackgroundKind::Rays => {
            let panel = PathBuilder::new()
                .move_to(Pt::new(0.0, 0.0))
                .line_to(Pt::new(CANVAS_SIZE, 0.0))
                .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
                .line_to(Pt::new(0.0, CANVAS_SIZE))
                .close()
                .build()
                .expect("panel");
            let cmd = outline(
                &panel,
                Paint::LinearGradient {
                    start: Pt::new(0.0, 0.0),
                    end: Pt::new(0.0, CANVAS_SIZE),
                    from: p.bg_top,
                    to: p.bg_bottom,
                },
                p.bg_bottom,
                0.0,
            );
            c.add(&cmd);
            if kind == BackgroundKind::Rays {
                draw_rays(c, p);
            }
        }
        BackgroundKind::Night => {
            let panel = PathBuilder::new()
                .move_to(Pt::new(0.0, 0.0))
                .line_to(Pt::new(CANVAS_SIZE, 0.0))
                .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
                .line_to(Pt::new(0.0, CANVAS_SIZE))
                .close()
                .build()
                .expect("panel");
            let cmd = outline(
                &panel,
                Paint::LinearGradient {
                    start: Pt::new(0.0, 0.0),
                    end: Pt::new(0.0, CANVAS_SIZE),
                    from: p.bg_top,
                    to: p.bg_bottom,
                },
                p.bg_bottom,
                0.0,
            );
            c.add(&cmd);
            draw_stars(c);
        }
        BackgroundKind::Gears => {
            let panel = PathBuilder::new()
                .move_to(Pt::new(0.0, 0.0))
                .line_to(Pt::new(CANVAS_SIZE, 0.0))
                .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
                .line_to(Pt::new(0.0, CANVAS_SIZE))
                .close()
                .build()
                .expect("panel");
            let cmd = outline(
                &panel,
                Paint::LinearGradient {
                    start: Pt::new(0.0, 0.0),
                    end: Pt::new(0.0, CANVAS_SIZE),
                    from: p.bg_top,
                    to: p.bg_bottom,
                },
                p.bg_bottom,
                0.0,
            );
            c.add(&cmd);
            draw_gears(c, p);
            draw_collage_patches(c, p);
            draw_solar_light(c);
        }
    }
}

fn draw_rays(c: &mut dyn Canvas, p: &Palette) {
    for i in 0..RAY_COUNT {
        let angle = (i as f32) * std::f32::consts::TAU / RAY_COUNT as f32;
        let spread = std::f32::consts::PI / RAY_COUNT as f32 * 0.5;
        let r = CANVAS_SIZE;
        let ray = PathBuilder::new()
            .move_to(Pt::new(CENTER, CENTER))
            .line_to(Pt::new(CENTER + r * angle.cos(), CENTER + r * angle.sin()))
            .line_to(Pt::new(
                CENTER + r * (angle + spread).cos(),
                CENTER + r * (angle + spread).sin(),
            ))
            .close()
            .build()
            .expect("ray");
        let cmd = outline(
            &ray,
            Paint::Solid(p.bg_bottom.with_alpha(60)),
            p.bg_bottom.with_alpha(0),
            0.0,
        );
        c.add(&cmd);
    }
}

fn draw_stars(c: &mut dyn Canvas) {
    for i in 0..STAR_COUNT {
        let t = i as f32 / STAR_COUNT as f32;
        let x = 24.0 + ((i * 97) % 464) as f32;
        let y = 24.0 + ((i * 211) % 464) as f32;
        let r = 2.5 + t * 2.5;
        if i % 3 == 0 {
            if let Some(star) = star_path(x, y, r * 2.4) {
                let cmd = outline(
                    &star,
                    Paint::Solid(Rgba::opaque(255, 245, 200)),
                    Rgba::new(0, 0, 0, 0),
                    0.0,
                );
                c.add(&cmd);
            }
        } else if let Some(dot) = circle_path(x, y, r) {
            let cmd = outline(
                &dot,
                Paint::Solid(Rgba::opaque(255, 250, 230)),
                Rgba::new(0, 0, 0, 0),
                0.0,
            );
            c.add(&cmd);
        }
    }
}

const GEAR_TEETH: usize = 9;
const GEAR_TOOTH_DEPTH: f32 = 0.78;
const GEAR_HUB_RATIO: f32 = 0.34;
const GEARS: [(f32, f32, f32); 5] = [
    (76.0, 84.0, 62.0),
    (430.0, 150.0, 78.0),
    (60.0, 330.0, 48.0),
    (452.0, 396.0, 58.0),
    (210.0, 30.0, 40.0),
];

const SUN_ORIGIN_X: f32 = 110.0;
const SUN_ORIGIN_Y: f32 = 20.0;
const SUN_RAY_COUNT: usize = 5;
const SUN_RAY_SPREAD: f32 = 0.16;
const SUN_COLOR: Rgba = Rgba::new(0xff, 0xe2, 0xa0, 0x3c);

const PATCH_ALPHA: u8 = 70;
const PATCHES: [(f32, f32, f32, f32); 6] = [
    (300.0, 60.0, 150.0, 90.0),
    (30.0, 180.0, 110.0, 140.0),
    (360.0, 260.0, 120.0, 120.0),
    (90.0, 420.0, 160.0, 80.0),
    (280.0, 400.0, 100.0, 100.0),
    (430.0, 470.0, 70.0, 40.0),
];

const VIGNETTE_COLOR: Rgba = Rgba::new(0x14, 0x10, 0x0c, 0x66);
const VIGNETTE_WIDTH: f32 = 70.0;

const TITLE_BAR_Y: f32 = 434.0;
const TITLE_BAR_H: f32 = 78.0;
const TITLE_BAR_ALPHA: u8 = 210;
const TITLE_GEAR_R: f32 = 34.0;
const TITLE_GEAR_CY: f32 = TITLE_BAR_Y + TITLE_BAR_H * 0.5;
const TITLE_TEXT_H: f32 = 9.0;
const TITLE_ROW_COUNT: usize = 3;
const TITLE_ROW_GAP: f32 = 6.0;
const TITLE_ROW_TOP_DY: f32 = 18.0;
const TITLE_TEXT_INNER_DX: f32 = 56.0;
const TITLE_TEXT_OUTER_DX: f32 = 216.0;

fn panel_path() -> Option<crate::geometry::Path> {
    PathBuilder::new()
        .move_to(Pt::new(0.0, 0.0))
        .line_to(Pt::new(CANVAS_SIZE, 0.0))
        .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
        .line_to(Pt::new(0.0, CANVAS_SIZE))
        .close()
        .build()
}

fn draw_collage_patches(c: &mut dyn Canvas, p: &Palette) {
    for &(x, y, w, h) in PATCHES.iter() {
        let patch = PathBuilder::new()
            .move_to(Pt::new(x, y))
            .line_to(Pt::new(x + w, y))
            .line_to(Pt::new(x + w, y + h))
            .line_to(Pt::new(x, y + h))
            .close()
            .build();
        if let Some(patch) = patch {
            let cmd = outline(
                &patch,
                Paint::Solid(p.line.with_alpha(PATCH_ALPHA)),
                p.line.with_alpha(0),
                0.0,
            );
            c.add(&cmd);
        }
    }
}

fn draw_solar_light(c: &mut dyn Canvas) {
    let base = -std::f32::consts::FRAC_PI_4;
    for i in 0..SUN_RAY_COUNT {
        let a = base + i as f32 * SUN_RAY_SPREAD * 2.2;
        let r = CANVAS_SIZE * 1.5;
        let ray = PathBuilder::new()
            .move_to(Pt::new(SUN_ORIGIN_X, SUN_ORIGIN_Y))
            .line_to(Pt::new(
                SUN_ORIGIN_X + r * (a - SUN_RAY_SPREAD).cos(),
                SUN_ORIGIN_Y + r * (a - SUN_RAY_SPREAD).sin(),
            ))
            .line_to(Pt::new(
                SUN_ORIGIN_X + r * (a + SUN_RAY_SPREAD).cos(),
                SUN_ORIGIN_Y + r * (a + SUN_RAY_SPREAD).sin(),
            ))
            .close()
            .build();
        if let Some(ray) = ray {
            let cmd = outline(&ray, Paint::Solid(SUN_COLOR), Rgba::new(0, 0, 0, 0), 0.0);
            c.add(&cmd);
        }
    }
}

pub fn draw_vignette(c: &mut dyn Canvas) {
    if let Some(panel) = panel_path() {
        let cmd = DrawCmd {
            path: panel,
            paint: Paint::Solid(Rgba::new(0, 0, 0, 0)),
            stroke: Some(Stroke::new(VIGNETTE_COLOR, VIGNETTE_WIDTH)),
        };
        c.add(&cmd);
    }
}

pub fn draw_title_band(c: &mut dyn Canvas, p: &Palette) {
    let bar = PathBuilder::new()
        .move_to(Pt::new(0.0, TITLE_BAR_Y))
        .line_to(Pt::new(CANVAS_SIZE, TITLE_BAR_Y))
        .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
        .line_to(Pt::new(0.0, CANVAS_SIZE))
        .close()
        .build()
        .expect("title bar");
    let ink = Rgba::opaque(0xf2, 0xee, 0xe2);
    let cmd = outline(
        &bar,
        Paint::Solid(p.line.with_alpha(TITLE_BAR_ALPHA)),
        ink.with_alpha(160),
        2.0,
    );
    c.add(&cmd);

    if let Some(emblem) = gear_path(256.0, TITLE_GEAR_CY, TITLE_GEAR_R) {
        let cmd = outline(&emblem, Paint::Solid(ink), p.line.with_alpha(0), 0.0);
        c.add(&cmd);
    }
    if let Some(hub) = circle_path(256.0, TITLE_GEAR_CY, TITLE_GEAR_R * GEAR_HUB_RATIO) {
        let cmd = outline(
            &hub,
            Paint::Solid(p.line.with_alpha(TITLE_BAR_ALPHA)),
            p.line.with_alpha(0),
            0.0,
        );
        c.add(&cmd);
    }

    for side in [-1.0f32, 1.0f32] {
        let x0 = 256.0 + side * TITLE_TEXT_INNER_DX;
        let x1 = 256.0 + side * TITLE_TEXT_OUTER_DX;
        let left = x0.min(x1);
        let w = (x0 - x1).abs();
        for row in 0..TITLE_ROW_COUNT {
            let y = TITLE_GEAR_CY - TITLE_ROW_TOP_DY + row as f32 * (TITLE_TEXT_H + TITLE_ROW_GAP);
            let seg = PathBuilder::new()
                .move_to(Pt::new(left, y))
                .line_to(Pt::new(left + w, y))
                .line_to(Pt::new(left + w, y + TITLE_TEXT_H))
                .line_to(Pt::new(left, y + TITLE_TEXT_H))
                .close()
                .build()
                .expect("title text seg");
            let cmd = outline(&seg, Paint::Solid(ink), p.line.with_alpha(0), 0.0);
            c.add(&cmd);
        }
    }
}

fn draw_gears(c: &mut dyn Canvas, p: &Palette) {
    let tooth = p.bg_bottom.darken(0.35);
    let body = p.bg_bottom.darken(0.2);
    for (i, &(cx, cy, r)) in GEARS.iter().enumerate() {
        let Some(ring) = gear_path(cx, cy, r) else {
            continue;
        };
        let fill = if i % 2 == 0 { body } else { tooth };
        let cmd = outline(
            &ring,
            Paint::Solid(fill.with_alpha(160)),
            fill.with_alpha(220),
            3.0,
        );
        c.add(&cmd);
        if let Some(hub) = circle_path(cx, cy, r * GEAR_HUB_RATIO) {
            let cmd = outline(
                &hub,
                Paint::Solid(p.bg_top.with_alpha(200)),
                fill.with_alpha(120),
                2.0,
            );
            c.add(&cmd);
        }
    }
}

fn gear_path(cx: f32, cy: f32, outer: f32) -> Option<crate::geometry::Path> {
    let root = outer * GEAR_TOOTH_DEPTH;
    let step = std::f32::consts::TAU / GEAR_TEETH as f32;
    let half_tooth = step * 0.28;
    let mut pb = PathBuilder::new();
    for i in 0..GEAR_TEETH {
        let a = i as f32 * step - std::f32::consts::FRAC_PI_2;
        let tip_a = a + half_tooth;
        let root_a = tip_a + half_tooth;
        let pts = [
            Pt::new(cx + root * a.cos(), cy + root * a.sin()),
            Pt::new(cx + outer * tip_a.cos(), cy + outer * tip_a.sin()),
            Pt::new(cx + root * root_a.cos(), cy + root * root_a.sin()),
        ];
        for (j, pt) in pts.into_iter().enumerate() {
            pb = if i == 0 && j == 0 {
                pb.move_to(pt)
            } else {
                pb.line_to(pt)
            };
        }
    }
    pb.close().build()
}

fn star_path(cx: f32, cy: f32, outer: f32) -> Option<crate::geometry::Path> {
    let inner = outer * 0.42;
    let mut pb = PathBuilder::new();
    let mut angle = -std::f32::consts::FRAC_PI_2;
    for i in 0..10 {
        let r = if i % 2 == 0 { outer } else { inner };
        let point = Pt::new(cx + r * angle.cos(), cy + r * angle.sin());
        if i == 0 {
            pb = pb.move_to(point);
        } else {
            pb = pb.line_to(point);
        }
        angle += std::f32::consts::PI / 5.0;
    }
    pb.close().build()
}
