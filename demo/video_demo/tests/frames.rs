//! Visual regression: renders a few frames and compares them with the approved PNGs in
//! `_frame_snapshots/`. The first run only creates them, so look at the PNGs before you
//! commit them. `FFRAMES_UPDATE_SNAPSHOTS=1 cargo test` accepts intentional changes; failing
//! frames leave `.actual.png` and `.diff.png` files.
use demo::{DemoMedia, DemoVideo};
use fframes::{
    snapshot, CombinedMediaProvider, CpuFrameRenderer, MediaDirectory, Previewer, RenderOptions,
    StaticMediaProvider,
};

/// Same combination as `main.rs`: embedded static media (font, voice) plus the
/// runtime `assets/` folder (footage, music, subtitles). Leaked on purpose —
/// providers must outlive the previewer for the whole test.
fn media<'a>() -> CombinedMediaProvider<'a, 2> {
    let static_media = Box::leak(Box::new(DemoMedia::prepare().unwrap()));
    let dir = Box::leak(Box::new(MediaDirectory::read_folder("assets").unwrap()));
    let runtime = Box::leak(Box::new(dir.process_media_source().unwrap()));
    CombinedMediaProvider::from([
        static_media as &dyn fframes::MediaProvider,
        runtime as &dyn fframes::MediaProvider,
    ])
}

#[test]
fn key_frames_match_snapshots() {
    let media = media();
    let video = DemoVideo::new("solsulsol demo");
    let options = RenderOptions {
        media: Some(&media),
        scale_resolution: 0.5,
        ..Default::default()
    };
    let mut previewer = Previewer::new(&video, &options).unwrap();

    snapshot::assert_frames(
        &mut previewer,
        &mut CpuFrameRenderer::default(),
        // Settled frames: the middle of a scene, not the end of it where it fades out.
        &["IntroScene@1.5s", "DemoScene@10s", "OutroScene@1.5s"],
        &snapshot::SnapshotOptions::default(),
    );
}

#[test]
fn no_problems_in_any_frame() {
    // Converting a frame without rasterizing it is fast, so every frame is checked.
    let media = media();
    let video = DemoVideo::new("solsulsol demo");
    let options = RenderOptions {
        media: Some(&media),
        ..Default::default()
    };
    let mut previewer = Previewer::new(&video, &options).unwrap();

    let duration = previewer.timeline().duration_in_frames;
    for frame in 0..duration {
        let report = previewer.inspect(frame).unwrap();
        let problems: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.severity >= fframes::diagnostics::Severity::Warning)
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            problems.is_empty(),
            "frame {frame} ({:.2}s): {problems:?}",
            report.seconds
        );
    }
}
