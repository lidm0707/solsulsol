use crate::geometry::{ellipse_path, outline, Canvas, Paint, PathBuilder, Pt};
use crate::palette::Palette;

pub const BODY_TOP_Y: f32 = 352.0;
pub const SHOULDER_HALF_W: f32 = 78.0;
pub const HEM_HALF_W: f32 = 118.0;
pub const CANVAS_BOTTOM: f32 = 512.0;
pub const NECK_HALF_W: f32 = 17.0;
pub const ARM_WIDTH: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    Stand,
    Wave,
    HandsOnHips,
}

pub const POSES: [Pose; 3] = [Pose::Stand, Pose::Wave, Pose::HandsOnHips];

impl Pose {
    pub fn name(self) -> &'static str {
        match self {
            Self::Stand => "stand",
            Self::Wave => "wave",
            Self::HandsOnHips => "hands_on_hips",
        }
    }
}

pub fn draw_neck(c: &mut dyn Canvas, p: &Palette) {
    let neck = PathBuilder::new()
        .move_to(Pt::new(256.0 - NECK_HALF_W, 316.0))
        .line_to(Pt::new(256.0 - NECK_HALF_W, BODY_TOP_Y + 12.0))
        .line_to(Pt::new(256.0 + NECK_HALF_W, BODY_TOP_Y + 12.0))
        .line_to(Pt::new(256.0 + NECK_HALF_W, 316.0))
        .close()
        .build()
        .expect("neck");
    let cmd = outline(&neck, Paint::Solid(p.skin_shade), p.line, 2.0);
    c.add(&cmd);
}

pub fn draw_body(c: &mut dyn Canvas, p: &Palette, pose: Pose) {
    let top = BODY_TOP_Y;
    let torso = PathBuilder::new()
        .move_to(Pt::new(256.0 - NECK_HALF_W - 6.0, top))
        .cubic_to(
            Pt::new(256.0 - SHOULDER_HALF_W, top + 8.0),
            Pt::new(256.0 - SHOULDER_HALF_W - 10.0, top + 26.0),
            Pt::new(256.0 - HEM_HALF_W, CANVAS_BOTTOM),
        )
        .line_to(Pt::new(256.0 + HEM_HALF_W, CANVAS_BOTTOM))
        .cubic_to(
            Pt::new(256.0 + SHOULDER_HALF_W + 10.0, top + 26.0),
            Pt::new(256.0 + SHOULDER_HALF_W, top + 8.0),
            Pt::new(256.0 + NECK_HALF_W + 6.0, top),
        )
        .close()
        .build()
        .expect("torso");
    let cmd = outline(&torso, Paint::Solid(p.clothes_main), p.line, 3.0);
    c.add(&cmd);

    let collar = PathBuilder::new()
        .move_to(Pt::new(256.0 - 30.0, top + 2.0))
        .quad_to(Pt::new(256.0, top + 30.0), Pt::new(256.0 + 30.0, top + 2.0))
        .quad_to(Pt::new(256.0, top + 16.0), Pt::new(256.0 - 30.0, top + 2.0))
        .close()
        .build()
        .expect("collar");
    let cmd = outline(&collar, Paint::Solid(p.clothes_accent), p.line, 2.0);
    c.add(&cmd);

    draw_arms(c, p, pose);
}

fn draw_arms(c: &mut dyn Canvas, p: &Palette, pose: Pose) {
    match pose {
        Pose::Stand => {
            arm_down(c, p, -1.0);
            arm_down(c, p, 1.0);
        }
        Pose::Wave => {
            arm_down(c, p, -1.0);
            arm_wave(c, p);
        }
        Pose::HandsOnHips => {
            arm_hip(c, p, -1.0);
            arm_hip(c, p, 1.0);
        }
    }
}

fn arm_down(c: &mut dyn Canvas, p: &Palette, side: f32) {
    let x = 256.0 + side * (SHOULDER_HALF_W + ARM_WIDTH * 0.1);
    let arm = PathBuilder::new()
        .move_to(Pt::new(x - side * ARM_WIDTH * 0.5, BODY_TOP_Y + 14.0))
        .quad_to(
            Pt::new(x + side * 26.0, BODY_TOP_Y + 90.0),
            Pt::new(x + side * 18.0, CANVAS_BOTTOM),
        )
        .line_to(Pt::new(x + side * (18.0 + ARM_WIDTH), CANVAS_BOTTOM))
        .quad_to(
            Pt::new(x + side * (26.0 + ARM_WIDTH), BODY_TOP_Y + 86.0),
            Pt::new(x + side * ARM_WIDTH * 0.5, BODY_TOP_Y + 20.0),
        )
        .close()
        .build()
        .expect("arm");
    let cmd = outline(&arm, Paint::Solid(p.clothes_main), p.line, 3.0);
    c.add(&cmd);
}

fn arm_wave(c: &mut dyn Canvas, p: &Palette) {
    let shoulder = Pt::new(256.0 + SHOULDER_HALF_W - 6.0, BODY_TOP_Y + 14.0);
    let hand = Pt::new(256.0 + 138.0, BODY_TOP_Y - 58.0);
    let arm = PathBuilder::new()
        .move_to(shoulder)
        .quad_to(Pt::new(shoulder.x + 66.0, shoulder.y + 6.0), hand)
        .line_to(Pt::new(hand.x + 6.0, hand.y + 26.0))
        .quad_to(
            Pt::new(shoulder.x + 78.0, shoulder.y + 40.0),
            Pt::new(shoulder.x + 6.0, shoulder.y + 26.0),
        )
        .close()
        .build()
        .expect("wave arm");
    let cmd = outline(&arm, Paint::Solid(p.clothes_main), p.line, 3.0);
    c.add(&cmd);

    let palm = ellipse_path(hand.x + 4.0, hand.y - 6.0, 16.0, 15.0).expect("palm");
    let cmd = outline(&palm, Paint::Solid(p.skin), p.line, 2.0);
    c.add(&cmd);
}

fn arm_hip(c: &mut dyn Canvas, p: &Palette, side: f32) {
    let shoulder = Pt::new(256.0 + side * (SHOULDER_HALF_W - 4.0), BODY_TOP_Y + 16.0);
    let hip = Pt::new(256.0 + side * (HEM_HALF_W - 34.0), BODY_TOP_Y + 118.0);
    let arm = PathBuilder::new()
        .move_to(shoulder)
        .quad_to(Pt::new(shoulder.x + side * 66.0, shoulder.y + 58.0), hip)
        .line_to(Pt::new(hip.x - side * 4.0, hip.y + 24.0))
        .quad_to(
            Pt::new(shoulder.x + side * 80.0, shoulder.y + 66.0),
            Pt::new(shoulder.x + side * 16.0, shoulder.y + 12.0),
        )
        .close()
        .build()
        .expect("hip arm");
    let cmd = outline(&arm, Paint::Solid(p.clothes_shade), p.line, 3.0);
    c.add(&cmd);
}
