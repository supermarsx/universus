use game_planet_visuals::{
    PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderOptions, RenderSize,
};
use image::RgbaImage;

const SEED: u64 = 0x5EED_1208_0CEA;
const OCEAN_ARCHETYPE: &str = "catalog.archetype.global-ocean";
const MATERIAL_SIZE: RenderSize = RenderSize {
    width: 160,
    height: 80,
};
const WATER_THRESHOLD: u8 = 220;

#[derive(Debug, Clone, Copy)]
struct DirectionalGradientStats {
    horizontal_edges: usize,
    vertical_edges: usize,
    horizontal_mean: f32,
    vertical_mean: f32,
    axis_ratio: f32,
}

#[derive(Debug, Clone, Copy)]
struct LineRepetitionStats {
    adjacent_row_match_fraction: f32,
    adjacent_column_match_fraction: f32,
    repeated_row_fraction: f32,
    repeated_column_fraction: f32,
    max_row_occurrences: usize,
    max_column_occurrences: usize,
}

#[derive(Debug, Clone, Copy)]
struct StridePhaseStats {
    channel_range: f32,
    max_x_phase_bias: f32,
    max_y_phase_bias: f32,
    worst_x_stride: u32,
    worst_y_stride: u32,
}

fn global_ocean_renderer() -> PlanetRenderer {
    let mut profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(SEED)
            .with_archetype_key(OCEAN_ARCHETYPE)
            .with_modifier_budget(0)
            .without_rare_modifiers(),
    );
    profile.ocean_fraction = 1.0;
    PlanetRenderer::new(profile)
}

#[test]
fn global_ocean_material_maps_reject_directional_stripes_and_repeated_lines() {
    let renderer = global_ocean_renderer();
    let biome = renderer.render_surface_biome_map(MATERIAL_SIZE);
    let height = renderer.render_surface_height_map(MATERIAL_SIZE);
    let normal = renderer.render_surface_normal_map(MATERIAL_SIZE);
    let roughness = renderer.render_surface_roughness_wetness_map(MATERIAL_SIZE);
    let total_pixels = (MATERIAL_SIZE.width * MATERIAL_SIZE.height) as usize;
    let water_pixels = biome
        .pixels()
        .filter(|pixel| pixel[2] >= WATER_THRESHOLD)
        .count();

    assert!(
        water_pixels > total_pixels * 96 / 100,
        "global-ocean artifact checks require a mostly-water public biome map; got {water_pixels}/{total_pixels}"
    );

    assert_balanced_gradient(
        "global-ocean height map",
        directional_gradient_stats(&height, &biome, &[0]),
        0.05,
        10.0,
    );
    assert_balanced_gradient(
        "global-ocean normal map",
        directional_gradient_stats(&normal, &biome, &[0, 1]),
        0.05,
        10.0,
    );
    assert_balanced_gradient(
        "global-ocean roughness map",
        directional_gradient_stats(&roughness, &biome, &[0]),
        0.10,
        10.0,
    );

    assert_line_repetition_is_low(
        "global-ocean height map",
        line_repetition_stats(&height, &biome, 0, 1),
    );
    assert_line_repetition_is_low(
        "global-ocean normal map X",
        line_repetition_stats(&normal, &biome, 0, 1),
    );
    assert_line_repetition_is_low(
        "global-ocean roughness map",
        line_repetition_stats(&roughness, &biome, 0, 1),
    );
}

