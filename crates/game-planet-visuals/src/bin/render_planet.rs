#![recursion_limit = "256"]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use game_planet_visuals::{
    pathtrace::{
        Camera, CpuTraceKernel, MaterialSample, PathTraceSettings, Sphere, Tile, TraceImage,
        TracePlan, TraceScene, TraceStats, TraceSurfaceControls, TraceSurfaceModel, Vec3,
    },
    ConfiguredRenderBackend, DistantLight, GpuBackendStatus, PlanetPhysicsModel, PlanetRenderer,
    PlanetVisualProfile, ProfileSeedInput, RenderBackendConfiguration,
    RenderBackendConfigurationReport, RenderBackendPreference, RenderExecutionMode, RenderOptions,
    RenderPhase, RenderProgress, RenderProgressEvent, RenderSize, RenderTile,
};
use image::RgbaImage;
use serde_json::{json, Value};

const DEFAULT_SEED: u64 = 0x5EED_1208_0001_u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SizePreset {
    P480,
    P720,
    P1080,
    K4,
    K8,
    Square1K,
    Square2K,
    Square4K,
    Vertical720,
    Vertical1080,
    Vertical4K,
}

impl SizePreset {
    const DEFAULT: Self = Self::P1080;

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "480p" | "480" | "sd" => Ok(Self::P480),
            "720p" | "720" | "hd" => Ok(Self::P720),
            "1080p" | "1080" | "fullhd" | "full-hd" => Ok(Self::P1080),
            "4k" | "2160p" | "2160" | "uhd" => Ok(Self::K4),
            "8k" | "4320p" | "4320" => Ok(Self::K8),
            "square-1k" | "square1k" | "1k-square" | "square-1024" => Ok(Self::Square1K),
            "square-2k" | "square2k" | "2k-square" | "square-2048" => Ok(Self::Square2K),
            "square-4k" | "square4k" | "4k-square" | "square-4096" => Ok(Self::Square4K),
            "vertical-720p" | "vertical-720" | "portrait-720p" | "portrait-720" => {
                Ok(Self::Vertical720)
            }
            "vertical-1080p" | "vertical-1080" | "portrait-1080p" | "portrait-1080" => {
                Ok(Self::Vertical1080)
            }
            "vertical-4k" | "vertical-2160p" | "portrait-4k" | "portrait-2160p" => {
                Ok(Self::Vertical4K)
            }
            _ => bail!(
                "unknown preset '{value}'; expected one of: 480p, 720p, 1080p, 4k, 8k, square-1k, square-2k, square-4k, vertical-720p, vertical-1080p, vertical-4k"
            ),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::P480 => "480p",
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::K4 => "4k",
            Self::K8 => "8k",
            Self::Square1K => "square-1k",
            Self::Square2K => "square-2k",
            Self::Square4K => "square-4k",
            Self::Vertical720 => "vertical-720p",
            Self::Vertical1080 => "vertical-1080p",
            Self::Vertical4K => "vertical-4k",
        }
    }

    const fn file_suffix(self) -> &'static str {
        match self {
            Self::P1080 => "",
            Self::P480 => "-480p",
            Self::P720 => "-720p",
            Self::K4 => "-4k",
            Self::K8 => "-8k",
            Self::Square1K => "-square-1k",
            Self::Square2K => "-square-2k",
            Self::Square4K => "-square-4k",
            Self::Vertical720 => "-vertical-720p",
            Self::Vertical1080 => "-vertical-1080p",
            Self::Vertical4K => "-vertical-4k",
        }
    }

    const fn dimensions(self) -> OutputDimensions {
        match self {
            Self::P480 => OutputDimensions {
                icon: 480,
                banner: RenderSize {
                    width: 854,
                    height: 480,
                },
                map: RenderSize {
                    width: 960,
                    height: 480,
                },
            },
            Self::P720 => OutputDimensions {
                icon: 720,
                banner: RenderSize {
                    width: 1280,
                    height: 720,
                },
                map: RenderSize {
                    width: 1440,
                    height: 720,
                },
            },
            Self::P1080 => OutputDimensions {
                icon: 1080,
                banner: RenderSize {
                    width: 1920,
                    height: 1080,
                },
                map: RenderSize {
                    width: 2160,
                    height: 1080,
                },
            },
            Self::K4 => OutputDimensions {
                icon: 2160,
                banner: RenderSize {
                    width: 3840,
                    height: 2160,
                },
                map: RenderSize {
                    width: 4320,
                    height: 2160,
                },
            },
            Self::K8 => OutputDimensions {
                icon: 4320,
                banner: RenderSize {
                    width: 7680,
                    height: 4320,
                },
                map: RenderSize {
                    width: 8640,
                    height: 4320,
                },
            },
            Self::Square1K => OutputDimensions {
                icon: 1024,
                banner: RenderSize {
                    width: 1024,
                    height: 1024,
                },
                map: RenderSize {
                    width: 2048,
                    height: 1024,
                },
            },
            Self::Square2K => OutputDimensions {
                icon: 2048,
                banner: RenderSize {
                    width: 2048,
                    height: 2048,
                },
                map: RenderSize {
                    width: 4096,
                    height: 2048,
                },
            },
            Self::Square4K => OutputDimensions {
                icon: 4096,
                banner: RenderSize {
                    width: 4096,
                    height: 4096,
                },
                map: RenderSize {
                    width: 8192,
                    height: 4096,
                },
            },
            Self::Vertical720 => OutputDimensions {
                icon: 720,
                banner: RenderSize {
                    width: 720,
                    height: 1280,
                },
                map: RenderSize {
                    width: 1440,
                    height: 720,
                },
            },
            Self::Vertical1080 => OutputDimensions {
                icon: 1080,
                banner: RenderSize {
                    width: 1080,
                    height: 1920,
                },
                map: RenderSize {
                    width: 2160,
                    height: 1080,
                },
            },
            Self::Vertical4K => OutputDimensions {
                icon: 2160,
                banner: RenderSize {
                    width: 2160,
                    height: 3840,
                },
                map: RenderSize {
                    width: 4320,
                    height: 2160,
                },
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OutputDimensions {
    icon: u32,
    banner: RenderSize,
    map: RenderSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QualityPreset {
    Preview,
    Standard,
    Ultra,
}

impl QualityPreset {
    const DEFAULT: Self = Self::Standard;

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "preview" | "draft" | "fast" => Ok(Self::Preview),
            "standard" | "normal" => Ok(Self::Standard),
            "ultra" | "high" | "archive" => Ok(Self::Ultra),
            _ => bail!("unknown quality '{value}'; expected one of: preview, standard, ultra"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Standard => "standard",
            Self::Ultra => "ultra",
        }
    }

    const fn options(self) -> RenderOptions {
        match self {
            Self::Preview => RenderOptions::preview(),
            Self::Standard => RenderOptions::standard(),
            Self::Ultra => RenderOptions::ultra(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RendererMode {
    Raster,
    Hybrid,
    Raytrace,
}

impl RendererMode {
    const DEFAULT: Self = Self::Hybrid;

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "raster" | "fast" => Ok(Self::Raster),
            "hybrid" | "main" | "default" => Ok(Self::Hybrid),
            "raytrace" | "pathtrace" | "cpu-raytrace" | "cpu-pathtrace" => Ok(Self::Raytrace),
            _ => bail!("unknown renderer '{value}'; expected one of: raster, hybrid, raytrace"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Raster => "raster",
            Self::Hybrid => "hybrid",
            Self::Raytrace => "raytrace",
        }
    }

    const fn uses_raytraced_icons(self) -> bool {
        matches!(self, Self::Hybrid | Self::Raytrace)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProgressMode {
    Human,
    Json,
    Quiet,
}

impl ProgressMode {
    const DEFAULT: Self = Self::Human;

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "human" | "text" => Ok(Self::Human),
            "json" | "ndjson" => Ok(Self::Json),
            "quiet" | "silent" | "none" => Ok(Self::Quiet),
            _ => bail!("unknown progress mode '{value}'; expected one of: human, json, quiet"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Quiet => "quiet",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CliOptions {
    preset: SizePreset,
    quality: QualityPreset,
    renderer: RendererMode,
    backend_preference: RenderBackendPreference,
    allow_cpu_fallback: bool,
    supersample: Option<u32>,
    planet_size: PlanetSize,
    seed: u64,
    snapshot_time_days: f32,
    archetype: Option<String>,
    output_dir: Option<PathBuf>,
    emit_material_maps: bool,
    emit_physics_maps: bool,
    emit_manifest: bool,
    emit_raytrace_preview: bool,
    trace_size: u32,
    trace_samples: Option<u32>,
    progress: ProgressMode,
    threads: Option<usize>,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            preset: SizePreset::DEFAULT,
            quality: QualityPreset::DEFAULT,
            renderer: RendererMode::DEFAULT,
            backend_preference: RenderBackendPreference::Automatic,
            allow_cpu_fallback: true,
            supersample: None,
            planet_size: PlanetSize::DEFAULT,
            seed: DEFAULT_SEED,
            snapshot_time_days: 0.0,
            archetype: None,
            output_dir: None,
            emit_material_maps: false,
            emit_physics_maps: false,
            emit_manifest: false,
            emit_raytrace_preview: false,
            trace_size: 192,
            trace_samples: None,
            progress: ProgressMode::DEFAULT,
            threads: None,
        }
    }
}

impl CliOptions {
    fn execution_mode(&self) -> RenderExecutionMode {
        match self.threads {
            Some(threads) => RenderExecutionMode::MultiThreaded { threads },
            None => RenderExecutionMode::Automatic,
        }
    }

    fn render_options(&self) -> RenderOptions {
        let mut options = self.quality.options();
        if let Some(supersample) = self.supersample {
            options.supersample = supersample;
        }
        options
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanetSize {
    Small,
    Medium,
    Large,
}

impl PlanetSize {
    const DEFAULT: Self = Self::Medium;

    fn parse(value: &str) -> anyhow::Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "small" | "s" => Ok(Self::Small),
            "medium" | "m" | "default" => Ok(Self::Medium),
            "large" | "l" => Ok(Self::Large),
            _ => bail!("unknown planet size '{value}'; expected one of: small, medium, large"),
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    const fn file_suffix(self) -> &'static str {
        match self {
            Self::Small => "-small",
            Self::Medium => "",
            Self::Large => "-large",
        }
    }

    const fn radius_ratio(self) -> (i64, i64) {
        match self {
            Self::Small => (3, 5),
            Self::Medium => (1, 1),
            Self::Large => (8, 5),
        }
    }

    fn apply_to_profile(self, profile: &mut PlanetVisualProfile) {
        let (numerator, denominator) = self.radius_ratio();
        let radius = i64::from(profile.radius_km);
        profile.radius_km = ((radius * numerator + denominator / 2) / denominator)
            .clamp(1, i64::from(i32::MAX)) as i32;
    }
}

#[derive(Debug, Clone)]
struct OutputFile {
    kind: &'static str,
    file_name: String,
    path: PathBuf,
}

#[derive(Debug)]
struct ProgressReporter {
    mode: ProgressMode,
    last_human_marker: Option<HumanProgressMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HumanProgressMarker {
    output: String,
    phase: RenderPhase,
    bucket: u32,
}

impl ProgressReporter {
    fn new(mode: ProgressMode) -> Self {
        Self {
            mode,
            last_human_marker: None,
        }
    }

    fn warning(&mut self, message: &str) {
        match self.mode {
            ProgressMode::Human => eprintln!("warning: {message}"),
            ProgressMode::Json => {
                println!("{}", json!({ "event": "warning", "message": message }));
            }
            ProgressMode::Quiet => {}
        }
    }

    fn backend_resolution(
        &mut self,
        options: &CliOptions,
        backend_report: RenderBackendConfigurationReport,
    ) {
        match self.mode {
            ProgressMode::Human => {
                eprintln!("{}", backend_resolution_summary(options, backend_report))
            }
            ProgressMode::Json => println!(
                "{}",
                json!({
                    "event": "backend-resolution",
                    "backendPreference": backend_preference_label(options.backend_preference),
                    "allowCpuBackendFallback": options.allow_cpu_fallback,
                    "selectedBackend": configured_backend_label(backend_report.selected),
                    "cpuFallback": backend_report.cpu_fallback,
                    "unavailableReason": backend_report.unavailable_reason,
                    "requestedGpuBackend": backend_report.gpu_status.map(|status| status.kind.key()),
                    "requestedGpuRenderSupported": backend_report
                        .gpu_status
                        .map(|status| status.render_supported),
                    "requestedGpuPathTraceSupported": backend_report
                        .gpu_status
                        .map(|status| status.path_trace_supported),
                    "activeHardwareAccelerated": backend_report.capabilities.hardware_accelerated,
                    "note": backend_resolution_note(backend_report),
                })
            ),
            ProgressMode::Quiet => {}
        }
    }

    fn render_start(&mut self, output: &str, size: RenderSize, path: &Path) {
        match self.mode {
            ProgressMode::Human => eprintln!(
                "rendering {output} {}x{} -> {}",
                size.width,
                size.height,
                path.display()
            ),
            ProgressMode::Json => println!(
                "{}",
                json!({
                    "event": "render-start",
                    "output": output,
                    "width": size.width,
                    "height": size.height,
                    "path": path.display().to_string(),
                })
            ),
            ProgressMode::Quiet => {}
        }
    }

    fn progress(&mut self, output: &str, event: RenderProgressEvent) {
        match self.mode {
            ProgressMode::Human => self.human_progress(output, event),
            ProgressMode::Json => self.json_progress(output, event),
            ProgressMode::Quiet => {}
        }
    }

    fn wrote_file(&mut self, kind: &str, path: &Path) {
        if self.mode == ProgressMode::Json {
            println!(
                "{}",
                json!({
                    "event": "file-written",
                    "kind": kind,
                    "path": path.display().to_string(),
                })
            );
        }
    }

    fn complete(
        &mut self,
        files: &[OutputFile],
        options: &CliOptions,
        dimensions: OutputDimensions,
        backend_report: RenderBackendConfigurationReport,
    ) {
        match self.mode {
            ProgressMode::Human => print_human_summary(files, options, dimensions, backend_report),
            ProgressMode::Json => println!(
                "{}",
                json!({
                    "event": "run-complete",
                    "seed": options.seed,
                    "snapshotTimeDays": options.snapshot_time_days,
                    "preset": options.preset.label(),
                    "quality": options.quality.label(),
                    "renderer": options.renderer.label(),
                    "backendPreference": backend_preference_label(options.backend_preference),
                    "allowCpuBackendFallback": options.allow_cpu_fallback,
                    "selectedBackend": configured_backend_label(backend_report.selected),
                    "selectedBackendCpuFallback": backend_report.cpu_fallback,
                    "selectedBackendUnavailableReason": backend_report.unavailable_reason,
                    "activeHardwareAccelerated": backend_report.capabilities.hardware_accelerated,
                    "planetSize": options.planet_size.label(),
                    "materialMaps": options.emit_material_maps,
                    "physicsMaps": options.emit_physics_maps,
                    "manifest": options.emit_manifest,
                    "files": files
                        .iter()
                        .map(file_json)
                        .collect::<Vec<_>>(),
                })
            ),
            ProgressMode::Quiet => {}
        }
    }

    fn human_progress(&mut self, output: &str, event: RenderProgressEvent) {
        let phase = event.progress.phase;
        let phase_label = render_phase_label(phase);

        if phase == RenderPhase::Complete {
            eprintln!("{output}: complete");
            self.last_human_marker = None;
            return;
        }

        if phase == RenderPhase::Planning {
            if event.progress.completed_pixels == event.progress.total_pixels
                && event.progress.total_pixels > 0
            {
                eprintln!(
                    "{output}: planned {} tile(s), {} pixel(s), {} worker(s)",
                    event.progress.total_tiles, event.progress.total_pixels, event.worker_threads
                );
            }
            return;
        }

        if event.progress.completed_pixels == 0 {
            eprintln!(
                "{output}: {phase_label} started ({} tile(s), {} pixel(s), {} worker(s))",
                event.progress.total_tiles, event.progress.total_pixels, event.worker_threads
            );
            self.last_human_marker = Some(HumanProgressMarker {
                output: output.to_string(),
                phase,
                bucket: 0,
            });
            return;
        }

        let bucket = human_progress_bucket(event.progress);
        if bucket == 0 || !should_emit_human_progress(event.progress) {
            return;
        }

        let marker = HumanProgressMarker {
            output: output.to_string(),
            phase,
            bucket,
        };
        if self.last_human_marker.as_ref() == Some(&marker) {
            return;
        }
        self.last_human_marker = Some(marker);

        let percent = event.progress.fraction() * 100.0;
        match event.tile {
            Some(tile) => eprintln!(
                "{output}: {phase_label} {percent:.1}% - tile {}/{} at {},{} {}x{}; pixels {}/{}",
                event.progress.completed_tiles,
                event.progress.total_tiles,
                tile.x,
                tile.y,
                tile.width,
                tile.height,
                event.progress.completed_pixels,
                event.progress.total_pixels
            ),
            None => eprintln!(
                "{output}: {phase_label} {percent:.1}% - tile {}/{}; pixels {}/{}",
                event.progress.completed_tiles,
                event.progress.total_tiles,
                event.progress.completed_pixels,
                event.progress.total_pixels
            ),
        }
    }

    fn json_progress(&mut self, output: &str, event: RenderProgressEvent) {
        let tile = match event.tile {
            Some(tile) => json!({
                "x": tile.x,
                "y": tile.y,
                "width": tile.width,
                "height": tile.height,
            }),
            None => Value::Null,
        };

        println!(
            "{}",
            json!({
                "event": "render-progress",
                "output": output,
                "phase": render_phase_label(event.progress.phase),
                "completedTiles": event.progress.completed_tiles,
                "totalTiles": event.progress.total_tiles,
                "completedPixels": event.progress.completed_pixels,
                "totalPixels": event.progress.total_pixels,
                "fraction": event.progress.fraction(),
                "percent": event.progress.fraction() * 100.0,
                "executionMode": execution_mode_label(event.execution_mode),
                "workerThreads": event.worker_threads,
                "tile": tile,
            })
        );
    }
}

fn backend_resolution_summary(
    options: &CliOptions,
    backend_report: RenderBackendConfigurationReport,
) -> String {
    let preference = backend_preference_label(options.backend_preference);
    let selected = configured_backend_label(backend_report.selected);

    if backend_report.cpu_fallback {
        let reason = backend_report
            .unavailable_reason
            .unwrap_or("no compiled GPU renderer is available");
        return if matches!(
            options.backend_preference,
            RenderBackendPreference::Automatic
        ) {
            format!(
                "backend: no compiled GPU renderer available; effective {selected} fallback ({reason})"
            )
        } else {
            format!(
                "backend: requested {preference} unavailable; effective {selected} fallback ({reason})"
            )
        };
    }

    if let Some(reason) = backend_report.unavailable_reason {
        return format!(
            "backend: requested {preference} unavailable; strict mode will fail before rendering ({reason})"
        );
    }

    match backend_report.selected {
        ConfiguredRenderBackend::Cpu => "backend: effective cpu renderer".to_string(),
        ConfiguredRenderBackend::Gpu(kind) => {
            format!("backend: effective gpu:{} ({})", kind.key(), kind.label())
        }
    }
}

fn backend_resolution_note(backend_report: RenderBackendConfigurationReport) -> &'static str {
    if backend_report.cpu_fallback {
        "requested GPU backend is unavailable; CPU renderer is active"
    } else if backend_report.unavailable_reason.is_some() {
        "requested GPU backend is unavailable and CPU fallback is disabled"
    } else if matches!(backend_report.selected, ConfiguredRenderBackend::Cpu) {
        "CPU renderer is active"
    } else {
        "GPU renderer is active"
    }
}

fn human_progress_bucket(progress: RenderProgress) -> u32 {
    if progress.completed_tiles == 0 {
        return 0;
    }

    let total_tiles = progress.total_tiles.max(1);
    let completed_tiles = progress.completed_tiles.min(total_tiles);
    let buckets = total_tiles.min(20);
    let numerator = u64::from(completed_tiles) * u64::from(buckets);
    ((numerator + u64::from(total_tiles) - 1) / u64::from(total_tiles)) as u32
}

fn should_emit_human_progress(progress: RenderProgress) -> bool {
    progress.completed_tiles > 0 || progress.completed_pixels >= progress.total_pixels
}

fn main() -> anyhow::Result<()> {
    let options = match parse_cli()? {
        Some(options) => options,
        None => return Ok(()),
    };
    let mut reporter = ProgressReporter::new(options.progress);

    let backend_report = RenderBackendConfiguration {
        preference: options.backend_preference,
        allow_cpu_fallback: options.allow_cpu_fallback,
    }
    .resolve(game_planet_visuals::GpuBackendReport::current());
    reporter.backend_resolution(&options, backend_report);
    if !backend_report.cpu_fallback && backend_report.unavailable_reason.is_some() {
        bail!(
            "requested backend {} is unavailable and --strict-backend disabled CPU fallback: {}. No real CUDA, ROCm/HIP, or wgpu renderer is compiled in this crate yet",
            backend_preference_label(options.backend_preference),
            backend_report
                .unavailable_reason
                .unwrap_or("unknown backend error")
        );
    }

    let mut seed_input = ProfileSeedInput::new(options.seed);
    if let Some(archetype) = &options.archetype {
        seed_input = seed_input.with_archetype_key(archetype.clone());
    }

    let mut profile = PlanetVisualProfile::from_seed_input(seed_input);
    profile.set_snapshot_time_days(options.snapshot_time_days);
    if let Some(archetype) = &options.archetype {
        if profile.archetype_key != *archetype {
            reporter.warning(&format!(
                "--archetype '{archetype}' did not resolve to that profile key; generated archetype is '{}'",
                profile.archetype_key
            ));
        }
    }
    options.planet_size.apply_to_profile(&mut profile);
    let renderer = PlanetRenderer::new(profile.clone());

    let out_dir = match &options.output_dir {
        Some(path) => path.clone(),
        None => default_output_dir()?,
    };
    fs::create_dir_all(&out_dir).with_context(|| format!("creating {}", out_dir.display()))?;

    let suffix = format!(
        "{}{}",
        options.planet_size.file_suffix(),
        options.preset.file_suffix()
    );
    let icon_file = format!("new-terra-rust{suffix}-icon.png");
    let banner_file = format!("new-terra-rust{suffix}-overview-banner.png");
    let night_icon_file = format!("new-terra-rust{suffix}-night-icon.png");
    let night_banner_file = format!("new-terra-rust{suffix}-night-overview-banner.png");
    let surface_file = format!("new-terra-rust{suffix}-surface-map.png");
    let reflection_file = format!("new-terra-rust{suffix}-reflection-map.png");
    let normal_file = format!("new-terra-rust{suffix}-normal-map.png");
    let height_file = format!("new-terra-rust{suffix}-height-map.png");
    let vegetation_file = format!("new-terra-rust{suffix}-vegetation-map.png");
    let roughness_file = format!("new-terra-rust{suffix}-roughness-map.png");
    let physics_file = format!("new-terra-rust{suffix}-physics-map.png");
    let density_file = format!("new-terra-rust{suffix}-density-map.png");
    let raytrace_file = format!("new-terra-rust{suffix}-raytrace-preview.png");
    let profile_file = format!("new-terra-rust{suffix}-profile.json");
    let manifest_file = format!("new-terra-rust{suffix}-manifest.json");
    let preview_file = format!("preview-rust{suffix}.html");
    let icon_path = out_dir.join(&icon_file);
    let banner_path = out_dir.join(&banner_file);
    let night_icon_path = out_dir.join(&night_icon_file);
    let night_banner_path = out_dir.join(&night_banner_file);
    let surface_path = out_dir.join(&surface_file);
    let reflection_path = out_dir.join(&reflection_file);
    let normal_path = out_dir.join(&normal_file);
    let height_path = out_dir.join(&height_file);
    let vegetation_path = out_dir.join(&vegetation_file);
    let roughness_path = out_dir.join(&roughness_file);
    let physics_path = out_dir.join(&physics_file);
    let density_path = out_dir.join(&density_file);
    let raytrace_path = out_dir.join(&raytrace_file);
    let profile_path = out_dir.join(&profile_file);
    let manifest_path = out_dir.join(&manifest_file);
    let preview_path = out_dir.join(&preview_file);
    let dimensions = options.preset.dimensions();
    let quality = options.render_options();
    let execution_mode = options.execution_mode();
    let mut files = Vec::new();
    let mut trace_preview = None;

    reporter.render_start(
        "icon",
        RenderSize {
            width: dimensions.icon,
            height: dimensions.icon,
        },
        &icon_path,
    );
    let (icon, icon_renderer) = render_icon_image(
        &renderer,
        &options,
        quality,
        execution_mode,
        dimensions.icon,
        false,
        &mut reporter,
    )?;
    icon.save(&icon_path)
        .with_context(|| format!("writing {}", icon_path.display()))?;
    record_file(&mut files, "icon", icon_file.clone(), icon_path.clone());
    reporter.wrote_file("icon", &icon_path);

    reporter.render_start(
        "night-icon",
        RenderSize {
            width: dimensions.icon,
            height: dimensions.icon,
        },
        &night_icon_path,
    );
    let (night_icon, night_icon_renderer) = render_icon_image(
        &renderer,
        &options,
        quality,
        execution_mode,
        dimensions.icon,
        true,
        &mut reporter,
    )?;
    night_icon
        .save(&night_icon_path)
        .with_context(|| format!("writing {}", night_icon_path.display()))?;
    record_file(
        &mut files,
        "night-icon",
        night_icon_file.clone(),
        night_icon_path.clone(),
    );
    reporter.wrote_file("night-icon", &night_icon_path);

    reporter.render_start("banner", dimensions.banner, &banner_path);
    renderer
        .render_banner_with_progress(dimensions.banner, quality, execution_mode, |event| {
            reporter.progress("banner", event);
        })
        .save(&banner_path)
        .with_context(|| format!("writing {}", banner_path.display()))?;
    record_file(
        &mut files,
        "banner",
        banner_file.clone(),
        banner_path.clone(),
    );
    reporter.wrote_file("banner", &banner_path);

    reporter.render_start("night-banner", dimensions.banner, &night_banner_path);
    renderer
        .render_night_banner_with_progress(dimensions.banner, quality, execution_mode, |event| {
            reporter.progress("night-banner", event);
        })
        .save(&night_banner_path)
        .with_context(|| format!("writing {}", night_banner_path.display()))?;
    record_file(
        &mut files,
        "night-banner",
        night_banner_file.clone(),
        night_banner_path.clone(),
    );
    reporter.wrote_file("night-banner", &night_banner_path);

    reporter.render_start("surface-map", dimensions.map, &surface_path);
    renderer
        .render_surface_map_with_progress(dimensions.map, execution_mode, |event| {
            reporter.progress("surface-map", event);
        })
        .save(&surface_path)
        .with_context(|| format!("writing {}", surface_path.display()))?;
    record_file(
        &mut files,
        "surface-map",
        surface_file.clone(),
        surface_path.clone(),
    );
    reporter.wrote_file("surface-map", &surface_path);

    reporter.render_start("reflection-map", dimensions.map, &reflection_path);
    renderer
        .render_reflection_map_with_progress(dimensions.map, execution_mode, |event| {
            reporter.progress("reflection-map", event);
        })
        .save(&reflection_path)
        .with_context(|| format!("writing {}", reflection_path.display()))?;
    record_file(
        &mut files,
        "reflection-map",
        reflection_file.clone(),
        reflection_path.clone(),
    );
    reporter.wrote_file("reflection-map", &reflection_path);

    if options.emit_material_maps {
        reporter.render_start("normal-map", dimensions.map, &normal_path);
        renderer
            .render_normal_map_with_progress(dimensions.map, execution_mode, |event| {
                reporter.progress("normal-map", event);
            })
            .save(&normal_path)
            .with_context(|| format!("writing {}", normal_path.display()))?;
        record_file(&mut files, "normal-map", normal_file, normal_path.clone());
        reporter.wrote_file("normal-map", &normal_path);

        reporter.render_start("height-map", dimensions.map, &height_path);
        renderer
            .render_height_map_with_progress(dimensions.map, execution_mode, |event| {
                reporter.progress("height-map", event);
            })
            .save(&height_path)
            .with_context(|| format!("writing {}", height_path.display()))?;
        record_file(&mut files, "height-map", height_file, height_path.clone());
        reporter.wrote_file("height-map", &height_path);

        reporter.render_start("vegetation-map", dimensions.map, &vegetation_path);
        renderer
            .render_vegetation_map_with_progress(dimensions.map, execution_mode, |event| {
                reporter.progress("vegetation-map", event);
            })
            .save(&vegetation_path)
            .with_context(|| format!("writing {}", vegetation_path.display()))?;
        record_file(
            &mut files,
            "vegetation-map",
            vegetation_file,
            vegetation_path.clone(),
        );
        reporter.wrote_file("vegetation-map", &vegetation_path);

        reporter.render_start("roughness-map", dimensions.map, &roughness_path);
        renderer
            .render_roughness_map_with_progress(dimensions.map, execution_mode, |event| {
                reporter.progress("roughness-map", event);
            })
            .save(&roughness_path)
            .with_context(|| format!("writing {}", roughness_path.display()))?;
        record_file(
            &mut files,
            "roughness-map",
            roughness_file,
            roughness_path.clone(),
        );
        reporter.wrote_file("roughness-map", &roughness_path);
    }

    if options.emit_physics_maps {
        reporter.render_start("physics-map", dimensions.map, &physics_path);
        renderer
            .render_physics_map_with_progress(dimensions.map, execution_mode, |event| {
                reporter.progress("physics-map", event);
            })
            .save(&physics_path)
            .with_context(|| format!("writing {}", physics_path.display()))?;
        record_file(
            &mut files,
            "physics-map",
            physics_file,
            physics_path.clone(),
        );
        reporter.wrote_file("physics-map", &physics_path);

        reporter.render_start("density-map", dimensions.map, &density_path);
        renderer
            .render_density_map_with_progress(dimensions.map, execution_mode, |event| {
                reporter.progress("density-map", event);
            })
            .save(&density_path)
            .with_context(|| format!("writing {}", density_path.display()))?;
        record_file(
            &mut files,
            "density-map",
            density_file,
            density_path.clone(),
        );
        reporter.wrote_file("density-map", &density_path);
    }

    if options.emit_raytrace_preview {
        let trace_size = RenderSize {
            width: options.trace_size,
            height: options.trace_size,
        };
        reporter.render_start("raytrace-preview", trace_size, &raytrace_path);
        let trace_image = render_cpu_trace_preview(&profile, &options, &mut reporter)?;
        trace_image_to_rgba(&trace_image)
            .save(&raytrace_path)
            .with_context(|| format!("writing {}", raytrace_path.display()))?;
        trace_preview = Some(trace_preview_json(&trace_image, &options));
        record_file(
            &mut files,
            "raytrace-preview",
            raytrace_file,
            raytrace_path.clone(),
        );
        reporter.wrote_file("raytrace-preview", &raytrace_path);
    }

    fs::write(&profile_path, serde_json::to_string_pretty(&profile)?)
        .with_context(|| format!("writing {}", profile_path.display()))?;
    record_file(&mut files, "profile", profile_file, profile_path.clone());
    reporter.wrote_file("profile", &profile_path);

    fs::write(
        &preview_path,
        preview_html(
            &icon_file,
            &banner_file,
            &night_icon_file,
            &night_banner_file,
            &surface_file,
            &reflection_file,
            &profile,
            options.preset,
            options.quality,
            options.planet_size,
            options.renderer,
            dimensions,
        ),
    )
    .with_context(|| format!("writing {}", preview_path.display()))?;
    record_file(&mut files, "preview", preview_file, preview_path.clone());
    reporter.wrote_file("preview", &preview_path);

    if options.emit_manifest {
        let manifest = manifest_json(
            &options,
            &profile,
            dimensions,
            execution_mode,
            &out_dir,
            &manifest_file,
            &files,
            icon_renderer,
            night_icon_renderer,
            trace_preview.as_ref(),
        )?;
        fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)
            .with_context(|| format!("writing {}", manifest_path.display()))?;
        record_file(&mut files, "manifest", manifest_file, manifest_path.clone());
        reporter.wrote_file("manifest", &manifest_path);
    }

    reporter.complete(&files, &options, dimensions, backend_report);

    Ok(())
}

fn parse_cli() -> anyhow::Result<Option<CliOptions>> {
    let mut options = CliOptions::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "--preset" | "--size" => {
                let value = take_value(&args, &mut index, arg)?;
                options.preset = SizePreset::parse(value)?;
            }
            "--quality" => {
                let value = take_value(&args, &mut index, arg)?;
                options.quality = QualityPreset::parse(value)?;
            }
            "--renderer" => {
                let value = take_value(&args, &mut index, arg)?;
                options.renderer = RendererMode::parse(value)?;
            }
            "--backend" | "--render-backend" => {
                let value = take_value(&args, &mut index, arg)?;
                options.backend_preference = parse_backend_preference(value)?;
            }
            "--strict-backend" => {
                options.allow_cpu_fallback = false;
            }
            "--supersample" => {
                let value = take_value(&args, &mut index, arg)?;
                options.supersample = Some(parse_supersample(value)?);
            }
            "--planet-size" => {
                let value = take_value(&args, &mut index, arg)?;
                options.planet_size = PlanetSize::parse(value)?;
            }
            "--seed" => {
                let value = take_value(&args, &mut index, arg)?;
                options.seed = parse_seed(value)?;
            }
            "--time-days" | "--snapshot-days" | "--time" => {
                let value = take_value(&args, &mut index, arg)?;
                options.snapshot_time_days = parse_snapshot_time_days(value)?;
            }
            "--archetype" => {
                let value = take_value(&args, &mut index, arg)?;
                options.archetype = Some(parse_nonempty_string("--archetype", value)?);
            }
            "--output-dir" => {
                let value = take_value(&args, &mut index, arg)?;
                options.output_dir = Some(parse_output_dir(value)?);
            }
            "--emit-material-maps" => {
                options.emit_material_maps = true;
            }
            "--emit-physics-maps" => {
                options.emit_physics_maps = true;
            }
            "--emit-manifest" => {
                options.emit_manifest = true;
            }
            "--emit-raytrace-preview" => {
                options.emit_raytrace_preview = true;
            }
            "--progress" => {
                let value = take_value(&args, &mut index, arg)?;
                options.progress = ProgressMode::parse(value)?;
            }
            "--trace-size" => {
                let value = take_value(&args, &mut index, arg)?;
                options.trace_size = parse_trace_size(value)?;
            }
            "--trace-samples" => {
                let value = take_value(&args, &mut index, arg)?;
                options.trace_samples = Some(parse_trace_samples(value)?);
            }
            "--threads" => {
                let value = take_value(&args, &mut index, arg)?;
                options.threads = Some(parse_threads(value)?);
            }
            _ if arg.starts_with("--preset=") => {
                options.preset = SizePreset::parse(&arg["--preset=".len()..])?;
            }
            _ if arg.starts_with("--size=") => {
                options.preset = SizePreset::parse(&arg["--size=".len()..])?;
            }
            _ if arg.starts_with("--quality=") => {
                options.quality = QualityPreset::parse(&arg["--quality=".len()..])?;
            }
            _ if arg.starts_with("--renderer=") => {
                options.renderer = RendererMode::parse(&arg["--renderer=".len()..])?;
            }
            _ if arg.starts_with("--backend=") => {
                options.backend_preference = parse_backend_preference(&arg["--backend=".len()..])?;
            }
            _ if arg.starts_with("--render-backend=") => {
                options.backend_preference =
                    parse_backend_preference(&arg["--render-backend=".len()..])?;
            }
            _ if arg.starts_with("--supersample=") => {
                options.supersample = Some(parse_supersample(&arg["--supersample=".len()..])?);
            }
            _ if arg.starts_with("--planet-size=") => {
                options.planet_size = PlanetSize::parse(&arg["--planet-size=".len()..])?;
            }
            _ if arg.starts_with("--seed=") => {
                options.seed = parse_seed(&arg["--seed=".len()..])?;
            }
            _ if arg.starts_with("--time-days=") => {
                options.snapshot_time_days =
                    parse_snapshot_time_days(&arg["--time-days=".len()..])?;
            }
            _ if arg.starts_with("--snapshot-days=") => {
                options.snapshot_time_days =
                    parse_snapshot_time_days(&arg["--snapshot-days=".len()..])?;
            }
            _ if arg.starts_with("--time=") => {
                options.snapshot_time_days = parse_snapshot_time_days(&arg["--time=".len()..])?;
            }
            _ if arg.starts_with("--archetype=") => {
                options.archetype = Some(parse_nonempty_string(
                    "--archetype",
                    &arg["--archetype=".len()..],
                )?);
            }
            _ if arg.starts_with("--output-dir=") => {
                options.output_dir = Some(parse_output_dir(&arg["--output-dir=".len()..])?);
            }
            _ if arg.starts_with("--progress=") => {
                options.progress = ProgressMode::parse(&arg["--progress=".len()..])?;
            }
            _ if arg.starts_with("--trace-size=") => {
                options.trace_size = parse_trace_size(&arg["--trace-size=".len()..])?;
            }
            _ if arg.starts_with("--trace-samples=") => {
                options.trace_samples =
                    Some(parse_trace_samples(&arg["--trace-samples=".len()..])?);
            }
            _ if arg.starts_with("--threads=") => {
                options.threads = Some(parse_threads(&arg["--threads=".len()..])?);
            }
            _ => {
                bail!("unknown argument '{arg}'; run with --help for usage");
            }
        }
        index += 1;
    }

    Ok(Some(options))
}

fn print_help() {
    println!(
        r#"Render the New Terra Rust planet prototype.

Usage:
  cargo run -p game-planet-visuals --bin render_planet -- [options]

Defaults:
  --preset 1080p --quality standard --renderer hybrid --planet-size medium --seed 0x5EED_1208_0001 --progress human

Options:
  --preset, --size <name>                 Output size preset.
  --quality <preview|standard|ultra>      Renderer quality preset.
  --renderer <raster|hybrid|raytrace>     Main renderer. Hybrid/raytrace use CPU path tracing for planet icons.
  --backend <auto|cpu|wgpu|cuda|rocm>     Backend preference. GPU options report unavailable capability and fall back to CPU unless strict.
  --strict-backend                        Fail instead of falling back when a requested GPU backend is unavailable.
  --supersample <1..4>                    Override quality supersampling before native safety caps.
  --planet-size <small|medium|large>      Physical planet scale metadata.
  --seed <u64|0xhex>                      Deterministic profile seed.
  --time-days <days>                      Advance the seeded planet simulation by this many days for currents, clouds, waves, densities, and magnetism.
  --archetype <key>                       Request a catalog archetype key for profile generation.
  --output-dir <path>                     Output directory. Defaults to assets/planet-rust-prototype under the workspace root.
  --emit-material-maps                    Also emit normal, height, vegetation, and roughness/wetness maps.
  --emit-physics-maps                     Also emit physics and density maps for currents, clouds, magnetism, and densities.
  --emit-raytrace-preview                 Also emit a bounded CPU path-traced preview image.
  --emit-manifest                         Emit a JSON run manifest next to the generated files.
  --progress <human|json|quiet>           Live progress output mode.
  --trace-size <px>                       Square CPU trace preview size. Default: 192.
  --trace-samples <count>                 CPU trace preview samples per pixel. Default: preview settings.
  --threads <count>                       Request a specific worker count for threaded renderer phases.

Planet sizes:
  small, medium, large
  This is physical planet scale metadata, not output image resolution.

Preset names:
  480p, 720p, 1080p, 4k, 8k
  square-1k, square-2k, square-4k
  vertical-720p, vertical-1080p, vertical-4k

Notes:
  The default medium 1080p render preserves the existing output names.
  Non-default planet sizes and presets add clear filename suffixes, for example new-terra-rust-small-icon.png or new-terra-rust-large-4k-icon.png.
  CUDA, ROCm/HIP, and wgpu rendering are not compiled in this crate yet; GPU backend selections are proof/reporting controls today.
  Relative --output-dir paths are resolved from the current working directory.
  --progress json emits newline-delimited JSON progress and file events to stdout.
  Use --preset 8k explicitly for 7680x4320 banner renders."#
    );
}

fn take_value<'a>(args: &'a [String], index: &mut usize, arg: &str) -> anyhow::Result<&'a str> {
    *index += 1;
    args.get(*index)
        .map(String::as_str)
        .with_context(|| format!("{arg} requires a value"))
}

fn parse_seed(value: &str) -> anyhow::Result<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("--seed requires a non-empty value");
    }

    let normalized = trimmed.replace('_', "");
    if let Some(hex) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).with_context(|| format!("parsing hex seed '{value}' as u64"))
    } else {
        normalized
            .parse::<u64>()
            .with_context(|| format!("parsing seed '{value}' as u64"))
    }
}

