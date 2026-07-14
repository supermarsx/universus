use game_planet_visuals::pathtrace::{
    Camera, CpuTraceKernel, MaterialSample, PathTraceSettings, Sphere, TraceImage, TraceScene,
    TraceSurfaceControls, TraceSurfaceModel, Vec3,
};
use game_planet_visuals::{
    PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderOptions, RenderSize,
};
use image::RgbaImage;
use std::collections::{HashMap, HashSet};

const OCEAN_SEED: u64 = 0x5EED_1208_0CEA_0001;
const TRACE_SEED: u64 = 0x5EED_1208_0CEA_0002;
const GLOBAL_OCEAN_ARCHETYPE: &str = "catalog.archetype.global-ocean";

#[derive(Debug, Clone, Copy)]
struct Region {
    x0: u32,
    x1: u32,
    y0: u32,
    y1: u32,
}

#[derive(Debug, Clone, Copy)]
struct LumaStats {
    samples: usize,
    min: f32,
    max: f32,
    std_dev: f32,
    distinct_bins: usize,
}

#[derive(Debug, Clone, Copy)]
struct DirectionalDeltaStats {
    horizontal: f32,
    vertical: f32,
    diagonal_down: f32,
    diagonal_up: f32,
    min_direction: f32,
    max_direction: f32,
}

#[derive(Debug, Clone, Copy)]
struct HorizonStats {
    row: u32,
    row_delta: f32,
    sky_luma: f32,
    horizon_luma: f32,
    ground_luma: f32,
    sky_ground_color_delta: f32,
    sky_blue_bias: f32,
}

#[derive(Debug, Clone, Copy)]
struct RasterBandingStats {
    samples: usize,
    distinct_rgb_bins: usize,
    row_mean_bins: usize,
    column_mean_bins: usize,
    adjacent_row_match_fraction: f32,
    adjacent_column_match_fraction: f32,
    max_row_reuse_fraction: f32,
    max_column_reuse_fraction: f32,
}

#[derive(Debug, Clone, Copy)]
struct BandingThresholds {
    min_samples: usize,
    min_rgb_bins: usize,
    min_row_mean_bins: usize,
    min_column_mean_bins: usize,
    max_adjacent_match_fraction: f32,
    max_reuse_fraction: f32,
}

