use game_planet_visuals::{
    PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderOptions, RenderSize,
};
use image::RgbaImage;
use std::collections::HashSet;

const SEED: u64 = 0x5EED_1208_AA01;
const MAP_SIZE: RenderSize = RenderSize {
    width: 160,
    height: 80,
};
const OCEAN_ARCHETYPE: &str = "catalog.archetype.global-ocean";
const DESERT_ARCHETYPE: &str = "catalog.archetype.red-dune-desert";
const GAS_GIANT_ARCHETYPE: &str = "catalog.archetype.banded-gas-giant";
const HOT_JUPITER_ARCHETYPE: &str = "catalog.archetype.hot-jupiter";
const COLD_JUPITER_ARCHETYPE: &str = "catalog.archetype.cold-jupiter";
const SATURN_LIKE_ARCHETYPE: &str = "catalog.archetype.saturn-like";
const PUFFY_GIANT_ARCHETYPE: &str = "catalog.archetype.puffy-giant";
const TERRESTRIAL_ARCHETYPE: &str = "catalog.archetype.temperate-continents";
const SULFUR_IO_ARCHETYPE: &str = "catalog.archetype.sulfur-io-like";
const ACID_CLOUD_ARCHETYPE: &str = "catalog.archetype.acid-cloud";
const CARBON_ARCHETYPE: &str = "catalog.archetype.carbon";
const CRYOGENIC_ICE_ARCHETYPE: &str = "catalog.archetype.cryogenic-ice";
const SIGNATURE_MAP_SIZE: RenderSize = RenderSize {
    width: 96,
    height: 48,
};
const SIGNATURE_ICON_SIZE: u32 = 32;

#[derive(Debug, Clone, Copy)]
struct BandStats {
    horizontal_delta: f32,
    vertical_delta: f32,
    row_luma_range: f32,
}

#[derive(Debug, Clone, Copy)]
struct DirectionalTextureStats {
    samples: u32,
    distinct_rgb: usize,
    horizontal_delta: f32,
    vertical_delta: f32,
    diagonal_down_delta: f32,
    diagonal_up_delta: f32,
    medium_horizontal_delta: f32,
    medium_vertical_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct ImageVariationStats {
    visible_pixels: u32,
    distinct_rgba: usize,
    luma_range: f32,
    row_luma_range: f32,
    column_luma_range: f32,
    mean_adjacent_luma_delta: f32,
}

#[derive(Debug, Clone)]
struct VariantSignature {
    key: &'static str,
    profile: PlanetVisualProfile,
    surface: ImageColorSignature,
    icon: ImageColorSignature,
    bands: BandStats,
    material: MaterialMapSignature,
}

#[derive(Debug, Clone, Copy)]
struct ImageColorSignature {
    visible_pixels: u32,
    mean_rgb: [f32; 3],
    mean_chroma: f32,
    chroma_range: f32,
    luma_range: f32,
    distinct_rgb: usize,
    mean_adjacent_rgb_delta: f32,
    row_luma_range: f32,
    warm_yellow_ratio: f32,
    cold_ice_ratio: f32,
    dark_ratio: f32,
}

#[derive(Debug, Clone, Copy)]
struct MaterialMapSignature {
    water_ratio: f32,
    vegetation_mean: f32,
    roughness_mean: f32,
    roughness_range: u8,
    wetness_mean: f32,
    wetness_range: u8,
}

#[derive(Debug, Clone, Copy)]
struct HorizonContrastStats {
    horizon_row: u32,
    horizon_edge_delta: f32,
    horizon_band_delta: f32,
    sky_luma: f32,
    near_ground_luma: f32,
    foreground_luma: f32,
    foreground_luma_range: f32,
    foreground_local_delta: f32,
    foreground_medium_delta: f32,
    foreground_distinct_rgb: usize,
}

fn forced_profile(archetype_key: &str) -> PlanetVisualProfile {
    let profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(SEED)
            .with_archetype_key(archetype_key)
            .with_modifier_budget(0)
            .without_rare_modifiers(),
    );

    assert_eq!(
        profile.archetype_key, archetype_key,
        "forced profile should preserve the requested archetype key"
    );
    profile
}

fn forced_renderer(archetype_key: &str) -> PlanetRenderer {
    PlanetRenderer::new(forced_profile(archetype_key))
}