fn parse_threads(value: &str) -> anyhow::Result<usize> {
    let threads = value
        .trim()
        .parse::<usize>()
        .with_context(|| format!("parsing --threads '{value}' as a positive integer"))?;
    if threads == 0 {
        bail!("--threads must be at least 1");
    }
    Ok(threads)
}

fn parse_trace_size(value: &str) -> anyhow::Result<u32> {
    let size = value
        .trim()
        .parse::<u32>()
        .with_context(|| format!("parsing --trace-size '{value}' as a positive integer"))?;
    if size == 0 {
        bail!("--trace-size must be at least 1");
    }
    let pixels = u64::from(size) * u64::from(size);
    if pixels > TraceImage::MAX_PIXELS {
        bail!(
            "--trace-size {size} would render {pixels} pixels, above the CPU trace preview limit of {}",
            TraceImage::MAX_PIXELS
        );
    }
    Ok(size)
}

fn parse_trace_samples(value: &str) -> anyhow::Result<u32> {
    let samples = value
        .trim()
        .parse::<u32>()
        .with_context(|| format!("parsing --trace-samples '{value}' as a positive integer"))?;
    if samples == 0 {
        bail!("--trace-samples must be at least 1");
    }
    Ok(samples)
}

fn parse_snapshot_time_days(value: &str) -> anyhow::Result<f32> {
    let days = value
        .trim()
        .parse::<f32>()
        .with_context(|| format!("parsing --time-days '{value}' as a finite day offset"))?;
    if !days.is_finite() {
        bail!("--time-days must be finite");
    }
    if days.abs() > 10_000_000.0 {
        bail!("--time-days must be between -10000000 and 10000000");
    }
    Ok(days)
}

