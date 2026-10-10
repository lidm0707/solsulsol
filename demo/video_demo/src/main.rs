use demo::{DemoMedia, DemoVideo, HEIGHT, WIDTH};
use fframes::cli::clap; // the derive below expands to `clap::...`
use fframes::{cli, EncoderOptions, RenderOptions, StaticMediaProvider};
use fframes_skia_renderer::{
    metal::SkiaMetalCtx, SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig,
};
use std::process::ExitCode;

/// Runtime media (the recorded walkthrough + its subtitles); the static folder
/// only holds fonts, so footage changes don't need a rebuild.
const ASSETS_DIR: &str = "assets";

/// Flags of this video next to the standard ones of `fframes::cli` (render, frame, strip,
/// inspect, audio, ...). Run `cargo run --release -- --help`.
#[derive(Debug, clap::Args)]
struct VideoArgs {
    #[arg(long, default_value = "solsulsol demo", global = true)]
    title: String,
}

fn main() -> ExitCode {
    let args = cli::parse::<VideoArgs>();
    let static_media = DemoMedia::prepare().expect("media");
    let dir = fframes::MediaDirectory::read_folder(ASSETS_DIR).expect("assets dir");
    let runtime_media = dir.process_media_source().expect("assets media");
    let media = fframes::CombinedMediaProvider::from([
        &static_media as &dyn fframes::MediaProvider,
        &runtime_media as &dyn fframes::MediaProvider,
    ]);
    let title = args.app.title.clone();
    let video = DemoVideo::new(&title);
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[
                    ("crf", "20"),
                    ("preset", "medium"),
                    ("tune", "animation"),
                    // fframes decodes sources as bt601 (swscale defaults), so the
                    // output must be tagged the same or players will shift colors.
                    ("colorspace", "smpte170m"),
                    ("color_primaries", "smpte170m"),
                    ("color_trc", "smpte170m"),
                ]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .args(args)
    // Frame previews (frame, strip, onion, snapshot) render with the same Skia backend.
    .backend(
        SkiaFFramesRenderer::new_metal(
            &gpu,
            SkiaPipelineConfig {
                concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
                ..Default::default()
            },
        )
        .expect("skia renderer"),
    )
    // `preview` opens the real-time GPU player.
    .preview(fframes_native_player::cli_preview)
    .run()
}