#[test]
fn ocean_archetype_is_visibly_oceanic_not_sand_land_at_map_and_icon_level() {
    let ocean_profile = forced_profile(OCEAN_ARCHETYPE);
    let desert_profile = forced_profile(DESERT_ARCHETYPE);

    assert!(
        ocean_profile.ocean_fraction >= 0.70,
        "ocean archetype should generate high ocean coverage; got {}",
        ocean_profile.ocean_fraction
    );
    assert!(
        desert_profile.ocean_fraction <= 0.12,
        "desert control archetype should remain low water; got {}",
        desert_profile.ocean_fraction
    );

    let ocean_renderer = PlanetRenderer::new(ocean_profile);
    let desert_renderer = PlanetRenderer::new(desert_profile);
    let ocean_map = ocean_renderer.render_surface_map(MAP_SIZE);
    let desert_map = desert_renderer.render_surface_map(MAP_SIZE);
    let ocean_material = ocean_renderer.render_vegetation_map(MAP_SIZE);
    let desert_material = desert_renderer.render_vegetation_map(MAP_SIZE);

    let ocean_material_water = channel_ratio_at_least(&ocean_material, 2, 192);
    let desert_material_water = channel_ratio_at_least(&desert_material, 2, 192);
    assert!(
        ocean_material_water > 0.92,
        "ocean material map should be dominated by public water-channel pixels; got {ocean_material_water:.3}"
    );
    assert!(
        desert_material_water < 0.10,
        "desert material map should not expose ocean coverage; got {desert_material_water:.3}"
    );
    assert!(
        ocean_material_water > desert_material_water + 0.80,
        "ocean material water channel should separate from sand/land control; ocean={ocean_material_water:.3}, desert={desert_material_water:.3}"
    );

    let ocean_map_visible_water = aquatic_ratio(&ocean_map);
    let desert_map_visible_water = aquatic_ratio(&desert_map);
    assert!(
        ocean_map_visible_water > 0.58,
        "ocean surface map should be dominated by blue/cyan water pixels; got {ocean_map_visible_water:.3}"
    );
    assert!(
        desert_map_visible_water < 0.10,
        "desert surface map should not satisfy the visible-water color predicate; got {desert_map_visible_water:.3}"
    );
    assert!(
        ocean_map_visible_water > desert_map_visible_water + 0.50,
        "ocean surface colors should separate from sand/land control; ocean={ocean_map_visible_water:.3}, desert={desert_map_visible_water:.3}"
    );

    let ocean_icon = ocean_renderer.render_icon(128);
    let desert_icon = desert_renderer.render_icon(128);
    let ocean_icon_water = aquatic_ratio(&ocean_icon);
    let desert_icon_water = aquatic_ratio(&desert_icon);

    assert!(
        ocean_icon_water > 0.20,
        "ocean icon should retain visible water color after lighting/atmosphere; got {ocean_icon_water:.3}"
    );
    assert!(
        ocean_icon_water > desert_icon_water + 0.12,
        "ocean icon should not collapse to the sand/land control; ocean={ocean_icon_water:.3}, desert={desert_icon_water:.3}"
    );
}

#[test]
fn ocean_archetype_texture_is_multifrequency_not_diagonal_stripes() {
    let renderer = forced_renderer(OCEAN_ARCHETYPE);
    let surface_map = renderer.render_surface_map(MAP_SIZE);
    let icon = renderer.render_icon(128);

    let surface = directional_texture_stats(&surface_map, |pixel| {
        pixel[3] > 16 && is_aquatic_pixel(pixel)
    });
    let icon = directional_texture_stats(&icon, |pixel| pixel[3] > 16 && is_aquatic_pixel(pixel));

    assert!(
        surface.samples > 4_000,
        "ocean surface map should expose enough visible water samples for spectrum checks; got {surface:?}"
    );
    assert!(
        surface.distinct_rgb >= 48,
        "ocean surface map should contain varied water colors, not a flat fill; got {surface:?}"
    );
    assert!(
        surface.horizontal_delta > 0.60 && surface.vertical_delta > 0.60,
        "ocean surface texture should vary in both screen axes; got {surface:?}"
    );
    assert!(
        surface.medium_horizontal_delta > surface.horizontal_delta * 1.03
            && surface.medium_vertical_delta > surface.vertical_delta * 1.03,
        "ocean surface texture should carry medium-scale swell in addition to adjacent ripples; got {surface:?}"
    );
    let surface_axis_floor = surface
        .horizontal_delta
        .min(surface.vertical_delta)
        .max(0.001);
    assert!(
        surface.diagonal_down_delta > surface_axis_floor * 0.45
            && surface.diagonal_up_delta > surface_axis_floor * 0.45,
        "ocean surface texture should not collapse into one obvious diagonal stripe family; got {surface:?}"
    );

    assert!(
        icon.samples > 700,
        "ocean icon should retain enough visible water samples after lighting and atmosphere; got {icon:?}"
    );
    assert!(
        icon.distinct_rgb >= 36,
        "ocean icon should retain varied water colors after lighting and atmosphere; got {icon:?}"
    );
    assert!(
        icon.horizontal_delta > 0.45 && icon.vertical_delta > 0.45,
        "ocean icon water texture should survive planet shading in both screen axes; got {icon:?}"
    );
    let icon_axis_floor = icon.horizontal_delta.min(icon.vertical_delta).max(0.001);
    assert!(
        icon.diagonal_down_delta > icon_axis_floor * 0.40
            && icon.diagonal_up_delta > icon_axis_floor * 0.40,
        "ocean icon water texture should not read as a single diagonal stripe overlay; got {icon:?}"
    );
}

#[test]
fn gas_giant_archetype_surface_map_has_horizontal_atmospheric_banding() {
    let gas_profile = forced_profile(GAS_GIANT_ARCHETYPE);
    assert!(
        gas_profile.render_model.contains("gas"),
        "gas giant profile should advertise a gas render model; got {}",
        gas_profile.render_model
    );
    assert_eq!(
        gas_profile.ocean_fraction, 0.0,
        "gas giant archetype should not generate ocean coverage"
    );

    let gas_map = PlanetRenderer::new(gas_profile).render_surface_map(MAP_SIZE);
    let terrestrial_map = forced_renderer(TERRESTRIAL_ARCHETYPE).render_surface_map(MAP_SIZE);
    let gas_bands = band_stats(&gas_map);
    let terrestrial_bands = band_stats(&terrestrial_map);

    assert!(
        aquatic_ratio(&gas_map) < 0.18,
        "gas giant surface map should not contain terrestrial water blobs"
    );
    assert!(
        gas_bands.vertical_delta > gas_bands.horizontal_delta * 1.18,
        "gas giant bands should vary more between rows than along rows; got {gas_bands:?}"
    );
    assert!(
        gas_bands.horizontal_delta < terrestrial_bands.horizontal_delta * 0.82,
        "gas giant bands should be more horizontally coherent than a terrestrial surface; gas={gas_bands:?}, terrestrial={terrestrial_bands:?}"
    );
    assert!(
        gas_bands.row_luma_range > 15.0,
        "gas giant bands should have visible row-to-row tonal range; got {gas_bands:?}"
    );
}

