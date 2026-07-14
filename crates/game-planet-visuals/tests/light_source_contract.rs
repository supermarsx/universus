use game_planet_visuals::pathtrace::{
    intersect_ray_sphere, Camera, CpuTraceKernel, MaterialSample, PathTraceSettings, Ray, Sphere,
    TraceImage, TraceScene, TraceSurfaceControls, Vec3,
};
use game_planet_visuals::{
    DistantLight, PlanetRenderer, PlanetVisualProfile, ProfileSeedInput, RenderOptions, RenderSize,
};
use image::RgbaImage;

const EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy)]
struct WeightedCentroid {
    x: f32,
    y: f32,
    weight: f32,
}

#[test]
fn default_pathtrace_light_direction_is_normalized() {
    let scene = TraceScene::default();
    let length = scene.light_direction.length();

    assert!(
        (length - 1.0).abs() < EPSILON,
        "default pathtrace light should be a normalized distant direction, got {:?} with length {length}",
        scene.light_direction
    );
    assert_eq!(
        CpuTraceKernel::default().scene.light_direction,
        scene.light_direction,
        "default CPU traces should use the public TraceScene light direction"
    );
}

#[test]
fn raster_and_pathtrace_light_data_are_shared_once_public_api_exists() {
    let raster = DistantLight::solar_default();
    let trace = TraceScene::default();
    let raster_dir = Vec3::new(
        raster.direction[0],
        raster.direction[1],
        raster.direction[2],
    );
    let trace_dir = trace.light_direction.normalize();
    let projected = raster.projected_overview_screen(16.0 / 9.0);

    assert!(
        (raster_dir.length() - 1.0).abs() < EPSILON,
        "shared raster light direction should be normalized; got {raster_dir:?}"
    );
    assert!(
        raster_dir.dot(trace_dir) > 0.999,
        "raster and pathtrace default lights should share one distant solar direction; raster={raster_dir:?}, trace={trace_dir:?}"
    );
    assert!(
        (0.06..=0.94).contains(&projected[0]) && (0.055..=0.86).contains(&projected[1]),
        "projected distant light should land in stable overview screen bounds; got {projected:?}"
    );
    assert!(
        raster.sun_disk_cosine_threshold() > 0.99,
        "default solar angular radius should describe a distant small disk, got threshold {}",
        raster.sun_disk_cosine_threshold()
    );
}

#[test]
fn raster_water_glint_centroid_tracks_day_sun_projection() {
    let mut profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(0x5EED_1208_5101)
            .with_archetype_key("catalog.archetype.global-ocean"),
    );
    profile.ocean_fraction = 1.0;
    profile.cloud_density = 0.0;
    profile.atmosphere_density = 0.95;
    profile.ringed = false;

    let size = RenderSize {
        width: 240,
        height: 135,
    };
    let image = PlanetRenderer::new(profile)
        .render_terrain_overview_with_options(size, RenderOptions::preview());
    let centroid = raster_glint_centroid(&image);
    let expected_x = expected_day_overview_sun_x(size);

    assert!(
        centroid.weight > 5.0,
        "day ocean glint should produce measurable highlight weight, got {centroid:?}"
    );
    assert!(
        (centroid.x - expected_x).abs() < 0.14,
        "day ocean glint centroid should stay near the sun projection; centroid={centroid:?}, expected_x={expected_x:.3}"
    );
    assert!(
        centroid.y > 0.42,
        "glint centroid should be on the ocean surface rather than the sky; got {centroid:?}"
    );
}

#[test]
fn pathtrace_reflection_lobe_tracks_scene_light_direction() {
    let left_light = Vec3::new(-0.58, 0.0, 0.82).normalize();
    let right_light = Vec3::new(0.58, 0.0, 0.82).normalize();
    let left = render_smooth_specular_planet(left_light);
    let right = render_smooth_specular_planet(right_light);

    let left_centroid = positive_trace_delta_centroid(&left, &right);
    let right_centroid = positive_trace_delta_centroid(&right, &left);

    assert!(
        left_centroid.x < 0.46,
        "left-side scene light should place the specular reflection lobe left of center; got {left_centroid:?}"
    );
    assert!(
        right_centroid.x > 0.54,
        "right-side scene light should place the specular reflection lobe right of center; got {right_centroid:?}"
    );
    assert!(
        right_centroid.x - left_centroid.x > 0.16,
        "changing scene light direction should move the reflection lobe; left={left_centroid:?}, right={right_centroid:?}"
    );
}

