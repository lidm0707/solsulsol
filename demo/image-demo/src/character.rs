use crate::background::{draw_background, BackgroundKind};
use crate::expression::{
    draw_blush, draw_eyebrows, draw_eyes, draw_mouth, draw_nose, Expression, FaceLayout,
};
use crate::geometry::{
    ellipse_path, outline, Canvas, DrawCmd, Paint, Path, PathBuilder, Pt, Stroke,
};
use crate::palette::Palette;
use crate::pose::{draw_body, draw_neck, Pose};

pub const HEAD_CY: f32 = 205.0;
pub const HEAD_RX: f32 = 112.0;
pub const EAR_RX: f32 = 16.0;
pub const EAR_RY: f32 = 24.0;
pub const OUTLINE_WIDTH: f32 = 3.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairStyle {
    Bob,
    TwinTails,
    Spiky,
}

pub const HAIR_STYLES: [HairStyle; 3] = [HairStyle::Bob, HairStyle::TwinTails, HairStyle::Spiky];

impl HairStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bob => "bob",
            Self::TwinTails => "twin_tails",
            Self::Spiky => "spiky",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accessory {
    None,
    StarPin,
    Glasses,
    Ribbon,
}

pub const ACCESSORIES: [Accessory; 4] = [
    Accessory::None,
    Accessory::StarPin,
    Accessory::Glasses,
    Accessory::Ribbon,
];

impl Accessory {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::StarPin => "star_pin",
            Self::Glasses => "glasses",
            Self::Ribbon => "ribbon",
        }
    }
}

pub fn head_path() -> Option<Path> {
    let cx = 256.0;
    let top = HEAD_CY - HEAD_RX - 12.0;
    let chin = Pt::new(cx, HEAD_CY + 128.0);
    PathBuilder::new()
        .move_to(Pt::new(cx - HEAD_RX, HEAD_CY - 10.0))
        .cubic_to(
            Pt::new(cx - HEAD_RX, top + 30.0),
            Pt::new(cx - HEAD_RX + 34.0, top),
            Pt::new(cx, top),
        )
        .cubic_to(
            Pt::new(cx + HEAD_RX - 34.0, top),
            Pt::new(cx + HEAD_RX, top + 30.0),
            Pt::new(cx + HEAD_RX, HEAD_CY - 10.0),
        )
        .cubic_to(
            Pt::new(cx + HEAD_RX, HEAD_CY + 48.0),
            Pt::new(cx + 62.0, HEAD_CY + 96.0),
            Pt::new(chin.x + 22.0, chin.y - 12.0),
        )
        .cubic_to(
            Pt::new(chin.x + 10.0, chin.y),
            Pt::new(chin.x - 10.0, chin.y),
            Pt::new(chin.x - 22.0, chin.y - 12.0),
        )
        .cubic_to(
            Pt::new(cx - 62.0, HEAD_CY + 96.0),
            Pt::new(cx - HEAD_RX, HEAD_CY + 48.0),
            Pt::new(cx - HEAD_RX, HEAD_CY - 10.0),
        )
        .close()
        .build()
}

pub fn draw_character(
    c: &mut dyn Canvas,
    p: &Palette,
    expr: Expression,
    pose: Pose,
    bg: BackgroundKind,
    hair: HairStyle,
    accessory: Accessory,
) {
    draw_background(c, p, bg);
    draw_hair_back(c, p, hair);
    draw_ears(c, p);
    draw_neck(c, p);
    draw_head(c, p);
    draw_body(c, p, pose);
    draw_face(c, p, expr);
    draw_hair_front(c, p, hair);
    draw_accessory(c, p, hair, accessory);
}

fn draw_ears(c: &mut dyn Canvas, p: &Palette) {
    for side in [-1.0, 1.0] {
        let ear = ellipse_path(
            256.0 + side * (HEAD_RX - 2.0),
            HEAD_CY + 16.0,
            EAR_RX,
            EAR_RY,
        )
        .expect("ear");
        let cmd = outline(&ear, Paint::Solid(p.skin), p.line, 2.5);
        c.add(&cmd);
    }
}

fn draw_head(c: &mut dyn Canvas, p: &Palette) {
    let head = head_path().expect("head path");
    let cmd = outline(&head, Paint::Solid(p.skin), p.line, OUTLINE_WIDTH);
    c.add(&cmd);

    let shade = PathBuilder::new()
        .move_to(Pt::new(256.0 - HEAD_RX + 6.0, HEAD_CY + 20.0))
        .quad_to(
            Pt::new(256.0, HEAD_CY + 60.0),
            Pt::new(256.0 + HEAD_RX - 6.0, HEAD_CY + 20.0),
        )
        .quad_to(
            Pt::new(256.0, HEAD_CY + 150.0),
            Pt::new(256.0 - HEAD_RX + 6.0, HEAD_CY + 20.0),
        )
        .close()
        .build()
        .expect("shade");
    let cmd = outline(
        &shade,
        Paint::Solid(p.skin_shade.with_alpha(50)),
        p.skin_shade.with_alpha(0),
        0.0,
    );
    c.add(&cmd);
}

