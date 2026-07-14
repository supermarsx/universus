use game_planet_visuals::pathtrace::{
    self, intersect_ray_sphere, AccumulatorError, Camera, CpuTraceKernel, MaterialSample,
    PathTraceSettings, PathTraceSettingsError, Ray, SampleAccumulator, Sphere, Tile,
    TraceBackendKind, TraceError, TraceImage, TracePlan, TracePlanError, TraceScene, TraceStats,
    TraceSurfaceControls, TraceSurfaceModel, Vec3,
};
use game_planet_visuals::{
    CpuBackend, PlanetVisualProfile, RenderBackend, RenderOutputKind, RenderQuality, RenderRequest,
    RenderSize, WgpuBackend,
};
use std::{any::TypeId, collections::HashSet, fmt::Debug};

#[derive(Debug, Clone, Copy)]
struct TraceImageColorStats {
    finite_pixels: usize,
    distinct_rgb: usize,
    luma_sum: f32,
    luma_range: f32,
    mean_adjacent_luma_delta: f32,
}

fn assert_vec3_close(actual: Vec3, expected: Vec3) {
    let delta = actual - expected;
    assert!(
        delta.length() < 0.000_01,
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn tiles_cover_image_bounds_in_row_major_order() {
    let settings = PathTraceSettings {
        tile_width: 4,
        tile_height: 3,
        ..PathTraceSettings::default()
    };

    let tiles = settings
        .tiles_for_image(10, 7)
        .expect("valid settings should tile");

    assert_eq!(tiles.len(), 9);
    assert_eq!(
        (tiles[0].x, tiles[0].y, tiles[0].width, tiles[0].height),
        (0, 0, 4, 3)
    );
    assert_eq!(
        (tiles[2].x, tiles[2].y, tiles[2].width, tiles[2].height),
        (8, 0, 2, 3)
    );
    assert_eq!(
        (tiles[6].x, tiles[6].y, tiles[6].width, tiles[6].height),
        (0, 6, 4, 1)
    );
    assert_eq!(
        (tiles[8].x, tiles[8].y, tiles[8].width, tiles[8].height),
        (8, 6, 2, 1)
    );

    assert!(tiles[8].contains(9, 6));
    assert!(!tiles[8].contains(10, 6));
    assert_eq!(tiles.iter().map(|tile| tile.pixel_count()).sum::<u64>(), 70);

    let clipped = pathtrace::Tile::new(8, 5, 9, 9)
        .clipped_to(10, 7)
        .expect("overlapping tile should clip");
    assert_eq!(
        (clipped.x, clipped.y, clipped.width, clipped.height),
        (8, 5, 2, 2)
    );
    assert!(pathtrace::Tile::new(10, 0, 1, 1)
        .clipped_to(10, 7)
        .is_none());
}

#[test]
fn settings_validation_rejects_invalid_contract_values() {
    assert_eq!(
        PathTraceSettings {
            samples_per_pixel: 0,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::ZeroSamplesPerPixel)
    );
    assert_eq!(
        PathTraceSettings {
            tile_width: 0,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::ZeroTileWidth)
    );
    assert_eq!(
        PathTraceSettings {
            tile_height: 0,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::ZeroTileHeight)
    );
    assert_eq!(
        PathTraceSettings {
            samples_per_pixel: PathTraceSettings::MAX_SAMPLES_PER_PIXEL + 1,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::SamplesPerPixelTooHigh {
            value: PathTraceSettings::MAX_SAMPLES_PER_PIXEL + 1,
            max: PathTraceSettings::MAX_SAMPLES_PER_PIXEL,
        })
    );
    assert_eq!(
        PathTraceSettings {
            max_bounces: PathTraceSettings::MAX_BOUNCES + 1,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::MaxBouncesTooHigh {
            value: PathTraceSettings::MAX_BOUNCES + 1,
            max: PathTraceSettings::MAX_BOUNCES,
        })
    );
    assert_eq!(
        PathTraceSettings {
            atmosphere_samples: PathTraceSettings::MAX_ATMOSPHERE_SAMPLES + 1,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::AtmosphereSamplesTooHigh {
            value: PathTraceSettings::MAX_ATMOSPHERE_SAMPLES + 1,
            max: PathTraceSettings::MAX_ATMOSPHERE_SAMPLES,
        })
    );
    assert_eq!(
        PathTraceSettings {
            tile_width: PathTraceSettings::MAX_TILE_EDGE + 1,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::TileWidthTooHigh {
            value: PathTraceSettings::MAX_TILE_EDGE + 1,
            max: PathTraceSettings::MAX_TILE_EDGE,
        })
    );
    assert_eq!(
        PathTraceSettings {
            tile_height: PathTraceSettings::MAX_TILE_EDGE + 1,
            ..PathTraceSettings::default()
        }
        .validate(),
        Err(PathTraceSettingsError::TileHeightTooHigh {
            value: PathTraceSettings::MAX_TILE_EDGE + 1,
            max: PathTraceSettings::MAX_TILE_EDGE,
        })
    );
}

#[test]
fn trace_plan_counts_tiles_pixels_and_samples() {
    let settings = PathTraceSettings {
        samples_per_pixel: 2,
        tile_width: 3,
        tile_height: 2,
        ..PathTraceSettings::default()
    };

    let plan = TracePlan::new(7, 5, settings).expect("valid trace plan should build");

    assert_eq!(plan.image_width, 7);
    assert_eq!(plan.image_height, 5);
    assert_eq!(plan.tile_count(), 9);
    assert_eq!(plan.total_pixels, 35);
    assert_eq!(plan.total_samples, 70);
    assert_eq!(plan.tiles[0], Tile::new(0, 0, 3, 2));
    assert_eq!(plan.tiles[8], Tile::new(6, 4, 1, 1));

    assert_eq!(
        TracePlan::new(0, 5, settings),
        Err(TracePlanError::EmptyImage {
            width: 0,
            height: 5
        })
    );
}

#[test]
fn cpu_trace_kernel_resolves_full_trace_image_deterministically() {
    let kernel = CpuTraceKernel::default();
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        45.0,
        4.0 / 3.0,
    );
    let settings = PathTraceSettings {
        samples_per_pixel: 2,
        seeded_jitter: false,
        atmosphere_samples: 2,
        tile_width: 2,
        tile_height: 2,
        ..PathTraceSettings::preview()
    };

    let first = kernel
        .trace_image(camera, 4, 3, settings)
        .expect("small trace image should render");
    let second = kernel
        .trace_image(camera, 4, 3, settings)
        .expect("same trace image should render deterministically");

    assert_eq!(first, second);
    assert_eq!(first.dimensions(), (4, 3));
    assert_eq!(first.pixel_len(), 12);
    assert_eq!(first.pixels().len(), 12);
    assert_eq!(first.pixel_at(4, 0), None);
    assert_eq!(first.plan.image_width, 4);
    assert_eq!(first.plan.image_height, 3);
    assert_eq!(first.plan.tile_count(), 4);
    assert_eq!(first.plan.total_pixels, 12);
    assert_eq!(first.plan.total_samples, 24);
    assert_eq!(first.stats.tiles_completed, 4);
    assert_eq!(first.stats.samples_completed, 24);
    assert_eq!(first.stats.primary_rays, 24);
    assert!(first.stats.rays_traced >= first.plan.total_samples);
    assert!(first.pixels.iter().all(|color| color.is_finite()));
    assert!(first
        .pixels
        .iter()
        .any(|color| color.length_squared() > 0.000_001));
    assert!(
        first
            .pixel_at(1, 1)
            .expect("center image pixel should be addressable")
            .length_squared()
            > 0.000_001
    );
}

#[test]
fn trace_image_rejects_invalid_or_oversized_requests() {
    let kernel = CpuTraceKernel::default();
    let camera = Camera::default();
    let settings = PathTraceSettings::preview();

    assert_eq!(
        kernel.trace_image(camera, 0, 2, settings),
        Err(TraceError::EmptyImage {
            width: 0,
            height: 2
        })
    );
    assert_eq!(
        kernel.trace_image(
            camera,
            2,
            2,
            PathTraceSettings {
                samples_per_pixel: 0,
                ..settings
            }
        ),
        Err(TraceError::Settings(
            PathTraceSettingsError::ZeroSamplesPerPixel
        ))
    );

    let oversized_width = TraceImage::MAX_PIXELS as u32 + 1;
    assert_eq!(
        kernel.trace_image(camera, oversized_width, 1, settings),
        Err(TraceError::ImageTooLarge {
            pixels: TraceImage::MAX_PIXELS + 1,
            max_pixels: TraceImage::MAX_PIXELS,
        })
    );
}

#[test]
fn ray_sphere_intersection_returns_nearest_hit_and_surface_frame() {
    let material = MaterialSample {
        albedo: Vec3::new(0.8, 0.2, 0.1),
        ..MaterialSample::default()
    };
    let sphere = Sphere::new(Vec3::ZERO, 1.0, material);
    let ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0));

    let hit = intersect_ray_sphere(ray, sphere).expect("ray should hit sphere");

    assert!((hit.t - 2.0).abs() < 0.000_01);
    assert_vec3_close(hit.position, Vec3::new(0.0, 0.0, 1.0));
    assert_vec3_close(hit.normal, Vec3::new(0.0, 0.0, 1.0));
    assert!(hit.front_face);
    assert_eq!(hit.material, material);

    let inside_ray = Ray::normalized(Vec3::ZERO, Vec3::Z);
    let exit_hit = intersect_ray_sphere(inside_ray, sphere).expect("inside ray should exit");
    assert!((exit_hit.t - 1.0).abs() < 0.000_01);
    assert!(!exit_hit.front_face);
    assert_vec3_close(exit_hit.normal, Vec3::new(0.0, 0.0, -1.0));

    let miss = Ray::normalized(Vec3::new(0.0, 3.0, 3.0), Vec3::new(0.0, 0.0, -1.0));
    assert!(intersect_ray_sphere(miss, sphere).is_none());
}