fn parse_backend_preference(value: &str) -> anyhow::Result<RenderBackendPreference> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" | "automatic" => Ok(RenderBackendPreference::Automatic),
        "cpu" => Ok(RenderBackendPreference::Cpu),
        "gpu" | "wgpu" | "open-standard" | "open-standard-gpu" | "vulkan" | "dx12" => {
            Ok(RenderBackendPreference::OpenStandardGpu)
        }
        "cuda" | "nvidia" => Ok(RenderBackendPreference::Cuda),
        "rocm" | "hip" | "amd" | "amd-rocm" => Ok(RenderBackendPreference::AmdRocm),
        _ => bail!("unknown backend '{value}'; expected one of: auto, cpu, wgpu, cuda, rocm"),
    }
}

fn backend_preference_label(preference: RenderBackendPreference) -> &'static str {
    match preference {
        RenderBackendPreference::Cpu => "cpu",
        RenderBackendPreference::Automatic => "automatic",
        RenderBackendPreference::OpenStandardGpu => "wgpu",
        RenderBackendPreference::Cuda => "cuda",
        RenderBackendPreference::AmdRocm => "rocm-hip",
    }
}

fn configured_backend_label(backend: ConfiguredRenderBackend) -> String {
    match backend {
        ConfiguredRenderBackend::Cpu => "cpu".to_string(),
        ConfiguredRenderBackend::Gpu(kind) => format!("gpu:{}", kind.key()),
    }
}