#[test]
fn global_ocean_material_maps_reject_pixel_stride_phase_locking() {
    let renderer = global_ocean_renderer();
    let biome = renderer.render_surface_biome_map(MATERIAL_SIZE);
    let height = renderer.render_surface_height_map(MATERIAL_SIZE);
    let normal = renderer.render_surface_normal_map(MATERIAL_SIZE);
    let roughness = renderer.render_surface_roughness_wetness_map(MATERIAL_SIZE);

    assert_stride_phase_bias_is_low(
        "global-ocean height map",
        stride_phase_stats(&height, &biome, &[0], 2..=16),
        12.0,
        0.18,
    );
    assert_stride_phase_bias_is_low(
        "global-ocean normal map",
        stride_phase_stats(&normal, &biome, &[0, 1], 2..=16),
        12.0,
        0.18,
    );
    assert_stride_phase_bias_is_low(
        "global-ocean roughness map",
        stride_phase_stats(&roughness, &biome, &[0], 2..=16),
        14.0,
        0.20,
    );
}

#[test]
fn global_ocean_material_maps_keep_multiscale_entropy() {
    let renderer = global_ocean_renderer();
    let biome = renderer.render_surface_biome_map(MATERIAL_SIZE);
    let height = renderer.render_surface_height_map(MATERIAL_SIZE);
    let normal = renderer.render_surface_normal_map(MATERIAL_SIZE);
    let roughness = renderer.render_surface_roughness_wetness_map(MATERIAL_SIZE);

    let height_entropy = masked_channel_entropy(&height, &biome, 0, 1);
    let roughness_entropy = masked_channel_entropy(&roughness, &biome, 0, 3);
    let normal_joint_entropy = masked_joint_entropy(&normal, &biome, 0, 1, 3);
    let coarse_height_entropy = coarse_tile_mean_entropy(&height, &biome, 0, 8, 4);
    let coarse_roughness_entropy = coarse_tile_mean_entropy(&roughness, &biome, 0, 8, 4);

    assert!(
        height_entropy > 1.20,
        "global-ocean height map should have nonlinear water relief distribution; entropy={height_entropy:.3}"
    );
    assert!(
        roughness_entropy > 1.35,
        "global-ocean roughness map should have varied water material distribution; entropy={roughness_entropy:.3}"
    );
    assert!(
        normal_joint_entropy > 1.80,
        "global-ocean normal map should vary both tangent axes, not collapse into a stripe field; joint entropy={normal_joint_entropy:.3}"
    );
    assert!(
        coarse_height_entropy > 0.85,
        "global-ocean height map should retain multi-scale swell after coarse tiling; entropy={coarse_height_entropy:.3}"
    );
    assert!(
        coarse_roughness_entropy > 0.85,
        "global-ocean roughness map should retain multi-scale texture after coarse tiling; entropy={coarse_roughness_entropy:.3}"
    );
}

#[test]
fn global_ocean_square_and_portrait_banners_are_not_stretched_landscape_outputs() {
    let renderer = global_ocean_renderer();
    let landscape = renderer.render_banner_with_options(
        RenderSize {
            width: 180,
            height: 100,
        },
        RenderOptions::preview(),
    );
    let square = renderer.render_banner_with_options(
        RenderSize {
            width: 128,
            height: 128,
        },
        RenderOptions::preview(),
    );
    let portrait = renderer.render_banner_with_options(
        RenderSize {
            width: 100,
            height: 180,
        },
        RenderOptions::preview(),
    );

    let square_delta = mean_luma_delta_from_resampled_source(&square, &landscape);
    let portrait_delta = mean_luma_delta_from_resampled_source(&portrait, &landscape);

    assert!(
        square_delta > 6.0,
        "square global-ocean banner should be independently framed, not a stretched landscape render; mean delta={square_delta:.3}"
    );
    assert!(
        portrait_delta > 8.0,
        "portrait global-ocean banner should be independently framed, not a stretched landscape render; mean delta={portrait_delta:.3}"
    );
}

fn assert_balanced_gradient(
    label: &str,
    stats: DirectionalGradientStats,
    min_axis_mean: f32,
    max_axis_ratio: f32,
) {
    assert!(
        stats.horizontal_edges > 1_000 && stats.vertical_edges > 1_000,
        "{label} should expose enough adjacent water samples for directional artifact checks; got {stats:?}"
    );
    assert!(
        stats.horizontal_mean >= min_axis_mean && stats.vertical_mean >= min_axis_mean,
        "{label} should vary in both image axes; min_axis_mean={min_axis_mean}, got {stats:?}"
    );
    assert!(
        stats.axis_ratio <= max_axis_ratio,
        "{label} should not concentrate water gradient energy into one raster axis; max_axis_ratio={max_axis_ratio}, got {stats:?}"
    );
}