#[test]
fn public_cpu_and_wgpu_backend_marker_types_remain_available() {
    let cpu = public_backend_marker::<CpuBackend>();
    let wgpu = public_backend_marker::<WgpuBackend>();

    assert_eq!(format!("{cpu:?}"), "CpuBackend");
    assert_eq!(format!("{wgpu:?}"), "WgpuBackend");
    assert_ne!(TypeId::of::<CpuBackend>(), TypeId::of::<WgpuBackend>());
}

#[test]
fn cpu_backend_reports_trace_capabilities_and_executes_trace_tile() {
    let backend = CpuBackend;
    let trace_caps = backend.trace_capabilities();

    assert_eq!(trace_caps.kind, TraceBackendKind::Cpu);
    assert!(trace_caps.available);
    assert!(trace_caps.deterministic);
    assert!(!trace_caps.hardware_accelerated);
    assert!(trace_caps.supports_tiling);
    assert!(trace_caps.supports_ray_sphere_intersection);
    assert!(trace_caps.supports_reflections);
    assert!(trace_caps.supports_refractions);
    assert!(trace_caps.supports_atmosphere_sampling);
    assert_eq!(trace_caps.unavailable_reason, None);

    let render_caps = backend.capabilities();
    assert_eq!(render_caps.name, CpuBackend::NAME);
    assert!(!render_caps.hardware_accelerated);

    let profile = PlanetVisualProfile::from_seed(0x5EED_1208_1000);
    let request = RenderRequest {
        profile: &profile,
        size: RenderSize {
            width: 64,
            height: 64,
        },
        output: RenderOutputKind::Icon,
        quality: RenderQuality::Draft,
    };
    let metadata = backend
        .render(request)
        .expect("CPU backend should return render metadata");
    assert_eq!(metadata.profile_seed, profile.seed);
    assert_eq!(metadata.trace_capabilities, trace_caps);

    let settings = PathTraceSettings {
        samples_per_pixel: 1,
        seeded_jitter: false,
        atmosphere_samples: 2,
        tile_width: 2,
        tile_height: 2,
        ..PathTraceSettings::default()
    };
    let plan = backend
        .trace_plan(3, 3, settings)
        .expect("CPU backend should plan path trace tiles");
    assert_eq!(plan.tile_count(), 4);
    assert_eq!(plan.total_samples, 9);

    let camera = Camera::look_at(Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO, Vec3::Y, 45.0, 1.0);
    let tile = backend
        .trace_tile(camera, 3, 3, Tile::new(0, 0, 2, 2), settings)
        .expect("CPU backend should trace small tiles");

    assert_eq!(tile.tile, Tile::new(0, 0, 2, 2));
    assert_eq!(tile.pixels.len(), 4);
    assert_eq!(tile.stats.tiles_completed, 1);
    assert_eq!(tile.stats.samples_completed, 4);
    assert_eq!(tile.stats.primary_rays, 4);
    assert!(tile.pixels.iter().all(|color| color.is_finite()));

    let image = backend
        .trace_image(camera, 3, 3, settings)
        .expect("CPU backend should trace a full image artifact");

    assert_eq!(image.dimensions(), (3, 3));
    assert_eq!(image.pixel_len(), 9);
    assert_eq!(image.plan.total_samples, 9);
    assert_eq!(
        image.stats.tiles_completed as usize,
        image.plan.tile_count()
    );
    assert_eq!(image.stats.samples_completed, image.plan.total_samples);
    assert_eq!(image.stats.primary_rays, image.plan.total_samples);
    assert!(image.pixels.iter().all(|color| color.is_finite()));
    assert!(image
        .pixels
        .iter()
        .any(|color| color.length_squared() > 0.000_001));
}

