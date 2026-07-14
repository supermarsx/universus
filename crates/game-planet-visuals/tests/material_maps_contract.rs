use game_planet_visuals::{PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderSize};
use image::RgbaImage;
use std::collections::HashSet;

const SEED: u64 = 0x5EED_1208_00AA;
const MAP_SIZE: RenderSize = RenderSize {
    width: 96,
    height: 48,
};
const DETAIL_MAP_SIZE: RenderSize = RenderSize {
    width: 160,
    height: 80,
};
const OCEAN_ARCHETYPE: &str = "catalog.archetype.global-ocean";
const TEMPERATE_ARCHETYPE: &str = "catalog.archetype.temperate-continents";

#[derive(Debug, Clone, Copy)]
struct ShorelineGradientStats {
    edge_count: usize,
    mean_height_delta: f32,
    open_water_edge_count: usize,
    open_water_mean_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct ReliefRegionStats {
    pixel_count: usize,
    height_range: u8,
    coarse_height_range: f32,
    mean_height_delta: f32,
    mean_normal_xy_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct MaterialMapCase {
    label: &'static str,
    expected_method: &'static str,
    render: fn(&PlanetRenderer, RenderSize) -> RgbaImage,
    min_distinct_pixels: usize,
}

fn renderer() -> PlanetRenderer {
    PlanetRenderer::new(PlanetVisualProfile::from_seed(SEED))
}

fn forced_renderer(archetype_key: &str) -> PlanetRenderer {
    PlanetRenderer::new(PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(SEED)
            .with_archetype_key(archetype_key)
            .with_modifier_budget(0)
            .without_rare_modifiers(),
    ))
}

fn material_map_cases() -> [MaterialMapCase; 4] {
    [
        MaterialMapCase {
            label: "surface normal map",
            expected_method: "PlanetRenderer::render_normal_map",
            render: PlanetRenderer::render_normal_map,
            min_distinct_pixels: 16,
        },
        MaterialMapCase {
            label: "surface height/altitude map",
            expected_method: "PlanetRenderer::render_height_map",
            render: PlanetRenderer::render_height_map,
            min_distinct_pixels: 16,
        },
        MaterialMapCase {
            label: "surface vegetation/biome map",
            expected_method: "PlanetRenderer::render_vegetation_map",
            render: PlanetRenderer::render_vegetation_map,
            min_distinct_pixels: 8,
        },
        MaterialMapCase {
            label: "surface roughness/wetness map",
            expected_method: "PlanetRenderer::render_roughness_map",
            render: PlanetRenderer::render_roughness_map,
            min_distinct_pixels: 8,
        },
    ]
}

#[test]
fn normal_and_height_maps_expose_relief_slopes_and_shoreline_structure() {
    let renderer = forced_renderer(TEMPERATE_ARCHETYPE);
    let normal = renderer.render_surface_normal_map(DETAIL_MAP_SIZE);
    let height = renderer.render_surface_height_map(DETAIL_MAP_SIZE);
    let biome = renderer.render_surface_biome_map(DETAIL_MAP_SIZE);

    for (image, label) in [
        (&normal, "surface normal map"),
        (&height, "surface height map"),
        (&biome, "surface biome/water map"),
    ] {
        assert_eq!(
            image.dimensions(),
            (DETAIL_MAP_SIZE.width, DETAIL_MAP_SIZE.height),
            "{label} should preserve requested public API dimensions"
        );
        assert_opaque(image, label);
    }

    assert!(
        channel_range(&normal, 0) > 40 && channel_range(&normal, 1) > 40,
        "normal map should carry horizontal and vertical terrain slope variation; ranges were R={}, G={}",
        channel_range(&normal, 0),
        channel_range(&normal, 1)
    );
    assert!(
        channel_range(&height, 0) > 120,
        "height map should expose broad altitude relief; range was {}",
        channel_range(&height, 0)
    );
    assert!(
        mean_adjacent_channel_delta(&height, 0) > 2.0,
        "height map should contain local slope changes, not only flat altitude bands"
    );

    let shoreline = shoreline_gradient_stats(&height, &biome);
    assert!(
        shoreline.edge_count > 128,
        "temperate public water channel should expose enough shoreline edges to evaluate relief; got {shoreline:?}"
    );
    assert!(
        shoreline.mean_height_delta > 8.0,
        "shoreline boundaries should carry visible height relief/slope; got {shoreline:?}"
    );
    assert!(
        shoreline.open_water_edge_count > 128,
        "open-water control should have enough samples for a shoreline relief comparison; got {shoreline:?}"
    );
    assert!(
        shoreline.mean_height_delta > shoreline.open_water_mean_delta * 1.35,
        "shoreline relief should stand out from open-water wave height; got {shoreline:?}"
    );
}

#[test]
fn height_and_normal_maps_expose_multiscale_land_and_water_relief() {
    let temperate_renderer = forced_renderer(TEMPERATE_ARCHETYPE);
    let temperate_height = temperate_renderer.render_surface_height_map(DETAIL_MAP_SIZE);
    let temperate_normal = temperate_renderer.render_surface_normal_map(DETAIL_MAP_SIZE);
    let temperate_biome = temperate_renderer.render_surface_biome_map(DETAIL_MAP_SIZE);
    let land = relief_region_stats(
        &temperate_height,
        &temperate_normal,
        &temperate_biome,
        |water| water <= 96,
    );

    assert!(
        land.pixel_count > (DETAIL_MAP_SIZE.width * DETAIL_MAP_SIZE.height / 3) as usize,
        "temperate terrain should expose enough land samples for relief checks; got {land:?}"
    );
    assert!(
        land.height_range > 72,
        "land height map should expose broad low-frequency altitude relief, not a plane; got {land:?}"
    );
    assert!(
        land.coarse_height_range > 18.0,
        "land height map should retain low-frequency regional relief after coarse sampling; got {land:?}"
    );
    assert!(
        land.mean_height_delta > 1.20,
        "land height map should expose high-frequency microgeometry in adjacent pixels; got {land:?}"
    );
    assert!(
        land.mean_normal_xy_delta > 0.95,
        "land normal map should expose high-frequency slope changes; got {land:?}"
    );

    let ocean_renderer = forced_renderer(OCEAN_ARCHETYPE);
    let ocean_height = ocean_renderer.render_surface_height_map(DETAIL_MAP_SIZE);
    let ocean_normal = ocean_renderer.render_surface_normal_map(DETAIL_MAP_SIZE);
    let ocean_biome = ocean_renderer.render_surface_biome_map(DETAIL_MAP_SIZE);
    let water = relief_region_stats(&ocean_height, &ocean_normal, &ocean_biome, |water| {
        water >= 220
    });

    assert!(
        water.pixel_count > (DETAIL_MAP_SIZE.width * DETAIL_MAP_SIZE.height * 85 / 100) as usize,
        "global ocean should expose enough open-water samples for wave relief checks; got {water:?}"
    );
    assert!(
        water.height_range > 7,
        "open water height map should carry wave/shelf relief, not a flat water plane; got {water:?}"
    );
    assert!(
        water.coarse_height_range > 1.8,
        "open water height map should retain low-frequency swell after coarse sampling; got {water:?}"
    );
    assert!(
        water.mean_height_delta > 0.18,
        "open water height map should expose high-frequency wave relief in adjacent pixels; got {water:?}"
    );
    assert!(
        water.mean_normal_xy_delta > 0.18,
        "open water normal map should expose wave slope changes; got {water:?}"
    );
}

#[test]
fn ocean_material_maps_expose_wet_textured_water() {
    let renderer = forced_renderer(OCEAN_ARCHETYPE);
    let biome = renderer.render_surface_biome_map(DETAIL_MAP_SIZE);
    let normal = renderer.render_surface_normal_map(DETAIL_MAP_SIZE);
    let roughness_wetness = renderer.render_surface_roughness_wetness_map(DETAIL_MAP_SIZE);
    let total_pixels = (DETAIL_MAP_SIZE.width * DETAIL_MAP_SIZE.height) as usize;
    let water_pixels = channel_count_at_least(&biome, 2, 240);

    assert!(
        water_pixels > total_pixels * 95 / 100,
        "global ocean material map should be overwhelmingly water; got {water_pixels}/{total_pixels}"
    );
    assert!(
        channel_range(&normal, 0) > 16 || channel_range(&normal, 1) > 16,
        "ocean normal map should include deterministic wave/shelf slope variation"
    );
    assert!(
        channel_range(&roughness_wetness, 0) > 32,
        "ocean roughness channel should vary with water texture; range was {}",
        channel_range(&roughness_wetness, 0)
    );
    assert!(
        channel_mean(&roughness_wetness, 1) > 220.0,
        "ocean wetness channel should classify water as wet; mean was {:.1}",
        channel_mean(&roughness_wetness, 1)
    );
    assert!(
        mean_abs_channel_delta(&roughness_wetness, 0, 1) > 120.0,
        "ocean roughness and wetness channels should encode different material signals"
    );
}

#[test]
fn roughness_wetness_map_keeps_material_channels_distinct() {
    let renderer = forced_renderer(TEMPERATE_ARCHETYPE);
    let roughness_wetness = renderer.render_surface_roughness_wetness_map(DETAIL_MAP_SIZE);

    assert!(
        channel_range(&roughness_wetness, 0) > 80,
        "roughness channel should vary across terrain and water; range was {}",
        channel_range(&roughness_wetness, 0)
    );
    assert!(
        channel_range(&roughness_wetness, 1) > 80,
        "wetness channel should vary across land, shore, and water; range was {}",
        channel_range(&roughness_wetness, 1)
    );
    assert_eq!(
        channel_range(&roughness_wetness, 2),
        0,
        "roughness/wetness map should reserve RGB channels for roughness, wetness, and currently-unused data"
    );
    assert!(
        mean_abs_channel_delta(&roughness_wetness, 0, 1) > 48.0,
        "roughness and wetness channels should not be grayscale duplicates"
    );
}

#[test]
fn material_maps_are_deterministic_opaque_nonblank_and_correctly_sized() {
    let first_renderer = renderer();
    let second_renderer = renderer();

    for case in material_map_cases() {
        let first = (case.render)(&first_renderer, MAP_SIZE);
        let second = (case.render)(&second_renderer, MAP_SIZE);

        assert_eq!(
            first.dimensions(),
            (MAP_SIZE.width, MAP_SIZE.height),
            "{} should render through {} at the requested size",
            case.label,
            case.expected_method
        );
        assert_eq!(
            first.as_raw(),
            second.as_raw(),
            "{} should be deterministic for the same profile and size",
            case.label
        );
        assert_opaque(&first, case.label);
        assert_nonblank_and_varied(&first, case.label, case.min_distinct_pixels);
    }
}

fn assert_opaque(image: &RgbaImage, label: &str) {
    let transparent = image.pixels().filter(|pixel| pixel[3] == 0).count();
    let partial = image
        .pixels()
        .filter(|pixel| pixel[3] != 0 && pixel[3] != 255)
        .count();

    assert_eq!(transparent, 0, "{label} should not have transparent pixels");
    assert_eq!(partial, 0, "{label} should not have partial-alpha pixels");
}

fn assert_nonblank_and_varied(image: &RgbaImage, label: &str, min_distinct_pixels: usize) {
    let visible_luma: u64 = image
        .pixels()
        .map(|pixel| pixel[0] as u64 + pixel[1] as u64 + pixel[2] as u64)
        .sum();
    let distinct_pixels = image
        .pixels()
        .map(|pixel| pixel.0)
        .collect::<HashSet<_>>()
        .len();

    assert!(
        visible_luma > 10_000,
        "{label} should not be blank; got luma sum {visible_luma}"
    );
    assert!(
        distinct_pixels >= min_distinct_pixels,
        "{label} should contain varied pixels; got {distinct_pixels} distinct RGBA values, expected at least {min_distinct_pixels}"
    );
}

fn channel_count_at_least(image: &RgbaImage, channel: usize, threshold: u8) -> usize {
    image
        .pixels()
        .filter(|pixel| pixel[3] > 16 && pixel[channel] >= threshold)
        .count()
}

fn channel_range(image: &RgbaImage, channel: usize) -> u8 {
    let mut min = u8::MAX;
    let mut max = u8::MIN;
    for pixel in image.pixels() {
        min = min.min(pixel[channel]);
        max = max.max(pixel[channel]);
    }
    max - min
}

fn channel_mean(image: &RgbaImage, channel: usize) -> f32 {
    let total: u64 = image.pixels().map(|pixel| pixel[channel] as u64).sum();
    total as f32 / (image.width() * image.height()).max(1) as f32
}

fn mean_abs_channel_delta(image: &RgbaImage, left_channel: usize, right_channel: usize) -> f32 {
    let total: u64 = image
        .pixels()
        .map(|pixel| pixel[left_channel].abs_diff(pixel[right_channel]) as u64)
        .sum();
    total as f32 / (image.width() * image.height()).max(1) as f32
}

fn mean_adjacent_channel_delta(image: &RgbaImage, channel: usize) -> f32 {
    let mut total = 0_u64;
    let mut edges = 0_u64;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let value = image.get_pixel(x, y)[channel];
            if x + 1 < image.width() {
                total += value.abs_diff(image.get_pixel(x + 1, y)[channel]) as u64;
                edges += 1;
            }
            if y + 1 < image.height() {
                total += value.abs_diff(image.get_pixel(x, y + 1)[channel]) as u64;
                edges += 1;
            }
        }
    }

    total as f32 / edges.max(1) as f32
}