fn assert_line_repetition_is_low(label: &str, stats: LineRepetitionStats) {
    assert!(
        stats.adjacent_row_match_fraction < 0.12 && stats.adjacent_column_match_fraction < 0.12,
        "{label} should not contain repeated adjacent scanlines or columns; got {stats:?}"
    );
    assert!(
        stats.repeated_row_fraction < 0.35 && stats.repeated_column_fraction < 0.35,
        "{label} should not reuse quantized line signatures across the map; got {stats:?}"
    );
    assert!(
        stats.max_row_occurrences <= 6 && stats.max_column_occurrences <= 6,
        "{label} should not contain a repeated stride pattern; got {stats:?}"
    );
}

fn assert_stride_phase_bias_is_low(
    label: &str,
    stats: StridePhaseStats,
    max_phase_bias: f32,
    max_range_fraction: f32,
) {
    let max_bias = stats.max_x_phase_bias.max(stats.max_y_phase_bias);
    let max_fraction = if stats.channel_range <= f32::EPSILON {
        0.0
    } else {
        max_bias / stats.channel_range
    };

    assert!(
        stats.worst_x_stride >= 2 && stats.worst_y_stride >= 2,
        "{label} should evaluate pixel phase bias across multiple raster strides; got {stats:?}"
    );
    assert!(
        max_bias <= max_phase_bias,
        "{label} should not lock water texture to repeated pixel stride phases; max_phase_bias={max_phase_bias}, got {stats:?}"
    );
    assert!(
        max_fraction <= max_range_fraction,
        "{label} should not spend too much channel range on raster phase bias; max_range_fraction={max_range_fraction}, got fraction={max_fraction:.3}, stats={stats:?}"
    );
}

fn directional_gradient_stats(
    image: &RgbaImage,
    biome: &RgbaImage,
    channels: &[usize],
) -> DirectionalGradientStats {
    assert_eq!(image.dimensions(), biome.dimensions());
    assert!(!channels.is_empty());

    let mut horizontal_total = 0.0_f32;
    let mut vertical_total = 0.0_f32;
    let mut horizontal_edges = 0_usize;
    let mut vertical_edges = 0_usize;

    for y in 0..image.height() {
        for x in 0..image.width() {
            if !is_water(biome, x, y) {
                continue;
            }

            if x + 1 < image.width() && is_water(biome, x + 1, y) {
                horizontal_total += channel_delta(image, x, y, x + 1, y, channels);
                horizontal_edges += 1;
            }
            if y + 1 < image.height() && is_water(biome, x, y + 1) {
                vertical_total += channel_delta(image, x, y, x, y + 1, channels);
                vertical_edges += 1;
            }
        }
    }

    let horizontal_mean = horizontal_total / horizontal_edges.max(1) as f32;
    let vertical_mean = vertical_total / vertical_edges.max(1) as f32;
    let axis_ratio =
        horizontal_mean.max(vertical_mean) / horizontal_mean.min(vertical_mean).max(0.001);

    DirectionalGradientStats {
        horizontal_edges,
        vertical_edges,
        horizontal_mean,
        vertical_mean,
        axis_ratio,
    }
}