#[test]
fn wgpu_backend_reports_path_tracing_unavailable_without_fake_gpu_support() {
    let backend = WgpuBackend;
    let capabilities = backend.trace_capabilities();

    assert_eq!(capabilities.kind, TraceBackendKind::Gpu);
    assert!(!capabilities.available);
    assert!(capabilities.hardware_accelerated);
    assert!(!capabilities.deterministic);
    assert!(!capabilities.supports_tiling);
    assert_eq!(
        capabilities.unavailable_reason,
        Some(WgpuBackend::UNAVAILABLE_REASON)
    );
    assert!(capabilities
        .unavailable_reason
        .expect("unavailable GPU path should explain why")
        .contains("does not compile a wgpu integration"));
}

#[test]
fn sample_accumulator_tracks_counts_sums_and_resolved_color() {
    let mut accumulator = SampleAccumulator::empty(3, 2);

    assert_eq!(accumulator.pixel_len(), 6);
    assert_eq!(accumulator.resolved_pixel(1, 1), Ok(Vec3::ZERO));

    assert_eq!(
        accumulator.add_sample(1, 1, Vec3::new(0.2, 0.4, 0.6)),
        Ok(1)
    );
    assert_eq!(
        accumulator.add_sample(1, 1, Vec3::new(0.6, 0.2, 0.0)),
        Ok(2)
    );
    assert_eq!(accumulator.sample_count_at(1, 1), Ok(2));
    assert_vec3_close(
        accumulator.color_sum_at(1, 1).unwrap(),
        Vec3::new(0.8, 0.6, 0.6),
    );
    assert_vec3_close(
        accumulator.resolved_pixel(1, 1).unwrap(),
        Vec3::new(0.4, 0.3, 0.3),
    );

    assert_eq!(
        accumulator.add_sample(3, 0, Vec3::ONE),
        Err(AccumulatorError::OutOfBounds {
            x: 3,
            y: 0,
            width: 3,
            height: 2
        })
    );

    accumulator.clear();
    assert_eq!(accumulator.sample_count_at(1, 1), Ok(0));
    assert_eq!(accumulator.resolved_pixel(1, 1), Ok(Vec3::ZERO));
}

#[test]
fn seeded_jitter_and_camera_rays_are_deterministic() {
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::ZERO,
        Vec3::Y,
        60.0,
        16.0 / 9.0,
    );
    let settings = PathTraceSettings {
        jitter_seed: 0xCAFE_F00D,
        ..PathTraceSettings::default()
    };
    let same_settings = settings;
    let other_seed = PathTraceSettings {
        jitter_seed: 0xCAFE_F00E,
        ..settings
    };

    assert_eq!(
        settings.sample_jitter(10, 4, 2),
        same_settings.sample_jitter(10, 4, 2)
    );
    assert_ne!(
        settings.sample_jitter(10, 4, 2),
        other_seed.sample_jitter(10, 4, 2)
    );

    let first_ray = camera.ray_for_pixel(10, 4, 1920, 1080, 2, settings);
    let second_ray = camera.ray_for_pixel(10, 4, 1920, 1080, 2, same_settings);
    let different_seed_ray = camera.ray_for_pixel(10, 4, 1920, 1080, 2, other_seed);

    assert_eq!(first_ray, second_ray);
    assert_ne!(first_ray.direction, different_seed_ray.direction);
    assert_vec3_close(first_ray.origin, camera.origin);
    assert!((first_ray.direction.length() - 1.0).abs() < 0.000_01);
    assert!(first_ray.has_valid_bounds());

    let unseeded_a = PathTraceSettings {
        seeded_jitter: false,
        jitter_seed: 1,
        ..PathTraceSettings::default()
    };
    let unseeded_b = PathTraceSettings {
        seeded_jitter: false,
        jitter_seed: 2,
        ..PathTraceSettings::default()
    };
    assert_eq!(unseeded_a.sample_jitter(10, 4, 2), (0.5, 0.5));
    assert_eq!(
        camera.ray_for_pixel(10, 4, 1920, 1080, 2, unseeded_a),
        camera.ray_for_pixel(10, 4, 1920, 1080, 2, unseeded_b)
    );

    let degenerate_up = Camera::look_at(Vec3::ZERO, Vec3::X, Vec3::X, 45.0, 1.0);
    assert!(degenerate_up.forward.dot(degenerate_up.right).abs() < 0.000_01);
    assert!(degenerate_up.forward.dot(degenerate_up.up).abs() < 0.000_01);
}

