use crate::geometry::{
    ellipse_path, outline, Canvas, DrawCmd, Paint, Path, PathBuilder, Pt, Stroke,
};
use crate::palette::{Palette, Rgba, BLUSH_ALPHA};

pub const FACE_CENTER_X: f32 = 256.0;
pub const FACE_CENTER_Y: f32 = 215.0;
pub const EYE_OFFSET_X: f32 = 47.0;
pub const EYE_CENTER_Y: f32 = 232.0;
pub const EYE_RX: f32 = 25.0;
pub const EYE_RY: f32 = 30.0;
pub const BROW_Y: f32 = 188.0;
pub const NOSE_Y: f32 = 262.0;
pub const MOUTH_Y: f32 = 288.0;
pub const LASH_WIDTH: f32 = 5.5;
pub const IRIS_SCALE: f32 = 0.68;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expression {
    Neutral,
    Happy,
    Wink,
    Surprised,
}

pub const EXPRESSIONS: [Expression; 4] = [
    Expression::Neutral,
    Expression::Happy,
    Expression::Wink,
    Expression::Surprised,
];

impl Expression {
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Happy => "happy",
            Self::Wink => "wink",
            Self::Surprised => "surprised",
        }
    }
}

pub struct FaceLayout {
    pub center: Pt,
}

impl FaceLayout {
    pub fn eye(&self, side: f32) -> Pt {
        Pt::new(self.center.x + side * EYE_OFFSET_X, EYE_CENTER_Y)
    }

    pub fn mouth(&self) -> Pt {
        Pt::new(self.center.x, MOUTH_Y)
    }

    pub fn nose(&self) -> Pt {
        Pt::new(self.center.x, NOSE_Y)
    }

    pub fn brow(&self, side: f32) -> Pt {
        Pt::new(self.center.x + side * EYE_OFFSET_X, BROW_Y)
    }

    pub fn blush(&self, side: f32) -> Pt {
        Pt::new(
            self.center.x + side * (EYE_OFFSET_X + 22.0),
            EYE_CENTER_Y + 34.0,
        )
    }
}

fn draw_eye_open(c: &mut dyn Canvas, p: &Palette, center: Pt) {
    let white = ellipse_path(center.x, center.y, EYE_RX, EYE_RY).expect("eye white");
    let iris_r = EYE_RY * IRIS_SCALE;
    let iris = ellipse_path(center.x, center.y + 2.0, iris_r, iris_r).expect("iris");
    let pupil = ellipse_path(center.x, center.y + 4.0, iris_r * 0.45, iris_r * 0.5).expect("pupil");
    let shine = ellipse_path(
        center.x - iris_r * 0.35,
        center.y - iris_r * 0.35,
        iris_r * 0.3,
        iris_r * 0.3,
    )
    .expect("shine");
    let lash = upper_lash(center);

    let cmds = [
        outline(&white, Paint::Solid(p.eye_white), p.line, 1.5),
        outline(&iris, Paint::Solid(p.eye_iris), p.line, 1.0),
        outline(&pupil, Paint::Solid(p.line), p.line, 0.0),
        outline(
            &shine,
            Paint::Solid(Rgba::opaque(255, 255, 255)),
            p.line,
            0.0,
        ),
        DrawCmd {
            path: lash,
            paint: Paint::Solid(p.line),
            stroke: Some(Stroke::new(p.line, LASH_WIDTH)),
        },
    ];
    for cmd in cmds {
        c.add(&cmd);
    }
}

fn upper_lash(center: Pt) -> Path {
    PathBuilder::new()
        .move_to(Pt::new(center.x - EYE_RX - 3.0, center.y - EYE_RY * 0.4))
        .quad_to(
            Pt::new(center.x, center.y - EYE_RY - 10.0),
            Pt::new(center.x + EYE_RX + 3.0, center.y - EYE_RY * 0.4),
        )
        .build()
        .expect("lash")
}

fn draw_eye_closed(c: &mut dyn Canvas, p: &Palette, center: Pt) {
    let lash = PathBuilder::new()
        .move_to(Pt::new(center.x - EYE_RX, center.y + 4.0))
        .quad_to(
            Pt::new(center.x, center.y + 14.0),
            Pt::new(center.x + EYE_RX, center.y + 4.0),
        )
        .build()
        .expect("closed eye");
    let cmd = DrawCmd {
        path: lash,
        paint: Paint::Solid(p.line),
        stroke: Some(Stroke::new(p.line, LASH_WIDTH)),
    };
    c.add(&cmd);
}