#[derive(Debug, Clone, Copy)]
struct OceanDepthBandStats {
    atmosphere_luma: f32,
    cloud_luma: f32,
    far_ocean_luma: f32,
    sky_atmosphere_delta: f32,
    atmosphere_cloud_delta: f32,
    atmosphere_far_ocean_delta: f32,
    cloud_far_ocean_delta: f32,
    far_foreground_ocean_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct TracePreviewStats {
    finite_pixels: usize,
    distinct_rgb: usize,
    luma_sum: f32,
    luma_range: f32,
    mean_adjacent_luma_delta: f32,
}

#[test]
fn ocean_terrain_overview_retains_foreground_texture_without_projection_streaks() {
    let renderer = global_ocean_renderer();
    let size = RenderSize {
        width: 192,
        height: 112,
    };
    let image = renderer.render_terrain_overview_with_options(size, RenderOptions::preview());
    let foreground = foreground_region(&image);

    let luma = luma_stats(&image, foreground);
    assert!(
        luma.samples > 4_000,
        "foreground ocean overview region should have enough samples; got {luma:?}"
    );
    assert!(
        luma.max - luma.min > 24.0,
        "foreground ocean overview should not collapse to a flat tone; got {luma:?}"
    );
    assert!(
        luma.std_dev > 7.0,
        "foreground ocean overview should retain wave/terrain contrast; got {luma:?}"
    );
    assert!(
        luma.distinct_bins >= 8,
        "foreground ocean overview should keep a broad tonal distribution; got {luma:?}"
    );

    let directions = directional_delta_stats(&image, foreground);
    let directional_ratio = directions.max_direction / directions.min_direction.max(0.001);
    assert!(
        directions.horizontal > 0.35
            && directions.vertical > 0.35
            && directions.diagonal_down > 0.35
            && directions.diagonal_up > 0.35,
        "foreground ocean overview should vary along every sampled direction; got {directions:?}"
    );
    assert!(
        directional_ratio < 5.5,
        "foreground ocean overview should not concentrate texture into projection-aligned streaks; ratio={directional_ratio:.3}, stats={directions:?}"
    );
}

#[test]
fn ocean_terrain_overview_preserves_horizon_and_atmosphere_separation() {
    let renderer = global_ocean_renderer();
    let size = RenderSize {
        width: 192,
        height: 112,
    };
    let image = renderer.render_terrain_overview_with_options(size, RenderOptions::preview());
    let horizon = horizon_stats(&image);

    assert!(
        horizon.row > size.height * 34 / 100 && horizon.row < size.height * 62 / 100,
        "terrain overview horizon should remain in the expected sky/ground band; got {horizon:?}"
    );
    assert!(
        horizon.row_delta > 7.0,
        "terrain overview should preserve a visible horizon transition; got {horizon:?}"
    );
    assert!(
        horizon.sky_luma > horizon.ground_luma + 35.0
            && horizon.horizon_luma > horizon.ground_luma + 35.0,
        "atmosphere bands should remain visually separated from terrain; got {horizon:?}"
    );
    assert!(
        horizon.sky_ground_color_delta > 24.0,
        "sky and terrain should not merge into one smooth band; got {horizon:?}"
    );
    assert!(
        horizon.sky_blue_bias > 18.0,
        "upper atmosphere should retain a blue sky bias over the ocean terrain; got {horizon:?}"
    );
}

#[test]
fn ocean_terrain_overview_rejects_visible_sky_horizon_and_water_banding() {
    let renderer = global_ocean_renderer();
    let size = RenderSize {
        width: 192,
        height: 112,
    };
    let image = renderer.render_terrain_overview_with_options(size, RenderOptions::preview());
    let horizon = horizon_stats(&image);

    let sky = Region {
        x0: image.width() / 6,
        x1: image.width() * 5 / 6,
        y0: image.height() / 20,
        y1: horizon.row.saturating_sub(10),
    };
    let horizon_air = Region {
        x0: image.width() / 6,
        x1: image.width() * 5 / 6,
        y0: horizon.row.saturating_sub(5),
        y1: (horizon.row + 8).min(image.height()),
    };
    let ocean = foreground_region(&image);

    assert_banding_is_low(
        "global-ocean terrain overview sky",
        raster_banding_stats(&image, sky),
        BandingThresholds {
            min_samples: 2_000,
            min_rgb_bins: 18,
            min_row_mean_bins: 8,
            min_column_mean_bins: 6,
            max_adjacent_match_fraction: 0.32,
            max_reuse_fraction: 0.40,
        },
    );
    assert_banding_is_low(
        "global-ocean terrain overview horizon atmosphere",
        raster_banding_stats(&image, horizon_air),
        BandingThresholds {
            min_samples: 1_100,
            min_rgb_bins: 16,
            min_row_mean_bins: 4,
            min_column_mean_bins: 8,
            max_adjacent_match_fraction: 0.28,
            max_reuse_fraction: 0.36,
        },
    );
    assert_banding_is_low(
        "global-ocean terrain overview foreground water",
        raster_banding_stats(&image, ocean),
        BandingThresholds {
            min_samples: 4_000,
            min_rgb_bins: 32,
            min_row_mean_bins: 8,
            min_column_mean_bins: 14,
            max_adjacent_match_fraction: 0.16,
            max_reuse_fraction: 0.24,
        },
    );
}

#[test]
fn ocean_terrain_overview_preserves_cloud_atmosphere_and_ocean_depth_bands() {
    let renderer = global_ocean_renderer();
    let size = RenderSize {
        width: 192,
        height: 112,
    };
    let image = renderer.render_terrain_overview_with_options(size, RenderOptions::preview());
    let horizon = horizon_stats(&image);
    let depth = ocean_depth_band_stats(&image, horizon.row);

    assert!(
        depth.sky_atmosphere_delta > 5.0,
        "refracted horizon atmosphere should not collapse into the upper sky band; got {depth:?}"
    );
    assert!(
        depth.atmosphere_cloud_delta > 3.0,
        "horizon atmosphere and cloud/mist shelf should remain separately measurable; got {depth:?}"
    );
    assert!(
        depth.atmosphere_far_ocean_delta > 14.0,
        "horizon atmosphere should stay visually separated from far ocean water; got {depth:?}"
    );
    assert!(
        depth.cloud_far_ocean_delta > 7.0,
        "cloud/mist shelf should read as a layer above the ocean rather than merging into water; got {depth:?}"
    );
    assert!(
        depth.far_foreground_ocean_delta > 4.0,
        "far ocean haze should preserve depth falloff against foreground water; got {depth:?}"
    );
    assert!(
        depth.atmosphere_luma > depth.far_ocean_luma + 18.0
            && depth.cloud_luma > depth.far_ocean_luma + 6.0,
        "ocean overview should preserve ordered atmosphere/cloud/far-water depth cues; got {depth:?}"
    );
}

#[test]
fn cpu_raytrace_preview_remains_varied_nonblank_and_deterministic() {
    let kernel = ocean_trace_kernel();
    let width = 20;
    let height = 14;
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.04, 3.15),
        Vec3::ZERO,
        Vec3::Y,
        42.0,
        width as f32 / height as f32,
    );
    let settings = PathTraceSettings {
        jitter_seed: TRACE_SEED,
        ..PathTraceSettings::preview()
    };

    let first = kernel
        .trace_image(camera, width, height, settings)
        .expect("CPU preview trace should render");
    let second = kernel
        .trace_image(camera, width, height, settings)
        .expect("same CPU preview trace should render deterministically");

    assert_eq!(first.dimensions(), (width, height));
    assert_eq!(first.pixels, second.pixels);
    assert_eq!(first.stats, second.stats);
    assert_eq!(first.plan.settings, settings);
    assert_eq!(first.plan.total_samples, u64::from(width * height));
    assert_eq!(first.stats.samples_completed, first.plan.total_samples);
    assert_eq!(first.stats.primary_rays, first.plan.total_samples);
    assert!(
        first.stats.ambient_occlusion_samples > 0,
        "CPU preview trace should sample contact AO; got {:?}",
        first.stats
    );
    assert!(
        first.stats.cloud_depth_samples > 0,
        "CPU preview trace should sample cloud/ocean depth interaction; got {:?}",
        first.stats
    );
    assert!(
        first.stats.refraction_rays > 0 && first.stats.atmosphere_samples > 0,
        "CPU preview trace should exercise ocean refraction and atmosphere sampling; got {:?}",
        first.stats
    );

    let stats = trace_preview_stats(&first);
    assert_eq!(stats.finite_pixels, first.pixels.len());
    assert!(
        stats.luma_sum > 5.0,
        "CPU preview trace should not be blank; got {stats:?}"
    );
    assert!(
        stats.luma_range > 0.11,
        "CPU preview trace should preserve tonal variation; got {stats:?}"
    );
    assert!(
        stats.distinct_rgb >= 32,
        "CPU preview trace should contain varied quantized colors; got {stats:?}"
    );
    assert!(
        stats.mean_adjacent_luma_delta > 0.006,
        "CPU preview trace should retain local pixel variation; got {stats:?}"
    );
}

