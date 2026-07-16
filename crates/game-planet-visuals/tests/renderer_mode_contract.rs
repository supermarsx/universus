use game_planet_visuals::{
    PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderExecutionMode, RenderOptions,
};
use image::RgbaImage;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const CONTRACT_SEED: u64 = 0x5EED_1208_7101;

#[test]
fn cli_help_documents_renderer_and_time_controls_without_rendering() {
    let output = Command::new(env!("CARGO_BIN_EXE_render_planet"))
        .arg("--help")
        .output()
        .expect("render_planet --help should run without rendering");

    assert!(
        output.status.success(),
        "render_planet --help should exit successfully; status={:?}",
        output.status.code()
    );

    let help = command_text(&output);

    assert!(
        help.contains("--renderer <raster|hybrid|raytrace>"),
        "help should document --renderer and accepted modes; help was:\n{help}"
    );
    assert!(
        help.contains("--time-days <days>"),
        "help should document --time-days; help was:\n{help}"
    );
    assert!(
        help.contains("--renderer hybrid"),
        "defaults should name the hybrid renderer mode; help was:\n{help}"
    );
    assert!(
        help.contains("Hybrid/raytrace use CPU path tracing for planet icons"),
        "help should explain hybrid/raytrace icon behavior; help was:\n{help}"
    );
    assert!(
        help.contains("currents") && help.contains("clouds") && help.contains("waves"),
        "time-days help should name time-advanced visual systems; help was:\n{help}"
    );
    assert!(
        help.contains("GPU options report unavailable capability")
            && help.contains("CUDA, ROCm/HIP, and wgpu rendering are not compiled"),
        "help should be clear that GPU modes are reporting/fallback controls today; help was:\n{help}"
    );
}

#[test]
fn strict_gpu_backend_reports_unavailable_without_starting_render() {
    let out_dir = temp_output_dir("strict-gpu-backend");
    let output = Command::new(env!("CARGO_BIN_EXE_render_planet"))
        .args([
            "--backend",
            "cuda",
            "--strict-backend",
            "--progress",
            "json",
            "--output-dir",
        ])
        .arg(&out_dir)
        .output()
        .expect("render_planet should parse a strict CUDA backend request");

    let text = command_text(&output);
    let generated_icon = generated_icon_exists(&out_dir);
    let _ = fs::remove_dir_all(&out_dir);

    assert!(
        !output.status.success(),
        "strict unavailable CUDA backend should fail before rendering; output was:\n{text}"
    );
    assert!(
        text.contains("\"event\":\"backend-resolution\"")
            && text.contains("\"selectedBackend\":\"gpu:cuda\"")
            && text.contains("\"requestedGpuRenderSupported\":false")
            && text.contains("\"activeHardwareAccelerated\":false"),
        "strict GPU request should emit an honest backend-resolution event; output was:\n{text}"
    );
    assert!(
        text.contains("No real CUDA, ROCm/HIP, or wgpu renderer is compiled in this crate yet"),
        "strict GPU failure should not imply CUDA rendering exists; output was:\n{text}"
    );
    assert!(
        !generated_icon,
        "strict backend failure should happen before writing rendered outputs"
    );
}

