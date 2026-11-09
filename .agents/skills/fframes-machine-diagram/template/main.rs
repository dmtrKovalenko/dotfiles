use codex_loop::{CodexLoopMedia, CodexLoopVideo, HEIGHT, WIDTH};
use fframes::{EncoderOptions, RenderOptions, StaticMediaProvider, cli};
#[cfg(all(
    not(test),
    feature = "dynamic-linking",
    any(target_os = "macos", target_os = "linux"),
    not(target_feature = "crt-static"),
))]
use fframes_dylib as _;
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use std::process::ExitCode;

fn main() -> ExitCode {
    let media = CodexLoopMedia::prepare().expect("media");
    let video = CodexLoopVideo::new(&media);
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[("crf", "20"), ("preset", "medium"), ("tune", "animation")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    // Frame previews (frame, strip, onion) render with the same Skia backend.
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