fn global_ocean_renderer() -> PlanetRenderer {
    let mut profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(OCEAN_SEED)
            .with_archetype_key(GLOBAL_OCEAN_ARCHETYPE)
            .with_modifier_budget(0)
            .without_rare_modifiers(),
    );
    profile.ocean_fraction = 1.0;
    profile.ice_fraction = 0.0;
    profile.atmosphere_density = profile.atmosphere_density.max(0.82);
    profile.cloud_density = profile.cloud_density.max(0.42);
    PlanetRenderer::new(profile)
}

fn ocean_trace_kernel() -> CpuTraceKernel {
    let material = MaterialSample {
        albedo: Vec3::new(0.05, 0.24, 0.42),
        roughness: 0.05,
        metallic: 0.02,
        transmission: 0.38,
        opacity: 0.72,
        index_of_refraction: 1.333,
        ..MaterialSample::default()
    };
    CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_radius: 1.13,
            atmosphere_density: 0.66,
            light_direction: Vec3::new(-0.34, 0.58, 0.74),
            sky_color: Vec3::new(0.010, 0.018, 0.050),
            horizon_color: Vec3::new(0.46, 0.64, 0.92),
        },
        TraceSurfaceControls {
            seed: TRACE_SEED,
            surface_model: TraceSurfaceModel::Ocean,
            ocean_fraction: 1.0,
            cloud_coverage: 0.26,
            cloud_opacity: 0.36,
            atmosphere_strength: 0.90,
            ..TraceSurfaceControls::DEFAULT
        },
    )
}