fn relief_region_stats<F>(
    height: &RgbaImage,
    normal: &RgbaImage,
    biome: &RgbaImage,
    include_water: F,
) -> ReliefRegionStats
where
    F: Fn(u8) -> bool + Copy,
{
    assert_eq!(height.dimensions(), normal.dimensions());
    assert_eq!(height.dimensions(), biome.dimensions());

    let mut min_height = u8::MAX;
    let mut max_height = u8::MIN;
    let mut pixel_count = 0_usize;
    let mut adjacent_height_total = 0_u64;
    let mut adjacent_normal_total = 0_u64;
    let mut adjacent_edges = 0_usize;

    for y in 0..height.height() {
        for x in 0..height.width() {
            if !include_water(biome.get_pixel(x, y)[2]) {
                continue;
            }

            let h = height.get_pixel(x, y)[0];
            min_height = min_height.min(h);
            max_height = max_height.max(h);
            pixel_count += 1;

            if x + 1 < height.width() && include_water(biome.get_pixel(x + 1, y)[2]) {
                collect_relief_edge(
                    h,
                    height.get_pixel(x + 1, y)[0],
                    normal.get_pixel(x, y).0,
                    normal.get_pixel(x + 1, y).0,
                    &mut adjacent_height_total,
                    &mut adjacent_normal_total,
                    &mut adjacent_edges,
                );
            }
            if y + 1 < height.height() && include_water(biome.get_pixel(x, y + 1)[2]) {
                collect_relief_edge(
                    h,
                    height.get_pixel(x, y + 1)[0],
                    normal.get_pixel(x, y).0,
                    normal.get_pixel(x, y + 1).0,
                    &mut adjacent_height_total,
                    &mut adjacent_normal_total,
                    &mut adjacent_edges,
                );
            }
        }
    }

    ReliefRegionStats {
        pixel_count,
        height_range: if pixel_count == 0 {
            0
        } else {
            max_height - min_height
        },
        coarse_height_range: coarse_region_height_range(height, biome, include_water),
        mean_height_delta: adjacent_height_total as f32 / adjacent_edges.max(1) as f32,
        mean_normal_xy_delta: adjacent_normal_total as f32 / adjacent_edges.max(1) as f32,
    }
}

