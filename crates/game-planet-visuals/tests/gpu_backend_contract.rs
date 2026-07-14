use game_planet_visuals::{
    backend::{ConfiguredRenderBackend, RenderBackendConfiguration, RenderBackendPreference},
    gpu::{
        CudaBackend, GpuBackend, GpuBackendEnvironment, GpuBackendFamily, GpuBackendKind,
        GpuBackendReadiness, GpuBackendReport, GpuRenderError, RocmBackend, WgpuBackend,
    },
    pathtrace::TraceBackendKind,
    CpuBackend, PlanetVisualProfile, RenderBackend, RenderOutputKind, RenderQuality, RenderRequest,
    RenderSize,
};

#[test]
fn gpu_report_lists_open_standard_cuda_and_amd_statuses_without_sdk_links() {
    let report = GpuBackendReport::from_environment(GpuBackendEnvironment {
        open_standard_runtime_hint: true,
        cuda_sdk_hint: true,
        cuda_runtime_hint: true,
        rocm_sdk_hint: true,
        rocm_runtime_hint: false,
    });

    assert_eq!(
        report.statuses.map(|status| status.kind),
        [
            GpuBackendKind::Wgpu,
            GpuBackendKind::Cuda,
            GpuBackendKind::Rocm
        ]
    );

    let wgpu = report.open_standard();
    assert_eq!(wgpu.family, GpuBackendFamily::OpenStandard);
    assert!(wgpu.open_standard);
    assert!(!wgpu.requires_external_sdk);
    assert!(!wgpu.sdk_hint_detected);
    assert!(wgpu.runtime_hint_detected);
    assert!(!wgpu.capabilities().hardware_accelerated);
    assert_eq!(wgpu.readiness, GpuBackendReadiness::HostHintsOnly);

    let cuda = report
        .status(GpuBackendKind::Cuda)
        .expect("CUDA status should be reported");
    assert_eq!(cuda.family, GpuBackendFamily::NvidiaProprietary);
    assert!(!cuda.open_standard);
    assert!(cuda.requires_external_sdk);
    assert!(cuda.sdk_hint_detected);
    assert!(cuda.runtime_hint_detected);

    let rocm = report
        .status(GpuBackendKind::Rocm)
        .expect("ROCm/HIP status should be reported");
    assert_eq!(rocm.family, GpuBackendFamily::AmdProprietary);
    assert!(rocm.requires_external_sdk);
    assert!(rocm.sdk_hint_detected);
    assert!(!rocm.runtime_hint_detected);

    for status in report.statuses {
        assert!(!status.crate_integration_compiled);
        assert!(!status.render_supported);
        assert!(!status.path_trace_supported);
        assert!(!status.capabilities().hardware_accelerated);
        assert_eq!(status.trace_capabilities.kind, TraceBackendKind::Gpu);
        assert!(!status.trace_capabilities.available);
        assert_eq!(
            status.trace_capabilities.unavailable_reason,
            Some(status.unavailable_reason)
        );
    }
}

#[test]
fn backend_configuration_falls_back_to_cpu_unless_gpu_is_strictly_required() {
    let report = GpuBackendReport::from_environment(GpuBackendEnvironment::empty());

    let automatic = RenderBackendConfiguration::automatic().resolve(report);
    assert_eq!(automatic.selected, ConfiguredRenderBackend::Cpu);
    assert_eq!(automatic.capabilities.name, CpuBackend::NAME);
    assert!(automatic.cpu_fallback);
    assert_eq!(
        automatic
            .gpu_status
            .expect("automatic resolution should describe the unavailable GPU")
            .kind,
        GpuBackendKind::Wgpu
    );

    let fallback = RenderBackendConfiguration {
        preference: RenderBackendPreference::Cuda,
        allow_cpu_fallback: true,
    }
    .resolve(report);
    assert_eq!(fallback.selected, ConfiguredRenderBackend::Cpu);
    assert_eq!(fallback.capabilities.name, CpuBackend::NAME);
    assert!(fallback.cpu_fallback);
    assert_eq!(
        fallback
            .gpu_status
            .expect("fallback should preserve requested GPU status")
            .kind,
        GpuBackendKind::Cuda
    );
    assert_eq!(
        fallback.unavailable_reason,
        Some(CudaBackend::UNAVAILABLE_REASON)
    );

    let strict = RenderBackendConfiguration {
        preference: RenderBackendPreference::OpenStandardGpu,
        allow_cpu_fallback: false,
    }
    .resolve(report);
    assert_eq!(
        strict.selected,
        ConfiguredRenderBackend::Gpu(GpuBackendKind::Wgpu)
    );
    assert!(!strict.cpu_fallback);
    assert!(!strict.capabilities.hardware_accelerated);
    assert!(!strict.capabilities.supports_icon);
    assert!(!strict.capabilities.supports_banner);
    assert!(!strict.capabilities.supports_surface_map);
    assert_eq!(
        strict.unavailable_reason,
        Some(WgpuBackend::UNAVAILABLE_REASON)
    );
}

#[test]
fn gpu_render_backends_return_unavailable_errors_instead_of_fake_outputs() {
    let profile = PlanetVisualProfile::from_seed(0x5EED_1208_6000);
    let request = RenderRequest {
        profile: &profile,
        size: RenderSize {
            width: 128,
            height: 64,
        },
        output: RenderOutputKind::Banner,
        quality: RenderQuality::Draft,
    };

    for (backend, reason) in [
        (
            GpuBackend::new(GpuBackendKind::Wgpu),
            WgpuBackend::UNAVAILABLE_REASON,
        ),
        (
            GpuBackend::new(GpuBackendKind::Cuda),
            CudaBackend::UNAVAILABLE_REASON,
        ),
        (
            GpuBackend::new(GpuBackendKind::Rocm),
            RocmBackend::UNAVAILABLE_REASON,
        ),
    ] {
        let capabilities = backend.capabilities();
        assert!(!capabilities.hardware_accelerated);
        assert!(!capabilities.supports_banner);

        assert_eq!(
            backend.render(request),
            Err(GpuRenderError::Unavailable {
                kind: backend.kind,
                requested_size: request.size,
                requested_output: request.output,
                requested_quality: request.quality,
                reason,
            })
        );
    }
}