fn draw_face(c: &mut dyn Canvas, p: &Palette, expr: Expression) {
    let face = FaceLayout {
        center: Pt::new(256.0, crate::expression::FACE_CENTER_Y),
    };
    draw_eyebrows(c, p, expr, &face);
    draw_eyes(c, p, expr, &face);
    draw_nose(c, p, &face);
    draw_mouth(c, p, expr, &face);
    draw_blush(c, p, expr, &face);
}

fn draw_hair_back(c: &mut dyn Canvas, p: &Palette, style: HairStyle) {
    let mass =
        ellipse_path(256.0, HEAD_CY + 6.0, HEAD_RX + 26.0, HEAD_RX + 44.0).expect("hair back");
    let cmd = outline(&mass, Paint::Solid(p.hair_shade), p.line, OUTLINE_WIDTH);
    c.add(&cmd);

    match style {
        HairStyle::Bob | HairStyle::Spiky => {}
        HairStyle::TwinTails => {
            for side in [-1.0, 1.0] {
                let tail = PathBuilder::new()
                    .move_to(Pt::new(256.0 + side * (HEAD_RX + 8.0), HEAD_CY - 30.0))
                    .cubic_to(
                        Pt::new(256.0 + side * 210.0, HEAD_CY - 60.0),
                        Pt::new(256.0 + side * 224.0, HEAD_CY + 130.0),
                        Pt::new(256.0 + side * 168.0, HEAD_CY + 214.0),
                    )
                    .quad_to(
                        Pt::new(256.0 + side * 140.0, HEAD_CY + 160.0),
                        Pt::new(256.0 + side * (HEAD_RX + 2.0), HEAD_CY + 40.0),
                    )
                    .close()
                    .build()
                    .expect("tail");
                let cmd = outline(&tail, Paint::Solid(p.hair_shade), p.line, OUTLINE_WIDTH);
                c.add(&cmd);
            }
        }
    }
}

fn draw_hair_front(c: &mut dyn Canvas, p: &Palette, style: HairStyle) {
    let bangs = match style {
        HairStyle::Spiky => spiky_bangs(),
        HairStyle::Bob | HairStyle::TwinTails => hime_bangs(),
    };
    let cmd = outline(&bangs, Paint::Solid(p.hair_main), p.line, OUTLINE_WIDTH);
    c.add(&cmd);

    let shine = PathBuilder::new()
        .move_to(Pt::new(196.0, 128.0))
        .quad_to(Pt::new(238.0, 108.0), Pt::new(282.0, 118.0))
        .quad_to(Pt::new(244.0, 122.0), Pt::new(208.0, 140.0))
        .close()
        .build()
        .expect("shine");
    let cmd = outline(
        &shine,
        Paint::Solid(p.hair_shine.with_alpha(crate::palette::HAIR_SHINE_ALPHA)),
        p.hair_shine.with_alpha(0),
        0.0,
    );
    c.add(&cmd);

    if style == HairStyle::Bob {
        for side in [-1.0, 1.0] {
            let side_lock = PathBuilder::new()
                .move_to(Pt::new(256.0 + side * (HEAD_RX - 4.0), HEAD_CY - 46.0))
                .quad_to(
                    Pt::new(256.0 + side * (HEAD_RX + 30.0), HEAD_CY + 40.0),
                    Pt::new(256.0 + side * (HEAD_RX + 6.0), HEAD_CY + 118.0),
                )
                .quad_to(
                    Pt::new(256.0 + side * (HEAD_RX - 18.0), HEAD_CY + 60.0),
                    Pt::new(256.0 + side * (HEAD_RX - 26.0), HEAD_CY - 20.0),
                )
                .close()
                .build()
                .expect("side lock");
            let cmd = outline(&side_lock, Paint::Solid(p.hair_main), p.line, OUTLINE_WIDTH);
            c.add(&cmd);
        }
    }
}

fn hime_bangs() -> Path {
    PathBuilder::new()
        .move_to(Pt::new(144.0, 190.0))
        .cubic_to(
            Pt::new(140.0, 96.0),
            Pt::new(200.0, 74.0),
            Pt::new(256.0, 74.0),
        )
        .cubic_to(
            Pt::new(312.0, 74.0),
            Pt::new(372.0, 96.0),
            Pt::new(368.0, 190.0),
        )
        .quad_to(Pt::new(344.0, 160.0), Pt::new(326.0, 196.0))
        .quad_to(Pt::new(306.0, 152.0), Pt::new(288.0, 200.0))
        .quad_to(Pt::new(270.0, 150.0), Pt::new(252.0, 202.0))
        .quad_to(Pt::new(232.0, 150.0), Pt::new(214.0, 198.0))
        .quad_to(Pt::new(194.0, 154.0), Pt::new(176.0, 194.0))
        .quad_to(Pt::new(160.0, 162.0), Pt::new(144.0, 190.0))
        .close()
        .build()
        .expect("bangs")
}

