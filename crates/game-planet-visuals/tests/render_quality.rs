use game_planet_visuals::{
    PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderExecutionMode, RenderOptions,
    RenderPhase, RenderSize,
};
use image::RgbaImage;
use std::collections::HashSet;

const SEED: u64 = 0x5EED_1208_0001;

#[derive(Debug)]
struct AlphaStats {
    transparent: usize,
    partial: usize,
    opaque: usize,
}

#[derive(Debug, Clone, Copy)]
struct ImageDeltaStats {
    visible_pixels: u32,
    changed_visible_ratio: f32,
    mean_rgb_delta: f32,
    max_rgb_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct VisibleColorStats {
    visible_pixels: u32,
    mean_chroma: f32,
    chroma_range: f32,
    distinct_rgb: usize,
}

fn renderer() -> PlanetRenderer {
    PlanetRenderer::new(PlanetVisualProfile::from_seed(SEED))
}

fn forced_profile(seed: u64, archetype_key: &str) -> PlanetVisualProfile {
    PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(seed).with_archetype_key(archetype_key),
    )
}

fn alpha_stats(image: &RgbaImage) -> AlphaStats {
    image.pixels().fold(
        AlphaStats {
            transparent: 0,
            partial: 0,
            opaque: 0,
        },
        |mut stats, pixel| {
            match pixel[3] {
                0 => stats.transparent += 1,
                255 => stats.opaque += 1,
                _ => stats.partial += 1,
            }
            stats
        },
    )
}