#[test]
fn cpu_trace_kernel_is_deterministic_and_honors_feature_toggles() {
    let material = MaterialSample {
        albedo: Vec3::new(0.78, 0.34, 0.18),
        roughness: 0.08,
        metallic: 0.65,
        transmission: 0.45,
        opacity: 0.25,
        index_of_refraction: 1.33,
        ..MaterialSample::default()
    };
    let kernel = CpuTraceKernel::new(TraceScene {
        planet: Sphere::new(Vec3::ZERO, 1.0, material),
        ..TraceScene::default()
    });
    let camera = Camera::look_at(Vec3::new(0.0, 0.0, 3.0), Vec3::ZERO, Vec3::Y, 45.0, 1.0);
    let settings = PathTraceSettings {
        samples_per_pixel: 1,
        seeded_jitter: false,
        max_bounces: 2,
        atmosphere_samples: 4,
        enable_reflections: true,
        enable_refractions: true,
        ..PathTraceSettings::default()
    };

    let first = kernel
        .trace_pixel(camera, 1, 1, 3, 3, settings)
        .expect("center pixel should trace");
    let second = kernel
        .trace_pixel(camera, 1, 1, 3, 3, settings)
        .expect("same trace should be deterministic");

    assert_eq!(first, second);
    assert!(first.hit.is_some());
    assert_eq!(first.stats.samples_completed, 1);
    assert_eq!(first.stats.primary_rays, 1);
    assert_eq!(first.stats.shadow_rays, 1);
    assert_eq!(first.stats.reflection_rays, 1);
    assert_eq!(first.stats.refraction_rays, 1);
    assert_eq!(first.stats.atmosphere_samples, 4);
    assert_eq!(first.stats.rays_traced, 4);
    assert_eq!(first.stats.max_bounce_depth, 2);

    let toggles_off = PathTraceSettings {
        enable_reflections: false,
        enable_refractions: false,
        ..settings
    };
    let direct_only = kernel
        .trace_pixel(camera, 1, 1, 3, 3, toggles_off)
        .expect("feature-disabled trace should still render");

    assert_eq!(direct_only.stats.reflection_rays, 0);
    assert_eq!(direct_only.stats.refraction_rays, 0);
    assert_ne!(first.color, direct_only.color);
}

#[test]
fn reflected_environment_lobe_follows_scene_light_direction() {
    let material = MaterialSample {
        albedo: Vec3::ZERO,
        roughness: 0.015,
        metallic: 1.0,
        transmission: 0.0,
        opacity: 1.0,
        ..MaterialSample::default()
    };
    let ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.88, 0.0, -2.525));
    let hit = intersect_ray_sphere(ray, Sphere::new(Vec3::ZERO, 1.0, material))
        .expect("grazing reflection probe should hit the planet");
    let reflected_light = ray.direction.reflect(hit.normal).normalize();
    let settings = PathTraceSettings {
        samples_per_pixel: 1,
        seeded_jitter: false,
        max_bounces: 1,
        enable_reflections: true,
        enable_refractions: false,
        atmosphere_samples: 0,
        ..PathTraceSettings::preview()
    };

    let matched = reflection_probe(material, ray, reflected_light, settings, true);
    let matched_direct = reflection_probe(material, ray, reflected_light, settings, false);
    let side = reflection_probe(material, ray, Vec3::Y, settings, true);
    let side_direct = reflection_probe(material, ray, Vec3::Y, settings, false);

    assert_eq!(matched.stats.reflection_rays, 1);
    assert_eq!(side.stats.reflection_rays, 1);
    assert_eq!(matched_direct.stats.reflection_rays, 0);
    assert_eq!(side_direct.stats.reflection_rays, 0);

    let matched_lift = trace_luma(matched.color) - trace_luma(matched_direct.color);
    let side_lift = trace_luma(side.color) - trace_luma(side_direct.color);
    assert!(
        matched_lift > side_lift + 0.12,
        "mirror reflection should brighten most when the scene light matches the reflected ray; matched={matched_lift:.5}, side={side_lift:.5}, reflected_light={reflected_light:?}"
    );
}

#[test]
fn atmosphere_scatter_color_tracks_light_phase_without_changing_optical_depth() {
    let ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.1), Vec3::new(0.30, 0.0, -1.0));
    let settings = PathTraceSettings {
        samples_per_pixel: 1,
        seeded_jitter: false,
        atmosphere_samples: 8,
        ..PathTraceSettings::preview()
    };
    let forward = atmosphere_probe(ray, ray.direction, settings);
    let side = atmosphere_probe(ray, Vec3::Y, settings);
    let back = atmosphere_probe(ray, ray.direction * -1.0, settings);

    assert_eq!(forward.samples, settings.atmosphere_samples);
    assert_eq!(side.samples, settings.atmosphere_samples);
    assert_eq!(back.samples, settings.atmosphere_samples);
    assert!(
        (forward.optical_depth - side.optical_depth).abs() < 0.000_001
            && (forward.optical_depth - back.optical_depth).abs() < 0.000_001,
        "changing only the scene light direction should leave optical depth stable; forward={forward:?}, side={side:?}, back={back:?}"
    );

    let forward_side_delta = vec3_mean_abs_delta(forward.color, side.color);
    let side_back_delta = vec3_mean_abs_delta(side.color, back.color);
    let forward_back_delta = vec3_mean_abs_delta(forward.color, back.color);
    assert!(
        forward_side_delta > 0.006 && side_back_delta > 0.003 && forward_back_delta > 0.008,
        "atmosphere scatter color should vary across forward/side/back light phases; forward={forward:?}, side={side:?}, back={back:?}"
    );
    assert!(
        forward.color.x - forward.color.z > side.color.x - side.color.z + 0.004,
        "forward scatter should carry a warmer solar tint than side scatter; forward={forward:?}, side={side:?}"
    );
}

