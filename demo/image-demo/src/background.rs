use crate::geometry::{circle_path, outline, Canvas, Paint, PathBuilder, Pt, CANVAS_SIZE};
use crate::palette::{Palette, Rgba};

pub const CENTER: f32 = 256.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundKind {
    None,
    Gradient,
    Rays,
    Night,
}

pub const BACKGROUNDS: [BackgroundKind; 4] = [
    BackgroundKind::None,
    BackgroundKind::Gradient,
    BackgroundKind::Rays,
    BackgroundKind::Night,
];

impl BackgroundKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Gradient => "gradient",
            Self::Rays => "rays",
            Self::Night => "night",
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
