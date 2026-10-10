use crate::geometry::{
    circle_path, outline, rounded_rect_path, Canvas, DrawCmd, Paint, PathBuilder, Pt, Stroke,
    CANVAS_SIZE,
};
use crate::palette::{Palette, Rgba};
use crate::render::RasterCanvas;

pub const MONO_LINE_WIDTH: f32 = 16.0;

pub struct MonoCanvas<'a> {
    inner: &'a mut RasterCanvas,
    pub ink: Rgba,
    pub paper: Rgba,
    pub line_width: f32,
}

impl<'a> MonoCanvas<'a> {
    pub fn new(inner: &'a mut RasterCanvas, ink: Rgba, paper: Rgba) -> Self {
        Self {
            inner,
            ink,
            paper,
            line_width: MONO_LINE_WIDTH,
        }
    }
}

impl Canvas for MonoCanvas<'_> {
    fn add(&mut self, cmd: &DrawCmd) {
        let Some(_) = cmd.stroke else {
            return;
        };
        let width = cmd
            .stroke
            .map_or(self.line_width, |s| s.width.max(self.line_width));
        let mono = DrawCmd {
            path: cmd.path.clone(),
            paint: Paint::Solid(self.paper),
            stroke: Some(Stroke::new(self.ink, width)),
        };
        self.inner.add(&mono);
    }
}

pub const DESKTOP_COLOR: Rgba = Rgba::new(0x8a, 0xb0, 0x6a, 0xff);
pub const DESKTOP_SHADOW: Rgba = Rgba::new(0x74, 0x96, 0x58, 0xff);
pub const WINDOW_COLOR: Rgba = Rgba::new(0x5a, 0x50, 0x46, 0xff);
pub const TITLE_BAR_COLOR: Rgba = Rgba::new(0x2e, 0x2a, 0x26, 0xff);
pub const TITLE_TEXT_COLOR: Rgba = Rgba::new(0xd8, 0xd0, 0xc0, 0xff);
pub const SCREEN_TOP: Rgba = Rgba::new(0x33, 0x1e, 0x0c, 0xff);
pub const SCREEN_BOTTOM: Rgba = Rgba::new(0x1c, 0x10, 0x06, 0xff);
pub const AMBER: Rgba = Rgba::new(0xf5, 0xa6, 0x23, 0xff);
pub const AMBER_DIM: Rgba = Rgba::new(0xc0, 0x7d, 0x1c, 0xff);
pub const PHOSPHOR_TINT: Rgba = Rgba::new(0xf5, 0xa6, 0x23, 0x1c);
pub const SCANLINE: Rgba = Rgba::new(0x00, 0x00, 0x00, 0x1a);
pub const GLASS: Rgba = Rgba::new(0xff, 0xff, 0xff, 0x0e);
pub const VIGNETTE: Rgba = Rgba::new(0x00, 0x00, 0x00, 0x30);

pub const WINDOW: Rect = Rect {
    x: 30.0,
    y: 18.0,
    w: 452.0,
    h: 476.0,
};
pub const TITLE_H: f32 = 18.0;
pub const SCREEN: Rect = Rect {
    x: 48.0,
    y: 62.0,
    w: 416.0,
    h: 410.0,
};
pub const SCREEN_RADIUS: f32 = 26.0;
pub const BUTTON_R: f32 = 3.4;
pub const SCANLINE_STEP: f32 = 4.0;
pub const SCANLINE_H: f32 = 1.6;
pub const HEART_SCALE: f32 = 3.0;
pub const HEART_DOTS: usize = 48;
pub const DOT_R: f32 = 2.1;
pub const PROMPT_Y: f32 = 442.0;
pub const TEXT_H: f32 = 11.0;
pub const CURSOR_W: f32 = 8.0;

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn right(&self) -> f32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

pub fn rect_path(r: Rect) -> Option<crate::geometry::Path> {
    PathBuilder::new()
        .move_to(Pt::new(r.x, r.y))
        .line_to(Pt::new(r.right(), r.y))
        .line_to(Pt::new(r.right(), r.bottom()))
        .line_to(Pt::new(r.x, r.bottom()))
        .close()
        .build()
}

fn push_rect(c: &mut dyn Canvas, r: Rect, color: Rgba) {
    if let Some(path) = rect_path(r) {
        let cmd = outline(&path, Paint::Solid(color), color.with_alpha(0), 0.0);
        c.add(&cmd);
    }
}