#[test]
fn cpu_trace_image_artifact_is_deterministic_varied_and_honors_feature_toggles() {
    let material = MaterialSample {
        albedo: Vec3::new(0.70, 0.42, 0.22),
        roughness: 0.06,
        metallic: 0.72,
        transmission: 0.48,
        opacity: 0.18,
        index_of_refraction: 1.31,
        ..MaterialSample::default()
    };
    let kernel = CpuTraceKernel::new(TraceScene {
        planet: Sphere::new(Vec3::ZERO, 1.0, material),
        atmosphere_radius: 1.12,
        atmosphere_density: 0.60,
        ..TraceScene::default()
    });
    let width = 10;
    let height = 8;
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.06, 3.2),
        Vec3::ZERO,
        Vec3::Y,
        42.0,
        width as f32 / height as f32,
    );
    let settings = PathTraceSettings {
        samples_per_pixel: 2,
        jitter_seed: 0xCAFE_1208,
        seeded_jitter: true,
        max_bounces: 3,
        enable_reflections: true,
        enable_refractions: true,
        atmosphere_samples: 3,
        tile_width: 4,
        tile_height: 3,
    };

    let first = kernel
        .trace_image(camera, width, height, settings)
        .expect("small CPU trace image should render");
    let second = kernel
        .trace_image(camera, width, height, settings)
        .expect("same CPU trace image should render deterministically");

    assert_eq!(first.dimensions(), (width, height));
    assert_eq!(first.pixels.len(), (width * height) as usize);
    assert_eq!(
        first.pixels, second.pixels,
        "CPU trace image pixels should be deterministic for identical settings"
    );
    assert_eq!(first.stats, second.stats);
    assert_eq!(first.plan.image_width, width);
    assert_eq!(first.plan.image_height, height);
    assert_eq!(first.plan.tile_count(), 9);
    assert_eq!(first.plan.total_pixels, u64::from(width * height));
    assert_eq!(first.plan.total_samples, u64::from(width * height * 2));
    assert_eq!(
        first.stats.tiles_completed as usize,
        first.plan.tile_count()
    );
    assert_eq!(first.stats.samples_completed, first.plan.total_samples);
    assert_eq!(first.stats.primary_rays, first.plan.total_samples);
    assert!(
        first.stats.shadow_rays > 0,
        "small traced frame should include planet-surface shadow work; got {:?}",
        first.stats
    );
    assert!(
        first.stats.reflection_rays > 0,
        "reflection-enabled traced frame should report reflection rays; got {:?}",
        first.stats
    );
    assert!(
        first.stats.refraction_rays > 0,
        "refraction-enabled traced frame should report refraction rays; got {:?}",
        first.stats
    );
    assert!(first.stats.atmosphere_samples > 0);
    assert_eq!(first.stats.max_bounce_depth, 2);

    let color_stats = trace_frame_color_stats(&first);
    assert_eq!(color_stats.finite_pixels, first.pixels.len());
    assert!(
        color_stats.luma_sum > 2.0,
        "small traced frame should not be blank; got {color_stats:?}"
    );
    assert!(
        color_stats.luma_range > 0.08,
        "small traced frame should include tonal variation; got {color_stats:?}"
    );
    assert!(
        color_stats.distinct_rgb >= 12,
        "small traced frame should contain varied quantized pixels; got {color_stats:?}"
    );
    assert!(
        color_stats.mean_adjacent_luma_delta > 0.004,
        "small traced frame should expose local pixel variation; got {color_stats:?}"
    );

    let toggles_off = PathTraceSettings {
        enable_reflections: false,
        enable_refractions: false,
        ..settings
    };
    let direct_only = kernel
        .trace_image(camera, width, height, toggles_off)
        .expect("feature-disabled CPU trace image should still render");

    assert_eq!(direct_only.dimensions(), (width, height));
    assert_eq!(
        direct_only.stats.samples_completed,
        first.stats.samples_completed
    );
    assert_eq!(direct_only.stats.primary_rays, first.stats.primary_rays);
    assert_eq!(direct_only.stats.reflection_rays, 0);
    assert_eq!(direct_only.stats.refraction_rays, 0);

    let feature_delta = mean_abs_trace_color_delta(&first, &direct_only);
    assert!(
        feature_delta > 0.004,
        "reflection/refraction toggles should visibly affect the public CPU trace output; mean delta was {feature_delta:.5}"
    );
}