fn foreground_region(image: &RgbaImage) -> Region {
    Region {
        x0: image.width() / 8,
        x1: image.width() * 7 / 8,
        y0: image.height() * 63 / 100,
        y1: image.height() * 95 / 100,
    }
}

fn luma_stats(image: &RgbaImage, region: Region) -> LumaStats {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut sum = 0.0_f32;
    let mut sum_squares = 0.0_f32;
    let mut samples = 0_usize;
    let mut bins = HashSet::new();

    for y in region.y0..region.y1 {
        for x in region.x0..region.x1 {
            let luma = pixel_luma(image.get_pixel(x, y).0);
            min = min.min(luma);
            max = max.max(luma);
            sum += luma;
            sum_squares += luma * luma;
            samples += 1;
            bins.insert((luma / 4.0).floor() as u8);
        }
    }

    let mean = sum / samples.max(1) as f32;
    let variance = (sum_squares / samples.max(1) as f32 - mean * mean).max(0.0);
    LumaStats {
        samples,
        min,
        max,
        std_dev: variance.sqrt(),
        distinct_bins: bins.len(),
    }
}

fn directional_delta_stats(image: &RgbaImage, region: Region) -> DirectionalDeltaStats {
    let horizontal = mean_directional_delta(image, region, 1, 0);
    let vertical = mean_directional_delta(image, region, 0, 1);
    let diagonal_down = mean_directional_delta(image, region, 1, 1);
    let diagonal_up = mean_directional_delta(image, region, 1, -1);
    let min_direction = horizontal.min(vertical).min(diagonal_down).min(diagonal_up);
    let max_direction = horizontal.max(vertical).max(diagonal_down).max(diagonal_up);

    DirectionalDeltaStats {
        horizontal,
        vertical,
        diagonal_down,
        diagonal_up,
        min_direction,
        max_direction,
    }
}

fn assert_banding_is_low(label: &str, stats: RasterBandingStats, thresholds: BandingThresholds) {
    assert!(
        stats.samples >= thresholds.min_samples,
        "{label} should expose enough pixels for final-image banding checks; got {stats:?}"
    );
    assert!(
        stats.distinct_rgb_bins >= thresholds.min_rgb_bins,
        "{label} should keep enough quantized RGB variation to avoid visible posterized bands; got {stats:?}"
    );
    assert!(
        stats.row_mean_bins >= thresholds.min_row_mean_bins
            && stats.column_mean_bins >= thresholds.min_column_mean_bins,
        "{label} should vary across both row and column means, not collapse into broad raster bands; got {stats:?}"
    );
    assert!(
        stats.adjacent_row_match_fraction <= thresholds.max_adjacent_match_fraction
            && stats.adjacent_column_match_fraction <= thresholds.max_adjacent_match_fraction,
        "{label} should not contain many adjacent matching quantized scanlines or columns; got {stats:?}"
    );
    assert!(
        stats.max_row_reuse_fraction <= thresholds.max_reuse_fraction
            && stats.max_column_reuse_fraction <= thresholds.max_reuse_fraction,
        "{label} should not repeatedly reuse the same quantized row or column signature; got {stats:?}"
    );
}