fn stride_phase_stats(
    image: &RgbaImage,
    biome: &RgbaImage,
    channels: &[usize],
    strides: std::ops::RangeInclusive<u32>,
) -> StridePhaseStats {
    assert_eq!(image.dimensions(), biome.dimensions());
    assert!(!channels.is_empty());

    let channel_range = masked_channel_range(image, biome, channels);
    let mut max_x_phase_bias = 0.0_f32;
    let mut max_y_phase_bias = 0.0_f32;
    let mut worst_x_stride = 0_u32;
    let mut worst_y_stride = 0_u32;

    for stride in strides {
        let x_bias = phase_bias_for_stride(image, biome, channels, stride, Axis::X);
        if x_bias > max_x_phase_bias {
            max_x_phase_bias = x_bias;
            worst_x_stride = stride;
        }

        let y_bias = phase_bias_for_stride(image, biome, channels, stride, Axis::Y);
        if y_bias > max_y_phase_bias {
            max_y_phase_bias = y_bias;
            worst_y_stride = stride;
        }
    }

    StridePhaseStats {
        channel_range,
        max_x_phase_bias,
        max_y_phase_bias,
        worst_x_stride,
        worst_y_stride,
    }
}

#[derive(Debug, Clone, Copy)]
enum Axis {
    X,
    Y,
}

fn phase_bias_for_stride(
    image: &RgbaImage,
    biome: &RgbaImage,
    channels: &[usize],
    stride: u32,
    axis: Axis,
) -> f32 {
    let mut totals = vec![0.0_f32; stride as usize];
    let mut counts = vec![0_u32; stride as usize];

    for y in 0..image.height() {
        for x in 0..image.width() {
            if !is_water(biome, x, y) {
                continue;
            }

            let phase = match axis {
                Axis::X => x % stride,
                Axis::Y => y % stride,
            } as usize;
            totals[phase] += channel_mean_value(image.get_pixel(x, y).0, channels);
            counts[phase] += 1;
        }
    }

    let mut min_mean = f32::MAX;
    let mut max_mean = f32::MIN;
    let mut populated = 0_usize;
    for (total, count) in totals.into_iter().zip(counts) {
        if count == 0 {
            continue;
        }

        let mean = total / count as f32;
        min_mean = min_mean.min(mean);
        max_mean = max_mean.max(mean);
        populated += 1;
    }

    if populated < stride as usize {
        0.0
    } else {
        max_mean - min_mean
    }
}

fn masked_channel_range(image: &RgbaImage, biome: &RgbaImage, channels: &[usize]) -> f32 {
    let mut min_value = f32::MAX;
    let mut max_value = f32::MIN;
    let mut samples = 0_usize;

    for y in 0..image.height() {
        for x in 0..image.width() {
            if is_water(biome, x, y) {
                let value = channel_mean_value(image.get_pixel(x, y).0, channels);
                min_value = min_value.min(value);
                max_value = max_value.max(value);
                samples += 1;
            }
        }
    }

    if samples == 0 {
        0.0
    } else {
        max_value - min_value
    }
}

fn channel_mean_value(pixel: [u8; 4], channels: &[usize]) -> f32 {
    let total: u32 = channels.iter().map(|channel| pixel[*channel] as u32).sum();
    total as f32 / channels.len() as f32
}

fn channel_delta(
    image: &RgbaImage,
    x: u32,
    y: u32,
    neighbor_x: u32,
    neighbor_y: u32,
    channels: &[usize],
) -> f32 {
    let pixel = image.get_pixel(x, y);
    let neighbor = image.get_pixel(neighbor_x, neighbor_y);
    let total: u32 = channels
        .iter()
        .map(|channel| pixel[*channel].abs_diff(neighbor[*channel]) as u32)
        .sum();
    total as f32 / channels.len() as f32
}

