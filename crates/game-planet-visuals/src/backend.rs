use crate::{
    gpu::{GpuBackendKind, GpuBackendReport, GpuBackendStatus},
    pathtrace::{
        Camera, CpuTraceKernel, PathTraceBackend, PathTraceSettings, Tile,
        TraceBackendCapabilities, TraceError, TraceImage, TracePlan, TracePlanError,
        TraceTileOutput,
    },
    validate_render_size, PlanetVisualProfile, RenderSize, RenderSizeValidationError,
    MAX_NATIVE_RENDER_LONG_EDGE,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOutputKind {
    Icon,
    Banner,
    SurfaceMap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderQuality {
    Draft,
    Balanced,
    High,
}

#[derive(Debug, Clone, Copy)]
pub struct RenderRequest<'a> {
    pub profile: &'a PlanetVisualProfile,
    pub size: RenderSize,
    pub output: RenderOutputKind,
    pub quality: RenderQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub name: &'static str,
    pub hardware_accelerated: bool,
    pub supports_icon: bool,
    pub supports_banner: bool,
    pub supports_surface_map: bool,
    pub max_dimension: Option<u32>,
}

pub trait RenderBackend {
    type Output;
    type Error;

    fn capabilities(&self) -> BackendCapabilities;

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackendPreference {
    Cpu,
    Automatic,
    OpenStandardGpu,
    Cuda,
    AmdRocm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderBackendConfiguration {
    pub preference: RenderBackendPreference,
    pub allow_cpu_fallback: bool,
}

impl RenderBackendConfiguration {
    pub const fn cpu() -> Self {
        Self {
            preference: RenderBackendPreference::Cpu,
            allow_cpu_fallback: true,
        }
    }

    pub const fn automatic() -> Self {
        Self {
            preference: RenderBackendPreference::Automatic,
            allow_cpu_fallback: true,
        }
    }

    pub fn resolve(self, gpu_report: GpuBackendReport) -> RenderBackendConfigurationReport {
        match self.preference {
            RenderBackendPreference::Cpu => RenderBackendConfigurationReport {
                selected: ConfiguredRenderBackend::Cpu,
                capabilities: CpuBackend.capabilities(),
                gpu_status: None,
                cpu_fallback: false,
                unavailable_reason: None,
            },
            RenderBackendPreference::Automatic => resolve_automatic_backend(gpu_report),
            RenderBackendPreference::OpenStandardGpu => {
                self.resolve_requested_gpu(gpu_report, GpuBackendKind::Wgpu)
            }
            RenderBackendPreference::Cuda => {
                self.resolve_requested_gpu(gpu_report, GpuBackendKind::Cuda)
            }
            RenderBackendPreference::AmdRocm => {
                self.resolve_requested_gpu(gpu_report, GpuBackendKind::Rocm)
            }
        }
    }

    fn resolve_requested_gpu(
        self,
        gpu_report: GpuBackendReport,
        kind: GpuBackendKind,
    ) -> RenderBackendConfigurationReport {
        let status = gpu_report
            .status(kind)
            .expect("GPU report always includes every known backend kind");
        if status.render_supported {
            return RenderBackendConfigurationReport {
                selected: ConfiguredRenderBackend::Gpu(kind),
                capabilities: status.capabilities(),
                gpu_status: Some(status),
                cpu_fallback: false,
                unavailable_reason: None,
            };
        }

        if self.allow_cpu_fallback {
            return RenderBackendConfigurationReport {
                selected: ConfiguredRenderBackend::Cpu,
                capabilities: CpuBackend.capabilities(),
                gpu_status: Some(status),
                cpu_fallback: true,
                unavailable_reason: Some(status.unavailable_reason),
            };
        }

        RenderBackendConfigurationReport {
            selected: ConfiguredRenderBackend::Gpu(kind),
            capabilities: status.capabilities(),
            gpu_status: Some(status),
            cpu_fallback: false,
            unavailable_reason: Some(status.unavailable_reason),
        }
    }
}

impl Default for RenderBackendConfiguration {
    fn default() -> Self {
        Self::automatic()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfiguredRenderBackend {
    Cpu,
    Gpu(GpuBackendKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderBackendConfigurationReport {
    pub selected: ConfiguredRenderBackend,
    pub capabilities: BackendCapabilities,
    pub gpu_status: Option<GpuBackendStatus>,
    pub cpu_fallback: bool,
    pub unavailable_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuRenderMetadata {
    pub backend: &'static str,
    pub profile_seed: u64,
    pub size: RenderSize,
    pub output: RenderOutputKind,
    pub quality: RenderQuality,
    pub hardware_accelerated: bool,
    pub trace_capabilities: TraceBackendCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuRenderError {
    InvalidSize(RenderSizeValidationError),
    UnsupportedOutput(RenderOutputKind),
}

impl CpuBackend {
    pub const NAME: &'static str = "cpu-planet-renderer";

    pub fn trace_capabilities(&self) -> TraceBackendCapabilities {
        TraceBackendCapabilities::cpu()
    }

    pub fn trace_plan(
        &self,
        image_width: u32,
        image_height: u32,
        settings: PathTraceSettings,
    ) -> Result<TracePlan, TracePlanError> {
        CpuTraceKernel::default().plan(image_width, image_height, settings)
    }

    pub fn trace_tile(
        &self,
        camera: Camera,
        image_width: u32,
        image_height: u32,
        tile: Tile,
        settings: PathTraceSettings,
    ) -> Result<TraceTileOutput, TraceError> {
        CpuTraceKernel::default().trace_tile(camera, image_width, image_height, tile, settings)
    }

    pub fn trace_image(
        &self,
        camera: Camera,
        image_width: u32,
        image_height: u32,
        settings: PathTraceSettings,
    ) -> Result<TraceImage, TraceError> {
        CpuTraceKernel::default().trace_image(camera, image_width, image_height, settings)
    }
}

impl PathTraceBackend for CpuBackend {
    fn trace_capabilities(&self) -> TraceBackendCapabilities {
        CpuBackend::trace_capabilities(self)
    }
}

impl RenderBackend for CpuBackend {
    type Output = CpuRenderMetadata;
    type Error = CpuRenderError;

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            name: Self::NAME,
            hardware_accelerated: false,
            supports_icon: true,
            supports_banner: true,
            supports_surface_map: true,
            max_dimension: Some(MAX_NATIVE_RENDER_LONG_EDGE),
        }
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error> {
        let capabilities = self.capabilities();
        if !supports_output(capabilities, request.output) {
            return Err(CpuRenderError::UnsupportedOutput(request.output));
        }
        validate_render_size(request.size).map_err(CpuRenderError::InvalidSize)?;

        Ok(CpuRenderMetadata {
            backend: capabilities.name,
            profile_seed: request.profile.seed,
            size: request.size,
            output: request.output,
            quality: request.quality,
            hardware_accelerated: capabilities.hardware_accelerated,
            trace_capabilities: self.trace_capabilities(),
        })
    }
}

fn supports_output(capabilities: BackendCapabilities, output: RenderOutputKind) -> bool {
    match output {
        RenderOutputKind::Icon => capabilities.supports_icon,
        RenderOutputKind::Banner => capabilities.supports_banner,
        RenderOutputKind::SurfaceMap => capabilities.supports_surface_map,
    }
}

fn resolve_automatic_backend(gpu_report: GpuBackendReport) -> RenderBackendConfigurationReport {
    if let Some(status) = gpu_report
        .statuses
        .iter()
        .copied()
        .find(|status| status.render_supported)
    {
        return RenderBackendConfigurationReport {
            selected: ConfiguredRenderBackend::Gpu(status.kind),
            capabilities: status.capabilities(),
            gpu_status: Some(status),
            cpu_fallback: false,
            unavailable_reason: None,
        };
    }

    let open_standard_status = gpu_report.open_standard();
    RenderBackendConfigurationReport {
        selected: ConfiguredRenderBackend::Cpu,
        capabilities: CpuBackend.capabilities(),
        gpu_status: Some(open_standard_status),
        cpu_fallback: true,
        unavailable_reason: Some(open_standard_status.unavailable_reason),
    }
}