fn raster_banding_stats(image: &RgbaImage, region: Region) -> RasterBandingStats {
    assert!(region.x0 < region.x1, "region should have positive width");
    assert!(region.y0 < region.y1, "region should have positive height");

    let mut rgb_bins = HashSet::new();
    for y in region.y0..region.y1 {
        for x in region.x0..region.x1 {
            let pixel = image.get_pixel(x, y).0;
            rgb_bins.insert([pixel[0] >> 3, pixel[1] >> 3, pixel[2] >> 3]);
        }
    }

    let rows = row_luma_means(image, region);
    let columns = column_luma_means(image, region);
    let row_signatures = row_signatures(image, region, 3);
    let column_signatures = column_signatures(image, region, 3);
    let (_, max_row_occurrences) = repeated_signature_stats(&row_signatures);
    let (_, max_column_occurrences) = repeated_signature_stats(&column_signatures);

    RasterBandingStats {
        samples: ((region.x1 - region.x0) * (region.y1 - region.y0)) as usize,
        distinct_rgb_bins: rgb_bins.len(),
        row_mean_bins: quantized_mean_bins(&rows, 0.75),
        column_mean_bins: quantized_mean_bins(&columns, 0.75),
        adjacent_row_match_fraction: adjacent_signature_match_fraction(&row_signatures),
        adjacent_column_match_fraction: adjacent_signature_match_fraction(&column_signatures),
        max_row_reuse_fraction: max_row_occurrences as f32 / row_signatures.len().max(1) as f32,
        max_column_reuse_fraction: max_column_occurrences as f32
            / column_signatures.len().max(1) as f32,
    }
}

fn row_luma_means(image: &RgbaImage, region: Region) -> Vec<f32> {
    (region.y0..region.y1)
        .map(|y| {
            let mut total = 0.0_f32;
            let mut count = 0_u32;
            for x in region.x0..region.x1 {
                total += pixel_luma(image.get_pixel(x, y).0);
                count += 1;
            }
            total / count.max(1) as f32
        })
        .collect()
}

fn column_luma_means(image: &RgbaImage, region: Region) -> Vec<f32> {
    (region.x0..region.x1)
        .map(|x| {
            let mut total = 0.0_f32;
            let mut count = 0_u32;
            for y in region.y0..region.y1 {
                total += pixel_luma(image.get_pixel(x, y).0);
                count += 1;
            }
            total / count.max(1) as f32
        })
        .collect()
}

fn row_signatures(image: &RgbaImage, region: Region, quantize_shift: u8) -> Vec<Vec<u8>> {
    (region.y0..region.y1)
        .map(|y| {
            let mut signature = Vec::with_capacity((region.x1 - region.x0) as usize * 3);
            for x in region.x0..region.x1 {
                push_quantized_rgb(&mut signature, image.get_pixel(x, y).0, quantize_shift);
            }
            signature
        })
        .collect()
}

fn column_signatures(image: &RgbaImage, region: Region, quantize_shift: u8) -> Vec<Vec<u8>> {
    (region.x0..region.x1)
        .map(|x| {
            let mut signature = Vec::with_capacity((region.y1 - region.y0) as usize * 3);
            for y in region.y0..region.y1 {
                push_quantized_rgb(&mut signature, image.get_pixel(x, y).0, quantize_shift);
            }
            signature
        })
        .collect()
}

fn push_quantized_rgb(signature: &mut Vec<u8>, pixel: [u8; 4], quantize_shift: u8) {
    signature.push(pixel[0] >> quantize_shift);
    signature.push(pixel[1] >> quantize_shift);
    signature.push(pixel[2] >> quantize_shift);
}