#[test]
fn procedural_surface_controls_make_trace_images_seeded_varied_and_deterministic() {
    let surface = TraceSurfaceControls {
        seed: 0x5EED_1208_4401,
        surface_model: TraceSurfaceModel::Terrestrial,
        ocean_fraction: 0.57,
        cloud_coverage: 0.30,
        cloud_opacity: 0.44,
        atmosphere_strength: 0.95,
        ..TraceSurfaceControls::default()
    };

    let first = render_procedural_trace_image(surface);
    let second = render_procedural_trace_image(surface);

    assert_eq!(
        first.pixels, second.pixels,
        "same procedural trace controls should produce identical pixels"
    );
    assert_eq!(first.stats, second.stats);

    let color_stats = trace_frame_color_stats(&first);
    assert_eq!(color_stats.finite_pixels, first.pixels.len());
    assert!(
        color_stats.distinct_rgb >= 24,
        "procedural traced frame should expose varied quantized colors; got {color_stats:?}"
    );
    assert!(
        color_stats.luma_range > 0.11,
        "procedural traced frame should not collapse to a smooth sphere; got {color_stats:?}"
    );
    assert!(
        color_stats.mean_adjacent_luma_delta > 0.006,
        "procedural traced frame should include local surface detail; got {color_stats:?}"
    );

    let reseeded = render_procedural_trace_image(TraceSurfaceControls {
        seed: surface.seed + 1,
        ..surface
    });
    let seed_delta = mean_abs_trace_color_delta(&first, &reseeded);
    assert!(
        seed_delta > 0.004,
        "changing the procedural surface seed should alter traced output; mean delta was {seed_delta:.5}"
    );
}

#[test]
fn procedural_surface_models_and_feature_knobs_change_traced_output() {
    let base = TraceSurfaceControls {
        seed: 0x5EED_1208_4402,
        surface_model: TraceSurfaceModel::Terrestrial,
        ocean_fraction: 0.46,
        cloud_coverage: 0.0,
        cloud_opacity: 0.0,
        atmosphere_strength: 0.0,
        ..TraceSurfaceControls::default()
    };
    let terrestrial = render_procedural_trace_image(base);
    let gas = render_procedural_trace_image(TraceSurfaceControls {
        surface_model: TraceSurfaceModel::BandedGasGiant,
        ocean_fraction: 0.0,
        band_frequency: 13.0,
        band_contrast: 0.72,
        ..base
    });
    let ocean = render_procedural_trace_image(TraceSurfaceControls {
        surface_model: TraceSurfaceModel::Ocean,
        ocean_fraction: 1.0,
        ..base
    });

    let gas_delta = mean_abs_trace_color_delta(&terrestrial, &gas);
    let ocean_delta = mean_abs_trace_color_delta(&terrestrial, &ocean);
    assert!(
        gas_delta > 0.018,
        "banded gas giant surface model should change traced output; mean delta was {gas_delta:.5}"
    );
    assert!(
        ocean_delta > 0.012,
        "ocean surface model should change traced output; mean delta was {ocean_delta:.5}"
    );

    let cloudy = render_procedural_trace_image(TraceSurfaceControls {
        cloud_coverage: 0.78,
        cloud_opacity: 0.74,
        ..base
    });
    let atmosphere_tinted = render_procedural_trace_image(TraceSurfaceControls {
        atmosphere_color: Vec3::new(0.95, 0.52, 0.34),
        atmosphere_strength: 1.0,
        ..base
    });

    let cloud_delta = mean_abs_trace_color_delta(&terrestrial, &cloudy);
    let atmosphere_delta = mean_abs_trace_color_delta(&terrestrial, &atmosphere_tinted);
    assert!(
        cloud_delta > 0.006,
        "cloud coverage/opacity controls should alter traced output; mean delta was {cloud_delta:.5}"
    );
    assert!(
        atmosphere_delta > 0.003,
        "atmosphere color/strength controls should alter traced output; mean delta was {atmosphere_delta:.5}"
    );
}

#[test]
fn cpu_path_tracer_records_occlusion_cloud_depth_and_limb_optics() {
    let width = 18;
    let height = 12;
    let settings = realism_trace_settings();
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.04, 3.12),
        Vec3::ZERO,
        Vec3::Y,
        42.0,
        width as f32 / height as f32,
    );
    let full = realism_trace_kernel(0.78, 0.72, 0.92, 1.0)
        .trace_image(camera, width, height, settings)
        .expect("full realism CPU trace should render");

    assert!(
        full.stats.ambient_occlusion_samples > 0,
        "terrain/ocean contact AO should be sampled; got {:?}",
        full.stats
    );
    assert!(
        full.stats.cloud_depth_samples > 0,
        "cloud/ocean depth interaction should be sampled; got {:?}",
        full.stats
    );
    assert!(
        full.stats.shadow_rays > 0
            && full.stats.refraction_rays > 0
            && full.stats.atmosphere_samples > 0,
        "realism trace should exercise shadow, refraction, and atmosphere work; got {:?}",
        full.stats
    );

    let no_clouds = realism_trace_kernel(0.0, 0.0, 0.92, 1.0)
        .trace_image(camera, width, height, settings)
        .expect("cloud-disabled comparison trace should render");
    let no_atmosphere = realism_trace_kernel(0.78, 0.72, 0.0, 0.0)
        .trace_image(camera, width, height, settings)
        .expect("atmosphere-disabled comparison trace should render");

    let cloud_delta = mean_abs_trace_color_delta(&full, &no_clouds);
    let atmosphere_delta = mean_abs_trace_color_delta(&full, &no_atmosphere);
    assert!(
        cloud_delta > 0.004,
        "depth-sensitive clouds over ocean should visibly alter traced output; mean delta was {cloud_delta:.5}"
    );
    assert!(
        atmosphere_delta > 0.004,
        "optical-depth atmosphere should visibly alter traced output; mean delta was {atmosphere_delta:.5}"
    );

    let kernel = realism_trace_kernel(0.78, 0.72, 0.92, 1.0);
    let center_ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.12), Vec3::new(0.0, 0.0, -1.0));
    let limb_ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.12), Vec3::new(0.325, 0.0, -1.0));
    let center_hit = intersect_ray_sphere(center_ray, kernel.scene.planet)
        .expect("center atmosphere sample should terminate at planet");
    let limb_hit = intersect_ray_sphere(limb_ray, kernel.scene.planet)
        .expect("limb atmosphere sample should terminate at planet");
    let center_atmosphere = kernel.sample_atmosphere(center_ray, settings, Some(center_hit.t));
    let limb_atmosphere = kernel.sample_atmosphere(limb_ray, settings, Some(limb_hit.t));

    assert_eq!(center_atmosphere.samples, settings.atmosphere_samples);
    assert_eq!(limb_atmosphere.samples, settings.atmosphere_samples);
    assert!(
        limb_atmosphere.limb_factor > center_atmosphere.limb_factor + 0.10,
        "grazing rays should produce stronger atmosphere limb effects; center={center_atmosphere:?}, limb={limb_atmosphere:?}"
    );
    assert!(
        limb_atmosphere.refraction_bend > center_atmosphere.refraction_bend,
        "grazing rays should produce stronger atmosphere refraction; center={center_atmosphere:?}, limb={limb_atmosphere:?}"
    );
}