fn spiky_bangs() -> Path {
    PathBuilder::new()
        .move_to(Pt::new(140.0, 196.0))
        .cubic_to(
            Pt::new(134.0, 92.0),
            Pt::new(200.0, 70.0),
            Pt::new(256.0, 70.0),
        )
        .cubic_to(
            Pt::new(312.0, 70.0),
            Pt::new(378.0, 92.0),
            Pt::new(372.0, 196.0),
        )
        .line_to(Pt::new(340.0, 132.0))
        .line_to(Pt::new(322.0, 200.0))
        .line_to(Pt::new(298.0, 130.0))
        .line_to(Pt::new(274.0, 206.0))
        .line_to(Pt::new(250.0, 128.0))
        .line_to(Pt::new(226.0, 204.0))
        .line_to(Pt::new(202.0, 130.0))
        .line_to(Pt::new(178.0, 198.0))
        .line_to(Pt::new(162.0, 134.0))
        .close()
        .build()
        .expect("spiky bangs")
}

fn draw_accessory(c: &mut dyn Canvas, p: &Palette, hair: HairStyle, accessory: Accessory) {
    match accessory {
        Accessory::None => {}
        Accessory::StarPin => {
            if let Some(star) = star_path(322.0, 108.0, 18.0) {
                let cmd = outline(&star, Paint::Solid(p.clothes_accent), p.line, 2.0);
                c.add(&cmd);
            }
        }
        Accessory::Ribbon => {
            let anchor = match hair {
                HairStyle::TwinTails => Pt::new(256.0 + HEAD_RX + 10.0, HEAD_CY - 34.0),
                _ => Pt::new(256.0, 92.0),
            };
            for side in [-1.0, 1.0] {
                let loop_path = PathBuilder::new()
                    .move_to(anchor)
                    .quad_to(
                        Pt::new(anchor.x + side * 34.0, anchor.y - 26.0),
                        Pt::new(anchor.x + side * 40.0, anchor.y + 2.0),
                    )
                    .quad_to(Pt::new(anchor.x + side * 26.0, anchor.y + 12.0), anchor)
                    .close()
                    .build()
                    .expect("ribbon loop");
                let cmd = outline(&loop_path, Paint::Solid(p.clothes_accent), p.line, 2.5);
                c.add(&cmd);
            }
            let knot = ellipse_path(anchor.x, anchor.y + 2.0, 9.0, 9.0).expect("knot");
            let cmd = outline(&knot, Paint::Solid(p.clothes_shade), p.line, 2.0);
            c.add(&cmd);
        }
        Accessory::Glasses => {
            for side in [-1.0, 1.0] {
                let lens = ellipse_path(
                    256.0 + side * crate::expression::EYE_OFFSET_X,
                    crate::expression::EYE_CENTER_Y,
                    34.0,
                    28.0,
                )
                .expect("lens");
                let cmd = DrawCmd {
                    path: lens,
                    paint: Paint::Solid(crate::palette::Rgba::opaque(210, 235, 255).with_alpha(50)),
                    stroke: Some(Stroke::new(p.line, 3.0)),
                };
                c.add(&cmd);
            }
            let bridge = PathBuilder::new()
                .move_to(Pt::new(256.0 - 22.0, crate::expression::EYE_CENTER_Y - 4.0))
                .quad_to(
                    Pt::new(256.0, crate::expression::EYE_CENTER_Y - 14.0),
                    Pt::new(256.0 + 22.0, crate::expression::EYE_CENTER_Y - 4.0),
                )
                .build()
                .expect("bridge");
            let cmd = DrawCmd {
                path: bridge,
                paint: Paint::Solid(p.line),
                stroke: Some(Stroke::new(p.line, 3.0)),
            };
            c.add(&cmd);
        }
    }
}

fn star_path(cx: f32, cy: f32, outer: f32) -> Option<Path> {
    let inner = outer * 0.45;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::geometry::CANVAS_SIZE;
    use crate::render::SvgCanvas;

    struct CountCanvas {
        count: usize,
    }

    impl Canvas for CountCanvas {
        fn add(&mut self, _cmd: &DrawCmd) {
            self.count += 1;
        }
    }

    #[test]
    fn character_draws_every_layer() {
        let p = config::preset_sakura();
        for expr in crate::expression::EXPRESSIONS {
            for pose in crate::pose::POSES {
                for bg in crate::background::BACKGROUNDS {
                    for hair in HAIR_STYLES {
                        for acc in ACCESSORIES {
                            let mut cc = CountCanvas { count: 0 };
                            draw_character(&mut cc, &p, expr, pose, bg, hair, acc);
                            assert!(cc.count > 10, "too few layers for {expr:?} {pose:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn character_produces_valid_svg_markup() {
        let p = config::preset_ocean();
        let mut svg = SvgCanvas::new(512);
        draw_character(
            &mut svg,
            &p,
            Expression::Happy,
            Pose::Wave,
            BackgroundKind::Rays,
            HairStyle::TwinTails,
            Accessory::StarPin,
        );
        let out = svg.finish();
        assert!(out.matches("<path").count() > 10);
        assert!(out.contains("</svg>"));
        let _ = CANVAS_SIZE;
    }

    #[test]
    fn head_path_is_closed_multi_seg() {
        let head = head_path().expect("head");
        assert!(head.is_closed());
        assert!(head.seg_count() >= 5);
    }
}