#[test]
fn gas_giant_variants_have_distinct_color_banding_and_storm_signatures() {
    let keys = [
        GAS_GIANT_ARCHETYPE,
        HOT_JUPITER_ARCHETYPE,
        COLD_JUPITER_ARCHETYPE,
        SATURN_LIKE_ARCHETYPE,
        PUFFY_GIANT_ARCHETYPE,
    ];
    let signatures = keys
        .iter()
        .map(|key| variant_signature(key))
        .collect::<Vec<_>>();

    for signature in &signatures {
        assert_eq!(
            signature.profile.archetype_key, signature.key,
            "forced gas giant profile should not fall back to a random archetype"
        );
        assert_eq!(
            signature.profile.ocean_fraction, 0.0,
            "{} should remain a gas/cloud world with no ocean material",
            signature.key
        );
        assert!(
            signature.profile.render_model.contains("gas")
                || signature.profile.class_key.contains("jupiter")
                || signature.profile.class_key.contains("giant"),
            "{} should advertise a gas giant render family; profile={:?}",
            signature.key,
            signature.profile
        );
        assert!(
            signature.surface.visible_pixels > SIGNATURE_MAP_SIZE.width * SIGNATURE_MAP_SIZE.height * 95 / 100
                && signature.icon.visible_pixels > SIGNATURE_ICON_SIZE * SIGNATURE_ICON_SIZE / 3
                && signature.surface.distinct_rgb >= 80
                && signature.surface.mean_chroma > 18.0
                && signature.surface.chroma_range > 34.0,
            "{} gas surface/icon should contain rich cloud-band color variation; got surface={:?}, icon={:?}",
            signature.key,
            signature.surface,
            signature.icon
        );
        assert!(
            signature.surface.mean_adjacent_rgb_delta > 0.45
                && signature.bands.row_luma_range > 12.0
                && signature.bands.vertical_delta > signature.bands.horizontal_delta * 1.08,
            "{} gas surface should expose storm/band texture, not a flat reused ball; bands={:?}, surface={:?}",
            signature.key,
            signature.bands,
            signature.surface
        );
        assert!(
            signature.icon.distinct_rgb >= 56 && signature.icon.mean_chroma > 16.0,
            "{} gas icon should retain the variant color signature after planet lighting; got {:?}",
            signature.key,
            signature.icon
        );
    }

    let unique_bins = variant_signature_bins(&signatures);
    assert!(
        unique_bins >= 4,
        "gas giant variants should not collapse into one or two palettes; unique_bins={unique_bins}, signatures={signatures:?}"
    );

    let min_distance = min_pairwise_signature_distance(&signatures);
    assert!(
        min_distance > 6.0,
        "every gas giant variant should keep a measurable image/material signature distance; min_distance={min_distance:.3}, signatures={signatures:?}"
    );

    let hot = signature_for(&signatures, HOT_JUPITER_ARCHETYPE);
    let cold = signature_for(&signatures, COLD_JUPITER_ARCHETYPE);
    let puffy = signature_for(&signatures, PUFFY_GIANT_ARCHETYPE);
    let saturn = signature_for(&signatures, SATURN_LIKE_ARCHETYPE);
    assert!(
        variant_signature_distance(hot, cold) > 12.0,
        "hot and cold Jupiter variants should be visually/materially distinct; hot={hot:?}, cold={cold:?}"
    );
    assert!(
        variant_signature_distance(puffy, saturn) > 8.0,
        "puffy and Saturn-like giant variants should not reuse the same gas giant look; puffy={puffy:?}, saturn={saturn:?}"
    );
}

#[test]
fn extreme_atmosphere_and_material_worlds_have_distinct_public_signatures() {
    let sulfur = variant_signature(SULFUR_IO_ARCHETYPE);
    let acid = variant_signature(ACID_CLOUD_ARCHETYPE);
    let carbon = variant_signature(CARBON_ARCHETYPE);
    let cryogenic = variant_signature(CRYOGENIC_ICE_ARCHETYPE);
    let signatures = [&sulfur, &acid, &carbon, &cryogenic];

    for signature in signatures {
        assert_eq!(
            signature.profile.archetype_key, signature.key,
            "forced extreme profile should not fall back to another archetype"
        );
        assert!(
            signature.surface.visible_pixels
                > SIGNATURE_MAP_SIZE.width * SIGNATURE_MAP_SIZE.height * 95 / 100
                && signature.icon.visible_pixels > SIGNATURE_ICON_SIZE * SIGNATURE_ICON_SIZE / 3
                && signature.surface.distinct_rgb >= 64
                && signature.surface.mean_adjacent_rgb_delta > 0.40,
            "{} surface/icon should contain varied material detail; got surface={:?}, icon={:?}",
            signature.key,
            signature.surface,
            signature.icon
        );
        assert!(
            signature.icon.distinct_rgb >= 40,
            "{} icon should preserve visible material variation after lighting; got {:?}",
            signature.key,
            signature.icon
        );
    }

    assert!(
        sulfur.profile.volcanic_activity >= 0.55
            && sulfur.surface.warm_yellow_ratio > carbon.surface.warm_yellow_ratio + 0.08
            && sulfur.surface.warm_yellow_ratio > cryogenic.surface.warm_yellow_ratio + 0.08,
        "sulfur/Io-like world should read as sulfurous volcanic material, not generic rock; sulfur={sulfur:?}, carbon={carbon:?}, cryogenic={cryogenic:?}"
    );
    assert!(
        acid.profile.atmosphere_density >= 0.55
            && acid.profile.cloud_density >= 0.42
            && acid.surface.warm_yellow_ratio > 0.10,
        "acid-cloud world should expose dense yellow/orange atmosphere and clouds; got {acid:?}"
    );
    assert!(
        carbon.profile.ice_fraction < 0.12
            && carbon.material.roughness_mean > 120.0
            && carbon.surface.dark_ratio > sulfur.surface.dark_ratio + 0.10
            && carbon.surface.dark_ratio > cryogenic.surface.dark_ratio + 0.10,
        "carbon/diamond-like material control should be dark, dry, and rough rather than reusing sulfur or ice colors; carbon={carbon:?}, sulfur={sulfur:?}, cryogenic={cryogenic:?}"
    );
    assert!(
        cryogenic.profile.ice_fraction >= 0.45
            && cryogenic.surface.cold_ice_ratio > carbon.surface.cold_ice_ratio + 0.12
            && cryogenic.surface.cold_ice_ratio > sulfur.surface.cold_ice_ratio + 0.10,
        "frozen worlds should expose cold ice palettes distinct from carbon and sulfur surfaces; cryogenic={cryogenic:?}, carbon={carbon:?}, sulfur={sulfur:?}"
    );

    for (left, right, minimum) in [
        (&sulfur, &acid, 10.0),
        (&sulfur, &carbon, 18.0),
        (&sulfur, &cryogenic, 18.0),
        (&acid, &carbon, 18.0),
        (&carbon, &cryogenic, 18.0),
    ] {
        let distance = variant_signature_distance(left, right);
        assert!(
            distance > minimum,
            "{} and {} should have distinct rendered material signatures; distance={distance:.3}, left={left:?}, right={right:?}",
            left.key,
            right.key
        );
    }
}