pub fn draw_eyes(c: &mut dyn Canvas, p: &Palette, expr: Expression, face: &FaceLayout) {
    let left = face.eye(-1.0);
    let right = face.eye(1.0);
    match expr {
        Expression::Wink => {
            draw_eye_open(c, p, left);
            draw_eye_closed(c, p, right);
        }
        Expression::Happy => {
            draw_eye_closed(c, p, left);
            draw_eye_closed(c, p, right);
        }
        Expression::Surprised | Expression::Neutral => {
            draw_eye_open(c, p, left);
            draw_eye_open(c, p, right);
        }
    }
    if expr == Expression::Surprised {
        for center in [left, right] {
            let iris =
                ellipse_path(center.x, center.y, EYE_RX * 0.5, EYE_RY * 0.5).expect("wide iris");
            let cmd = outline(&iris, Paint::Solid(p.eye_iris), p.line, 1.0);
            c.add(&cmd);
        }
    }
}

pub fn draw_eyebrows(c: &mut dyn Canvas, p: &Palette, expr: Expression, face: &FaceLayout) {
    for (side, lift) in [(-1.0, 0.0), (1.0, 0.0)] {
        let base = face.brow(side);
        let dy = match expr {
            Expression::Surprised => -6.0,
            Expression::Happy => -2.0,
            _ => 0.0,
        };
        let brow = PathBuilder::new()
            .move_to(Pt::new(base.x - 16.0, base.y + 3.0 + dy + lift))
            .quad_to(
                Pt::new(base.x, base.y - 3.0 + dy + lift),
                Pt::new(base.x + 16.0, base.y + 1.0 + dy + lift),
            )
            .build()
            .expect("brow");
        let cmd = DrawCmd {
            path: brow,
            paint: Paint::Solid(p.hair_shade),
            stroke: Some(Stroke::new(p.hair_shade, 4.0)),
        };
        c.add(&cmd);
    }
}

pub fn draw_nose(c: &mut dyn Canvas, p: &Palette, face: &FaceLayout) {
    let n = face.nose();
    let nose = PathBuilder::new()
        .move_to(Pt::new(n.x - 3.5, n.y - 2.0))
        .quad_to(Pt::new(n.x, n.y + 3.0), Pt::new(n.x + 2.0, n.y - 1.0))
        .build()
        .expect("nose");
    let cmd = DrawCmd {
        path: nose,
        paint: Paint::Solid(p.skin_shade),
        stroke: Some(Stroke::new(p.skin_shade, 3.0)),
    };
    c.add(&cmd);
}

pub fn draw_mouth(c: &mut dyn Canvas, p: &Palette, expr: Expression, face: &FaceLayout) {
    let m = face.mouth();
    let stroke = Stroke::new(p.line, 3.0);
    let path = match expr {
        Expression::Happy | Expression::Wink => open_smile(m, p),
        Expression::Surprised => ellipse_path(m.x, m.y + 3.0, 9.0, 12.0).expect("o mouth"),
        Expression::Neutral => PathBuilder::new()
            .move_to(Pt::new(m.x - 11.0, m.y))
            .quad_to(Pt::new(m.x, m.y + 5.0), Pt::new(m.x + 11.0, m.y))
            .build()
            .expect("neutral mouth"),
    };
    let cmd = DrawCmd {
        path,
        paint: Paint::Solid(p.line),
        stroke: Some(stroke),
    };
    c.add(&cmd);
}

fn open_smile(m: Pt, _p: &Palette) -> Path {
    PathBuilder::new()
        .move_to(Pt::new(m.x - 16.0, m.y - 2.0))
        .quad_to(Pt::new(m.x, m.y + 18.0), Pt::new(m.x + 16.0, m.y - 2.0))
        .quad_to(Pt::new(m.x, m.y + 4.0), Pt::new(m.x - 16.0, m.y - 2.0))
        .close()
        .build()
        .expect("smile")
}

pub fn draw_blush(c: &mut dyn Canvas, p: &Palette, expr: Expression, face: &FaceLayout) {
    if expr == Expression::Surprised {
        return;
    }
    for side in [-1.0, 1.0] {
        let b = face.blush(side);
        let blush = ellipse_path(b.x, b.y, 14.0, 8.0).expect("blush");
        let cmd = outline(
            &blush,
            Paint::Solid(p.blush.with_alpha(BLUSH_ALPHA)),
            Rgba::new(0, 0, 0, 0),
            0.0,
        );
        c.add(&cmd);
    }
}