#[test]
fn distant_light_direction_does_not_encode_local_spotlight_attenuation() {
    let light = Vec3::new(-0.42, 0.31, 0.85).normalize();
    let scaled_light = light * 12.0;
    let settings = direct_trace_settings();
    let ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0));

    let unit_light = trace_center_ray(light, ray, settings);
    let magnitude_encoded_light = trace_center_ray(scaled_light, ray, settings);
    let near_camera = trace_center_ray(
        light,
        Ray::normalized(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0)),
        settings,
    );
    let far_camera = trace_center_ray(
        light,
        Ray::normalized(Vec3::new(0.0, 0.0, 8.0), Vec3::new(0.0, 0.0, -1.0)),
        settings,
    );

    assert!(
        color_delta(unit_light, magnitude_encoded_light) < 0.000_01,
        "light magnitude should not act like local source intensity or range; unit={unit_light:?}, scaled={magnitude_encoded_light:?}"
    );
    assert!(
        color_delta(near_camera, far_camera) < 0.000_01,
        "same surface/view/light direction should not dim with camera-source distance; near={near_camera:?}, far={far_camera:?}"
    );
}

#[test]
fn atmosphere_scattering_changes_with_solar_angle_while_optical_depth_stays_stable() {
    let ray = Ray::normalized(Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, -1.0));
    let settings = PathTraceSettings {
        atmosphere_samples: 8,
        ..PathTraceSettings::preview()
    };
    let forward_light = Vec3::new(0.0, 0.0, -1.0);
    let side_light = Vec3::new(1.0, 0.0, 0.0);
    let forward = sample_center_atmosphere(forward_light, ray, settings);
    let side = sample_center_atmosphere(side_light, ray, settings);

    assert_eq!(forward.samples, settings.atmosphere_samples);
    assert_eq!(side.samples, settings.atmosphere_samples);
    assert!(
        (forward.optical_depth - side.optical_depth).abs() < 0.000_001,
        "solar angle should not change geometric optical depth; forward={forward:?}, side={side:?}"
    );
    assert!(
        color_delta(forward.color, side.color) > 0.015,
        "solar angle should change atmosphere scatter color/phase; forward={forward:?}, side={side:?}"
    );
}

fn render_smooth_specular_planet(light_direction: Vec3) -> TraceImage {
    let material = MaterialSample {
        albedo: Vec3::splat(0.018),
        roughness: 0.02,
        metallic: 0.0,
        transmission: 0.0,
        opacity: 1.0,
        index_of_refraction: 1.45,
        ..MaterialSample::default()
    };
    let kernel = CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_density: 0.0,
            light_direction,
            sky_color: Vec3::ZERO,
            horizon_color: Vec3::ZERO,
            ..TraceScene::default()
        },
        TraceSurfaceControls::smooth(0x5EED_1208_5102),
    );
    let width = 56;
    let height = 56;
    let camera = Camera::look_at(
        Vec3::new(0.0, 0.0, 3.2),
        Vec3::ZERO,
        Vec3::Y,
        34.0,
        width as f32 / height as f32,
    );

    kernel
        .trace_image(camera, width, height, direct_trace_settings())
        .expect("small specular pathtrace should render")
}

fn trace_center_ray(light_direction: Vec3, ray: Ray, settings: PathTraceSettings) -> Vec3 {
    let material = MaterialSample {
        albedo: Vec3::new(0.36, 0.42, 0.48),
        roughness: 0.64,
        metallic: 0.0,
        transmission: 0.0,
        opacity: 1.0,
        ..MaterialSample::default()
    };
    let kernel = CpuTraceKernel::new_with_surface(
        TraceScene {
            planet: Sphere::new(Vec3::ZERO, 1.0, material),
            atmosphere_density: 0.0,
            light_direction,
            sky_color: Vec3::ZERO,
            horizon_color: Vec3::ZERO,
            ..TraceScene::default()
        },
        TraceSurfaceControls::smooth(0x5EED_1208_5103),
    );

    kernel
        .trace_ray(ray, settings)
        .expect("center ray should trace")
        .color
}