fn parse_supersample(value: &str) -> anyhow::Result<u32> {
    let supersample = value
        .trim()
        .parse::<u32>()
        .with_context(|| format!("parsing --supersample '{value}' as an integer from 1 to 4"))?;
    if !(1..=4).contains(&supersample) {
        bail!("--supersample must be between 1 and 4");
    }
    Ok(supersample)
}

fn parse_nonempty_string(flag: &str, value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{flag} requires a non-empty value");
    }
    Ok(value.to_string())
}

fn parse_output_dir(value: &str) -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(parse_nonempty_string("--output-dir", value)?))
}

fn default_output_dir() -> anyhow::Result<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .context("crate should live under workspace crates directory")?
        .to_path_buf();
    Ok(root.join("assets").join("planet-rust-prototype"))
}

fn record_file(files: &mut Vec<OutputFile>, kind: &'static str, file_name: String, path: PathBuf) {
    files.push(OutputFile {
        kind,
        file_name,
        path,
    });
}

fn render_icon_image(
    renderer: &PlanetRenderer,
    options: &CliOptions,
    quality: RenderOptions,
    execution_mode: RenderExecutionMode,
    size: u32,
    night: bool,
    reporter: &mut ProgressReporter,
) -> anyhow::Result<(RgbaImage, &'static str)> {
    let output = if night { "night-icon" } else { "icon" };
    if options.renderer.uses_raytraced_icons() {
        let traced = if night {
            renderer.try_render_raytraced_night_icon_with_progress(
                size,
                quality,
                execution_mode,
                |event| reporter.progress(output, event),
            )
        } else {
            renderer.try_render_raytraced_icon_with_progress(
                size,
                quality,
                execution_mode,
                |event| reporter.progress(output, event),
            )
        };

        match traced {
            Ok(image) => return Ok((image, "cpu-pathtrace-main-icon")),
            Err(error) if options.renderer == RendererMode::Hybrid => {
                reporter.warning(&format!(
                    "CPU pathtrace icon failed ({error:?}); falling back to raster icon"
                ));
            }
            Err(error) => bail!("CPU pathtrace icon failed: {error:?}"),
        }
    }

    let image = if night {
        renderer.render_night_icon_with_progress(size, quality, execution_mode, |event| {
            reporter.progress(output, event);
        })
    } else {
        renderer.render_icon_with_progress(size, quality, execution_mode, |event| {
            reporter.progress(output, event);
        })
    };
    Ok((image, "raster-procedural-icon"))
}