pub fn draw_crt_backdrop(c: &mut dyn Canvas) {
    push_rect(
        c,
        Rect {
            x: 0.0,
            y: 0.0,
            w: CANVAS_SIZE,
            h: CANVAS_SIZE,
        },
        DESKTOP_COLOR,
    );
    push_rect(
        c,
        Rect {
            x: WINDOW.x + 5.0,
            y: WINDOW.y + 7.0,
            w: WINDOW.w,
            h: WINDOW.h,
        },
        DESKTOP_SHADOW,
    );
    push_rect(c, WINDOW, WINDOW_COLOR);

    let title = Rect {
        x: WINDOW.x,
        y: WINDOW.y,
        w: WINDOW.w,
        h: TITLE_H,
    };
    push_rect(c, title, TITLE_BAR_COLOR);

    let title_dots = PathBuilder::new()
        .move_to(Pt::new(WINDOW.x + 40.0, WINDOW.y + TITLE_H * 0.68))
        .line_to(Pt::new(WINDOW.right() - 40.0, WINDOW.y + TITLE_H * 0.68))
        .build()
        .expect("title rule");
    let cmd = DrawCmd {
        path: title_dots,
        paint: Paint::Solid(TITLE_TEXT_COLOR.with_alpha(90)),
        stroke: Some(Stroke::new(TITLE_TEXT_COLOR.with_alpha(90), 1.2)),
    };
    c.add(&cmd);

    for i in 0..3u32 {
        let bx = WINDOW.right() - 14.0 - (i as f32) * 12.0;
        if let Some(dot) = circle_path(bx, WINDOW.y + TITLE_H * 0.5, BUTTON_R) {
            let cmd = outline(
                &dot,
                Paint::Solid(TITLE_TEXT_COLOR.with_alpha(160)),
                TITLE_BAR_COLOR,
                1.0,
            );
            c.add(&cmd);
        }
    }

    if let Some(screen) = rounded_rect_path(SCREEN.x, SCREEN.y, SCREEN.w, SCREEN.h, SCREEN_RADIUS) {
        let cmd = outline(
            &screen,
            Paint::LinearGradient {
                start: Pt::new(SCREEN.x, SCREEN.y),
                end: Pt::new(SCREEN.x, SCREEN.bottom()),
                from: SCREEN_TOP,
                to: SCREEN_BOTTOM,
            },
            Rgba::new(0, 0, 0, 0),
            0.0,
        );
        c.add(&cmd);
    }
}

pub fn mask_outside_screen(c: &mut dyn Canvas) {
    let inner_top = Rect {
        x: WINDOW.x,
        y: WINDOW.y + TITLE_H,
        w: WINDOW.w,
        h: SCREEN.y - WINDOW.y - TITLE_H,
    };
    let inner_bottom = Rect {
        x: WINDOW.x,
        y: SCREEN.bottom(),
        w: WINDOW.w,
        h: WINDOW.bottom() - SCREEN.bottom(),
    };
    let inner_left = Rect {
        x: WINDOW.x,
        y: SCREEN.y,
        w: SCREEN.x - WINDOW.x,
        h: SCREEN.h,
    };
    let inner_right = Rect {
        x: SCREEN.right(),
        y: SCREEN.y,
        w: WINDOW.right() - SCREEN.right(),
        h: SCREEN.h,
    };
    for r in [inner_top, inner_bottom, inner_left, inner_right] {
        push_rect(c, r, WINDOW_COLOR);
    }
    push_rect(
        c,
        Rect {
            x: 0.0,
            y: 0.0,
            w: CANVAS_SIZE,
            h: WINDOW.y,
        },
        DESKTOP_COLOR,
    );
    push_rect(
        c,
        Rect {
            x: 0.0,
            y: WINDOW.bottom(),
            w: CANVAS_SIZE,
            h: CANVAS_SIZE - WINDOW.bottom(),
        },
        DESKTOP_COLOR,
    );
    push_rect(
        c,
        Rect {
            x: 0.0,
            y: WINDOW.y,
            w: WINDOW.x,
            h: WINDOW.h,
        },
        DESKTOP_COLOR,
    );
    push_rect(
        c,
        Rect {
            x: WINDOW.right(),
            y: WINDOW.y,
            w: CANVAS_SIZE - WINDOW.right(),
            h: WINDOW.h,
        },
        DESKTOP_COLOR,
    );
}

pub fn draw_crt_overlay(c: &mut dyn Canvas) {
    mask_outside_screen(c);

    if let Some(screen) = rounded_rect_path(SCREEN.x, SCREEN.y, SCREEN.w, SCREEN.h, SCREEN_RADIUS) {
        let cmd = outline(
            &screen,
            Paint::Solid(PHOSPHOR_TINT),
            Rgba::new(0, 0, 0, 0),
            0.0,
        );
        c.add(&cmd);

        let mut y = SCREEN.y + SCANLINE_STEP;
        while y < SCREEN.bottom() {
            let line = PathBuilder::new()
                .move_to(Pt::new(SCREEN.x, y))
                .line_to(Pt::new(SCREEN.right(), y))
                .build()
                .expect("scanline");
            let cmd = DrawCmd {
                path: line,
                paint: Paint::Solid(Rgba::new(0, 0, 0, 0)),
                stroke: Some(Stroke::new(SCANLINE, SCANLINE_H)),
            };
            c.add(&cmd);
            y += SCANLINE_STEP;
        }

        let vignette = rounded_rect_path(
            SCREEN.x + 4.0,
            SCREEN.y + 4.0,
            SCREEN.w - 8.0,
            SCREEN.h - 8.0,
            SCREEN_RADIUS - 4.0,
        )
        .expect("vignette ring");
        let cmd = DrawCmd {
            path: vignette,
            paint: Paint::Solid(Rgba::new(0, 0, 0, 0)),
            stroke: Some(Stroke::new(VIGNETTE, 22.0)),
        };
        c.add(&cmd);

        let glass = PathBuilder::new()
            .move_to(Pt::new(SCREEN.x + 20.0, SCREEN.y + 10.0))
            .line_to(Pt::new(SCREEN.x + SCREEN.w * 0.45, SCREEN.y + 10.0))
            .line_to(Pt::new(SCREEN.x + 20.0, SCREEN.y + SCREEN.h * 0.45))
            .close()
            .build()
            .expect("glass");
        let cmd = outline(&glass, Paint::Solid(GLASS), Rgba::new(0, 0, 0, 0), 0.0);
        c.add(&cmd);
    }

    draw_heart(c, 96.0, 104.0);
    draw_heart(c, 416.0, 104.0);
    draw_prompt(c);
}

