//! solsulsol demo: title card, the recorded e2e product walkthrough with
//! subtitles, and an outro.
//!
//! Each scene is a struct returning an SVG tree per frame (`svgr!`); the
//! recorded UI footage plays inside a browser-window card. Check results with
//! the CLI in `main.rs` (`cargo run --release -- --help`).
use fframes::{
    animation::{AnimationRuntime, Easing},
    include_media_dir, AnimateRuntimeInput, AudioMap, Color, Duration, FFramesContext,
    FFramesSyncedVideoFrame, Frame, Scene, Scenes, Svgr, SyncVideoFrameInput, Transform, Video,
};
use std::sync::LazyLock;

include_media_dir!(pub struct DemoMedia, "media");

pub const WIDTH: usize = 1920;
pub const HEIGHT: usize = 1080;
pub const INTRO_SECONDS: f32 = 3.0;
pub const OUTRO_SECONDS: f32 = 3.0;
/// The walkthrough clip: each of the 5 features plays at native recording
/// speed (~1s) then holds its last frame, so every feature gets 5 seconds.
pub const DEMO_SECONDS: f32 = 25.0;

// App theme (crates/ui/ui_wasm STYLE): dark plum bg/cards, orange accent,
// pink ghost buttons.
const PAPER: &str = "#221418";
const INK: &str = "#321e26";
const INK_BORDER: &str = "#452a35";
const MUTED: &str = "#c9a8a0";
const TEXT: &str = "#fff3e9";
const GREEN: &str = "#ffab73";
const LIME: &str = "#ff9eb5";
const DOT_OFF: &str = "#452a35";
const FONT: &str = "DM Sans";
const WEIGHT: u16 = 500;

/// Browser-window card geometry around the 16:9 footage.
const CARD_X: f32 = 260.0;
const CARD_Y: f32 = 130.0;
const CARD_W: f32 = 1400.0;
const CARD_H: f32 = 788.0;

static EASE: LazyLock<AnimationRuntime> =
    LazyLock::new(|| AnimationRuntime::new(0.8, &Easing::CubicBezier(0.16, 1.0, 0.3, 1.0)));
static SPRING: LazyLock<AnimationRuntime> = LazyLock::new(|| {
    AnimationRuntime::new(
        3.0,
        &Easing::Spring {
            mass: 1.0,
            stiffness: 180.0,
            damping: 20.0,
        },
    )
});

pub struct DemoVideo<'a> {
    intro: IntroScene<'a>,
    demo: DemoScene,
    outro: OutroScene,
}

impl<'a> DemoVideo<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            intro: IntroScene { title },
            demo: DemoScene,
            outro: OutroScene,
        }
    }
}

impl std::fmt::Debug for DemoVideo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DemoVideo").finish()
    }
}

impl Video for DemoVideo<'_> {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::hex("#221418");

    fn duration(&self) -> Duration<'_> {
        Duration::Auto
    }

    fn audio(&self) -> AudioMap<'_> {
        use fframes::{AudioTimestamp::Eof, AudioTimestamp::Second, AudioTrack};
        const DEMO_START: f32 = INTRO_SECONDS - 0.4;
        const FEATURE_STEP: f32 = 5.0;
        let feature_starts = [0.0, 1.0, 2.0, 3.0, 4.0].map(|step| DEMO_START + step * FEATURE_STEP);
        let chimes: Vec<AudioTrack> = feature_starts
            .into_iter()
            .map(|at| AudioTrack::new("chime.wav", Second(at)..Second(at + 3.0)))
            .collect();
        // Ticks mark the mid-slot shot changes inside features 2, 4 and 5.
        const SHOT_CHANGES: [f32; 3] = [7.5, 17.5, 22.5];
        let ticks: Vec<AudioTrack> = SHOT_CHANGES
            .into_iter()
            .map(|at| {
                AudioTrack::new(
                    "tick.wav",
                    Second(DEMO_START + at)..Second(DEMO_START + at + 0.5),
                )
            })
            .collect();
        let mut tracks = vec![
            AudioTrack::new("pad.wav", Second(0.2)..Eof).fade_in(3.0),
            AudioTrack::new(
                "voice_over.wav",
                Second(INTRO_SECONDS)..Second(INTRO_SECONDS + DEMO_SECONDS),
            ),
        ];
        tracks.extend(chimes);
        tracks.extend(ticks);
        let outro_at = DEMO_START + DEMO_SECONDS;
        tracks.push(AudioTrack::new(
            "gong.wav",
            Second(outro_at)..Second(outro_at + 4.0),
        ));
        AudioMap::from(tracks)
    }

    fn define_scenes(&self) -> Scenes<'_> {
        Scenes::from(vec![&self.intro as &dyn Scene, &self.demo, &self.outro])
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" width={WIDTH} height={HEIGHT} viewBox="0 0 1920 1080">
                <rect width="1920" height="1080" fill={PAPER} />
                {ctx.render_scenes(&frame)}
            </svg>
        )
    }
}

fn ramp(frame: &Frame, start: f32) -> f32 {
    frame.animate_runtime(AnimateRuntimeInput {
        on_second: start,
        from: 0.0,
        to: 1.0,
        animation_runtime: &EASE,
    })
}

fn rise(frame: &Frame, start: f32) -> f32 {
    frame.animate_runtime(AnimateRuntimeInput {
        on_second: start,
        from: 56.0,
        to: 0.0,
        animation_runtime: &SPRING,
    })
}