#[test]
fn cpu_trace_resolve_adds_deterministic_subtle_anti_banding_dither() {
    let material = MaterialSample {
        albedo: Vec3::ZERO,
        emission: Vec3::splat(0.24),
        roughness: 1.0,
        metallic: 0.0,
        transmission: 0.0,
        index_of_refraction: 1.0,
        opacity: 1.0,
        ..MaterialSample::default()
    };
    let kernel = CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_density: 0.0,
            light_direction: Vec3::new(0.0, 0.0, -1.0),
            ..TraceScene::default()
        },
        TraceSurfaceControls::smooth(0xD17E_5EED),
    );
    let width = 16;
    let height = 10;
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::ZERO,
        Vec3::Y,
        10.0,
        width as f32 / height as f32,
    );
    let settings = PathTraceSettings {
        samples_per_pixel: 1,
        seeded_jitter: false,
        atmosphere_samples: 0,
        tile_width: 8,
        tile_height: 5,
        ..PathTraceSettings::preview()
    };

    let first = kernel
        .trace_image(camera, width, height, settings)
        .expect("flat anti-banding trace should render");
    let second = kernel
        .trace_image(
            camera,
            width,
            height,
            PathTraceSettings {
                jitter_seed: settings.jitter_seed + 1,
                ..settings
            },
        )
        .expect("flat anti-banding trace should remain deterministic without seeded jitter");

    assert_eq!(
        first.pixels, second.pixels,
        "seeded_jitter=false should keep resolve dither seed-independent"
    );
    let stats = trace_frame_color_stats(&first);
    assert_eq!(stats.finite_pixels, first.pixels.len());
    assert!(
        stats.distinct_rgb >= 4,
        "subtle anti-banding dither should break otherwise flat quantized bands; got {stats:?}"
    );
    assert!(
        stats.luma_range > 0.001 && stats.luma_range < 0.010,
        "anti-banding dither should be visible but subtle; got {stats:?}"
    );
}

#[test]
fn trace_stats_counters_are_saturating_and_mergeable() {
    let mut stats = TraceStats::default();

    stats.record_tile_completed();
    stats.record_samples_completed(16);
    stats.record_primary_rays(16);
    stats.record_shadow_rays(8);
    stats.record_reflection_rays(4);
    stats.record_refraction_rays(2);
    stats.record_atmosphere_samples(32);
    stats.record_ambient_occlusion_samples(6);
    stats.record_cloud_depth_samples(3);
    stats.record_bounce_depth(3);
    stats.record_bounce_depth(2);
    stats.record_elapsed_millis(7);

    assert_eq!(stats.tiles_completed, 1);
    assert_eq!(stats.samples_completed, 16);
    assert_eq!(stats.primary_rays, 16);
    assert_eq!(stats.shadow_rays, 8);
    assert_eq!(stats.reflection_rays, 4);
    assert_eq!(stats.refraction_rays, 2);
    assert_eq!(stats.rays_traced, 30);
    assert_eq!(stats.atmosphere_samples, 32);
    assert_eq!(stats.ambient_occlusion_samples, 6);
    assert_eq!(stats.cloud_depth_samples, 3);
    assert_eq!(stats.max_bounce_depth, 3);
    assert_eq!(stats.elapsed_millis, 7);

    let mut other = TraceStats::default();
    other.record_tile_completed();
    other.record_samples_completed(4);
    other.record_primary_rays(4);
    other.record_ambient_occlusion_samples(2);
    other.record_cloud_depth_samples(1);
    other.record_bounce_depth(6);
    other.record_elapsed_millis(5);

    stats.merge(other);
    assert_eq!(stats.tiles_completed, 2);
    assert_eq!(stats.samples_completed, 20);
    assert_eq!(stats.primary_rays, 20);
    assert_eq!(stats.rays_traced, 34);
    assert_eq!(stats.ambient_occlusion_samples, 8);
    assert_eq!(stats.cloud_depth_samples, 4);
    assert_eq!(stats.max_bounce_depth, 6);
    assert_eq!(stats.elapsed_millis, 12);
}

fn public_backend_marker<T>() -> T
where
    T: Copy + Debug + Default + 'static,
{
    T::default()
}

fn reflection_probe(
    material: MaterialSample,
    ray: Ray,
    light_direction: Vec3,
    settings: PathTraceSettings,
    enable_reflections: bool,
) -> pathtrace::TraceSample {
    CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_density: 0.0,
            light_direction,
            sky_color: Vec3::new(0.004, 0.006, 0.012),
            horizon_color: Vec3::new(0.030, 0.040, 0.070),
            ..TraceScene::default()
        },
        TraceSurfaceControls::smooth(0x501A_1208),
    )
    .trace_ray(
        ray,
        PathTraceSettings {
            enable_reflections,
            ..settings
        },
    )
    .expect("reflection probe should trace")
}