#[test]
fn atmospheric_and_backdrop_renders_are_dimension_correct_nonblank_and_varied() {
    let renderer = forced_renderer(TERRESTRIAL_ARCHETYPE);
    let terrain_size = RenderSize {
        width: 192,
        height: 128,
    };
    let portrait_size = RenderSize {
        width: 112,
        height: 176,
    };
    let orbital_size = RenderSize {
        width: 224,
        height: 126,
    };

    let cases = vec![
        (
            "terrain overview atmosphere",
            renderer.render_terrain_overview_with_options(terrain_size, RenderOptions::preview()),
            terrain_size,
            96,
        ),
        (
            "portrait terrain banner atmosphere",
            renderer.render_banner(portrait_size),
            portrait_size,
            96,
        ),
        (
            "orbital backdrop",
            renderer.render_orbital_banner_with_options(orbital_size, RenderOptions::preview()),
            orbital_size,
            120,
        ),
    ];

    for (label, image, size, min_distinct) in cases {
        assert_eq!(
            image.dimensions(),
            (size.width, size.height),
            "{label} should preserve requested dimensions"
        );
        let stats = image_variation_stats(&image);
        assert!(
            stats.visible_pixels > size.width * size.height * 95 / 100,
            "{label} should render an opaque atmospheric/backdrop frame; got {stats:?}"
        );
        assert!(
            stats.distinct_rgba >= min_distinct,
            "{label} should contain varied atmospheric/backdrop pixels; got {stats:?}"
        );
        assert!(
            stats.luma_range > 28.0,
            "{label} should not be blank or tonally flat; got {stats:?}"
        );
        assert!(
            stats.row_luma_range > 12.0 && stats.column_luma_range > 4.0,
            "{label} should vary across both image dimensions; got {stats:?}"
        );
        assert!(
            stats.mean_adjacent_luma_delta > 0.45,
            "{label} should contain local texture/feature variation, not only a smooth fill; got {stats:?}"
        );
    }
}

#[test]
fn terrain_banner_horizon_and_foreground_have_measurable_occlusion_contrast() {
    let renderer = forced_renderer(TERRESTRIAL_ARCHETYPE);
    let size = RenderSize {
        width: 224,
        height: 128,
    };
    let banner = renderer.render_terrain_overview_with_options(size, RenderOptions::preview());
    let stats = horizon_contrast_stats(&banner);

    assert!(
        stats.horizon_row >= size.height * 38 / 100 && stats.horizon_row <= size.height * 70 / 100,
        "terrain horizon should remain in the expected public banner band; got {stats:?}"
    );
    assert!(
        stats.horizon_edge_delta > 4.5,
        "terrain banner should have measurable tonal separation at the sky/terrain horizon; got {stats:?}"
    );
    assert!(
        stats.horizon_band_delta > 8.0,
        "terrain banner should separate the sky band from the terrain band without relying on a golden image; got {stats:?}"
    );
    assert!(
        (stats.near_ground_luma - stats.foreground_luma).abs() > 5.0,
        "terrain banner should expose horizon/foreground atmospheric and occlusion contrast; got {stats:?}"
    );
    assert!(
        (stats.sky_luma - stats.foreground_luma).abs() > 16.0,
        "terrain banner foreground depth should separate from the sky field; got {stats:?}"
    );
    assert!(
        stats.foreground_luma_range > 34.0,
        "terrain foreground should retain local tonal contrast across ridges and basins; got {stats:?}"
    );
    assert!(
        stats.foreground_local_delta > 2.0,
        "terrain foreground should contain measurable terrain shadow/relief contrast; got {stats:?}"
    );
    assert!(
        stats.foreground_medium_delta > stats.foreground_local_delta * 1.12,
        "terrain foreground should retain coherent detail beyond one-pixel noise, not smear into a smooth gradient; got {stats:?}"
    );
    assert!(
        stats.foreground_distinct_rgb > 128,
        "terrain foreground should contain varied public pixels rather than a smeared fill; got {stats:?}"
    );
}