/// Scale from `from` to 1.0 with a spring at `start`, around the card center.
fn punch(frame: &Frame, start: f32, from: f32) -> Transform {
    let s = frame.animate_runtime(AnimateRuntimeInput {
        on_second: start,
        from,
        to: 1.0,
        animation_runtime: &SPRING,
    });
    let cx = CARD_X + CARD_W / 2.0;
    let cy = CARD_Y + CARD_H / 2.0;
    Transform {
        translate_x: f64::from(cx * (1.0 - s)),
        translate_y: f64::from(cy * (1.0 - s)),
        scale: fframes::Scale {
            x: f64::from(s),
            y: f64::from(s),
        },
        ..Transform::default()
    }
}

#[derive(Debug)]
struct IntroScene<'a> {
    title: &'a str,
}

impl Scene for IntroScene<'_> {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(INTRO_SECONDS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(
            <g font-family={FONT} font-weight={WEIGHT}>
                <g opacity={ramp(&frame, 0.1)} transform={Transform::translate(0, rise(&frame, 0.1))}>
                    <text x="960" y="500" text-anchor="middle" font-size="180" letter-spacing="-8" fill={TEXT}>{self.title}</text>
                </g>
                <g opacity={ramp(&frame, 0.5)} transform={Transform::translate(0, rise(&frame, 0.5))}>
                    <rect x="760" y="580" width="400" height="70" rx="35" fill={INK} />
                    <text x="960" y="627" text-anchor="middle" font-size="30" fill={LIME}>"SOLANA POINT OF SALE"</text>
                </g>
                <g opacity={ramp(&frame, 1.0)}>
                    <circle cx="905" cy="740" r="7" fill={GREEN} />
                    <circle cx="960" cy="740" r="7" fill={DOT_OFF} />
                    <circle cx="1015" cy="740" r="7" fill={DOT_OFF} />
                </g>
            </g>
        )
    }
}

#[derive(Debug)]
struct DemoScene;

impl Scene for DemoScene {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(DEMO_SECONDS)
    }

    fn overlap(&self) -> fframes::Overlap {
        fframes::Overlap::Previous(0.4)
    }

    fn render_frame<'a>(&'a self, frame: Frame, ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let input = SyncVideoFrameInput {
            start_from: 0.0,
            looping: false,
            editor_fallback_image: None,
        };
        let Some(clip) = frame.get_synced_video_frame(ctx, "clip.mp4", &input) else {
            return Svgr::empty();
        };
        let footage = clip.into_image();
        let subs = ctx
            .get_subtitles("subs.vtt")
            .and_then(|s| frame.get_subtitle_phrase(s));
        let caption = subs
            .map(|text| {
                fframes::svgr!(
                    <g opacity={ramp(&frame, 0.05)} transform={Transform::translate(0, rise(&frame, 0.05))}>
                        <rect x="560" y="920" width="800" height="86" rx="20" fill={INK} opacity="0.92" />
                        <text x="960" y="976" text-anchor="middle" font-size="38" fill={TEXT}>{text.to_string()}</text>
                    </g>
                )
            })
            .unwrap_or_else(Svgr::empty);

        fframes::svgr!(
            <g font-family={FONT} font-weight={WEIGHT}>
                <defs>
                    <clipPath id="clip-screen">
                        <rect x={CARD_X + 16.0} y={CARD_Y + 64.0} width={CARD_W - 32.0} height={CARD_H - 80.0} rx="12" />
                    </clipPath>
                </defs>
                <g opacity={ramp(&frame, 0.0)} transform={punch(&frame, 0.0, 0.94)}>
                    <rect x={CARD_X - 8.0} y={CARD_Y - 8.0} width={CARD_W + 16.0} height={CARD_H + 16.0} rx="36" fill={INK_BORDER} />
                    <rect x={CARD_X} y={CARD_Y} width={CARD_W} height={CARD_H} rx="28" fill={INK} />
                    <circle cx={CARD_X + 44.0} cy={CARD_Y + 34.0} r="10" fill={GREEN} />
                    <circle cx={CARD_X + 76.0} cy={CARD_Y + 34.0} r="10" fill={INK_BORDER} />
                    <circle cx={CARD_X + 108.0} cy={CARD_Y + 34.0} r="10" fill={INK_BORDER} />
                    <text x={CARD_X + CARD_W - 40.0} y={CARD_Y + 46.0} text-anchor="end" font-size="26" fill={MUTED}>"solsulsol — full workflow"</text>
                    <g clip-path="url(#clip-screen)">
                        <image href={footage.href()} x={CARD_X + 16.0} y={CARD_Y + 64.0} width={CARD_W - 32.0} height={CARD_H - 80.0} preserveAspectRatio="xMidYMid slice" />
                    </g>
                </g>
                {caption}
            </g>
        )
    }
}

#[derive(Debug)]
struct OutroScene;

impl Scene for OutroScene {
    fn duration(&self) -> Duration<'_> {
        Duration::Seconds(OUTRO_SECONDS)
    }

    fn render_frame<'a>(&'a self, frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        fframes::svgr!(
            <g font-family={FONT} font-weight={WEIGHT}>
                <g opacity={ramp(&frame, 0.1)} transform={Transform::translate(0, rise(&frame, 0.1))}>
                    <text x="960" y="470" text-anchor="middle" font-size="96" letter-spacing="-4" fill={TEXT}>"Own your checkout."</text>
                </g>
                <g opacity={ramp(&frame, 0.5)} transform={Transform::translate(0, rise(&frame, 0.5))}>
                    <text x="960" y="570" text-anchor="middle" font-size="44" fill={MUTED}>"Self-hosted on axum + Postgres, or serverless on Cloudflare Workers + D1."</text>
                </g>
                <g opacity={ramp(&frame, 0.9)}>
                    <rect x="810" y="640" width="300" height="70" rx="35" fill={GREEN} />
                    <text x="960" y="687" text-anchor="middle" font-size="30" fill={INK}>"TRY IT TODAY"</text>
                </g>
            </g>
        )
    }
}