fn atmosphere_probe(
    ray: Ray,
    light_direction: Vec3,
    settings: PathTraceSettings,
) -> pathtrace::AtmosphereSample {
    let kernel = CpuTraceKernel::new(TraceScene {
        atmosphere_radius: 1.18,
        atmosphere_density: 0.86,
        light_direction,
        sky_color: Vec3::new(0.012, 0.020, 0.060),
        horizon_color: Vec3::new(0.44, 0.60, 0.95),
        ..TraceScene::default()
    });
    let max_t = intersect_ray_sphere(ray, kernel.scene.planet).map(|hit| hit.t);
    kernel.sample_atmosphere(ray, settings, max_t)
}

fn realism_trace_settings() -> PathTraceSettings {
    PathTraceSettings {
        samples_per_pixel: 1,
        jitter_seed: 0xA0C0_D17E,
        seeded_jitter: false,
        max_bounces: 2,
        enable_reflections: true,
        enable_refractions: true,
        atmosphere_samples: 5,
        tile_width: 6,
        tile_height: 4,
    }
}

fn realism_trace_kernel(
    cloud_coverage: f32,
    cloud_opacity: f32,
    atmosphere_density: f32,
    atmosphere_strength: f32,
) -> CpuTraceKernel {
    let material = MaterialSample {
        albedo: Vec3::new(0.05, 0.23, 0.42),
        roughness: 0.045,
        metallic: 0.02,
        transmission: 0.42,
        opacity: 0.70,
        index_of_refraction: 1.333,
        ..MaterialSample::default()
    };
    CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_radius: 1.14,
            atmosphere_density,
            light_direction: Vec3::new(-0.34, 0.58, 0.74),
            sky_color: Vec3::new(0.010, 0.018, 0.050),
            horizon_color: Vec3::new(0.46, 0.64, 0.92),
        },
        TraceSurfaceControls {
            seed: 0xA0C0_D17E_5EED,
            surface_model: TraceSurfaceModel::Ocean,
            ocean_fraction: 1.0,
            cloud_coverage,
            cloud_opacity,
            atmosphere_strength,
            ..TraceSurfaceControls::DEFAULT
        },
    )
}

fn render_procedural_trace_image(surface: TraceSurfaceControls) -> TraceImage {
    let material = MaterialSample {
        albedo: Vec3::new(0.36, 0.46, 0.38),
        roughness: 0.42,
        metallic: 0.10,
        transmission: 0.0,
        opacity: 1.0,
        ..MaterialSample::default()
    };
    let kernel = CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_radius: 1.13,
            atmosphere_density: 0.72,
            light_direction: Vec3::new(-0.34, 0.58, 0.74),
            sky_color: Vec3::new(0.010, 0.016, 0.045),
            horizon_color: Vec3::new(0.36, 0.56, 0.94),
        },
        surface,
    );
    let width = 16;
    let height = 12;
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.05, 3.1),
        Vec3::ZERO,
        Vec3::Y,
        41.0,
        width as f32 / height as f32,
    );
    let settings = PathTraceSettings {
        samples_per_pixel: 1,
        jitter_seed: 0xC0DE_4400,
        seeded_jitter: false,
        max_bounces: 2,
        enable_reflections: true,
        enable_refractions: true,
        atmosphere_samples: 3,
        tile_width: 5,
        tile_height: 4,
    };

    kernel
        .trace_image(camera, width, height, settings)
        .expect("procedural CPU trace image should render")
}

fn trace_frame_color_stats(image: &TraceImage) -> TraceImageColorStats {
    let mut finite_pixels = 0_usize;
    let mut distinct_rgb = HashSet::new();
    let mut luma_sum = 0.0;
    let mut min_luma = f32::MAX;
    let mut max_luma = f32::MIN;
    let mut adjacent_total = 0.0;
    let mut adjacent_edges = 0_u32;

    for y in 0..image.height {
        for x in 0..image.width {
            let color = trace_pixel(image, x, y);
            if color.is_finite() {
                finite_pixels += 1;
            }
            distinct_rgb.insert(quantized_rgb(color));
            let pixel_luma = trace_luma(color);
            luma_sum += pixel_luma;
            min_luma = min_luma.min(pixel_luma);
            max_luma = max_luma.max(pixel_luma);

            if x + 1 < image.width {
                adjacent_total += (pixel_luma - trace_luma(trace_pixel(image, x + 1, y))).abs();
                adjacent_edges += 1;
            }
            if y + 1 < image.height {
                adjacent_total += (pixel_luma - trace_luma(trace_pixel(image, x, y + 1))).abs();
                adjacent_edges += 1;
            }
        }
    }

    TraceImageColorStats {
        finite_pixels,
        distinct_rgb: distinct_rgb.len(),
        luma_sum,
        luma_range: max_luma - min_luma,
        mean_adjacent_luma_delta: adjacent_total / adjacent_edges.max(1) as f32,
    }
}

fn mean_abs_trace_color_delta(left: &TraceImage, right: &TraceImage) -> f32 {
    assert_eq!(left.dimensions(), right.dimensions());
    let total: f32 = left
        .pixels
        .iter()
        .zip(&right.pixels)
        .map(|(left, right)| {
            ((left.x - right.x).abs() + (left.y - right.y).abs() + (left.z - right.z).abs()) / 3.0
        })
        .sum();
    total / left.pixels.len().max(1) as f32
}

fn trace_pixel(image: &TraceImage, x: u32, y: u32) -> Vec3 {
    image.pixels[(y * image.width + x) as usize]
}

fn trace_luma(color: Vec3) -> f32 {
    color.x * 0.2126 + color.y * 0.7152 + color.z * 0.0722
}

fn vec3_mean_abs_delta(left: Vec3, right: Vec3) -> f32 {
    ((left.x - right.x).abs() + (left.y - right.y).abs() + (left.z - right.z).abs()) / 3.0
}

fn quantized_rgb(color: Vec3) -> [u8; 3] {
    [
        quantized_channel(color.x),
        quantized_channel(color.y),
        quantized_channel(color.z),
    ]
}

fn quantized_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