#[test]
fn vertical_banner_keeps_requested_dimensions_and_is_not_a_stretched_landscape_clone() {
    let renderer = forced_renderer(TERRESTRIAL_ARCHETYPE);
    let portrait_size = RenderSize {
        width: 96,
        height: 160,
    };
    let landscape_size = RenderSize {
        width: 160,
        height: 96,
    };

    let portrait = renderer.render_banner(portrait_size);
    let landscape = renderer.render_banner(landscape_size);

    assert_eq!(
        portrait.dimensions(),
        (portrait_size.width, portrait_size.height),
        "vertical banner should preserve requested portrait dimensions"
    );
    assert_eq!(
        landscape.dimensions(),
        (landscape_size.width, landscape_size.height),
        "landscape control should preserve requested landscape dimensions"
    );

    let stretched_landscape = resize_nearest(&landscape, portrait_size);
    let clone_delta = mean_abs_rgb_delta(&portrait, &stretched_landscape);
    assert!(
        clone_delta > 8.0,
        "vertical banner should be aspect-aware, not a stretched landscape render; mean RGB delta from stretched landscape clone was {clone_delta:.2}"
    );
}

fn variant_signature(key: &'static str) -> VariantSignature {
    let profile = forced_profile(key);
    let renderer = PlanetRenderer::new(profile.clone());
    let surface = renderer.render_surface_map(SIGNATURE_MAP_SIZE);
    let icon = renderer.render_icon_with_options(SIGNATURE_ICON_SIZE, RenderOptions::standard());
    let biome = renderer.render_surface_biome_map(SIGNATURE_MAP_SIZE);
    let roughness = renderer.render_surface_roughness_wetness_map(SIGNATURE_MAP_SIZE);

    VariantSignature {
        key,
        profile,
        surface: image_color_signature(&surface),
        icon: image_color_signature(&icon),
        bands: band_stats(&surface),
        material: material_map_signature(&biome, &roughness),
    }
}

fn image_color_signature(image: &RgbaImage) -> ImageColorSignature {
    let mut visible_pixels = 0_u32;
    let mut mean_rgb = [0.0_f32; 3];
    let mut total_chroma = 0.0_f32;
    let mut min_chroma = f32::MAX;
    let mut max_chroma = f32::MIN;
    let mut min_luma = f32::MAX;
    let mut max_luma = f32::MIN;
    let mut distinct_rgb = HashSet::new();
    let mut adjacent_delta = 0.0_f32;
    let mut adjacent_edges = 0_u32;
    let mut row_luma = vec![0.0; image.height() as usize];
    let mut row_counts = vec![0_u32; image.height() as usize];
    let mut warm_yellow = 0_u32;
    let mut cold_ice = 0_u32;
    let mut dark = 0_u32;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, y).0;
            if pixel[3] <= 16 {
                continue;
            }

            let pixel_luma = luma(pixel);
            let max_channel = pixel[0].max(pixel[1]).max(pixel[2]) as f32;
            let min_channel = pixel[0].min(pixel[1]).min(pixel[2]) as f32;
            let chroma = max_channel - min_channel;

            visible_pixels += 1;
            mean_rgb[0] += pixel[0] as f32;
            mean_rgb[1] += pixel[1] as f32;
            mean_rgb[2] += pixel[2] as f32;
            total_chroma += chroma;
            min_chroma = min_chroma.min(chroma);
            max_chroma = max_chroma.max(chroma);
            min_luma = min_luma.min(pixel_luma);
            max_luma = max_luma.max(pixel_luma);
            distinct_rgb.insert([pixel[0], pixel[1], pixel[2]]);
            row_luma[y as usize] += pixel_luma;
            row_counts[y as usize] += 1;

            warm_yellow += u32::from(is_warm_yellow_material(pixel));
            cold_ice += u32::from(is_cold_ice_material(pixel));
            dark += u32::from(pixel_luma < 55.0);

            if x + 1 < image.width() {
                let neighbor = image.get_pixel(x + 1, y).0;
                if neighbor[3] > 16 {
                    adjacent_delta += rgb_delta(pixel, neighbor);
                    adjacent_edges += 1;
                }
            }
            if y + 1 < image.height() {
                let neighbor = image.get_pixel(x, y + 1).0;
                if neighbor[3] > 16 {
                    adjacent_delta += rgb_delta(pixel, neighbor);
                    adjacent_edges += 1;
                }
            }
        }
    }

    let visible = visible_pixels.max(1) as f32;
    ImageColorSignature {
        visible_pixels,
        mean_rgb: [
            mean_rgb[0] / visible,
            mean_rgb[1] / visible,
            mean_rgb[2] / visible,
        ],
        mean_chroma: total_chroma / visible,
        chroma_range: if visible_pixels == 0 {
            0.0
        } else {
            max_chroma - min_chroma
        },
        luma_range: if visible_pixels == 0 {
            0.0
        } else {
            max_luma - min_luma
        },
        distinct_rgb: distinct_rgb.len(),
        mean_adjacent_rgb_delta: adjacent_delta / adjacent_edges.max(1) as f32,
        row_luma_range: mean_range(&row_luma, &row_counts),
        warm_yellow_ratio: warm_yellow as f32 / visible,
        cold_ice_ratio: cold_ice as f32 / visible,
        dark_ratio: dark as f32 / visible,
    }
}

fn material_map_signature(biome: &RgbaImage, roughness: &RgbaImage) -> MaterialMapSignature {
    MaterialMapSignature {
        water_ratio: channel_ratio_at_least(biome, 2, 128),
        vegetation_mean: channel_mean(biome, 1),
        roughness_mean: channel_mean(roughness, 0),
        roughness_range: channel_range(roughness, 0),
        wetness_mean: channel_mean(roughness, 1),
        wetness_range: channel_range(roughness, 1),
    }
}