fn line_repetition_stats(
    image: &RgbaImage,
    biome: &RgbaImage,
    channel: usize,
    quantize_shift: u8,
) -> LineRepetitionStats {
    assert_eq!(image.dimensions(), biome.dimensions());

    let rows = row_signatures(image, biome, channel, quantize_shift);
    let columns = column_signatures(image, biome, channel, quantize_shift);
    let adjacent_row_matches = adjacent_match_count(&rows);
    let adjacent_column_matches = adjacent_match_count(&columns);
    let (max_row_occurrences, repeated_row_fraction) = repeated_signature_stats(&rows);
    let (max_column_occurrences, repeated_column_fraction) = repeated_signature_stats(&columns);

    LineRepetitionStats {
        adjacent_row_match_fraction: adjacent_row_matches as f32
            / rows.len().saturating_sub(1).max(1) as f32,
        adjacent_column_match_fraction: adjacent_column_matches as f32
            / columns.len().saturating_sub(1).max(1) as f32,
        repeated_row_fraction,
        repeated_column_fraction,
        max_row_occurrences,
        max_column_occurrences,
    }
}

fn row_signatures(
    image: &RgbaImage,
    biome: &RgbaImage,
    channel: usize,
    quantize_shift: u8,
) -> Vec<Vec<u8>> {
    (0..image.height())
        .map(|y| {
            (0..image.width())
                .map(|x| quantized_masked_value(image, biome, x, y, channel, quantize_shift))
                .collect()
        })
        .collect()
}

fn column_signatures(
    image: &RgbaImage,
    biome: &RgbaImage,
    channel: usize,
    quantize_shift: u8,
) -> Vec<Vec<u8>> {
    (0..image.width())
        .map(|x| {
            (0..image.height())
                .map(|y| quantized_masked_value(image, biome, x, y, channel, quantize_shift))
                .collect()
        })
        .collect()
}

fn quantized_masked_value(
    image: &RgbaImage,
    biome: &RgbaImage,
    x: u32,
    y: u32,
    channel: usize,
    quantize_shift: u8,
) -> u8 {
    if is_water(biome, x, y) {
        image.get_pixel(x, y)[channel] >> quantize_shift
    } else {
        u8::MAX >> quantize_shift
    }
}

fn adjacent_match_count(signatures: &[Vec<u8>]) -> usize {
    signatures
        .windows(2)
        .filter(|pair| pair[0] == pair[1])
        .count()
}

fn repeated_signature_stats(signatures: &[Vec<u8>]) -> (usize, f32) {
    if signatures.is_empty() {
        return (0, 0.0);
    }

    let mut sorted = signatures.to_vec();
    sorted.sort_unstable();

    let mut max_occurrences = 1_usize;
    let mut repeated = 0_usize;
    let mut index = 0_usize;
    while index < sorted.len() {
        let start = index;
        index += 1;
        while index < sorted.len() && sorted[index] == sorted[start] {
            index += 1;
        }

        let occurrences = index - start;
        max_occurrences = max_occurrences.max(occurrences);
        if occurrences > 1 {
            repeated += occurrences;
        }
    }

    (max_occurrences, repeated as f32 / signatures.len() as f32)
}

fn masked_channel_entropy(
    image: &RgbaImage,
    biome: &RgbaImage,
    channel: usize,
    quantize_shift: u8,
) -> f32 {
    assert_eq!(image.dimensions(), biome.dimensions());

    let bin_count = 256_usize >> quantize_shift;
    let mut bins = vec![0_usize; bin_count.max(1)];
    let mut samples = 0_usize;

    for y in 0..image.height() {
        for x in 0..image.width() {
            if is_water(biome, x, y) {
                let bin = (image.get_pixel(x, y)[channel] >> quantize_shift) as usize;
                bins[bin] += 1;
                samples += 1;
            }
        }
    }

    entropy(&bins, samples)
}

fn masked_joint_entropy(
    image: &RgbaImage,
    biome: &RgbaImage,
    left_channel: usize,
    right_channel: usize,
    quantize_shift: u8,
) -> f32 {
    assert_eq!(image.dimensions(), biome.dimensions());

    let bins_per_axis = 256_usize >> quantize_shift;
    let mut bins = vec![0_usize; bins_per_axis.max(1) * bins_per_axis.max(1)];
    let mut samples = 0_usize;

    for y in 0..image.height() {
        for x in 0..image.width() {
            if is_water(biome, x, y) {
                let pixel = image.get_pixel(x, y);
                let left = (pixel[left_channel] >> quantize_shift) as usize;
                let right = (pixel[right_channel] >> quantize_shift) as usize;
                bins[left * bins_per_axis + right] += 1;
                samples += 1;
            }
        }
    }

    entropy(&bins, samples)
}

