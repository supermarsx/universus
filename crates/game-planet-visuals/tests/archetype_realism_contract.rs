use game_planet_visuals::{
    PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderExecutionMode, RenderOptions,
    RenderSize,
};
use image::RgbaImage;
use std::collections::HashSet;

const SEED: u64 = 0x5EED_1208_A11C;
const MAP_SIZE: RenderSize = RenderSize {
    width: 128,
    height: 64,
};
const OVERVIEW_SIZE: RenderSize = RenderSize {
    width: 160,
    height: 96,
};
const OCEAN_ARCHETYPE: &str = "catalog.archetype.global-ocean";
const VOLCANIC_ARCHETYPE: &str = "catalog.archetype.active-volcanic";
const ROCKY_ARCHETYPE: &str = "catalog.archetype.mars-like";

#[derive(Debug)]
struct ArchetypeSignature {
    profile: PlanetVisualProfile,
    terrain: TerrainStats,
    material: MaterialStats,
    surface: ColorStats,
    overview: ColorStats,
    horizon: HorizonStats,
    foreground_direction: DirectionalDeltaStats,
}

#[derive(Debug, Clone, Copy)]
struct TerrainStats {
    mean_height: f32,
    height_std_dev: f32,
    height_range: u8,
    coarse_height_range: f32,
    mean_height_delta: f32,
    mean_normal_xy_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct MaterialStats {
    water_ratio: f32,
    deep_water_ratio: f32,
    vegetation_mean: f32,
    vegetation_ratio: f32,
    roughness_mean: f32,
    roughness_range: u8,
    wetness_mean: f32,
}

#[derive(Debug, Clone, Copy)]
struct ColorStats {
    mean_rgb: [f32; 3],
    luma_range: f32,
    distinct_rgb: usize,
    mean_adjacent_luma_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct HorizonStats {
    row: u32,
    edge_delta: f32,
    sky_luma: f32,
    ground_luma: f32,
    sky_ground_color_delta: f32,
    sky_blue_bias: f32,
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

#[test]
fn archetypes_have_distinct_deterministic_public_realism_signatures() {
    let ocean = archetype_signature("ocean", OCEAN_ARCHETYPE);
    let volcanic = archetype_signature("volcanic", VOLCANIC_ARCHETYPE);
    let rocky = archetype_signature("rocky", ROCKY_ARCHETYPE);

    assert_eq!(ocean.profile.archetype_key, OCEAN_ARCHETYPE);
    assert_eq!(volcanic.profile.archetype_key, VOLCANIC_ARCHETYPE);
    assert_eq!(rocky.profile.archetype_key, ROCKY_ARCHETYPE);

    assert_ne!(ocean.profile.class_key, volcanic.profile.class_key);
    assert_ne!(ocean.profile.class_key, rocky.profile.class_key);
    assert_ne!(volcanic.profile.class_key, rocky.profile.class_key);
    assert_ne!(ocean.profile.render_model, volcanic.profile.render_model);
    assert_ne!(ocean.profile.render_model, rocky.profile.render_model);
    assert_ne!(volcanic.profile.render_model, rocky.profile.render_model);

    assert_signature_distance(
        "ocean/volcanic terrain",
        &ocean.terrain,
        &volcanic.terrain,
        42.0,
    );
    assert_signature_distance("ocean/rocky terrain", &ocean.terrain, &rocky.terrain, 34.0);
    assert_signature_distance(
        "volcanic/rocky terrain",
        &volcanic.terrain,
        &rocky.terrain,
        12.0,
    );
    assert_material_distance(
        "ocean/volcanic material",
        &ocean.material,
        &volcanic.material,
        96.0,
    );
    assert_material_distance(
        "ocean/rocky material",
        &ocean.material,
        &rocky.material,
        96.0,
    );
    assert_material_distance(
        "volcanic/rocky material",
        &volcanic.material,
        &rocky.material,
        14.0,
    );
    assert_color_distance(
        "ocean/volcanic surface",
        &ocean.surface,
        &volcanic.surface,
        24.0,
    );
    assert_color_distance("ocean/rocky surface", &ocean.surface, &rocky.surface, 22.0);
    assert_color_distance(
        "volcanic/rocky surface",
        &volcanic.surface,
        &rocky.surface,
        7.0,
    );
    assert_color_distance(
        "ocean/volcanic overview",
        &ocean.overview,
        &volcanic.overview,
        16.0,
    );
    assert_color_distance(
        "ocean/rocky overview",
        &ocean.overview,
        &rocky.overview,
        14.0,
    );
    assert_color_distance(
        "volcanic/rocky overview",
        &volcanic.overview,
        &rocky.overview,
        8.0,
    );

    assert!(
        volcanic.profile.volcanic_activity >= 0.58,
        "volcanic archetype should advertise strong public volcanic activity; got {}",
        volcanic.profile.volcanic_activity
    );
    assert!(
        volcanic.terrain.height_range > 120,
        "volcanic height map should expose high relief; got {:?}",
        volcanic.terrain
    );
    assert!(
        volcanic.terrain.coarse_height_range > 38.0,
        "volcanic height map should keep regional relief, not only pixel noise; got {:?}",
        volcanic.terrain
    );
    assert!(
        volcanic.terrain.mean_normal_xy_delta > 1.20,
        "volcanic normal map should show rough slope changes; got {:?}",
        volcanic.terrain
    );
    assert!(
        volcanic.material.roughness_mean > 150.0,
        "volcanic roughness map should read as rugged terrain; got {:?}",
        volcanic.material
    );
    assert!(
        volcanic.material.vegetation_mean < 32.0 && volcanic.material.vegetation_ratio < 0.18,
        "volcanic material map should keep vegetation low; got {:?}",
        volcanic.material
    );
    assert!(
        volcanic.material.water_ratio < 0.14,
        "volcanic material map should remain dry; got {:?}",
        volcanic.material
    );

    assert!(
        rocky.profile.ocean_fraction <= 0.04,
        "rocky archetype should advertise sparse water; got {}",
        rocky.profile.ocean_fraction
    );
    assert!(
        rocky.material.water_ratio < 0.06 && rocky.material.wetness_mean < 22.0,
        "rocky material map should keep water sparse; got {:?}",
        rocky.material
    );
    assert!(
        rocky.material.roughness_mean > 155.0 && rocky.material.roughness_range > 58,
        "rocky material map should expose rough terrain; got {:?}",
        rocky.material
    );
    assert!(
        rocky.terrain.height_range > 110
            && rocky.terrain.mean_height_delta > 1.25
            && rocky.terrain.mean_normal_xy_delta > 1.10,
        "rocky height/normal maps should retain cratered terrain roughness; got {:?}",
        rocky.terrain
    );

    assert!(
        ocean.profile.ocean_fraction >= 0.72,
        "ocean archetype should advertise high water coverage; got {}",
        ocean.profile.ocean_fraction
    );
    assert!(
        ocean.profile.atmosphere_density >= 0.52,
        "ocean archetype should advertise a visible atmosphere; got {}",
        ocean.profile.atmosphere_density
    );
    assert!(
        ocean.material.water_ratio > 0.86 && ocean.material.deep_water_ratio > 0.70,
        "ocean biome map should remain water dominated; got {:?}",
        ocean.material
    );
    assert!(
        ocean.material.wetness_mean > 190.0,
        "ocean roughness/wetness map should preserve wet water material; got {:?}",
        ocean.material
    );
    assert!(
        ocean.terrain.height_range > 20
            && ocean.terrain.coarse_height_range > 5.0
            && ocean.terrain.mean_height_delta > 0.18,
        "ocean height map should preserve deterministic water depth and swell; got {:?}",
        ocean.terrain
    );
    assert!(
        ocean.horizon.row > OVERVIEW_SIZE.height * 32 / 100
            && ocean.horizon.row < OVERVIEW_SIZE.height * 66 / 100,
        "ocean overview horizon should remain in the expected atmosphere/terrain band; got {:?}",
        ocean.horizon
    );
    assert!(
        ocean.horizon.edge_delta > 5.5
            && ocean.horizon.sky_luma > ocean.horizon.ground_luma + 28.0
            && ocean.horizon.sky_ground_color_delta > 18.0
            && ocean.horizon.sky_blue_bias > 14.0,
        "ocean overview should preserve atmosphere separation over water; got {:?}",
        ocean.horizon
    );

    let stripe_ratio = ocean.foreground_direction.max_direction
        / ocean.foreground_direction.min_direction.max(0.001);
    assert!(
        ocean.foreground_direction.horizontal > 0.28
            && ocean.foreground_direction.vertical > 0.28
            && ocean.foreground_direction.diagonal_down > 0.28
            && ocean.foreground_direction.diagonal_up > 0.28,
        "ocean overview foreground should retain texture in every sampled direction; got {:?}",
        ocean.foreground_direction
    );
    assert!(
        stripe_ratio < 5.2,
        "ocean overview foreground should not collapse into stripe artifacts; ratio={stripe_ratio:.3}, got {:?}",
        ocean.foreground_direction
    );
}

fn archetype_signature(label: &'static str, archetype_key: &str) -> ArchetypeSignature {
    let first_profile = forced_profile(archetype_key);
    let second_profile = forced_profile(archetype_key);
    assert_eq!(
        first_profile, second_profile,
        "{label} profile generation should be deterministic"
    );
    let renderer = PlanetRenderer::new(first_profile.clone());

    let height = deterministic_image(label, "height map", &renderer, |renderer| {
        renderer.render_surface_height_map_with_progress(
            MAP_SIZE,
            RenderExecutionMode::Automatic,
            |_| {},
        )
    });
    let normal = deterministic_image(label, "normal map", &renderer, |renderer| {
        renderer.render_surface_normal_map_with_progress(
            MAP_SIZE,
            RenderExecutionMode::Automatic,
            |_| {},
        )
    });
    let biome = deterministic_image(label, "biome map", &renderer, |renderer| {
        renderer.render_surface_biome_map_with_progress(
            MAP_SIZE,
            RenderExecutionMode::Automatic,
            |_| {},
        )
    });
    let roughness = deterministic_image(label, "roughness/wetness map", &renderer, |renderer| {
        renderer.render_surface_roughness_wetness_map_with_progress(
            MAP_SIZE,
            RenderExecutionMode::Automatic,
            |_| {},
        )
    });
    let surface = deterministic_image(label, "surface map", &renderer, |renderer| {
        renderer.render_surface_map_with_progress(MAP_SIZE, RenderExecutionMode::Automatic, |_| {})
    });
    let overview = deterministic_image(label, "terrain overview", &renderer, |renderer| {
        renderer.render_terrain_overview_with_progress(
            OVERVIEW_SIZE,
            RenderOptions::preview(),
            RenderExecutionMode::Automatic,
            |_| {},
        )
    });

    ArchetypeSignature {
        profile: first_profile,
        terrain: terrain_stats(&height, &normal),
        material: material_stats(&biome, &roughness),
        surface: color_stats(&surface),
        overview: color_stats_in_region(&overview, foreground_region(&overview)),
        horizon: horizon_stats(&overview),
        foreground_direction: directional_delta_stats(&overview, foreground_region(&overview)),
    }
}

fn forced_profile(archetype_key: &str) -> PlanetVisualProfile {
    PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(SEED)
            .with_archetype_key(archetype_key)
            .with_modifier_budget(0)
            .without_rare_modifiers(),
    )
}

fn deterministic_image<F>(
    archetype_label: &str,
    image_label: &str,
    renderer: &PlanetRenderer,
    render: F,
) -> RgbaImage
where
    F: Fn(&PlanetRenderer) -> RgbaImage,
{
    let first_image = render(renderer);
    let second_image = render(renderer);
    assert_eq!(
        first_image.dimensions(),
        second_image.dimensions(),
        "{archetype_label} {image_label} deterministic control should preserve dimensions"
    );
    assert_eq!(
        first_image.as_raw(),
        second_image.as_raw(),
        "{archetype_label} {image_label} should be deterministic through the public API"
    );
    assert!(
        first_image.pixels().all(|pixel| pixel[3] > 16),
        "{archetype_label} {image_label} should render visible public pixels"
    );
    first_image
}

fn terrain_stats(height: &RgbaImage, normal: &RgbaImage) -> TerrainStats {
    assert_eq!(height.dimensions(), normal.dimensions());

    let mut min_height = u8::MAX;
    let mut max_height = u8::MIN;
    let mut height_total = 0.0_f32;
    let mut height_square_total = 0.0_f32;
    let mut pixels = 0_u32;
    let mut height_delta_total = 0_u64;
    let mut normal_delta_total = 0_u64;
    let mut edges = 0_u64;

    for y in 0..height.height() {
        for x in 0..height.width() {
            let current_height = height.get_pixel(x, y)[0];
            let current_normal = normal.get_pixel(x, y).0;
            min_height = min_height.min(current_height);
            max_height = max_height.max(current_height);
            height_total += current_height as f32;
            height_square_total += (current_height as f32).powi(2);
            pixels += 1;

            if x + 1 < height.width() {
                collect_terrain_edge(
                    current_height,
                    height.get_pixel(x + 1, y)[0],
                    current_normal,
                    normal.get_pixel(x + 1, y).0,
                    &mut height_delta_total,
                    &mut normal_delta_total,
                    &mut edges,
                );
            }
            if y + 1 < height.height() {
                collect_terrain_edge(
                    current_height,
                    height.get_pixel(x, y + 1)[0],
                    current_normal,
                    normal.get_pixel(x, y + 1).0,
                    &mut height_delta_total,
                    &mut normal_delta_total,
                    &mut edges,
                );
            }
        }
    }

    let mean_height = height_total / pixels.max(1) as f32;
    let variance =
        (height_square_total / pixels.max(1) as f32 - mean_height * mean_height).max(0.0);
    TerrainStats {
        mean_height,
        height_std_dev: variance.sqrt(),
        height_range: max_height - min_height,
        coarse_height_range: coarse_channel_range(height, 0),
        mean_height_delta: height_delta_total as f32 / edges.max(1) as f32,
        mean_normal_xy_delta: normal_delta_total as f32 / edges.max(1) as f32,
    }
}

fn collect_terrain_edge(
    height: u8,
    neighbor_height: u8,
    normal: [u8; 4],
    neighbor_normal: [u8; 4],
    height_delta_total: &mut u64,
    normal_delta_total: &mut u64,
    edges: &mut u64,
) {
    *height_delta_total += height.abs_diff(neighbor_height) as u64;
    *normal_delta_total += (normal[0].abs_diff(neighbor_normal[0]) as u64
        + normal[1].abs_diff(neighbor_normal[1]) as u64)
        / 2;
    *edges += 1;
}

fn material_stats(biome: &RgbaImage, roughness: &RgbaImage) -> MaterialStats {
    assert_eq!(biome.dimensions(), roughness.dimensions());

    MaterialStats {
        water_ratio: channel_ratio_at_least(biome, 2, 128),
        deep_water_ratio: channel_ratio_at_least(biome, 2, 220),
        vegetation_mean: channel_mean(biome, 1),
        vegetation_ratio: channel_ratio_at_least(biome, 1, 64),
        roughness_mean: channel_mean(roughness, 0),
        roughness_range: channel_range(roughness, 0),
        wetness_mean: channel_mean(roughness, 1),
    }
}

fn color_stats(image: &RgbaImage) -> ColorStats {
    color_stats_in_region(image, (0, image.width(), 0, image.height()))
}

fn color_stats_in_region(image: &RgbaImage, (x0, x1, y0, y1): (u32, u32, u32, u32)) -> ColorStats {
    let mut min_luma = f32::MAX;
    let mut max_luma = f32::MIN;
    let mut total_rgb = [0.0_f32; 3];
    let mut visible = 0_u32;
    let mut distinct_rgb = HashSet::new();
    let mut adjacent_luma_total = 0.0;
    let mut adjacent_edges = 0_u32;

    for y in y0..y1 {
        for x in x0..x1 {
            let pixel = image.get_pixel(x, y).0;
            if pixel[3] <= 16 {
                continue;
            }

            let luma = pixel_luma(pixel);
            min_luma = min_luma.min(luma);
            max_luma = max_luma.max(luma);
            total_rgb[0] += pixel[0] as f32;
            total_rgb[1] += pixel[1] as f32;
            total_rgb[2] += pixel[2] as f32;
            distinct_rgb.insert([pixel[0], pixel[1], pixel[2]]);
            visible += 1;

            if x + 1 < x1 {
                let neighbor = image.get_pixel(x + 1, y).0;
                if neighbor[3] > 16 {
                    adjacent_luma_total += (luma - pixel_luma(neighbor)).abs();
                    adjacent_edges += 1;
                }
            }
            if y + 1 < y1 {
                let neighbor = image.get_pixel(x, y + 1).0;
                if neighbor[3] > 16 {
                    adjacent_luma_total += (luma - pixel_luma(neighbor)).abs();
                    adjacent_edges += 1;
                }
            }
        }
    }

    let visible = visible.max(1) as f32;
    ColorStats {
        mean_rgb: [
            total_rgb[0] / visible,
            total_rgb[1] / visible,
            total_rgb[2] / visible,
        ],
        luma_range: if visible > 0.0 {
            max_luma - min_luma
        } else {
            0.0
        },
        distinct_rgb: distinct_rgb.len(),
        mean_adjacent_luma_delta: adjacent_luma_total / adjacent_edges.max(1) as f32,
    }
}

fn foreground_region(image: &RgbaImage) -> (u32, u32, u32, u32) {
    (
        image.width() / 8,
        image.width() * 7 / 8,
        image.height() * 63 / 100,
        image.height() * 95 / 100,
    )
}

fn directional_delta_stats(
    image: &RgbaImage,
    region: (u32, u32, u32, u32),
) -> DirectionalDeltaStats {
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

fn mean_directional_delta(
    image: &RgbaImage,
    (x0, x1, y0, y1): (u32, u32, u32, u32),
    dx: i32,
    dy: i32,
) -> f32 {
    let mut total = 0.0_f32;
    let mut count = 0_u32;

    for y in y0..y1 {
        for x in x0..x1 {
            let neighbor_x = x as i32 + dx;
            let neighbor_y = y as i32 + dy;
            if neighbor_x < x0 as i32
                || neighbor_x >= x1 as i32
                || neighbor_y < y0 as i32
                || neighbor_y >= y1 as i32
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
    let search_y0 = image.height() * 32 / 100;
    let search_y1 = image.height() * 66 / 100;
    let mut best_row = search_y0 + 1;
    let mut best_delta = 0.0_f32;

    for y in search_y0 + 1..search_y1 {
        let delta = row_delta(image, x0, x1, y - 1, y);
        if delta > best_delta {
            best_delta = delta;
            best_row = y;
        }
    }

    let sky = mean_rgb_in_rows(
        image,
        x0,
        x1,
        best_row.saturating_sub(22),
        best_row.saturating_sub(10),
    );
    let ground = mean_rgb_in_rows(
        image,
        x0,
        x1,
        (best_row + 10).min(image.height()),
        (best_row + 24).min(image.height()),
    );

    HorizonStats {
        row: best_row,
        edge_delta: best_delta,
        sky_luma: rgb_luma(sky),
        ground_luma: rgb_luma(ground),
        sky_ground_color_delta: rgb_mean_delta(sky, ground),
        sky_blue_bias: sky[2] - sky[0],
    }
}

fn row_delta(image: &RgbaImage, x0: u32, x1: u32, row_a: u32, row_b: u32) -> f32 {
    let mut total = 0.0_f32;
    let mut count = 0_u32;

    for x in x0..x1 {
        let a = image.get_pixel(x, row_a).0;
        let b = image.get_pixel(x, row_b).0;
        total += rgb_mean_delta(
            [a[0] as f32, a[1] as f32, a[2] as f32],
            [b[0] as f32, b[1] as f32, b[2] as f32],
        );
        count += 1;
    }

    total / count.max(1) as f32
}

fn mean_rgb_in_rows(image: &RgbaImage, x0: u32, x1: u32, y0: u32, y1: u32) -> [f32; 3] {
    let y0 = y0.min(image.height().saturating_sub(1));
    let y1 = y1.max(y0 + 1).min(image.height());
    let mut total = [0.0_f32; 3];
    let mut count = 0_u32;

    for y in y0..y1 {
        for x in x0..x1 {
            let pixel = image.get_pixel(x, y).0;
            total[0] += pixel[0] as f32;
            total[1] += pixel[1] as f32;
            total[2] += pixel[2] as f32;
            count += 1;
        }
    }

    [
        total[0] / count.max(1) as f32,
        total[1] / count.max(1) as f32,
        total[2] / count.max(1) as f32,
    ]
}

fn channel_ratio_at_least(image: &RgbaImage, channel: usize, threshold: u8) -> f32 {
    let mut visible = 0_u32;
    let mut matching = 0_u32;

    for pixel in image.pixels() {
        if pixel[3] <= 16 {
            continue;
        }
        visible += 1;
        if pixel[channel] >= threshold {
            matching += 1;
        }
    }

    matching as f32 / visible.max(1) as f32
}

fn channel_mean(image: &RgbaImage, channel: usize) -> f32 {
    let total: u64 = image.pixels().map(|pixel| pixel[channel] as u64).sum();
    total as f32 / (image.width() * image.height()).max(1) as f32
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

fn coarse_channel_range(image: &RgbaImage, channel: usize) -> f32 {
    let tile_width = (image.width() / 8).max(1);
    let tile_height = (image.height() / 8).max(1);
    let mut min_mean = f32::MAX;
    let mut max_mean = f32::MIN;

    for y0 in (0..image.height()).step_by(tile_height as usize) {
        for x0 in (0..image.width()).step_by(tile_width as usize) {
            let mut total = 0_u64;
            let mut count = 0_u32;
            for y in y0..(y0 + tile_height).min(image.height()) {
                for x in x0..(x0 + tile_width).min(image.width()) {
                    total += image.get_pixel(x, y)[channel] as u64;
                    count += 1;
                }
            }
            let mean = total as f32 / count.max(1) as f32;
            min_mean = min_mean.min(mean);
            max_mean = max_mean.max(mean);
        }
    }

    max_mean - min_mean
}

fn assert_signature_distance(label: &str, left: &TerrainStats, right: &TerrainStats, minimum: f32) {
    let distance = left.height_range.abs_diff(right.height_range) as f32
        + (left.mean_height - right.mean_height).abs() * 0.35
        + (left.height_std_dev - right.height_std_dev).abs() * 0.50
        + (left.coarse_height_range - right.coarse_height_range).abs()
        + (left.mean_height_delta - right.mean_height_delta).abs() * 6.0
        + (left.mean_normal_xy_delta - right.mean_normal_xy_delta).abs() * 6.0;
    assert!(
        distance > minimum,
        "{label} signatures should be distinct; distance={distance:.3}, left={left:?}, right={right:?}"
    );
}

fn assert_material_distance(
    label: &str,
    left: &MaterialStats,
    right: &MaterialStats,
    minimum: f32,
) {
    let distance = (left.water_ratio - right.water_ratio).abs() * 120.0
        + (left.deep_water_ratio - right.deep_water_ratio).abs() * 80.0
        + (left.vegetation_mean - right.vegetation_mean).abs()
        + (left.roughness_mean - right.roughness_mean).abs()
        + (left.wetness_mean - right.wetness_mean).abs() * 0.5;
    assert!(
        distance > minimum,
        "{label} signatures should be distinct; distance={distance:.3}, left={left:?}, right={right:?}"
    );
}

fn assert_color_distance(label: &str, left: &ColorStats, right: &ColorStats, minimum: f32) {
    let distance = rgb_mean_delta(left.mean_rgb, right.mean_rgb)
        + (left.luma_range - right.luma_range).abs() * 0.12
        + (left.mean_adjacent_luma_delta - right.mean_adjacent_luma_delta).abs() * 2.0;
    assert!(
        distance > minimum,
        "{label} color signatures should be distinct; distance={distance:.3}, left={left:?}, right={right:?}"
    );
    assert!(
        left.distinct_rgb > 32 && right.distinct_rgb > 32,
        "{label} images should be data-bearing, not flat fills; left={left:?}, right={right:?}"
    );
}

fn pixel_luma([r, g, b, _]: [u8; 4]) -> f32 {
    r as f32 * 0.2126 + g as f32 * 0.7152 + b as f32 * 0.0722
}

fn rgb_luma([r, g, b]: [f32; 3]) -> f32 {
    r * 0.2126 + g * 0.7152 + b * 0.0722
}

fn rgb_mean_delta(left: [f32; 3], right: [f32; 3]) -> f32 {
    ((left[0] - right[0]).abs() + (left[1] - right[1]).abs() + (left[2] - right[2]).abs()) / 3.0
}