fn quantized_mean_bins(values: &[f32], quantum: f32) -> usize {
    values
        .iter()
        .map(|value| (value / quantum).round() as i32)
        .collect::<HashSet<_>>()
        .len()
}

fn adjacent_signature_match_fraction(signatures: &[Vec<u8>]) -> f32 {
    signatures
        .windows(2)
        .filter(|pair| pair[0] == pair[1])
        .count() as f32
        / signatures.len().saturating_sub(1).max(1) as f32
}

fn repeated_signature_stats(signatures: &[Vec<u8>]) -> (usize, usize) {
    let mut counts: HashMap<&[u8], usize> = HashMap::new();
    for signature in signatures {
        *counts.entry(signature.as_slice()).or_insert(0) += 1;
    }

    let repeated = counts
        .values()
        .copied()
        .filter(|occurrences| *occurrences > 1)
        .sum();
    let max_occurrences = counts.values().copied().max().unwrap_or(0);
    (repeated, max_occurrences)
}

fn mean_directional_delta(image: &RgbaImage, region: Region, dx: i32, dy: i32) -> f32 {
    let mut total = 0.0_f32;
    let mut count = 0_u32;

    for y in region.y0..region.y1 {
        for x in region.x0..region.x1 {
            let neighbor_x = x as i32 + dx;
            let neighbor_y = y as i32 + dy;
            if neighbor_x < region.x0 as i32
                || neighbor_x >= region.x1 as i32
                || neighbor_y < region.y0 as i32
                || neighbor_y >= region.y1 as i32
            {
                continue;
            }

            let here = pixel_luma(image.get_pixel(x, y).0);
            let there = pixel_luma(image.get_pixel(neighbor_x as u32, neighbor_y as u32).0);
            total += (here - there).abs();
            count += 1;
        }
    }

    total / count.max(1) as f32
}

fn horizon_stats(image: &RgbaImage) -> HorizonStats {
    let x0 = image.width() / 5;
    let x1 = image.width() * 4 / 5;
    let search_y0 = image.height() * 34 / 100;
    let search_y1 = image.height() * 62 / 100;
    let mut best_row = search_y0 + 1;
    let mut best_delta = 0.0_f32;

    for y in search_y0 + 1..search_y1 {
        let delta = row_delta(image, x0, x1, y - 1, y);
        if delta > best_delta {
            best_delta = delta;
            best_row = y;
        }
    }

    let sky_region = row_band(
        image,
        x0,
        x1,
        best_row.saturating_sub(22),
        best_row.saturating_sub(10),
    );
    let horizon_region = row_band(
        image,
        x0,
        x1,
        best_row.saturating_sub(2),
        (best_row + 3).min(image.height()),
    );
    let ground_region = row_band(
        image,
        x0,
        x1,
        (best_row + 10).min(image.height()),
        (best_row + 24).min(image.height()),
    );

    HorizonStats {
        row: best_row,
        row_delta: best_delta,
        sky_luma: rgb_luma(sky_region),
        horizon_luma: rgb_luma(horizon_region),
        ground_luma: rgb_luma(ground_region),
        sky_ground_color_delta: color_delta(sky_region, ground_region),
        sky_blue_bias: sky_region.z - sky_region.x,
    }
}