fn is_warm_yellow_material([r, g, b, a]: [u8; 4]) -> bool {
    a > 16
        && r >= 72
        && g >= 56
        && r.saturating_sub(b) >= 22
        && g.saturating_sub(b) >= 12
        && (r as i16 - g as i16).abs() <= 90
}

fn is_cold_ice_material(pixel: [u8; 4]) -> bool {
    if pixel[3] <= 16 {
        return false;
    }
    let [r, g, b, _] = pixel;
    let pixel_luma = luma(pixel);
    (pixel_luma > 92.0 && b.saturating_add(10) >= r && g.saturating_add(8) >= r)
        || (pixel_luma > 150.0 && b >= r.saturating_sub(8) && g >= r.saturating_sub(8))
}

fn variant_signature_bins(signatures: &[VariantSignature]) -> usize {
    signatures
        .iter()
        .map(|signature| {
            (
                quantize(signature.surface.mean_rgb[0], 18.0),
                quantize(signature.surface.mean_rgb[1], 18.0),
                quantize(signature.surface.mean_rgb[2], 18.0),
                quantize(signature.surface.mean_chroma, 8.0),
                quantize(signature.surface.warm_yellow_ratio * 100.0, 8.0),
                quantize(signature.surface.cold_ice_ratio * 100.0, 8.0),
                quantize(signature.bands.row_luma_range, 5.0),
            )
        })
        .collect::<HashSet<_>>()
        .len()
}

fn quantize(value: f32, quantum: f32) -> i32 {
    (value / quantum).round() as i32
}

fn signature_for<'a>(signatures: &'a [VariantSignature], key: &str) -> &'a VariantSignature {
    signatures
        .iter()
        .find(|signature| signature.key == key)
        .unwrap_or_else(|| panic!("missing signature for {key}"))
}

fn min_pairwise_signature_distance(signatures: &[VariantSignature]) -> f32 {
    let mut minimum = f32::MAX;
    for left_index in 0..signatures.len() {
        for right_index in left_index + 1..signatures.len() {
            minimum = minimum.min(variant_signature_distance(
                &signatures[left_index],
                &signatures[right_index],
            ));
        }
    }
    minimum
}

fn variant_signature_distance(left: &VariantSignature, right: &VariantSignature) -> f32 {
    mean_rgb_signature_delta(left.surface.mean_rgb, right.surface.mean_rgb) * 0.62
        + mean_rgb_signature_delta(left.icon.mean_rgb, right.icon.mean_rgb) * 0.34
        + (left.surface.mean_chroma - right.surface.mean_chroma).abs() * 0.42
        + (left.surface.luma_range - right.surface.luma_range).abs() * 0.16
        + (left.surface.row_luma_range - right.surface.row_luma_range).abs() * 0.55
        + (left.surface.mean_adjacent_rgb_delta - right.surface.mean_adjacent_rgb_delta).abs() * 1.8
        + (left.surface.warm_yellow_ratio - right.surface.warm_yellow_ratio).abs() * 50.0
        + (left.surface.cold_ice_ratio - right.surface.cold_ice_ratio).abs() * 50.0
        + (left.surface.dark_ratio - right.surface.dark_ratio).abs() * 42.0
        + (left.bands.row_luma_range - right.bands.row_luma_range).abs() * 0.45
        + (left.material.water_ratio - right.material.water_ratio).abs() * 30.0
        + (left.material.vegetation_mean - right.material.vegetation_mean).abs() * 0.08
        + (left.material.roughness_mean - right.material.roughness_mean).abs() * 0.08
        + left
            .material
            .roughness_range
            .abs_diff(right.material.roughness_range) as f32
            * 0.05
        + (left.material.wetness_mean - right.material.wetness_mean).abs() * 0.06
        + left
            .material
            .wetness_range
            .abs_diff(right.material.wetness_range) as f32
            * 0.04
}

fn mean_rgb_signature_delta(left: [f32; 3], right: [f32; 3]) -> f32 {
    ((left[0] - right[0]).abs() + (left[1] - right[1]).abs() + (left[2] - right[2]).abs()) / 3.0
}

fn aquatic_ratio(image: &RgbaImage) -> f32 {
    let mut visible = 0_u32;
    let mut aquatic = 0_u32;

    for pixel in image.pixels() {
        if pixel[3] <= 16 {
            continue;
        }
        visible += 1;
        if is_aquatic_pixel(pixel.0) {
            aquatic += 1;
        }
    }

    if visible == 0 {
        0.0
    } else {
        aquatic as f32 / visible as f32
    }
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

    if visible == 0 {
        0.0
    } else {
        matching as f32 / visible as f32
    }
}