fn distinct_rgba_count(image: &RgbaImage) -> usize {
    image
        .pixels()
        .map(|pixel| pixel.0)
        .collect::<HashSet<_>>()
        .len()
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

fn row_luma_range(image: &RgbaImage) -> f32 {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for y in 0..image.height() {
        let mut row = 0.0;
        for x in 0..image.width() {
            let pixel = image.get_pixel(x, y);
            row += (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 3.0;
        }
        let mean = row / image.width().max(1) as f32;
        min = min.min(mean);
        max = max.max(mean);
    }
    max - min
}

fn assert_nonblank(image: &RgbaImage, label: &str) {
    let total_pixels = (image.width() * image.height()) as usize;
    let nontransparent = image.pixels().filter(|pixel| pixel[3] > 0).count();
    let visible_luma: u64 = image
        .pixels()
        .filter(|pixel| pixel[3] > 0)
        .map(|pixel| pixel[0] as u64 + pixel[1] as u64 + pixel[2] as u64)
        .sum();
    let distinct = distinct_rgba_count(image);

    assert!(
        nontransparent > total_pixels / 4,
        "{label} should have visible alpha coverage"
    );
    assert!(
        visible_luma > 10_000,
        "{label} should contain visible color, got luma sum {visible_luma}"
    );
    assert!(
        distinct > 128,
        "{label} should contain varied pixels, got {distinct} distinct colors"
    );
}

#[test]
fn global_ocean_profile_renders_as_high_coverage_ocean_material() {
    let mut profile = forced_profile(SEED + 11, "catalog.archetype.global-ocean");
    profile.ocean_fraction = 1.0;
    let renderer = PlanetRenderer::new(profile);
    let size = RenderSize {
        width: 160,
        height: 80,
    };

    let biome = renderer.render_vegetation_map(size);
    let normal = renderer.render_normal_map(size);
    let roughness = renderer.render_roughness_map(size);
    let water_pixels = biome.pixels().filter(|pixel| pixel[2] >= 235).count();
    let total_pixels = (size.width * size.height) as usize;

    assert!(
        water_pixels > total_pixels * 96 / 100,
        "global ocean should be overwhelmingly water, got {water_pixels}/{total_pixels}"
    );
    assert!(
        channel_range(&normal, 0) > 3 || channel_range(&normal, 1) > 3,
        "ocean normal map should carry deterministic wave/shelf variation"
    );
    assert!(
        channel_range(&roughness, 0) > 8,
        "ocean roughness should vary with waves and shallow water"
    );
    assert!(
        channel_mean(&roughness, 1) > 220.0,
        "ocean wetness channel should read as wet water material"
    );
}

#[test]
fn banded_gas_giant_profile_renders_bands_without_land_material() {
    let profile = forced_profile(SEED + 12, "catalog.archetype.banded-gas-giant");
    let renderer = PlanetRenderer::new(profile);
    let surface = renderer.render_surface_map(RenderSize {
        width: 192,
        height: 96,
    });
    let biome = renderer.render_vegetation_map(RenderSize {
        width: 128,
        height: 64,
    });
    let band_range = row_luma_range(&surface);

    assert!(
        band_range > 20.0,
        "gas giant surface should have visible horizontal band contrast, got {band_range}"
    );
    assert!(
        channel_mean(&biome, 0) < 3.0
            && channel_mean(&biome, 1) < 3.0
            && channel_mean(&biome, 2) < 3.0,
        "gas giant material map should not expose land, vegetation, or water channels"
    );
}

#[test]
fn icon_render_is_deterministic_and_nonblank() {
    let first = renderer().render_icon(128);
    let second = renderer().render_icon(128);

    assert_eq!(first.dimensions(), (128, 128));
    assert_eq!(first.as_raw(), second.as_raw());
    assert_nonblank(&first, "icon render");
}

#[test]
fn icon_alpha_coverage_has_transparent_margins_body_and_soft_edge() {
    let icon = renderer().render_icon(128);
    let stats = alpha_stats(&icon);
    let total_pixels = (icon.width() * icon.height()) as usize;

    assert!(
        stats.transparent > total_pixels / 5,
        "icon should preserve transparent margins, got {stats:?}"
    );
    assert!(
        stats.opaque > total_pixels / 4,
        "icon should include an opaque planet body, got {stats:?}"
    );
    assert!(
        stats.partial > total_pixels / 128,
        "icon should include partially transparent atmosphere/edge pixels, got {stats:?}"
    );
}

#[test]
fn banner_render_is_deterministic_opaque_and_nonblank() {
    let size = RenderSize {
        width: 320,
        height: 120,
    };
    let first = renderer().render_banner(size);
    let second = renderer().render_banner(size);
    let stats = alpha_stats(&first);

    assert_eq!(first.dimensions(), (size.width, size.height));
    assert_eq!(first.as_raw(), second.as_raw());
    assert_eq!(
        stats.transparent, 0,
        "banner should not have transparent holes"
    );
    assert_eq!(
        stats.partial, 0,
        "banner background should make all pixels opaque"
    );
    assert_eq!(stats.opaque, (size.width * size.height) as usize);
    assert_nonblank(&first, "banner render");
}

#[test]
fn surface_map_render_is_deterministic_opaque_and_nonblank() {
    let size = RenderSize {
        width: 128,
        height: 64,
    };
    let first = renderer().render_surface_map(size);
    let second = renderer().render_surface_map(size);
    let stats = alpha_stats(&first);

    assert_eq!(first.dimensions(), (size.width, size.height));
    assert_eq!(first.as_raw(), second.as_raw());
    assert_eq!(
        stats.transparent, 0,
        "surface map should not have transparent holes"
    );
    assert_eq!(stats.partial, 0, "surface map should be fully opaque");
    assert_eq!(stats.opaque, (size.width * size.height) as usize);
    assert_nonblank(&first, "surface map render");
}

#[test]
fn reflection_map_render_is_deterministic_opaque_and_nonblank() {
    let size = RenderSize {
        width: 128,
        height: 64,
    };
    let first = renderer().render_reflection_map(size);
    let second = renderer().render_reflection_map(size);
    let stats = alpha_stats(&first);

    assert_eq!(first.dimensions(), (size.width, size.height));
    assert_eq!(first.as_raw(), second.as_raw());
    assert_eq!(stats.transparent, 0);
    assert_eq!(stats.partial, 0);
    assert_eq!(stats.opaque, (size.width * size.height) as usize);
    assert_nonblank(&first, "reflection map render");
}

#[test]
fn supersampling_downscaling_contract_is_public_and_deterministic() {
    let renderer = renderer();
    let size = RenderSize {
        width: 64,
        height: 64,
    };
    let mut native_phases = Vec::new();
    let native = renderer.render_icon_with_progress(
        64,
        RenderOptions::preview(),
        RenderExecutionMode::Serial,
        |event| native_phases.push(event.progress),
    );
    let mut supersampled_phases = Vec::new();
    let supersampled = renderer.render_icon_with_progress(
        64,
        RenderOptions::ultra(),
        RenderExecutionMode::Serial,
        |event| supersampled_phases.push(event.progress),
    );
    let supersampled_again = renderer.render_icon_with_options(64, RenderOptions::ultra());

    assert_eq!(
        RenderOptions::preview().native_supersample_for_size(size),
        1
    );
    assert_eq!(RenderOptions::ultra().native_supersample_for_size(size), 2);
    assert_eq!(RenderOptions::preview().icon_supersample_for_size(size), 3);
    assert_eq!(RenderOptions::ultra().icon_supersample_for_size(size), 4);

    assert_eq!(native.dimensions(), (64, 64));
    assert_eq!(supersampled.dimensions(), (64, 64));
    assert_eq!(supersampled.as_raw(), supersampled_again.as_raw());
    assert_nonblank(&native, "native icon render");
    assert_nonblank(&supersampled, "supersampled icon render");

    assert!(
        native_phases
            .iter()
            .any(|progress| progress.phase == RenderPhase::Downscale),
        "preview icon render should emit a downscale phase for edge anti-aliasing; phases={native_phases:?}"
    );
    assert!(
        supersampled_phases
            .iter()
            .any(|progress| progress.phase == RenderPhase::Downscale),
        "ultra icon render should emit a downscale phase after 2x rendering; phases={supersampled_phases:?}"
    );
    assert!(
        native_phases
            .iter()
            .any(|progress| progress.total_pixels == 192 * 192),
        "preview icon render should plan 3x pixels before edge resolve; phases={native_phases:?}"
    );
    assert!(
        supersampled_phases
            .iter()
            .any(|progress| progress.total_pixels == 256 * 256),
        "ultra icon render should plan 4x pixels before downscale; phases={supersampled_phases:?}"
    );
    assert_ne!(
        native.as_raw(),
        supersampled.as_raw(),
        "ultra icon render should not collapse to the same pixels as native preview"
    );
}

#[test]
fn ultra_icon_quality_is_visibly_distinct_from_standard_without_washing_out_color() {
    let renderer = PlanetRenderer::new(forced_profile(SEED + 13, "catalog.archetype.global-ocean"));
    let icon_size = 72;
    let render_size = RenderSize {
        width: icon_size,
        height: icon_size,
    };

    assert_eq!(
        RenderOptions::standard().icon_supersample_for_size(render_size),
        3
    );
    assert_eq!(
        RenderOptions::ultra().icon_supersample_for_size(render_size),
        4
    );

    let standard = renderer.render_icon_with_options(icon_size, RenderOptions::standard());
    let ultra = renderer.render_icon_with_options(icon_size, RenderOptions::ultra());
    let standard_again = renderer.render_icon_with_options(icon_size, RenderOptions::standard());
    let ultra_again = renderer.render_icon_with_options(icon_size, RenderOptions::ultra());

    assert_eq!(standard.as_raw(), standard_again.as_raw());
    assert_eq!(ultra.as_raw(), ultra_again.as_raw());
    assert_nonblank(&standard, "standard global-ocean icon");
    assert_nonblank(&ultra, "ultra global-ocean icon");

    let delta = image_delta_stats(&standard, &ultra);
    let standard_color = visible_color_stats(&standard);
    let ultra_color = visible_color_stats(&ultra);

    assert!(
        delta.visible_pixels > icon_size * icon_size / 3,
        "standard/ultra comparison should cover the visible planet body; got {delta:?}"
    );
    assert!(
        delta.changed_visible_ratio > 0.18 && delta.mean_rgb_delta > 0.35,
        "ultra should visibly resolve to different final pixels than standard, not only flip a few samples; delta={delta:?}"
    );
    assert!(
        delta.max_rgb_delta > 4.0,
        "ultra should produce at least some materially different resolved pixels from standard; delta={delta:?}"
    );
    assert!(
        ultra_color.visible_pixels == standard_color.visible_pixels,
        "quality settings should preserve planet coverage; standard={standard_color:?}, ultra={ultra_color:?}"
    );
    assert!(
        ultra_color.mean_chroma >= standard_color.mean_chroma * 0.92
            && ultra_color.mean_chroma > 32.0
            && ultra_color.chroma_range > 70.0,
        "ultra quality should preserve strong ocean color instead of washing out the resolved image; standard={standard_color:?}, ultra={ultra_color:?}"
    );
    assert!(
        ultra_color.distinct_rgb >= standard_color.distinct_rgb * 3 / 4,
        "ultra quality should keep rich color variation after downscale; standard={standard_color:?}, ultra={ultra_color:?}"
    );
}

fn image_delta_stats(left: &RgbaImage, right: &RgbaImage) -> ImageDeltaStats {
    assert_eq!(left.dimensions(), right.dimensions());

    let mut visible_pixels = 0_u32;
    let mut changed_visible = 0_u32;
    let mut total_delta = 0.0_f32;
    let mut max_rgb_delta = 0.0_f32;

    for (left, right) in left.pixels().zip(right.pixels()) {
        if left[3] <= 16 && right[3] <= 16 {
            continue;
        }

        visible_pixels += 1;
        let delta = (left[0].abs_diff(right[0]) as f32
            + left[1].abs_diff(right[1]) as f32
            + left[2].abs_diff(right[2]) as f32)
            / 3.0;
        if delta > 0.0 || left[3] != right[3] {
            changed_visible += 1;
        }
        total_delta += delta;
        max_rgb_delta = max_rgb_delta.max(delta);
    }

    ImageDeltaStats {
        visible_pixels,
        changed_visible_ratio: changed_visible as f32 / visible_pixels.max(1) as f32,
        mean_rgb_delta: total_delta / visible_pixels.max(1) as f32,
        max_rgb_delta,
    }
}

fn visible_color_stats(image: &RgbaImage) -> VisibleColorStats {
    let mut visible_pixels = 0_u32;
    let mut total_chroma = 0.0_f32;
    let mut min_chroma = f32::MAX;
    let mut max_chroma = f32::MIN;
    let mut distinct_rgb = HashSet::new();

    for pixel in image.pixels() {
        if pixel[3] <= 16 {
            continue;
        }

        let max_channel = pixel[0].max(pixel[1]).max(pixel[2]) as f32;
        let min_channel = pixel[0].min(pixel[1]).min(pixel[2]) as f32;
        let chroma = max_channel - min_channel;
        visible_pixels += 1;
        total_chroma += chroma;
        min_chroma = min_chroma.min(chroma);
        max_chroma = max_chroma.max(chroma);
        distinct_rgb.insert([pixel[0], pixel[1], pixel[2]]);
    }

    VisibleColorStats {
        visible_pixels,
        mean_chroma: total_chroma / visible_pixels.max(1) as f32,
        chroma_range: if visible_pixels == 0 {
            0.0
        } else {
            max_chroma - min_chroma
        },
        distinct_rgb: distinct_rgb.len(),
    }
}