fn print_human_summary(
    files: &[OutputFile],
    options: &CliOptions,
    dimensions: OutputDimensions,
    backend_report: RenderBackendConfigurationReport,
) {
    println!(
        "seed {:#x}, time {} day(s), planet-size {}, preset {} quality {} renderer {} backend {} effective {}: icon {}x{}, banner {}x{}, maps {}x{}",
        options.seed,
        options.snapshot_time_days,
        options.planet_size.label(),
        options.preset.label(),
        options.quality.label(),
        options.renderer.label(),
        backend_preference_label(options.backend_preference),
        configured_backend_label(backend_report.selected),
        dimensions.icon,
        dimensions.icon,
        dimensions.banner.width,
        dimensions.banner.height,
        dimensions.map.width,
        dimensions.map.height
    );
    let render_options = options.render_options();
    let icon_scale = render_options.icon_supersample_for_size(RenderSize {
        width: dimensions.icon,
        height: dimensions.icon,
    });
    let banner_scale = render_options.native_supersample_for_size(dimensions.banner);
    println!(
        "supersampling request {}x, effective icon {}x, banner {}x",
        render_options.supersample, icon_scale, banner_scale
    );
    if let Some(archetype) = &options.archetype {
        println!("requested archetype {archetype}");
    }
    if let Some(threads) = options.threads {
        println!("requested {} render worker thread(s)", threads);
    }
    if backend_report.cpu_fallback {
        println!(
            "CPU backend fallback active: {}",
            backend_report
                .unavailable_reason
                .unwrap_or("requested GPU backend is unavailable")
        );
    }
    if options.renderer.uses_raytraced_icons() {
        println!("main icon renderer uses CPU path tracing");
    }
    if options.emit_material_maps {
        println!("emitted material maps");
    }
    if options.emit_physics_maps {
        println!("emitted physics maps");
    }
    if options.emit_raytrace_preview {
        println!(
            "emitted CPU raytrace preview {}x{}",
            options.trace_size, options.trace_size
        );
    }
    if options.emit_manifest {
        println!("emitted manifest");
    }
    for file in files {
        println!("wrote {}", file.path.display());
    }
}