fn coarse_tile_mean_entropy(
    image: &RgbaImage,
    biome: &RgbaImage,
    channel: usize,
    tile_columns: u32,
    tile_rows: u32,
) -> f32 {
    assert_eq!(image.dimensions(), biome.dimensions());

    let mut bins = vec![0_usize; 512];
    let mut samples = 0_usize;

    for tile_y in 0..tile_rows {
        for tile_x in 0..tile_columns {
            let x0 = tile_x * image.width() / tile_columns;
            let x1 = (tile_x + 1) * image.width() / tile_columns;
            let y0 = tile_y * image.height() / tile_rows;
            let y1 = (tile_y + 1) * image.height() / tile_rows;
            let mut total = 0_u64;
            let mut count = 0_u32;

            for y in y0..y1 {
                for x in x0..x1 {
                    if is_water(biome, x, y) {
                        total += image.get_pixel(x, y)[channel] as u64;
                        count += 1;
                    }
                }
            }

            let tile_area = (x1 - x0).max(1) * (y1 - y0).max(1);
            if count < tile_area / 3 {
                continue;
            }

            let mean = total as f32 / count.max(1) as f32;
            let bin = (mean * 2.0).floor().clamp(0.0, 511.0) as usize;
            bins[bin] += 1;
            samples += 1;
        }
    }

    entropy(&bins, samples)
}

fn entropy(bins: &[usize], samples: usize) -> f32 {
    if samples == 0 {
        return 0.0;
    }

    bins.iter()
        .copied()
        .filter(|count| *count > 0)
        .map(|count| {
            let p = count as f32 / samples as f32;
            -p * p.log2()
        })
        .sum()
}

fn mean_luma_delta_from_resampled_source(target: &RgbaImage, source: &RgbaImage) -> f32 {
    let mut total = 0.0_f32;
    let mut samples = 0_usize;

    for y in 0..target.height() {
        for x in 0..target.width() {
            let sx =
                ((x as f32 + 0.5) * source.width() as f32 / target.width().max(1) as f32) - 0.5;
            let sy =
                ((y as f32 + 0.5) * source.height() as f32 / target.height().max(1) as f32) - 0.5;
            let target_luma = pixel_luma(target.get_pixel(x, y).0);
            let source_luma = bilinear_luma(source, sx, sy);
            total += (target_luma - source_luma).abs();
            samples += 1;
        }
    }

    total / samples.max(1) as f32
}

fn bilinear_luma(image: &RgbaImage, x: f32, y: f32) -> f32 {
    let x = x.clamp(0.0, image.width().saturating_sub(1) as f32);
    let y = y.clamp(0.0, image.height().saturating_sub(1) as f32);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(image.width().saturating_sub(1));
    let y1 = (y0 + 1).min(image.height().saturating_sub(1));
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let top = lerp(
        pixel_luma(image.get_pixel(x0, y0).0),
        pixel_luma(image.get_pixel(x1, y0).0),
        tx,
    );
    let bottom = lerp(
        pixel_luma(image.get_pixel(x0, y1).0),
        pixel_luma(image.get_pixel(x1, y1).0),
        tx,
    );

    lerp(top, bottom, ty)
}

fn pixel_luma(pixel: [u8; 4]) -> f32 {
    pixel[0] as f32 * 0.2126 + pixel[1] as f32 * 0.7152 + pixel[2] as f32 * 0.0722
}

fn is_water(biome: &RgbaImage, x: u32, y: u32) -> bool {
    biome.get_pixel(x, y)[2] >= WATER_THRESHOLD
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
