use crate::palette::Rgba;

pub const CANVAS_SIZE: f32 = 512.0;
pub const KAPPA: f32 = 0.5523;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Seg {
    Line { to: Pt },
    Quad { ctrl: Pt, to: Pt },
    Cubic { c1: Pt, c2: Pt, to: Pt },
}

#[derive(Clone, Debug)]
pub struct Path {
    start: Pt,
    segs: Vec<Seg>,
    closed: bool,
}

impl Path {
    pub fn start(&self) -> Pt {
        self.start
    }

    pub fn segs(&self) -> &[Seg] {
        &self.segs
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn seg_count(&self) -> usize {
        self.segs.len()
    }

    pub fn to_svg_d(&self) -> String {
        let mut d = format!("M {:.2} {:.2}", self.start.x, self.start.y);
        for seg in &self.segs {
            match seg {
                Seg::Line { to } => d.push_str(&format!(" L {:.2} {:.2}", to.x, to.y)),
                Seg::Quad { ctrl, to } => d.push_str(&format!(
                    " Q {:.2} {:.2} {:.2} {:.2}",
                    ctrl.x, ctrl.y, to.x, to.y
                )),
                Seg::Cubic { c1, c2, to } => d.push_str(&format!(
                    " C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                    c1.x, c1.y, c2.x, c2.y, to.x, to.y
                )),
            }
        }
        if self.closed {
            d.push_str(" Z");
        }
        d
    }
}

#[derive(Default)]
pub struct PathBuilder {
    start: Option<Pt>,
    segs: Vec<Seg>,
    current: Option<Pt>,
    closed: bool,
}

impl PathBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(mut self, p: Pt) -> Self {
        self.start = Some(p);
        self.current = Some(p);
        self
    }

    pub fn line_to(mut self, p: Pt) -> Self {
        self.segs.push(Seg::Line { to: p });
        self.current = Some(p);
        self
    }

    pub fn quad_to(mut self, ctrl: Pt, to: Pt) -> Self {
        self.segs.push(Seg::Quad { ctrl, to });
        self.current = Some(to);
        self
    }

    pub fn cubic_to(mut self, c1: Pt, c2: Pt, to: Pt) -> Self {
        self.segs.push(Seg::Cubic { c1, c2, to });
        self.current = Some(to);
        self
    }

    pub fn close(mut self) -> Self {
        self.closed = true;
        self
    }

    pub fn build(self) -> Option<Path> {
        Some(Path {
            start: self.start?,
            segs: self.segs,
            closed: self.closed,
        })
    }
}

pub fn ellipse_path(cx: f32, cy: f32, rx: f32, ry: f32) -> Option<Path> {
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    PathBuilder::new()
        .move_to(Pt::new(cx + rx, cy))
        .cubic_to(
            Pt::new(cx + rx, cy + ky),
            Pt::new(cx + kx, cy + ry),
            Pt::new(cx, cy + ry),
        )
        .cubic_to(
            Pt::new(cx - kx, cy + ry),
            Pt::new(cx - rx, cy + ky),
            Pt::new(cx - rx, cy),
        )
        .cubic_to(
            Pt::new(cx - rx, cy - ky),
            Pt::new(cx - kx, cy - ry),
            Pt::new(cx, cy - ry),
        )
        .cubic_to(
            Pt::new(cx + kx, cy - ry),
            Pt::new(cx + rx, cy - ky),
            Pt::new(cx + rx, cy),
        )
        .close()
        .build()
}

pub fn circle_path(cx: f32, cy: f32, r: f32) -> Option<Path> {
    ellipse_path(cx, cy, r, r)
}