fn file_json(file: &OutputFile) -> Value {
    json!({
        "kind": file.kind,
        "fileName": file.file_name,
        "path": file.path.display().to_string(),
    })
}

fn gpu_status_json(status: GpuBackendStatus) -> Value {
    json!({
        "kind": status.kind.key(),
        "family": format!("{:?}", status.family),
        "label": status.label,
        "available": status.render_supported,
        "openStandard": status.open_standard,
        "requiresExternalSdk": status.requires_external_sdk,
        "sdkHintDetected": status.sdk_hint_detected,
        "runtimeHintDetected": status.runtime_hint_detected,
        "crateIntegrationCompiled": status.crate_integration_compiled,
        "renderSupported": status.render_supported,
        "pathTraceSupported": status.path_trace_supported,
        "activeHardwareAccelerated": status.capabilities().hardware_accelerated,
        "implementation": if status.render_supported { "compiled" } else { "unavailable" },
        "readiness": format!("{:?}", status.readiness),
        "unavailableReason": status.unavailable_reason,
    })
}

fn manifest_json(
    options: &CliOptions,
    profile: &PlanetVisualProfile,
    dimensions: OutputDimensions,
    execution_mode: RenderExecutionMode,
    out_dir: &Path,
    manifest_file: &str,
    files: &[OutputFile],
    icon_renderer: &'static str,
    night_icon_renderer: &'static str,
    trace_preview: Option<&Value>,
) -> anyhow::Result<Value> {
    let render_options = options.render_options();
    let gpu_report = game_planet_visuals::GpuBackendReport::current();
    let backend_report = RenderBackendConfiguration {
        preference: options.backend_preference,
        allow_cpu_fallback: options.allow_cpu_fallback,
    }
    .resolve(gpu_report);
    Ok(json!({
        "schema": "universus.planet-render-manifest.v1",
        "renderer": "game-planet-visuals/render_planet",
        "seed": options.seed,
        "requestedArchetype": options.archetype.as_deref(),
        "appliedArchetype": profile.archetype_key.as_str(),
        "archetypeStatus": archetype_status(options, profile),
        "preset": options.preset.label(),
        "quality": options.quality.label(),
        "rendererMode": options.renderer.label(),
        "backendPreference": backend_preference_label(options.backend_preference),
        "allowCpuBackendFallback": options.allow_cpu_fallback,
        "selectedBackend": configured_backend_label(backend_report.selected),
        "selectedBackendCpuFallback": backend_report.cpu_fallback,
        "selectedBackendUnavailableReason": backend_report.unavailable_reason,
        "backendResolution": {
            "backendPreference": backend_preference_label(options.backend_preference),
            "allowCpuBackendFallback": options.allow_cpu_fallback,
            "selectedBackend": configured_backend_label(backend_report.selected),
            "cpuFallback": backend_report.cpu_fallback,
            "unavailableReason": backend_report.unavailable_reason,
            "requestedGpuBackend": backend_report.gpu_status.map(|status| status.kind.key()),
            "requestedGpuRenderSupported": backend_report
                .gpu_status
                .map(|status| status.render_supported),
            "requestedGpuPathTraceSupported": backend_report
                .gpu_status
                .map(|status| status.path_trace_supported),
            "activeHardwareAccelerated": backend_report.capabilities.hardware_accelerated,
            "note": backend_resolution_note(backend_report),
        },
        "selectedBackendCapabilities": {
            "name": backend_report.capabilities.name,
            "hardwareAccelerated": backend_report.capabilities.hardware_accelerated,
            "supportsIcon": backend_report.capabilities.supports_icon,
            "supportsBanner": backend_report.capabilities.supports_banner,
            "supportsSurfaceMap": backend_report.capabilities.supports_surface_map,
            "maxDimension": backend_report.capabilities.max_dimension,
        },
        "mainIconRenderer": icon_renderer,
        "mainNightIconRenderer": night_icon_renderer,
        "gpuBackends": serde_json::to_value(gpu_report.statuses.map(gpu_status_json))?,
        "snapshotTimeDays": profile.snapshot_time_days,
        "qualitySupersample": options.quality.options().supersample,
        "requestedSupersample": options.supersample,
        "renderSupersample": render_options.supersample,
        "effectiveSupersample": {
            "icon": render_options.icon_supersample_for_size(RenderSize {
                width: dimensions.icon,
                height: dimensions.icon,
            }),
            "banner": render_options.native_supersample_for_size(dimensions.banner),
        },
        "planetSize": options.planet_size.label(),
        "progress": options.progress.label(),
        "threadOverride": options.threads,
        "executionMode": execution_mode_label(execution_mode),
        "emitMaterialMaps": options.emit_material_maps,
        "emitPhysicsMaps": options.emit_physics_maps,
        "emitRaytracePreview": options.emit_raytrace_preview,
        "tracePreview": trace_preview.cloned(),
        "physics": serde_json::to_value(PlanetPhysicsModel::from_profile(profile).summary())?,
        "simulation": {
            "timeDays": profile.snapshot_time_days,
            "climate": "deterministic seeded flow/density/magnetism fields",
            "terrainIdentity": "seed-stable; weather, clouds, waves, currents, densities, and magnetism advance with time",
        },
        "outputDir": out_dir.display().to_string(),
        "manifestFile": manifest_file,
        "dimensions": {
            "icon": {
                "width": dimensions.icon,
                "height": dimensions.icon,
            },
            "banner": {
                "width": dimensions.banner.width,
                "height": dimensions.banner.height,
            },
            "nightIcon": {
                "width": dimensions.icon,
                "height": dimensions.icon,
            },
            "nightBanner": {
                "width": dimensions.banner.width,
                "height": dimensions.banner.height,
            },
            "maps": {
                "width": dimensions.map.width,
                "height": dimensions.map.height,
            },
        },
        "profile": serde_json::to_value(profile)?,
        "outputs": files.iter().map(file_json).collect::<Vec<_>>(),
    }))
}

