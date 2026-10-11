use crate::background::{draw_background, BackgroundKind};
use crate::expression::{
    draw_blush, draw_eyebrows, draw_eyes, draw_mouth, draw_nose, Expression, FaceLayout,
};
use crate::geometry::{
    ellipse_path, outline, Canvas, DrawCmd, Paint, Path, PathBuilder, Pt, Stroke,
};
use crate::palette::{Palette, Rgba};
use crate::pose::{draw_body, draw_neck, Pose};

pub const HEAD_CY: f32 = 205.0;
const CANVAS_FLOOR: f32 = 512.0;
pub const HEAD_RX: f32 = 112.0;
pub const EAR_RX: f32 = 16.0;
pub const EAR_RY: f32 = 24.0;
pub const OUTLINE_WIDTH: f32 = 3.5;
const NECK_STROKE_W: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairStyle {
    Bob,
    TwinTails,
    Spiky,
    Long,
}

pub const HAIR_STYLES: [HairStyle; 4] = [
    HairStyle::Bob,
    HairStyle::TwinTails,
    HairStyle::Spiky,
    HairStyle::Long,
];

impl HairStyle {
    pub fn name(self) -> &'static str {
        match self {
            Self::Bob => "bob",
            Self::TwinTails => "twin_tails",
            Self::Spiky => "spiky",
            Self::Long => "long",
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

const COMPANION_HAIR: Rgba = Rgba::new(0x24, 0x20, 0x28, 0xff);
const COMPANION_SHIRT: Rgba = Rgba::new(0xec, 0xe8, 0xdc, 0xff);
const COMPANION_SHIRT_SHADE: Rgba = Rgba::new(0xc9, 0xc2, 0xb2, 0xff);

const COMPANION_HEAD_CX: f32 = 340.0;
const COMPANION_HEAD_CY: f32 = 64.0;
const COMPANION_TORSO_SHOULDER_DX: f32 = 96.0;
const COMPANION_TORSO_TOP_DY: f32 = 52.0;
const COMPANION_TORSO_OUT_DX: f32 = 130.0;
const COMPANION_TORSO_CTRL_1_DX: f32 = 130.0;
const COMPANION_TORSO_CTRL_1_DY: f32 = 76.0;
const COMPANION_TORSO_CTRL_2_DX: f32 = 138.0;
const COMPANION_TORSO_CTRL_2_DY: f32 = 176.0;
const COMPANION_TORSO_HEM_DX: f32 = 150.0;
const COMPANION_TORSO_RIGHT_TOP_DX: f32 = 76.0;
const COMPANION_TORSO_RIGHT_TOP_DY: f32 = 50.0;
const COMPANION_TORSO_RIGHT_C1_DX: f32 = 110.0;
const COMPANION_TORSO_RIGHT_C1_DY: f32 = 72.0;
const COMPANION_TORSO_RIGHT_C2_DX: f32 = 122.0;
const COMPANION_TORSO_RIGHT_C2_DY: f32 = 166.0;
const COMPANION_SHADE_ANCHOR_DX: f32 = 40.0;
const COMPANION_SHADE_ANCHOR_DY: f32 = 64.0;
const COMPANION_SHADE_CTRL_DX: f32 = 100.0;
const COMPANION_SHADE_CTRL_DY: f32 = 196.0;
const COMPANION_SHADE_END_DX: f32 = 96.0;
const COMPANION_NECK_HALF_W: f32 = 14.0;
const COMPANION_NECK_TOP_DY: f32 = 20.0;
const COMPANION_NECK_BOTTOM_Y: f32 = 122.0;
const COMPANION_FACE_DX: f32 = 2.0;
const COMPANION_FACE_DY: f32 = 12.0;
const COMPANION_FACE_RX: f32 = 38.0;
const COMPANION_FACE_RY: f32 = 42.0;
const COMPANION_FACE_STROKE_W: f32 = 2.5;
const COMPANION_HAIR_SIDE_DX: f32 = 42.0;
const COMPANION_HAIR_SIDE_DY: f32 = 8.0;
const COMPANION_HAIR_TOP_Y: f32 = 8.0;
const COMPANION_HAIR_SPIKE_DX: f32 = 48.0;
const COMPANION_HAIR_SPIKE_DY: f32 = 6.0;
const COMPANION_HAIR_C1_DX: f32 = 8.0;
const COMPANION_HAIR_C1_DY: f32 = 34.0;
const COMPANION_HAIR_C2_DX: f32 = 48.0;
const COMPANION_HAIR_C3_DX: f32 = 12.0;
const COMPANION_HAIR_C3_DY: f32 = 20.0;
const COMPANION_HAIR_C4_DX: f32 = 6.0;
const COMPANION_HAIR_C4_DY: f32 = 4.0;
const COMPANION_HAIR_SPIKES: [(f32, f32); 5] = [
    (36.0, -16.0),
    (18.0, -4.0),
    (4.0, -20.0),
    (-16.0, -6.0),
    (-30.0, -18.0),
];

pub fn draw_companion(c: &mut dyn Canvas, p: &Palette) {
    let hx = COMPANION_HEAD_CX;
    let hy = COMPANION_HEAD_CY;

    let torso = PathBuilder::new()
        .move_to(Pt::new(
            hx - COMPANION_TORSO_SHOULDER_DX,
            hy + COMPANION_TORSO_TOP_DY,
        ))
        .cubic_to(
            Pt::new(
                hx - COMPANION_TORSO_CTRL_1_DX,
                hy + COMPANION_TORSO_CTRL_1_DY,
            ),
            Pt::new(
                hx - COMPANION_TORSO_CTRL_2_DX,
                hy + COMPANION_TORSO_CTRL_2_DY,
            ),
            Pt::new(hx - COMPANION_TORSO_HEM_DX, CANVAS_FLOOR),
        )
        .line_to(Pt::new(hx + COMPANION_TORSO_OUT_DX, CANVAS_FLOOR))
        .cubic_to(
            Pt::new(
                hx + COMPANION_TORSO_RIGHT_C2_DX,
                hy + COMPANION_TORSO_RIGHT_C2_DY,
            ),
            Pt::new(
                hx + COMPANION_TORSO_RIGHT_C1_DX,
                hy + COMPANION_TORSO_RIGHT_C1_DY,
            ),
            Pt::new(
                hx + COMPANION_TORSO_RIGHT_TOP_DX,
                hy + COMPANION_TORSO_RIGHT_TOP_DY,
            ),
        )
        .close()
        .build()
        .expect("companion torso");
    let cmd = outline(&torso, Paint::Solid(COMPANION_SHIRT), p.line, OUTLINE_WIDTH);
    c.add(&cmd);

    let shade = PathBuilder::new()
        .move_to(Pt::new(
            hx + COMPANION_SHADE_ANCHOR_DX,
            hy + COMPANION_SHADE_ANCHOR_DY,
        ))
        .quad_to(
            Pt::new(hx + COMPANION_SHADE_CTRL_DX, hy + COMPANION_SHADE_CTRL_DY),
            Pt::new(hx + COMPANION_SHADE_END_DX, CANVAS_FLOOR),
        )
        .line_to(Pt::new(hx + COMPANION_TORSO_OUT_DX, CANVAS_FLOOR))
        .cubic_to(
            Pt::new(
                hx + COMPANION_TORSO_RIGHT_C2_DX,
                hy + COMPANION_TORSO_RIGHT_C2_DY,
            ),
            Pt::new(
                hx + COMPANION_TORSO_RIGHT_C1_DX,
                hy + COMPANION_TORSO_RIGHT_C1_DY,
            ),
            Pt::new(
                hx + COMPANION_SHADE_ANCHOR_DX,
                hy + COMPANION_SHADE_ANCHOR_DY,
            ),
        )
        .close()
        .build()
        .expect("companion shade");
    let cmd = outline(
        &shade,
        Paint::Solid(COMPANION_SHIRT_SHADE),
        COMPANION_SHIRT_SHADE.with_alpha(0),
        0.0,
    );
    c.add(&cmd);

    let neck = PathBuilder::new()
        .move_to(Pt::new(
            hx - COMPANION_NECK_HALF_W,
            hy + COMPANION_NECK_TOP_DY,
        ))
        .line_to(Pt::new(hx - COMPANION_NECK_HALF_W, COMPANION_NECK_BOTTOM_Y))
        .line_to(Pt::new(hx + COMPANION_NECK_HALF_W, COMPANION_NECK_BOTTOM_Y))
        .line_to(Pt::new(
            hx + COMPANION_NECK_HALF_W,
            hy + COMPANION_NECK_TOP_DY,
        ))
        .close()
        .build()
        .expect("companion neck");
    let cmd = outline(&neck, Paint::Solid(p.skin_shade), p.line, NECK_STROKE_W);
    c.add(&cmd);

    let face = ellipse_path(
        hx + COMPANION_FACE_DX,
        hy + COMPANION_FACE_DY,
        COMPANION_FACE_RX,
        COMPANION_FACE_RY,
    )
    .expect("companion face");
    let cmd = outline(&face, Paint::Solid(p.skin), p.line, COMPANION_FACE_STROKE_W);
    c.add(&cmd);

    let hair_top = hy - COMPANION_HAIR_TOP_Y;
    let mut hair_pb = PathBuilder::new()
        .move_to(Pt::new(
            hx - COMPANION_HAIR_SIDE_DX,
            hy + COMPANION_HAIR_SIDE_DY,
        ))
        .cubic_to(
            Pt::new(
                hx - COMPANION_HAIR_SIDE_DX - COMPANION_HAIR_C1_DX,
                hair_top - COMPANION_HAIR_C1_DY,
            ),
            Pt::new(hx + COMPANION_HAIR_C4_DX, hair_top - COMPANION_HAIR_C2_DX),
            Pt::new(hx + COMPANION_HAIR_SIDE_DX, hair_top - 30.0),
        )
        .cubic_to(
            Pt::new(
                hx + COMPANION_HAIR_SPIKE_DX + COMPANION_HAIR_C3_DX,
                hair_top - COMPANION_HAIR_C3_DY,
            ),
            Pt::new(
                hx + COMPANION_HAIR_SPIKE_DX + COMPANION_HAIR_C4_DX,
                hair_top + COMPANION_HAIR_C4_DY,
            ),
            Pt::new(hx + COMPANION_HAIR_SPIKE_DX, hy + COMPANION_HAIR_SPIKE_DY),
        );
    for &(dx, dy) in COMPANION_HAIR_SPIKES.iter() {
        hair_pb = hair_pb.line_to(Pt::new(hx + dx, hy + dy));
    }
    let hair = hair_pb.close().build().expect("companion hair");
    let cmd = outline(
        &hair,
        Paint::Solid(COMPANION_HAIR),
        p.line,
        COMPANION_FACE_STROKE_W,
    );
    c.add(&cmd);
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
    draw_companion(c, p);
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
        HairStyle::Long => {
            let mass = PathBuilder::new()
                .move_to(Pt::new(256.0 - (HEAD_RX + 18.0), HEAD_CY - 40.0))
                .cubic_to(
                    Pt::new(256.0 - (HEAD_RX + 78.0), HEAD_CY + 60.0),
                    Pt::new(256.0 - (HEAD_RX + 64.0), HEAD_CY + 230.0),
                    Pt::new(256.0 - (HEAD_RX + 34.0), crate::pose::CANVAS_BOTTOM),
                )
                .line_to(Pt::new(
                    256.0 + (HEAD_RX + 34.0),
                    crate::pose::CANVAS_BOTTOM,
                ))
                .cubic_to(
                    Pt::new(256.0 + (HEAD_RX + 64.0), HEAD_CY + 230.0),
                    Pt::new(256.0 + (HEAD_RX + 78.0), HEAD_CY + 60.0),
                    Pt::new(256.0 + (HEAD_RX + 18.0), HEAD_CY - 40.0),
                )
                .close()
                .build()
                .expect("long back hair");
            let cmd = outline(&mass, Paint::Solid(p.hair_shade), p.line, OUTLINE_WIDTH);
            c.add(&cmd);
        }
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
        HairStyle::Long => long_bangs(),
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

fn long_bangs() -> Path {
    PathBuilder::new()
        .move_to(Pt::new(140.0, 200.0))
        .cubic_to(
            Pt::new(134.0, 90.0),
            Pt::new(200.0, 68.0),
            Pt::new(256.0, 68.0),
        )
        .cubic_to(
            Pt::new(312.0, 68.0),
            Pt::new(378.0, 90.0),
            Pt::new(372.0, 200.0),
        )
        .line_to(Pt::new(352.0, 214.0))
        .quad_to(Pt::new(342.0, 150.0), Pt::new(322.0, 168.0))
        .quad_to(Pt::new(308.0, 120.0), Pt::new(284.0, 172.0))
        .quad_to(Pt::new(268.0, 118.0), Pt::new(248.0, 176.0))
        .quad_to(Pt::new(228.0, 122.0), Pt::new(210.0, 170.0))
        .quad_to(Pt::new(190.0, 132.0), Pt::new(172.0, 196.0))
        .quad_to(Pt::new(158.0, 168.0), Pt::new(140.0, 200.0))
        .close()
        .build()
        .expect("long bangs")
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
