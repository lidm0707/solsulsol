use std::fmt;
use std::path::Path as FsPath;

use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, Shader, Stroke as SkiaStroke, Transform,
};

use crate::geometry::{Canvas, DrawCmd, Paint as DemoPaint, Path as DemoPath, CANVAS_SIZE};
use crate::palette::Rgba;

pub const DEFAULT_PNG_SIZE: u32 = 1024;

#[derive(Debug)]
pub enum RenderError {
    PngEncode(String),
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PngEncode(e) => write!(f, "png encode failed: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for RenderError {}

fn to_skia_color(c: crate::palette::Rgba) -> Color {
    let [r, g, b, a] = c.components();
    Color::from_rgba(r, g, b, a).expect("components are normalized")
}

pub struct RasterCanvas {
    pixmap: Pixmap,
    scale: f32,
}

impl RasterCanvas {
    pub fn new(size: u32) -> Option<Self> {
        let pixmap = Pixmap::new(size, size)?;
        let scale = size as f32 / CANVAS_SIZE;
        Some(Self { pixmap, scale })
    }

    fn transform(&self) -> Transform {
        Transform::from_scale(self.scale, self.scale)
    }

    fn to_skia_path(&self, path: &DemoPath) -> Option<tiny_skia::Path> {
        let mut pb = PathBuilder::new();
        pb.move_to(path.start().x, path.start().y);
        for seg in path.segs() {
            match seg {
                crate::geometry::Seg::Line { to } => pb.line_to(to.x, to.y),
                crate::geometry::Seg::Quad { ctrl, to } => pb.quad_to(ctrl.x, ctrl.y, to.x, to.y),
                crate::geometry::Seg::Cubic { c1, c2, to } => {
                    pb.cubic_to(c1.x, c1.y, c2.x, c2.y, to.x, to.y)
                }
            };
        }
        if path.is_closed() {
            pb.close();
        }
        pb.finish()
    }

    fn shader_for(paint: &DemoPaint, s: f32) -> Shader<'static> {
        match paint {
            DemoPaint::Solid(c) => Shader::SolidColor(to_skia_color(*c)),
            DemoPaint::LinearGradient {
                start,
                end,
                from,
                to,
            } => {
                let g = tiny_skia::LinearGradient::new(
                    tiny_skia::Point::from_xy(start.x * s, start.y * s),
                    tiny_skia::Point::from_xy(end.x * s, end.y * s),
                    vec![
                        tiny_skia::GradientStop::new(0.0, to_skia_color(*from)),
                        tiny_skia::GradientStop::new(1.0, to_skia_color(*to)),
                    ],
                    tiny_skia::SpreadMode::Pad,
                    Transform::identity(),
                );
                g.expect("gradient with two stops is valid")
            }
        }
    }

    pub fn save_png<P: AsRef<FsPath>>(&self, path: P) -> Result<(), RenderError> {
        self.pixmap
            .save_png(path)
            .map_err(|e| RenderError::PngEncode(e.to_string()))
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn raw_data(&self) -> &[u8] {
        self.pixmap.data()
    }

    pub fn quantize(&self, palette: &[Rgba]) -> Option<RasterCanvas> {
        if palette.is_empty() {
            return None;
        }
        let mut out_pixmap = Pixmap::new(self.pixmap.width(), self.pixmap.height())?;
        let src = self.pixmap.data();
        let dst = out_pixmap.data_mut();
        for i in (0..src.len()).step_by(4) {
            let (r, g, b) = (src[i], src[i + 1], src[i + 2]);
            let best = palette
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| {
                    let dr = i32::from(r) - i32::from(c.r);
                    let dg = i32::from(g) - i32::from(c.g);
                    let db = i32::from(b) - i32::from(c.b);
                    dr * dr + dg * dg + db * db
                })
                .map(|(_, c)| (c.r, c.g, c.b))
                .unwrap_or((r, g, b));
            dst[i] = best.0;
            dst[i + 1] = best.1;
            dst[i + 2] = best.2;
            dst[i + 3] = 255;
        }
        Some(RasterCanvas {
            pixmap: out_pixmap,
            scale: self.scale,
        })
    }

    pub fn upscale_nearest(&self, out_size: u32) -> Option<RasterCanvas> {
        let w = self.pixmap.width();
        if out_size < w || !out_size.is_multiple_of(w) {
            return None;
        }
        let f = (out_size / w) as usize;
        let mut out = RasterCanvas::new(out_size)?;
        let src = self.pixmap.data();
        let dst = out.pixmap.data_mut();
        let stride = w as usize * 4;
        for y in 0..out_size as usize {
            let srow = (y / f) * stride;
            let drow = y * out_size as usize * 4;
            for x in 0..out_size as usize {
                let s = srow + (x / f) * 4;
                let d = drow + x * 4;
                dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
        Some(out)
    }
}

impl Canvas for RasterCanvas {
    fn add(&mut self, cmd: &DrawCmd) {
        let Some(path) = self.to_skia_path(&cmd.path) else {
            return;
        };
        let t = self.transform();
        let mut paint = Paint::default();
        paint.shader = Self::shader_for(&cmd.paint, self.scale);
        paint.anti_alias = true;
        self.pixmap
            .fill_path(&path, &paint, FillRule::Winding, t, None);
        if let Some(stroke) = cmd.stroke {
            let sk = SkiaStroke {
                width: stroke.width,
                ..SkiaStroke::default()
            };
            let sp = Paint {
                shader: Shader::SolidColor(to_skia_color(stroke.color)),
                anti_alias: true,
                ..Paint::default()
            };
            self.pixmap.stroke_path(&path, &sp, &sk, t, None);
        }
    }
}

pub struct SvgCanvas {
    width: u32,
    height: u32,
    body: String,
}

impl SvgCanvas {
    pub fn new(size: u32) -> Self {
        Self {
            width: size,
            height: size,
            body: String::new(),
        }
    }

    pub fn finish(self) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {CANVAS_SIZE} {CANVAS_SIZE}\">\n{}\n</svg>\n",
            self.width, self.height, self.body
        )
    }
}

fn paint_attr(paint: &DemoPaint) -> String {
    match paint {
        DemoPaint::Solid(c) => c.to_svg(),
        DemoPaint::LinearGradient { from, .. } => format!("url(#{})", gradient_id(from)),
    }
}

fn gradient_id(from: &crate::palette::Rgba) -> String {
    format!("grad_{:02x}{:02x}{:02x}", from.r, from.g, from.b)
}

impl Canvas for SvgCanvas {
    fn add(&mut self, cmd: &DrawCmd) {
        if let DemoPaint::LinearGradient {
            start,
            end,
            from,
            to,
        } = cmd.paint
        {
            self.body.push_str(&format!(
                "<defs><linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\"><stop offset=\"0\" stop-color=\"{}\"/><stop offset=\"1\" stop-color=\"{}\"/></linearGradient></defs>\n",
                gradient_id(&from),
                start.x,
                start.y,
                end.x,
                end.y,
                from.to_svg(),
                to.to_svg()
            ));
        }
        let stroke_part = match cmd.stroke {
            Some(s) => format!(
                " stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
                s.color.to_svg(),
                s.width
            ),
            None => String::new(),
        };
        self.body.push_str(&format!(
            "<path d=\"{}\" fill=\"{}\" fill-rule=\"evenodd\"{}/>\n",
            cmd.path.to_svg_d(),
            paint_attr(&cmd.paint),
            stroke_part
        ));
    }
}

#[cfg(feature = "svg")]
pub fn rasterize_svg(svg: &str, size: u32) -> Option<Vec<u8>> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &opt).ok()?;
    let size = usvg::Size::from_wh(size as f32, size as f32)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32)?;
    let scale = size.width() / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{PathBuilder, Pt, KAPPA};
    use crate::palette::Rgba;

    #[test]
    fn mono_stroke_reaches_full_ink() {
        use crate::palette::Rgba;
        use crate::scene::MonoCanvas;
        let mut small = RasterCanvas::new(64).expect("small");
        {
            let mut mono = MonoCanvas::new(
                &mut small,
                Rgba::opaque(245, 166, 35),
                Rgba::opaque(0, 0, 0),
            );
            let path = PathBuilder::new()
                .move_to(Pt::new(64.0, 64.0))
                .line_to(Pt::new(448.0, 64.0))
                .line_to(Pt::new(448.0, 448.0))
                .line_to(Pt::new(64.0, 448.0))
                .close()
                .build()
                .expect("square");
            mono.add(&crate::geometry::DrawCmd {
                path,
                paint: crate::geometry::Paint::Solid(Rgba::opaque(0, 0, 0)),
                stroke: Some(crate::geometry::Stroke::new(
                    Rgba::opaque(245, 166, 35),
                    8.0,
                )),
            });
        }
        let data = small.raw_data();
        let max_r = data.chunks(4).map(|p| p[0]).max().unwrap_or(0);
        println!("max red: {max_r}");
        assert!(max_r > 200, "ink never reaches full brightness: {max_r}");
    }

    #[test]
    fn upscale_repeats_source_pixels() {
        let mut small = RasterCanvas::new(4).expect("small");
        let cmd = DrawCmd {
            path: PathBuilder::new()
                .move_to(Pt::new(0.0, 0.0))
                .line_to(Pt::new(0.0, CANVAS_SIZE))
                .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
                .line_to(Pt::new(CANVAS_SIZE, 0.0))
                .close()
                .build()
                .expect("fill"),
            paint: DemoPaint::Solid(Rgba::opaque(255, 0, 0)),
            stroke: None,
        };
        small.add(&cmd);
        let big = small.upscale_nearest(16).expect("upscale");
        assert_eq!(big.width(), 16);
        let data = big.pixmap.data();
        assert!(data.contains(&255));
        assert_eq!(data[0], data[15 * 4]);
    }

    #[test]
    fn upscale_rejects_non_multiple() {
        let small = RasterCanvas::new(4).expect("small");
        assert!(small.upscale_nearest(10).is_none());
    }

    #[test]
    fn quantize_snaps_to_nearest_palette_color() {
        let mut small = RasterCanvas::new(4).expect("small");
        let cmd = DrawCmd {
            path: PathBuilder::new()
                .move_to(Pt::new(0.0, 0.0))
                .line_to(Pt::new(0.0, CANVAS_SIZE))
                .line_to(Pt::new(CANVAS_SIZE, CANVAS_SIZE))
                .line_to(Pt::new(CANVAS_SIZE, 0.0))
                .close()
                .build()
                .expect("fill"),
            paint: DemoPaint::Solid(Rgba::opaque(200, 140, 40)),
            stroke: None,
        };
        small.add(&cmd);
        let amber = Rgba::opaque(245, 166, 35);
        let q = small
            .quantize(&[Rgba::opaque(0, 0, 0), amber])
            .expect("quantize");
        assert!(q
            .pixmap
            .data()
            .windows(3)
            .step_by(4)
            .any(|w| w == [245, 166, 35]));
    }

    #[test]
    fn quantize_rejects_empty_palette() {
        let small = RasterCanvas::new(4).expect("small");
        assert!(small.quantize(&[]).is_none());
    }

    fn square() -> DemoPath {
        PathBuilder::new()
            .move_to(Pt::new(32.0, 32.0))
            .line_to(Pt::new(480.0, 32.0))
            .line_to(Pt::new(480.0, 480.0))
            .line_to(Pt::new(32.0, 480.0))
            .close()
            .build()
            .expect("square")
    }

    #[test]
    fn raster_canvas_fills_pixels() {
        let mut c = RasterCanvas::new(64).expect("canvas");
        let cmd = DrawCmd {
            path: square(),
            paint: DemoPaint::Solid(Rgba::opaque(255, 0, 0)),
            stroke: None,
        };
        c.add(&cmd);
        let data = c.pixmap.data();
        assert!(data.contains(&255));
    }

    #[test]
    fn svg_canvas_emits_path_element() {
        let mut c = SvgCanvas::new(64);
        let cmd = DrawCmd {
            path: square(),
            paint: DemoPaint::Solid(Rgba::opaque(255, 0, 0)),
            stroke: Some(crate::geometry::Stroke::new(Rgba::opaque(0, 0, 0), 2.0)),
        };
        c.add(&cmd);
        let svg = c.finish();
        assert!(svg.contains("<path d=\"M 32.00 32.00"));
        assert!(svg.contains("stroke="));
        assert!(svg.contains("viewBox=\"0 0 512 512\""));
        let _ = KAPPA;
    }
}