fn render_cpu_trace_preview(
    profile: &PlanetVisualProfile,
    options: &CliOptions,
    reporter: &mut ProgressReporter,
) -> anyhow::Result<TraceImage> {
    let mut settings = PathTraceSettings::preview();
    settings.jitter_seed = options.seed;
    if let Some(samples) = options.trace_samples {
        settings.samples_per_pixel = samples;
    }

    let camera = Camera::look_at(Vec3::new(0.0, 0.0, 3.15), Vec3::ZERO, Vec3::Y, 42.0, 1.0);
    let (scene, surface) = trace_scene_for_profile(profile);
    let kernel = CpuTraceKernel::new_with_surface(scene, surface);
    let plan = kernel
        .plan(options.trace_size, options.trace_size, settings)
        .map_err(|error| anyhow::anyhow!("planning CPU raytrace preview failed: {error:?}"))?;
    if plan.total_pixels > TraceImage::MAX_PIXELS {
        bail!(
            "CPU raytrace preview would render {} pixels, above the preview limit of {}",
            plan.total_pixels,
            TraceImage::MAX_PIXELS
        );
    }

    let pixel_len = (plan.image_width as usize)
        .checked_mul(plan.image_height as usize)
        .context("CPU raytrace preview dimensions overflowed usize")?;
    let mut pixels = vec![Vec3::ZERO; pixel_len];
    let mut stats = TraceStats::default();
    let mut completed_tiles = 0u32;
    let mut completed_pixels = 0u64;

    emit_trace_preview_progress(reporter, &plan, RenderPhase::Planning, 0, 0, None);
    emit_trace_preview_progress(
        reporter,
        &plan,
        RenderPhase::Planning,
        plan.tile_count().min(u32::MAX as usize) as u32,
        plan.total_pixels,
        None,
    );
    emit_trace_preview_progress(reporter, &plan, RenderPhase::Planet, 0, 0, None);

    for tile in plan.tiles.iter().copied() {
        let output = kernel
            .trace_tile(
                camera,
                plan.image_width,
                plan.image_height,
                tile,
                plan.settings,
            )
            .map_err(|error| anyhow::anyhow!("rendering CPU raytrace preview failed: {error:?}"))?;

        for local_y in 0..output.height {
            for local_x in 0..output.width {
                let src_index = local_y as usize * output.width as usize + local_x as usize;
                let dst_x = output.tile.x + local_x;
                let dst_y = output.tile.y + local_y;
                let dst_index = dst_y as usize * plan.image_width as usize + dst_x as usize;
                pixels[dst_index] = output.pixels[src_index];
            }
        }

        stats.merge(output.stats);
        completed_tiles = completed_tiles.saturating_add(1);
        completed_pixels = completed_pixels.saturating_add(output.tile.pixel_count());
        emit_trace_preview_progress(
            reporter,
            &plan,
            RenderPhase::Planet,
            completed_tiles,
            completed_pixels,
            Some(output.tile),
        );
    }

    emit_trace_preview_progress(
        reporter,
        &plan,
        RenderPhase::Planet,
        plan.tile_count().min(u32::MAX as usize) as u32,
        plan.total_pixels,
        None,
    );
    emit_trace_preview_progress(
        reporter,
        &plan,
        RenderPhase::Complete,
        plan.tile_count().min(u32::MAX as usize) as u32,
        plan.total_pixels,
        None,
    );

    Ok(TraceImage {
        width: plan.image_width,
        height: plan.image_height,
        pixels,
        stats,
        plan,
    })
}

fn emit_trace_preview_progress(
    reporter: &mut ProgressReporter,
    plan: &TracePlan,
    phase: RenderPhase,
    completed_tiles: u32,
    completed_pixels: u64,
    tile: Option<Tile>,
) {
    reporter.progress(
        "raytrace-preview",
        RenderProgressEvent {
            progress: RenderProgress {
                phase,
                completed_tiles,
                total_tiles: plan.tile_count().min(u32::MAX as usize) as u32,
                completed_pixels,
                total_pixels: plan.total_pixels,
            },
            tile: tile.map(render_tile_from_trace_tile),
            execution_mode: RenderExecutionMode::Serial,
            worker_threads: 1,
        },
    );
}

fn render_tile_from_trace_tile(tile: Tile) -> RenderTile {
    RenderTile {
        x: tile.x,
        y: tile.y,
        width: tile.width,
        height: tile.height,
    }
}

fn trace_scene_for_profile(profile: &PlanetVisualProfile) -> (TraceScene, TraceSurfaceControls) {
    let gas = profile.render_model.contains("gas") || profile.class_key.contains("gas");
    let ocean = profile.ocean_fraction > 0.70 || profile.class_key == "ocean-world";
    let albedo = if gas {
        Vec3::new(0.90, 0.74, 0.48)
    } else if ocean {
        Vec3::new(0.05, 0.22, 0.62)
    } else {
        Vec3::new(0.32, 0.42, 0.25).lerp(Vec3::new(0.55, 0.42, 0.26), 1.0 - profile.ocean_fraction)
    };
    let roughness = if ocean {
        0.18
    } else if gas {
        0.72
    } else {
        0.46
    };
    let metallic = if ocean { 0.08 } else { 0.0 };

    let light = DistantLight::solar_default();
    let scene = TraceScene {
        planet: Sphere::new(
            Vec3::ZERO,
            1.0,
            MaterialSample {
                albedo,
                roughness,
                metallic,
                transmission: 0.0,
                opacity: 1.0,
                ..MaterialSample::default()
            },
        ),
        atmosphere_radius: 1.055 + profile.atmosphere_density * 0.055,
        atmosphere_density: profile.atmosphere_density.max(0.02),
        light_direction: Vec3::new(light.direction[0], light.direction[1], light.direction[2]),
        sky_color: Vec3::new(0.008, 0.014, 0.040),
        horizon_color: if gas {
            Vec3::new(0.90, 0.72, 0.44)
        } else {
            Vec3::new(0.34, 0.55, 0.95)
        },
    };

    let surface_model = if gas {
        TraceSurfaceModel::BandedGasGiant
    } else if ocean {
        TraceSurfaceModel::Ocean
    } else {
        TraceSurfaceModel::Terrestrial
    };
    let surface = TraceSurfaceControls {
        seed: profile.seed,
        time_days: profile.snapshot_time_days,
        surface_model,
        ocean_fraction: profile.ocean_fraction.clamp(0.0, 1.0),
        band_frequency: if gas { 18.0 } else { 8.0 },
        band_contrast: if gas { 0.95 } else { 0.30 },
        cloud_coverage: if gas {
            0.18
        } else {
            profile.cloud_density.clamp(0.0, 1.0)
        },
        cloud_opacity: if gas {
            0.12
        } else {
            (0.22 + profile.cloud_density * 0.46).clamp(0.0, 0.82)
        },
        atmosphere_color: scene.horizon_color,
        atmosphere_strength: profile.atmosphere_density.clamp(0.0, 1.0),
    };

    (scene, surface)
}

