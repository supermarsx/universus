use crate::{
    backend::{BackendCapabilities, RenderBackend, RenderOutputKind, RenderQuality, RenderRequest},
    pathtrace::{PathTraceBackend, TraceBackendCapabilities},
    RenderSize,
};
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackendKind {
    Wgpu,
    Cuda,
    Rocm,
}

impl GpuBackendKind {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Wgpu => "wgpu",
            Self::Cuda => "cuda",
            Self::Rocm => "rocm-hip",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Wgpu => "wgpu open-standards GPU",
            Self::Cuda => "NVIDIA CUDA",
            Self::Rocm => "AMD ROCm/HIP",
        }
    }

    pub const fn family(self) -> GpuBackendFamily {
        match self {
            Self::Wgpu => GpuBackendFamily::OpenStandard,
            Self::Cuda => GpuBackendFamily::NvidiaProprietary,
            Self::Rocm => GpuBackendFamily::AmdProprietary,
        }
    }

    pub const fn backend_name(self) -> &'static str {
        match self {
            Self::Wgpu => "wgpu-planet-renderer-unavailable",
            Self::Cuda => "cuda-planet-renderer-unavailable",
            Self::Rocm => "rocm-hip-planet-renderer-unavailable",
        }
    }

    pub const fn unavailable_reason(self) -> &'static str {
        match self {
            Self::Wgpu => WgpuBackend::UNAVAILABLE_REASON,
            Self::Cuda => CudaBackend::UNAVAILABLE_REASON,
            Self::Rocm => RocmBackend::UNAVAILABLE_REASON,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackendFamily {
    OpenStandard,
    NvidiaProprietary,
    AmdProprietary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackendReadiness {
    MissingCrateIntegration,
    HostHintsOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBackendEnvironment {
    pub open_standard_runtime_hint: bool,
    pub cuda_sdk_hint: bool,
    pub cuda_runtime_hint: bool,
    pub rocm_sdk_hint: bool,
    pub rocm_runtime_hint: bool,
}

impl GpuBackendEnvironment {
    pub fn current() -> Self {
        Self {
            open_standard_runtime_hint: has_any_env(&[
                "WGPU_BACKEND",
                "VULKAN_SDK",
                "VK_ICD_FILENAMES",
                "DXIL_PATH",
            ]),
            cuda_sdk_hint: has_any_env(&["CUDA_PATH", "CUDA_HOME", "CUDA_ROOT"]),
            cuda_runtime_hint: has_any_env(&[
                "NVIDIA_VISIBLE_DEVICES",
                "CUDA_VISIBLE_DEVICES",
                "NV_GPU",
            ]),
            rocm_sdk_hint: has_any_env(&["ROCM_PATH", "ROCM_HOME", "HIP_PATH", "HIP_ROOT"]),
            rocm_runtime_hint: has_any_env(&[
                "HIP_VISIBLE_DEVICES",
                "ROCR_VISIBLE_DEVICES",
                "GPU_DEVICE_ORDINAL",
            ]),
        }
    }

    pub const fn empty() -> Self {
        Self {
            open_standard_runtime_hint: false,
            cuda_sdk_hint: false,
            cuda_runtime_hint: false,
            rocm_sdk_hint: false,
            rocm_runtime_hint: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBackendStatus {
    pub kind: GpuBackendKind,
    pub family: GpuBackendFamily,
    pub label: &'static str,
    pub open_standard: bool,
    pub requires_external_sdk: bool,
    pub sdk_hint_detected: bool,
    pub runtime_hint_detected: bool,
    pub crate_integration_compiled: bool,
    pub render_supported: bool,
    pub path_trace_supported: bool,
    pub readiness: GpuBackendReadiness,
    pub unavailable_reason: &'static str,
    pub trace_capabilities: TraceBackendCapabilities,
}

impl GpuBackendStatus {
    pub fn capabilities(self) -> BackendCapabilities {
        BackendCapabilities {
            name: self.kind.backend_name(),
            hardware_accelerated: false,
            supports_icon: false,
            supports_banner: false,
            supports_surface_map: false,
            max_dimension: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBackendReport {
    pub statuses: [GpuBackendStatus; 3],
}

impl GpuBackendReport {
    pub fn current() -> Self {
        Self::from_environment(GpuBackendEnvironment::current())
    }

    pub fn from_environment(environment: GpuBackendEnvironment) -> Self {
        Self {
            statuses: [
                status_for(
                    GpuBackendKind::Wgpu,
                    false,
                    environment.open_standard_runtime_hint,
                ),
                status_for(
                    GpuBackendKind::Cuda,
                    environment.cuda_sdk_hint,
                    environment.cuda_runtime_hint,
                ),
                status_for(
                    GpuBackendKind::Rocm,
                    environment.rocm_sdk_hint,
                    environment.rocm_runtime_hint,
                ),
            ],
        }
    }

    pub fn status(self, kind: GpuBackendKind) -> Option<GpuBackendStatus> {
        self.statuses
            .iter()
            .copied()
            .find(|status| status.kind == kind)
    }

    pub fn open_standard(self) -> GpuBackendStatus {
        self.status(GpuBackendKind::Wgpu)
            .expect("GPU report always includes wgpu status")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBackend {
    pub kind: GpuBackendKind,
}

impl GpuBackend {
    pub const fn new(kind: GpuBackendKind) -> Self {
        Self { kind }
    }

    pub fn status(self, report: GpuBackendReport) -> GpuBackendStatus {
        report
            .status(self.kind)
            .expect("GPU report always includes every known backend kind")
    }

    pub fn trace_capabilities(self) -> TraceBackendCapabilities {
        TraceBackendCapabilities::gpu_unavailable(self.kind.unavailable_reason())
    }
}

impl PathTraceBackend for GpuBackend {
    fn trace_capabilities(&self) -> TraceBackendCapabilities {
        (*self).trace_capabilities()
    }
}

impl RenderBackend for GpuBackend {
    type Output = GpuRenderMetadata;
    type Error = GpuRenderError;

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            name: self.kind.backend_name(),
            hardware_accelerated: false,
            supports_icon: false,
            supports_banner: false,
            supports_surface_map: false,
            max_dimension: None,
        }
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error> {
        Err(GpuRenderError::Unavailable {
            kind: self.kind,
            requested_size: request.size,
            requested_output: request.output,
            requested_quality: request.quality,
            reason: self.kind.unavailable_reason(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuRenderMetadata {
    pub backend: &'static str,
    pub kind: GpuBackendKind,
    pub size: RenderSize,
    pub output: RenderOutputKind,
    pub quality: RenderQuality,
    pub hardware_accelerated: bool,
    pub trace_capabilities: TraceBackendCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuRenderError {
    Unavailable {
        kind: GpuBackendKind,
        requested_size: RenderSize,
        requested_output: RenderOutputKind,
        requested_quality: RenderQuality,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WgpuBackend;

impl WgpuBackend {
    pub const UNAVAILABLE_REASON: &'static str =
        "wgpu path tracing is not available: this crate does not compile a wgpu integration yet";

    pub fn trace_capabilities(&self) -> TraceBackendCapabilities {
        GpuBackend::new(GpuBackendKind::Wgpu).trace_capabilities()
    }

    pub fn status(&self, report: GpuBackendReport) -> GpuBackendStatus {
        GpuBackend::new(GpuBackendKind::Wgpu).status(report)
    }
}

impl PathTraceBackend for WgpuBackend {
    fn trace_capabilities(&self) -> TraceBackendCapabilities {
        WgpuBackend::trace_capabilities(self)
    }
}

impl RenderBackend for WgpuBackend {
    type Output = GpuRenderMetadata;
    type Error = GpuRenderError;

    fn capabilities(&self) -> BackendCapabilities {
        GpuBackend::new(GpuBackendKind::Wgpu).capabilities()
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error> {
        GpuBackend::new(GpuBackendKind::Wgpu).render(request)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CudaBackend;

impl CudaBackend {
    pub const UNAVAILABLE_REASON: &'static str =
        "CUDA path tracing is not available: this crate does not compile a CUDA backend or link CUDA SDKs";

    pub fn trace_capabilities(&self) -> TraceBackendCapabilities {
        GpuBackend::new(GpuBackendKind::Cuda).trace_capabilities()
    }

    pub fn status(&self, report: GpuBackendReport) -> GpuBackendStatus {
        GpuBackend::new(GpuBackendKind::Cuda).status(report)
    }
}

impl PathTraceBackend for CudaBackend {
    fn trace_capabilities(&self) -> TraceBackendCapabilities {
        CudaBackend::trace_capabilities(self)
    }
}

impl RenderBackend for CudaBackend {
    type Output = GpuRenderMetadata;
    type Error = GpuRenderError;

    fn capabilities(&self) -> BackendCapabilities {
        GpuBackend::new(GpuBackendKind::Cuda).capabilities()
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error> {
        GpuBackend::new(GpuBackendKind::Cuda).render(request)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RocmBackend;

impl RocmBackend {
    pub const UNAVAILABLE_REASON: &'static str =
        "ROCm/HIP path tracing is not available: this crate does not compile a ROCm/HIP backend or link ROCm SDKs";

    pub fn trace_capabilities(&self) -> TraceBackendCapabilities {
        GpuBackend::new(GpuBackendKind::Rocm).trace_capabilities()
    }

    pub fn status(&self, report: GpuBackendReport) -> GpuBackendStatus {
        GpuBackend::new(GpuBackendKind::Rocm).status(report)
    }
}

impl PathTraceBackend for RocmBackend {
    fn trace_capabilities(&self) -> TraceBackendCapabilities {
        RocmBackend::trace_capabilities(self)
    }
}

impl RenderBackend for RocmBackend {
    type Output = GpuRenderMetadata;
    type Error = GpuRenderError;

    fn capabilities(&self) -> BackendCapabilities {
        GpuBackend::new(GpuBackendKind::Rocm).capabilities()
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error> {
        GpuBackend::new(GpuBackendKind::Rocm).render(request)
    }
}

fn status_for(
    kind: GpuBackendKind,
    sdk_hint_detected: bool,
    runtime_hint_detected: bool,
) -> GpuBackendStatus {
    let crate_integration_compiled = false;
    let render_supported = false;
    let path_trace_supported = false;
    let readiness = if sdk_hint_detected || runtime_hint_detected {
        GpuBackendReadiness::HostHintsOnly
    } else {
        GpuBackendReadiness::MissingCrateIntegration
    };

    GpuBackendStatus {
        kind,
        family: kind.family(),
        label: kind.label(),
        open_standard: matches!(kind.family(), GpuBackendFamily::OpenStandard),
        requires_external_sdk: !matches!(kind.family(), GpuBackendFamily::OpenStandard),
        sdk_hint_detected,
        runtime_hint_detected,
        crate_integration_compiled,
        render_supported,
        path_trace_supported,
        readiness,
        unavailable_reason: kind.unavailable_reason(),
        trace_capabilities: TraceBackendCapabilities::gpu_unavailable(kind.unavailable_reason()),
    }
}

fn has_any_env(keys: &[&str]) -> bool {
    keys.iter().any(|key| env::var_os(key).is_some())
}