fn sample_center_atmosphere(
    light_direction: Vec3,
    ray: Ray,
    settings: PathTraceSettings,
) -> game_planet_visuals::pathtrace::AtmosphereSample {
    let kernel = CpuTraceKernel::new(TraceScene {
        atmosphere_radius: 1.16,
        atmosphere_density: 0.95,
        light_direction,
        sky_color: Vec3::new(0.012, 0.028, 0.070),
        horizon_color: Vec3::new(0.55, 0.70, 0.92),
        ..TraceScene::default()
    });
    let hit = intersect_ray_sphere(ray, kernel.scene.planet)
        .expect("center atmosphere ray should hit the planet");

    kernel.sample_atmosphere(ray, settings, Some(hit.t))
}

fn direct_trace_settings() -> PathTraceSettings {
    PathTraceSettings {
        samples_per_pixel: 1,
        seeded_jitter: false,
        max_bounces: 2,
        enable_reflections: true,
        enable_refractions: false,
        atmosphere_samples: 0,
        tile_width: 16,
        tile_height: 16,
        ..PathTraceSettings::preview()
    }
}

fn expected_day_overview_sun_x(size: RenderSize) -> f32 {
    let aspect = size.width.max(1) as f32 / size.height.max(1) as f32;
    let wide = ((aspect - 1.0) / 0.80).clamp(0.0, 1.0);
    let tall = ((1.0 - aspect) / 0.55).clamp(0.0, 1.0);
    (DistantLight::solar_default().projected_overview_screen(aspect)[0] + tall * 0.020
        - wide * 0.010)
        .clamp(0.08, 0.92)
}

fn raster_glint_centroid(image: &RgbaImage) -> WeightedCentroid {
    let mut scores = Vec::new();
    for y in (image.height() * 43 / 100)..image.height() {
        for x in 0..image.width() {
            scores.push(raster_glint_score(image.get_pixel(x, y)));
        }
    }
    scores.sort_by(|left, right| left.total_cmp(right));
    let threshold_index = (scores.len() * 995 / 1000).min(scores.len().saturating_sub(1));
    let threshold = scores[threshold_index];

    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;
    let mut total = 0.0;
    let width = image.width().max(1) as f32;
    let height = image.height().max(1) as f32;

    for y in (image.height() * 43 / 100)..image.height() {
        for x in 0..image.width() {
            let weight = (raster_glint_score(image.get_pixel(x, y)) - threshold).max(0.0);
            if weight > 0.0 {
                weighted_x += ((x as f32 + 0.5) / width) * weight;
                weighted_y += ((y as f32 + 0.5) / height) * weight;
                total += weight;
            }
        }
    }

    WeightedCentroid {
        x: weighted_x / total.max(f32::EPSILON),
        y: weighted_y / total.max(f32::EPSILON),
        weight: total,
    }
}

fn raster_glint_score(pixel: &image::Rgba<u8>) -> f32 {
    let red = pixel[0] as f32;
    let green = pixel[1] as f32;
    let blue = pixel[2] as f32;
    let luma = red * 0.2126 + green * 0.7152 + blue * 0.0722;
    let warm_specular = ((red + green) * 0.5 - blue * 0.62).max(0.0);

    (luma - 86.0).max(0.0) * warm_specular / 255.0
}

fn positive_trace_delta_centroid(brighter: &TraceImage, darker: &TraceImage) -> WeightedCentroid {
    assert_eq!(brighter.dimensions(), darker.dimensions());
    let mut weighted_x = 0.0;
    let mut weighted_y = 0.0;
    let mut total = 0.0;

    for y in 0..brighter.height {
        for x in 0..brighter.width {
            let brighter_luma = trace_luma(
                brighter
                    .pixel_at(x, y)
                    .expect("loop should stay inside trace image bounds"),
            );
            let darker_luma = trace_luma(
                darker
                    .pixel_at(x, y)
                    .expect("loop should stay inside trace image bounds"),
            );
            let weight = (brighter_luma - darker_luma).max(0.0);
            if weight > 0.0 {
                weighted_x += ((x as f32 + 0.5) / brighter.width.max(1) as f32) * weight;
                weighted_y += ((y as f32 + 0.5) / brighter.height.max(1) as f32) * weight;
                total += weight;
            }
        }
    }

    WeightedCentroid {
        x: weighted_x / total.max(f32::EPSILON),
        y: weighted_y / total.max(f32::EPSILON),
        weight: total,
    }
}

fn trace_luma(color: Vec3) -> f32 {
    color.x * 0.2126 + color.y * 0.7152 + color.z * 0.0722
}

fn color_delta(left: Vec3, right: Vec3) -> f32 {
    ((left.x - right.x).abs() + (left.y - right.y).abs() + (left.z - right.z).abs()) / 3.0
}
