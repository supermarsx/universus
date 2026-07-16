use game_planet_visuals::{PlanetRenderer, PlanetVisualProfile, RenderSize, TerrainDiagnosticAov};
use image::RgbaImage;

const SEED: u64 = 0x5EED_1208_0001;
const SIZE: RenderSize = RenderSize {
    width: 96,
    height: 54,
};

fn luma(pixel: &image::Rgba<u8>) -> f32 {
    (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 3.0
}

fn maximum_adjacent_row_mean_delta(image: &RgbaImage) -> f32 {
    let row_means = (0..image.height())
        .map(|y| {
            (0..image.width())
                .map(|x| luma(image.get_pixel(x, y)))
                .sum::<f32>()
                / image.width().max(1) as f32
        })
        .collect::<Vec<_>>();
    row_means
        .windows(2)
        .map(|rows| (rows[1] - rows[0]).abs())
        .fold(0.0_f32, f32::max)
}

fn normalized_laplacian_energy(image: &RgbaImage) -> f32 {
    let mut energy = 0.0_f32;
    let mut samples = 0_usize;
    for y in 1..image.height().saturating_sub(1) {
        for x in 1..image.width().saturating_sub(1) {
            let center = luma(image.get_pixel(x, y));
            let neighbors = luma(image.get_pixel(x - 1, y))
                + luma(image.get_pixel(x + 1, y))
                + luma(image.get_pixel(x, y - 1))
                + luma(image.get_pixel(x, y + 1));
            energy += (center * 4.0 - neighbors).abs();
            samples += 1;
        }
    }
    energy / samples.max(1) as f32 / 255.0
}

#[test]
fn terrain_aovs_keep_limb_coasts_clouds_and_normals_band_limited() {
    let renderer = PlanetRenderer::new(PlanetVisualProfile::from_seed(SEED));
    let atmosphere = renderer.render_terrain_diagnostic_aov(
        SIZE,
        TerrainDiagnosticAov::AtmosphereTransmittance,
        false,
    );
    let cloud =
        renderer.render_terrain_diagnostic_aov(SIZE, TerrainDiagnosticAov::CloudDensity, false);
    let water =
        renderer.render_terrain_diagnostic_aov(SIZE, TerrainDiagnosticAov::WaterCoverage, false);
    let fresnel =
        renderer.render_terrain_diagnostic_aov(SIZE, TerrainDiagnosticAov::WaterFresnel, false);
    let geometric_normal = renderer.render_terrain_diagnostic_aov(
        SIZE,
        TerrainDiagnosticAov::TerrainGeometricNormal,
        false,
    );
    let shading_normal = renderer.render_terrain_diagnostic_aov(
        SIZE,
        TerrainDiagnosticAov::TerrainShadingNormal,
        false,
    );

    let atmosphere_row_jump = maximum_adjacent_row_mean_delta(&atmosphere);
    let atmosphere_black_pixels = atmosphere
        .pixels()
        .filter(|pixel| luma(pixel) < 12.0)
        .count();
    assert_eq!(
        atmosphere_black_pixels, 0,
        "shared terrain/sky limb classification must not introduce black seam pixels"
    );
    assert!(
        atmosphere_row_jump < 40.0,
        "atmosphere limb transition must remain supersampled; adjacent-row jump was {atmosphere_row_jump:.3}/255"
    );

    let fractional_water = water
        .pixels()
        .filter(|pixel| (5..=250).contains(&pixel[0]))
        .count();
    let fractional_water_ratio = fractional_water as f32 / (SIZE.width * SIZE.height) as f32;
    assert!(
        fractional_water_ratio > 0.008,
        "coast coverage must retain a measurable fractional footprint; ratio was {fractional_water_ratio:.5}"
    );

    let mut isolated_fresnel = 0_usize;
    for y in 1..SIZE.height - 1 {
        for x in 1..SIZE.width - 1 {
            let center = luma(fresnel.get_pixel(x, y));
            let neighbor_max = [
                luma(fresnel.get_pixel(x - 1, y)),
                luma(fresnel.get_pixel(x + 1, y)),
                luma(fresnel.get_pixel(x, y - 1)),
                luma(fresnel.get_pixel(x, y + 1)),
            ]
            .into_iter()
            .fold(0.0_f32, f32::max);
            if center > 180.0 && neighbor_max < 80.0 {
                isolated_fresnel += 1;
            }
        }
    }
    let fresnel_laplacian = normalized_laplacian_energy(&fresnel);
    assert_eq!(
        isolated_fresnel, 0,
        "footprint-averaged Fresnel must not contain isolated silver pixels"
    );
    assert!(
        fresnel_laplacian < 0.055,
        "Fresnel footprint should remain band-limited; normalized Laplacian was {fresnel_laplacian:.5}"
    );

    let cloud_values = cloud.pixels().map(luma).collect::<Vec<_>>();
    let cloud_peak = cloud_values.iter().copied().fold(0.0_f32, f32::max);
    let cloud_occupied_ratio = cloud_values.iter().filter(|value| **value > 4.0).count() as f32
        / cloud_values.len() as f32;
    let mut lower_third_peak = 0.0_f32;
    for y in (SIZE.height * 2 / 3)..SIZE.height {
        for x in 0..SIZE.width {
            lower_third_peak = lower_third_peak.max(luma(cloud.get_pixel(x, y)));
        }
    }
    assert!(
        cloud_peak > 18.0,
        "cloud shell must contain resolved volumetric bodies; peak was {cloud_peak:.2}/255"
    );
    assert!(
        (0.008..0.32).contains(&cloud_occupied_ratio),
        "cloud bodies should be broken rather than absent or a full-frame veil; occupancy was {cloud_occupied_ratio:.5}"
    );
    assert!(
        lower_third_peak <= 4.0,
        "cloud integration must stop before terrain/below-horizon rays; lower-third peak was {lower_third_peak:.2}/255"
    );

    let mut normal_delta = 0.0_f32;
    let mut land_samples = 0_usize;
    for ((geometric, shading), water_pixel) in geometric_normal
        .pixels()
        .zip(shading_normal.pixels())
        .zip(water.pixels())
    {
        if water_pixel[0] < 32 && luma(geometric) > 10.0 {
            normal_delta += ((geometric[0] as f32 - shading[0] as f32).abs()
                + (geometric[1] as f32 - shading[1] as f32).abs()
                + (geometric[2] as f32 - shading[2] as f32).abs())
                / 3.0;
            land_samples += 1;
        }
    }
    let mean_normal_delta = normal_delta / land_samples.max(1) as f32;
    assert!(
        mean_normal_delta > 1.0,
        "shading normals must retain footprint-safe mesoscopic relief; mean land delta was {mean_normal_delta:.3}/255"
    );

    eprintln!(
        "terrain AOV metrics: atmosphere_row_jump={atmosphere_row_jump:.3}, fractional_water_ratio={fractional_water_ratio:.5}, fresnel_laplacian={fresnel_laplacian:.5}, cloud_peak={cloud_peak:.2}, cloud_occupied_ratio={cloud_occupied_ratio:.5}, mean_normal_delta={mean_normal_delta:.3}"
    );
}