fn draw_heart(c: &mut dyn Canvas, cx: f32, cy: f32) {
    for i in 0..HEART_DOTS {
        let t = f32::from(i as u16) * std::f32::consts::TAU / HEART_DOTS as f32;
        let (st, ct) = (t.sin(), t.cos());
        let x = 16.0 * st * st * st;
        let y = 13.0 * ct - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
        if let Some(dot) = circle_path(cx + x * HEART_SCALE, cy - y * HEART_SCALE, DOT_R) {
            let cmd = outline(&dot, Paint::Solid(AMBER), Rgba::new(0, 0, 0, 0), 0.0);
            c.add(&cmd);
        }
    }
}

const PROMPT_SEGMENTS: [f32; 7] = [36.0, 9.0, 44.0, 9.0, 24.0, 7.0, 30.0];
const SEGMENT_GAP: f32 = 5.0;

fn draw_prompt(c: &mut dyn Canvas) {
    let mut x = SCREEN.x + 26.0;
    for (i, w) in PROMPT_SEGMENTS.into_iter().enumerate() {
        let color = if i % 2 == 0 { AMBER } else { AMBER_DIM };
        let seg = Rect {
            x,
            y: PROMPT_Y,
            w,
            h: TEXT_H,
        };
        if let Some(path) = rect_path(seg) {
            let cmd = outline(&path, Paint::Solid(color), Rgba::new(0, 0, 0, 0), 0.0);
            c.add(&cmd);
        }
        x += w + SEGMENT_GAP;
    }
    let cursor = Rect {
        x: x + 6.0,
        y: PROMPT_Y - 2.0,
        w: CURSOR_W,
        h: TEXT_H + 4.0,
    };
    if let Some(path) = rect_path(cursor) {
        let cmd = outline(&path, Paint::Solid(AMBER), Rgba::new(0, 0, 0, 0), 0.0);
        c.add(&cmd);
    }
}

pub fn phosphorize(p: &Palette) -> Palette {
    let amber_wash = |c: Rgba, mix: f32| c.lerp_to(AMBER, mix);
    Palette {
        skin: amber_wash(p.skin, 0.25),
        skin_shade: amber_wash(p.skin_shade, 0.3),
        hair_main: amber_wash(p.hair_main, 0.35),
        hair_shade: amber_wash(p.hair_shade, 0.4),
        hair_shine: amber_wash(p.hair_shine, 0.45),
        eye_iris: AMBER,
        eye_white: Rgba::opaque(0xf5, 0xe2, 0xc0),
        clothes_main: amber_wash(p.clothes_main, 0.35),
        clothes_shade: amber_wash(p.clothes_shade, 0.4),
        clothes_accent: AMBER_DIM,
        line: Rgba::opaque(0x2a, 0x18, 0x08),
        blush: amber_wash(p.blush, 0.4),
        bg_top: SCREEN_TOP,
        bg_bottom: SCREEN_BOTTOM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::preset_earthtone;

    struct CountCanvas {
        count: usize,
    }

    impl Canvas for CountCanvas {
        fn add(&mut self, _cmd: &DrawCmd) {
            self.count += 1;
        }
    }

    #[test]
    fn backdrop_and_overlay_draw_layers() {
        let mut cc = CountCanvas { count: 0 };
        draw_crt_backdrop(&mut cc);
        let after_backdrop = cc.count;
        draw_crt_overlay(&mut cc);
        assert!(after_backdrop > 5, "backdrop too thin");
        assert!(cc.count > after_backdrop + 100, "overlay missing scanlines");
    }

    #[test]
    fn phosphorize_pulls_toward_amber() {
        let p = phosphorize(&preset_earthtone());
        assert_eq!(p.eye_iris, AMBER);
        assert_eq!(p.bg_top, SCREEN_TOP);
        assert!(p.hair_main.r >= preset_earthtone().hair_main.r);
    }
}