fn channel_mean(image: &RgbaImage, channel: usize) -> f32 {
    let mut total = 0_u64;
    let mut pixels = 0_u32;

    for pixel in image.pixels() {
        total += pixel[channel] as u64;
        pixels += 1;
    }

    total as f32 / pixels.max(1) as f32
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

fn is_aquatic_pixel([r, g, b, _]: [u8; 4]) -> bool {
    let max_channel = r.max(g).max(b);
    let min_channel = r.min(g).min(b);
    b >= 48
        && b.saturating_sub(r) >= 24
        && g.saturating_sub(r) >= 8
        && b.saturating_add(18) >= g
        && max_channel.saturating_sub(min_channel) >= 24
}

fn band_stats(image: &RgbaImage) -> BandStats {
    let width = image.width();
    let height = image.height();
    let mut horizontal_delta = 0.0;
    let mut horizontal_edges = 0_u32;
    let mut vertical_delta = 0.0;
    let mut vertical_edges = 0_u32;
    let mut min_row_luma = f32::MAX;
    let mut max_row_luma = f32::MIN;

    for y in 0..height {
        let mut row_luma = 0.0;
        for x in 0..width {
            let pixel = image.get_pixel(x, y).0;
            row_luma += luma(pixel);

            if x > 0 {
                horizontal_delta += rgb_delta(pixel, image.get_pixel(x - 1, y).0);
                horizontal_edges += 1;
            }
            if y > 0 {
                vertical_delta += rgb_delta(pixel, image.get_pixel(x, y - 1).0);
                vertical_edges += 1;
            }
        }

        let row_luma = row_luma / width.max(1) as f32;
        min_row_luma = min_row_luma.min(row_luma);
        max_row_luma = max_row_luma.max(row_luma);
    }

    let horizontal_delta = horizontal_delta / horizontal_edges.max(1) as f32;
    let vertical_delta = vertical_delta / vertical_edges.max(1) as f32;
    BandStats {
        horizontal_delta,
        vertical_delta,
        row_luma_range: max_row_luma - min_row_luma,
    }
}

fn directional_texture_stats<F>(image: &RgbaImage, include: F) -> DirectionalTextureStats
where
    F: Fn([u8; 4]) -> bool + Copy,
{
    let samples = image
        .pixels()
        .filter(|pixel| include(pixel.0))
        .count()
        .min(u32::MAX as usize) as u32;
    let distinct_rgb = image
        .pixels()
        .filter(|pixel| include(pixel.0))
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect::<HashSet<_>>()
        .len();

    DirectionalTextureStats {
        samples,
        distinct_rgb,
        horizontal_delta: mean_rgb_delta_for_offset(image, 1, 0, include),
        vertical_delta: mean_rgb_delta_for_offset(image, 0, 1, include),
        diagonal_down_delta: mean_rgb_delta_for_offset(image, 1, 1, include),
        diagonal_up_delta: mean_rgb_delta_for_offset(image, 1, -1, include),
        medium_horizontal_delta: mean_rgb_delta_for_offset(image, 5, 0, include),
        medium_vertical_delta: mean_rgb_delta_for_offset(image, 0, 5, include),
    }
}

fn mean_rgb_delta_for_offset<F>(image: &RgbaImage, dx: i32, dy: i32, include: F) -> f32
where
    F: Fn([u8; 4]) -> bool + Copy,
{
    let mut total = 0.0;
    let mut pairs = 0_u32;
    let width = image.width() as i32;
    let height = image.height() as i32;

    for y in 0..height {
        for x in 0..width {
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx >= width || ny >= height {
                continue;
            }

            let pixel = image.get_pixel(x as u32, y as u32).0;
            let neighbor = image.get_pixel(nx as u32, ny as u32).0;
            if include(pixel) && include(neighbor) {
                total += rgb_delta(pixel, neighbor);
                pairs += 1;
            }
        }
    }

    total / pairs.max(1) as f32
}

fn image_variation_stats(image: &RgbaImage) -> ImageVariationStats {
    let mut visible_pixels = 0_u32;
    let mut min_luma = f32::MAX;
    let mut max_luma = f32::MIN;
    let mut distinct_rgba = HashSet::new();
    let mut row_luma = vec![0.0; image.height() as usize];
    let mut row_counts = vec![0_u32; image.height() as usize];
    let mut column_luma = vec![0.0; image.width() as usize];
    let mut column_counts = vec![0_u32; image.width() as usize];
    let mut adjacent_total = 0.0;
    let mut adjacent_edges = 0_u32;

    for y in 0..image.height() {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, y).0;
            if pixel[3] <= 16 {
                continue;
            }

            let pixel_luma = luma(pixel);
            visible_pixels += 1;
            min_luma = min_luma.min(pixel_luma);
            max_luma = max_luma.max(pixel_luma);
            distinct_rgba.insert(pixel);
            row_luma[y as usize] += pixel_luma;
            row_counts[y as usize] += 1;
            column_luma[x as usize] += pixel_luma;
            column_counts[x as usize] += 1;

            if x + 1 < image.width() {
                let neighbor = image.get_pixel(x + 1, y).0;
                if neighbor[3] > 16 {
                    adjacent_total += (pixel_luma - luma(neighbor)).abs();
                    adjacent_edges += 1;
                }
            }
            if y + 1 < image.height() {
                let neighbor = image.get_pixel(x, y + 1).0;
                if neighbor[3] > 16 {
                    adjacent_total += (pixel_luma - luma(neighbor)).abs();
                    adjacent_edges += 1;
                }
            }
        }
    }

    let row_luma_range = mean_range(&row_luma, &row_counts);
    let column_luma_range = mean_range(&column_luma, &column_counts);

    ImageVariationStats {
        visible_pixels,
        distinct_rgba: distinct_rgba.len(),
        luma_range: if visible_pixels == 0 {
            0.0
        } else {
            max_luma - min_luma
        },
        row_luma_range,
        column_luma_range,
        mean_adjacent_luma_delta: adjacent_total / adjacent_edges.max(1) as f32,
    }
}