fn collect_relief_edge(
    height: u8,
    neighbor_height: u8,
    normal: [u8; 4],
    neighbor_normal: [u8; 4],
    adjacent_height_total: &mut u64,
    adjacent_normal_total: &mut u64,
    adjacent_edges: &mut usize,
) {
    *adjacent_height_total += height.abs_diff(neighbor_height) as u64;
    *adjacent_normal_total += (normal[0].abs_diff(neighbor_normal[0]) as u64
        + normal[1].abs_diff(neighbor_normal[1]) as u64)
        / 2;
    *adjacent_edges += 1;
}

fn coarse_region_height_range<F>(height: &RgbaImage, biome: &RgbaImage, include_water: F) -> f32
where
    F: Fn(u8) -> bool + Copy,
{
    let tile_width = (height.width() / 8).max(1) as usize;
    let tile_height = (height.height() / 8).max(1) as usize;
    let mut min_mean = f32::MAX;
    let mut max_mean = f32::MIN;
    let mut populated_tiles = 0_usize;

    for y0 in (0..height.height() as usize).step_by(tile_height) {
        for x0 in (0..height.width() as usize).step_by(tile_width) {
            let mut total = 0_u64;
            let mut count = 0_u32;
            let y_end = (y0 + tile_height).min(height.height() as usize);
            let x_end = (x0 + tile_width).min(height.width() as usize);

            for y in y0..y_end {
                for x in x0..x_end {
                    if include_water(biome.get_pixel(x as u32, y as u32)[2]) {
                        total += height.get_pixel(x as u32, y as u32)[0] as u64;
                        count += 1;
                    }
                }
            }

            if count >= ((x_end - x0) * (y_end - y0)).max(1) as u32 / 4 {
                let mean = total as f32 / count.max(1) as f32;
                min_mean = min_mean.min(mean);
                max_mean = max_mean.max(mean);
                populated_tiles += 1;
            }
        }
    }

    if populated_tiles == 0 {
        0.0
    } else {
        max_mean - min_mean
    }
}