#[test]
fn cli_parser_and_manifest_source_accept_renderer_and_time_days() {
    let source = read_render_planet_source();

    assert!(
        source.contains("\"--renderer\"") && source.contains("RendererMode::parse(value)?"),
        "render_planet should accept --renderer <mode>"
    );
    assert!(
        source.contains("arg.starts_with(\"--renderer=\")")
            && source.contains("RendererMode::parse(&arg[\"--renderer=\".len()..])?"),
        "render_planet should accept --renderer=<mode>"
    );
    assert!(
        source.contains("\"--time-days\" | \"--snapshot-days\" | \"--time\"")
            && source.contains("parse_snapshot_time_days(value)?"),
        "render_planet should accept --time-days <days> and its aliases"
    );
    assert!(
        source.contains("arg.starts_with(\"--time-days=\")")
            && source.contains("parse_snapshot_time_days(&arg[\"--time-days=\".len()..])?"),
        "render_planet should accept --time-days=<days>"
    );
    assert!(
        source.contains("\"raster\" | \"fast\"")
            && source.contains("\"hybrid\" | \"main\" | \"default\"")
            && source.contains("\"raytrace\" | \"pathtrace\""),
        "RendererMode parser should accept raster, hybrid, and raytrace modes"
    );
    assert!(
        source.contains("\"rendererMode\": options.renderer.label()")
            && source.contains("\"snapshotTimeDays\": profile.snapshot_time_days")
            && source.contains("\"timeDays\": profile.snapshot_time_days")
            && source.contains("\"backendResolution\": {")
            && source.contains("\"selectedBackendCpuFallback\": backend_report.cpu_fallback")
            && source.contains("\"activeHardwareAccelerated\": backend_report.capabilities.hardware_accelerated")
            && source.contains("serde_json::from_str::<Value>(&serde_json::to_string(profile)?)")
            && source.contains("\"profile\": profile_json"),
        "manifest_json should expose rendererMode, time-day, and backend-resolution fields once the CLI emits a manifest"
    );
    assert!(
        source.contains("emit_trace_preview_progress(")
            && source.contains("\"percent\": event.progress.fraction() * 100.0")
            && source.contains("tile.x")
            && source.contains("tile {}/{} at {},{} {}x{}"),
        "CLI progress should expose stage/tile details for long raster and CPU trace renders"
    );
}

#[test]
#[ignore = "CPU path-traced icon smoke takes about 60s in debug; run explicitly when touching icon mode rendering"]
fn raytrace_and_hybrid_icon_modes_produce_nonblank_small_icons() {
    let source = read_render_planet_source();
    assert!(
        source.contains("matches!(self, Self::Hybrid | Self::Raytrace)")
            && source.contains("\"cpu-pathtrace-main-icon\""),
        "render_planet should route hybrid/raytrace icon modes through CPU path tracing"
    );

    let renderer = PlanetRenderer::new(
        PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(CONTRACT_SEED).with_archetype_key("catalog.archetype.earth-like"),
        )
        .with_snapshot_time_days(42.5),
    );
    let options = RenderOptions::preview();

    let traced_icon = renderer
        .try_render_raytraced_icon_with_progress(8, options, RenderExecutionMode::Serial, |_| {})
        .expect("hybrid/raytrace icon path should render a small CPU path-traced icon");
    assert_nonblank_icon(&traced_icon, "hybrid/raytrace icon");
}

fn command_text(output: &std::process::Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_nonblank_icon(image: &RgbaImage, label: &str) {
    assert_eq!(image.dimensions(), (8, 8), "{label} should stay small");

    let total = (image.width() * image.height()) as usize;
    let visible = image.pixels().filter(|pixel| pixel[3] > 0).count();
    let bright = image
        .pixels()
        .filter(|pixel| pixel[3] > 0)
        .filter(|pixel| pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16 > 36)
        .count();
    let luma_sum: u64 = image
        .pixels()
        .filter(|pixel| pixel[3] > 0)
        .map(|pixel| pixel[0] as u64 + pixel[1] as u64 + pixel[2] as u64)
        .sum();

    assert!(
        visible > total / 5,
        "{label} should have visible planet coverage; visible={visible}/{total}"
    );
    assert!(
        bright > total / 12,
        "{label} should include lit pixels; bright={bright}/{total}"
    );
    assert!(
        luma_sum > 180,
        "{label} should carry nonblank color; luma_sum={luma_sum}"
    );
}

fn read_render_planet_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("bin")
        .join("render_planet.rs");
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn temp_output_dir(label: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after UNIX_EPOCH")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "game-planet-visuals-{label}-{}-{nonce}",
        std::process::id(),
    ))
}

fn generated_icon_exists(out_dir: &Path) -> bool {
    out_dir.join("new-terra-rust-icon.png").exists()
}