fn trace_image_to_rgba(trace: &TraceImage) -> RgbaImage {
    let mut image = RgbaImage::new(trace.width, trace.height);
    for y in 0..trace.height {
        for x in 0..trace.width {
            let color = trace.pixel_at(x, y).unwrap_or(Vec3::ZERO);
            image.put_pixel(x, y, image::Rgba(tone_map_trace_color(color)));
        }
    }
    image
}

fn tone_map_trace_color(color: Vec3) -> [u8; 4] {
    let mapped = Vec3::new(
        color.x.max(0.0) / (1.0 + color.x.max(0.0)),
        color.y.max(0.0) / (1.0 + color.y.max(0.0)),
        color.z.max(0.0) / (1.0 + color.z.max(0.0)),
    );
    [
        (mapped.x.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8,
        (mapped.y.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8,
        (mapped.z.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8,
        255,
    ]
}

fn trace_preview_json(trace: &TraceImage, options: &CliOptions) -> Value {
    json!({
        "backend": "cpu-pathtrace-preview",
        "width": trace.width,
        "height": trace.height,
        "samplesPerPixel": trace.plan.settings.samples_per_pixel,
        "requestedTraceSize": options.trace_size,
        "requestedTraceSamples": options.trace_samples,
        "tiles": trace.plan.tile_count(),
        "totalPixels": trace.plan.total_pixels,
        "totalSamples": trace.plan.total_samples,
        "stats": {
            "tilesCompleted": trace.stats.tiles_completed,
            "samplesCompleted": trace.stats.samples_completed,
            "raysTraced": trace.stats.rays_traced,
            "primaryRays": trace.stats.primary_rays,
            "shadowRays": trace.stats.shadow_rays,
            "reflectionRays": trace.stats.reflection_rays,
            "refractionRays": trace.stats.refraction_rays,
            "atmosphereSamples": trace.stats.atmosphere_samples,
            "maxBounceDepth": trace.stats.max_bounce_depth,
        }
    })
}

fn archetype_status(options: &CliOptions, profile: &PlanetVisualProfile) -> &'static str {
    match options.archetype.as_deref() {
        Some(requested) if requested == profile.archetype_key => "applied",
        Some(_) => "requested-fell-back",
        None => "seed-selected",
    }
}

fn render_phase_label(phase: RenderPhase) -> &'static str {
    match phase {
        RenderPhase::Planning => "planning",
        RenderPhase::Background => "background",
        RenderPhase::Planet => "planet",
        RenderPhase::Rings => "rings",
        RenderPhase::Moon => "moon",
        RenderPhase::TerrainOverview => "terrain-overview",
        RenderPhase::SurfaceMap => "surface-map",
        RenderPhase::ReflectionMap => "reflection-map",
        RenderPhase::NormalMap => "normal-map",
        RenderPhase::HeightMap => "height-map",
        RenderPhase::VegetationMap => "vegetation-map",
        RenderPhase::RoughnessMap => "roughness-map",
        RenderPhase::PhysicsMap => "physics-map",
        RenderPhase::DensityMap => "density-map",
        RenderPhase::Sharpen => "sharpen",
        RenderPhase::Downscale => "downscale",
        RenderPhase::Complete => "complete",
    }
}

fn execution_mode_label(mode: RenderExecutionMode) -> String {
    match mode {
        RenderExecutionMode::Serial => "serial".to_string(),
        RenderExecutionMode::Automatic => "automatic".to_string(),
        RenderExecutionMode::MultiThreaded { threads } => format!("multi-threaded:{threads}"),
    }
}

fn preview_html(
    icon: &str,
    banner: &str,
    night_icon: &str,
    night_banner: &str,
    surface: &str,
    reflection: &str,
    profile: &PlanetVisualProfile,
    preset: SizePreset,
    quality: QualityPreset,
    planet_size: PlanetSize,
    renderer: RendererMode,
    dimensions: OutputDimensions,
) -> String {
    format!(
        r#"<!doctype html>
<meta charset="utf-8">
<title>Universus Rust Planet Renderer</title>
<style>
body {{
  margin: 0;
  min-height: 100vh;
  background: #050711;
  color: #dbeaf1;
  font-family: Arial, sans-serif;
  display: grid;
  place-items: center;
}}
.wrap {{
  width: min(1220px, calc(100vw - 32px));
  padding: 28px 0;
}}
.banner {{
  position: relative;
  overflow: hidden;
  border: 1px solid rgba(159, 205, 232, .20);
  background: #080b16;
}}
.banner img {{
  display: block;
  width: 100%;
  height: auto;
}}
.banner + .banner {{
  margin-top: 14px;
}}
.caption {{
  position: absolute;
  left: 32px;
  bottom: 28px;
  max-width: 620px;
  text-shadow: 0 2px 18px rgba(0,0,0,.90);
}}
h1 {{
  margin: 0 0 8px;
  font-size: 35px;
  letter-spacing: 0;
}}
p {{
  margin: 0;
  color: #bfd0d8;
}}
.row {{
  display: grid;
  grid-template-columns: 190px 190px 1fr;
  align-items: center;
  gap: 20px;
  margin-top: 18px;
}}
.icon {{
  width: 190px;
  height: 190px;
  background: radial-gradient(circle, rgba(74,145,205,.14), transparent 65%);
}}
.map {{
  width: 100%;
  margin-top: 14px;
  border: 1px solid rgba(159, 205, 232, .16);
}}
.maps {{
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 14px;
  margin-top: 14px;
}}
code {{
  color: #9fd4ff;
}}
</style>
<main class="wrap">
  <section class="banner" aria-label="Generated planet overview banner">
    <img src="{banner}" alt="Procedurally generated overview banner for New Terra">
    <div class="caption">
      <h1>New Terra</h1>
      <p>{planet_class} - {planet_size} planet, {radius_km} km radius - Rust {renderer} renderer - {preset} / {quality} - time {time_days} day(s) - seed <code>{seed}</code></p>
    </div>
  </section>
  <section class="banner" aria-label="Generated night overview banner">
    <img src="{night_banner}" alt="Procedurally generated night overview banner for New Terra">
  </section>
  <div class="row">
    <img class="icon" src="{icon}" alt="Procedurally generated planet icon">
    <img class="icon" src="{night_icon}" alt="Procedurally generated night planet icon">
    <p>Rust renderer: seeded terrain maps, CPU path-traced icon mode, relief normals, cloud shadows, ocean Fresnel/specular, reflection map sampling, atmospheric Rayleigh/Mie approximation, time-evolved currents/clouds/magnetism, city lights, rings, ACES tone mapping, and quality-controlled downscale. Outputs: icon <code>{icon_size}x{icon_size}</code>, banner <code>{banner_width}x{banner_height}</code>, maps <code>{map_width}x{map_height}</code>.</p>
  </div>
  <div class="maps">
    <img class="map" src="{surface}" alt="Generated equirectangular surface texture map">
    <img class="map" src="{reflection}" alt="Generated reflection environment map">
  </div>
</main>
"#,
        banner = banner,
        icon = icon,
        night_banner = night_banner,
        night_icon = night_icon,
        surface = surface,
        reflection = reflection,
        planet_class = profile.planet_class,
        planet_size = planet_size.label(),
        radius_km = profile.radius_km,
        seed = profile.seed,
        time_days = profile.snapshot_time_days,
        preset = preset.label(),
        quality = quality.label(),
        renderer = renderer.label(),
        icon_size = dimensions.icon,
        banner_width = dimensions.banner.width,
        banner_height = dimensions.banner.height,
        map_width = dimensions.map.width,
        map_height = dimensions.map.height
    )
}