fn horizon_contrast_stats(image: &RgbaImage) -> HorizonContrastStats {
    let rows = row_luma_means(image);
    let start = image.height() as usize * 38 / 100;
    let end = (image.height() as usize * 70 / 100).max(start + 1);
    let mut horizon_row = start.max(1);
    let mut horizon_edge_delta = 0.0;

    for row in start.max(1)..end.min(rows.len()) {
        let delta = (rows[row] - rows[row - 1]).abs();
        if delta > horizon_edge_delta {
            horizon_edge_delta = delta;
            horizon_row = row;
        }
    }

    let sky_luma = mean_luma_rows(
        image,
        horizon_row.saturating_sub(16),
        horizon_row.saturating_sub(4),
    );
    let near_ground_luma = mean_luma_rows(image, horizon_row + 2, horizon_row + 12);
    let foreground_start = image.height() as usize * 70 / 100;
    let foreground_end = image.height() as usize * 96 / 100;
    let foreground_luma = mean_luma_rows(image, foreground_start, foreground_end);
    let foreground_luma_range = luma_range_in_rows(image, foreground_start, foreground_end);
    let foreground_local_delta =
        mean_luma_delta_for_offset_in_rows(image, foreground_start, foreground_end, 1, 0).max(
            mean_luma_delta_for_offset_in_rows(image, foreground_start, foreground_end, 0, 1),
        );
    let foreground_medium_delta =
        (mean_luma_delta_for_offset_in_rows(image, foreground_start, foreground_end, 5, 0)
            + mean_luma_delta_for_offset_in_rows(image, foreground_start, foreground_end, 0, 5))
            * 0.5;
    let foreground_distinct_rgb = distinct_rgb_in_rows(image, foreground_start, foreground_end);

    HorizonContrastStats {
        horizon_row: horizon_row.min(u32::MAX as usize) as u32,
        horizon_edge_delta,
        horizon_band_delta: (sky_luma - near_ground_luma).abs(),
        sky_luma,
        near_ground_luma,
        foreground_luma,
        foreground_luma_range,
        foreground_local_delta,
        foreground_medium_delta,
        foreground_distinct_rgb,
    }
}

fn row_luma_means(image: &RgbaImage) -> Vec<f32> {
    (0..image.height())
        .map(|y| mean_luma_rows(image, y as usize, y as usize + 1))
        .collect()
}

fn mean_luma_rows(image: &RgbaImage, start: usize, end: usize) -> f32 {
    let start = start.min(image.height() as usize);
    if start >= image.height() as usize {
        return 0.0;
    }
    let end = end.min(image.height() as usize).max(start + 1);
    let mut total = 0.0;
    let mut pixels = 0_u32;

    for y in start..end {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, y as u32).0;
            if pixel[3] > 16 {
                total += luma(pixel);
                pixels += 1;
            }
        }
    }

    total / pixels.max(1) as f32
}

fn mean_luma_delta_for_offset_in_rows(
    image: &RgbaImage,
    start: usize,
    end: usize,
    dx: i32,
    dy: i32,
) -> f32 {
    let start = start.min(image.height() as usize);
    let end = end.min(image.height() as usize);
    let mut total = 0.0;
    let mut edges = 0_u32;
    let width = image.width() as i32;

    for y in start..end {
        for x in 0..image.width() {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < start as i32 || nx >= width || ny >= end as i32 {
                continue;
            }

            let pixel = image.get_pixel(x, y as u32).0;
            if pixel[3] <= 16 {
                continue;
            }
            let pixel_luma = luma(pixel);
            let neighbor = image.get_pixel(nx as u32, ny as u32).0;
            if neighbor[3] > 16 {
                total += (pixel_luma - luma(neighbor)).abs();
                edges += 1;
            }
        }
    }

    total / edges.max(1) as f32
}

fn luma_range_in_rows(image: &RgbaImage, start: usize, end: usize) -> f32 {
    let start = start.min(image.height() as usize);
    let end = end.min(image.height() as usize);
    let mut min_luma = f32::MAX;
    let mut max_luma = f32::MIN;
    let mut populated = false;

    for y in start..end {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, y as u32).0;
            if pixel[3] <= 16 {
                continue;
            }
            let pixel_luma = luma(pixel);
            min_luma = min_luma.min(pixel_luma);
            max_luma = max_luma.max(pixel_luma);
            populated = true;
        }
    }

    if populated {
        max_luma - min_luma
    } else {
        0.0
    }
}

fn distinct_rgb_in_rows(image: &RgbaImage, start: usize, end: usize) -> usize {
    let start = start.min(image.height() as usize);
    let end = end.min(image.height() as usize);
    let mut distinct = HashSet::new();

    for y in start..end {
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, y as u32).0;
            if pixel[3] > 16 {
                distinct.insert([pixel[0], pixel[1], pixel[2]]);
            }
        }
    }

    distinct.len()
}

fn mean_range(totals: &[f32], counts: &[u32]) -> f32 {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    let mut populated = false;

    for (total, count) in totals.iter().zip(counts) {
        if *count == 0 {
            continue;
        }
        populated = true;
        let mean = *total / *count as f32;
        min = min.min(mean);
        max = max.max(mean);
    }

    if populated {
        max - min
    } else {
        0.0
    }
}

fn resize_nearest(source: &RgbaImage, size: RenderSize) -> RgbaImage {
    let mut resized = RgbaImage::new(size.width, size.height);
    for y in 0..size.height {
        for x in 0..size.width {
            let source_x = x * source.width() / size.width.max(1);
            let source_y = y * source.height() / size.height.max(1);
            resized.put_pixel(x, y, *source.get_pixel(source_x, source_y));
        }
    }
    resized
}

fn mean_abs_rgb_delta(a: &RgbaImage, b: &RgbaImage) -> f32 {
    assert_eq!(a.dimensions(), b.dimensions());
    let total: u64 = a
        .pixels()
        .zip(b.pixels())
        .map(|(left, right)| {
            (0..3)
                .map(|channel| left[channel].abs_diff(right[channel]) as u64)
                .sum::<u64>()
        })
        .sum();

    total as f32 / (a.width() * a.height() * 3).max(1) as f32
}

fn rgb_delta(a: [u8; 4], b: [u8; 4]) -> f32 {
    (a[0].abs_diff(b[0]) as f32 + a[1].abs_diff(b[1]) as f32 + a[2].abs_diff(b[2]) as f32) / 3.0
}

fn luma([r, g, b, _]: [u8; 4]) -> f32 {
    r as f32 * 0.2126 + g as f32 * 0.7152 + b as f32 * 0.0722
}