fn ocean_depth_band_stats(image: &RgbaImage, horizon_row: u32) -> OceanDepthBandStats {
    let x0 = image.width() / 5;
    let x1 = image.width() * 4 / 5;
    let sky = row_band(
        image,
        x0,
        x1,
        horizon_row.saturating_sub(26),
        horizon_row.saturating_sub(14),
    );
    let atmosphere = row_band(
        image,
        x0,
        x1,
        horizon_row.saturating_sub(3),
        (horizon_row + 4).min(image.height()),
    );
    let cloud = row_band(
        image,
        x0,
        x1,
        (horizon_row + 5).min(image.height()),
        (horizon_row + 13).min(image.height()),
    );
    let far_ocean = row_band(
        image,
        x0,
        x1,
        (horizon_row + 16).min(image.height()),
        (horizon_row + 30).min(image.height()),
    );
    let foreground_ocean = row_band(
        image,
        x0,
        x1,
        image.height() * 72 / 100,
        image.height() * 92 / 100,
    );

    OceanDepthBandStats {
        atmosphere_luma: rgb_luma(atmosphere),
        cloud_luma: rgb_luma(cloud),
        far_ocean_luma: rgb_luma(far_ocean),
        sky_atmosphere_delta: color_delta(sky, atmosphere),
        atmosphere_cloud_delta: color_delta(atmosphere, cloud),
        atmosphere_far_ocean_delta: color_delta(atmosphere, far_ocean),
        cloud_far_ocean_delta: color_delta(cloud, far_ocean),
        far_foreground_ocean_delta: color_delta(far_ocean, foreground_ocean),
    }
}

fn row_delta(image: &RgbaImage, x0: u32, x1: u32, row_a: u32, row_b: u32) -> f32 {
    let mut total = 0.0_f32;
    let mut count = 0_u32;

    for x in x0..x1 {
        let a = image.get_pixel(x, row_a).0;
        let b = image.get_pixel(x, row_b).0;
        total += ((a[0] as f32 - b[0] as f32).abs()
            + (a[1] as f32 - b[1] as f32).abs()
            + (a[2] as f32 - b[2] as f32).abs())
            / 3.0;
        count += 1;
    }

    total / count.max(1) as f32
}

fn row_band(image: &RgbaImage, x0: u32, x1: u32, y0: u32, y1: u32) -> Vec3 {
    let y0 = y0.min(image.height().saturating_sub(1));
    let y1 = y1.max(y0 + 1).min(image.height());
    let mut total = Vec3::ZERO;
    let mut count = 0_u32;

    for y in y0..y1 {
        for x in x0..x1 {
            let pixel = image.get_pixel(x, y).0;
            total += Vec3::new(pixel[0] as f32, pixel[1] as f32, pixel[2] as f32);
            count += 1;
        }
    }

    total / count.max(1) as f32
}

fn trace_preview_stats(image: &TraceImage) -> TracePreviewStats {
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
            let luma = trace_luma(color);
            luma_sum += luma;
            min_luma = min_luma.min(luma);
            max_luma = max_luma.max(luma);

            if x + 1 < image.width {
                adjacent_total += (luma - trace_luma(trace_pixel(image, x + 1, y))).abs();
                adjacent_edges += 1;
            }
            if y + 1 < image.height {
                adjacent_total += (luma - trace_luma(trace_pixel(image, x, y + 1))).abs();
                adjacent_edges += 1;
            }
        }
    }

    TracePreviewStats {
        finite_pixels,
        distinct_rgb: distinct_rgb.len(),
        luma_sum,
        luma_range: max_luma - min_luma,
        mean_adjacent_luma_delta: adjacent_total / adjacent_edges.max(1) as f32,
    }
}

fn trace_pixel(image: &TraceImage, x: u32, y: u32) -> Vec3 {
    image.pixels[(y * image.width + x) as usize]
}

fn pixel_luma(pixel: [u8; 4]) -> f32 {
    pixel[0] as f32 * 0.2126 + pixel[1] as f32 * 0.7152 + pixel[2] as f32 * 0.0722
}

fn rgb_luma(color: Vec3) -> f32 {
    color.x * 0.2126 + color.y * 0.7152 + color.z * 0.0722
}

fn color_delta(left: Vec3, right: Vec3) -> f32 {
    ((left.x - right.x).abs() + (left.y - right.y).abs() + (left.z - right.z).abs()) / 3.0
}

fn trace_luma(color: Vec3) -> f32 {
    color.x * 0.2126 + color.y * 0.7152 + color.z * 0.0722
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