pub fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w < r * 2.0 || h < r * 2.0 {
        return None;
    }
    let k = r * KAPPA;
    PathBuilder::new()
        .move_to(Pt::new(x + r, y))
        .line_to(Pt::new(x + w - r, y))
        .cubic_to(
            Pt::new(x + w - r + k, y),
            Pt::new(x + w, y + r - k),
            Pt::new(x + w, y + r),
        )
        .line_to(Pt::new(x + w, y + h - r))
        .cubic_to(
            Pt::new(x + w, y + h - r + k),
            Pt::new(x + w - r + k, y + h),
            Pt::new(x + w - r, y + h),
        )
        .line_to(Pt::new(x + r, y + h))
        .cubic_to(
            Pt::new(x + r - k, y + h),
            Pt::new(x, y + h - r + k),
            Pt::new(x, y + h - r),
        )
        .line_to(Pt::new(x, y + r))
        .cubic_to(
            Pt::new(x, y + r - k),
            Pt::new(x + r - k, y),
            Pt::new(x + r, y),
        )
        .close()
        .build()
}

#[derive(Clone, Copy, Debug)]
pub enum Paint {
    Solid(Rgba),
    LinearGradient {
        start: Pt,
        end: Pt,
        from: Rgba,
        to: Rgba,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct Stroke {
    pub color: Rgba,
    pub width: f32,
}

impl Stroke {
    pub fn new(color: Rgba, width: f32) -> Self {
        Self { color, width }
    }
}

pub struct DrawCmd {
    pub path: Path,
    pub paint: Paint,
    pub stroke: Option<Stroke>,
}

pub trait Canvas {
    fn add(&mut self, cmd: &DrawCmd);
}

pub fn fill(path: &Path, paint: Paint) -> DrawCmd {
    DrawCmd {
        path: path.clone(),
        paint,
        stroke: None,
    }
}

pub fn outline(path: &Path, paint: Paint, color: Rgba, width: f32) -> DrawCmd {
    DrawCmd {
        path: path.clone(),
        paint,
        stroke: Some(Stroke::new(color, width)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_tracks_segments_and_close() {
        let path = PathBuilder::new()
            .move_to(Pt::new(0.0, 0.0))
            .line_to(Pt::new(10.0, 0.0))
            .quad_to(Pt::new(20.0, 0.0), Pt::new(20.0, 10.0))
            .cubic_to(Pt::new(20.0, 20.0), Pt::new(0.0, 20.0), Pt::new(0.0, 10.0))
            .close()
            .build()
            .expect("path with start");
        assert_eq!(path.seg_count(), 3);
        assert!(path.is_closed());
        assert_eq!(path.start(), Pt::new(0.0, 0.0));
    }

    #[test]
    fn builder_without_move_yields_none() {
        assert!(PathBuilder::new()
            .line_to(Pt::new(1.0, 1.0))
            .build()
            .is_none());
    }

    #[test]
    fn ellipse_has_four_cubics() {
        let path = ellipse_path(10.0, 10.0, 5.0, 3.0).expect("ellipse");
        assert_eq!(path.seg_count(), 4);
    }

    #[test]
    fn svg_d_is_valid_markup() {
        let path = PathBuilder::new()
            .move_to(Pt::new(1.0, 2.0))
            .line_to(Pt::new(3.0, 4.0))
            .close()
            .build()
            .expect("path");
        assert_eq!(path.to_svg_d(), "M 1.00 2.00 L 3.00 4.00 Z");
    }

    #[test]
    fn lerp_midpoint() {
        let mid = Pt::new(0.0, 0.0).lerp(Pt::new(10.0, 20.0), 0.5);
        assert_eq!(mid, Pt::new(5.0, 10.0));
    }

    #[test]
    fn rounded_rect_has_eight_segments() {
        let rr = rounded_rect_path(0.0, 0.0, 100.0, 60.0, 12.0).expect("rounded rect");
        assert!(rr.is_closed());
        assert_eq!(rr.seg_count(), 8);
    }

    #[test]
    fn rounded_rect_rejects_radius_larger_than_side() {
        assert!(rounded_rect_path(0.0, 0.0, 10.0, 60.0, 12.0).is_none());
    }
}