fn shoreline_gradient_stats(height: &RgbaImage, biome: &RgbaImage) -> ShorelineGradientStats {
    assert_eq!(height.dimensions(), biome.dimensions());

    let mut shoreline_total = 0_u64;
    let mut shoreline_edges = 0_usize;
    let mut open_water_total = 0_u64;
    let mut open_water_edges = 0_usize;

    for y in 0..height.height() {
        for x in 0..height.width() {
            let h = height.get_pixel(x, y)[0];
            let water = biome.get_pixel(x, y)[2];
            if x + 1 < height.width() {
                let neighbor_h = height.get_pixel(x + 1, y)[0];
                let neighbor_water = biome.get_pixel(x + 1, y)[2];
                collect_water_edge_delta(
                    h,
                    neighbor_h,
                    water,
                    neighbor_water,
                    &mut shoreline_total,
                    &mut shoreline_edges,
                    &mut open_water_total,
                    &mut open_water_edges,
                );
            }
            if y + 1 < height.height() {
                let neighbor_h = height.get_pixel(x, y + 1)[0];
                let neighbor_water = biome.get_pixel(x, y + 1)[2];
                collect_water_edge_delta(
                    h,
                    neighbor_h,
                    water,
                    neighbor_water,
                    &mut shoreline_total,
                    &mut shoreline_edges,
                    &mut open_water_total,
                    &mut open_water_edges,
                );
            }
        }
    }

    ShorelineGradientStats {
        edge_count: shoreline_edges,
        mean_height_delta: shoreline_total as f32 / shoreline_edges.max(1) as f32,
        open_water_edge_count: open_water_edges,
        open_water_mean_delta: open_water_total as f32 / open_water_edges.max(1) as f32,
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_water_edge_delta(
    height: u8,
    neighbor_height: u8,
    water: u8,
    neighbor_water: u8,
    shoreline_total: &mut u64,
    shoreline_edges: &mut usize,
    open_water_total: &mut u64,
    open_water_edges: &mut usize,
) {
    let height_delta = height.abs_diff(neighbor_height) as u64;
    let water_delta = water.abs_diff(neighbor_water);

    if water_delta > 80 {
        *shoreline_total += height_delta;
        *shoreline_edges += 1;
    } else if water > 220 && neighbor_water > 220 {
        *open_water_total += height_delta;
        *open_water_edges += 1;
    }
}
