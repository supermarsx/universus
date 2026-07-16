// These max/min chains intentionally coerce non-finite procedural inputs to a
// safe bound; `f32::clamp` preserves NaN and would change that robustness rule.
#![allow(clippy::manual_clamp)]

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const fn splat(value: f32) -> Self {
        Self::new(value, value, value)
    }

    pub fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    pub fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize_or(self, fallback: Self) -> Self {
        let length = self.length();
        if length <= f32::EPSILON || !length.is_finite() {
            fallback
        } else {
            self / length
        }
    }

    pub fn normalize(self) -> Self {
        self.normalize_or(Self::ZERO)
    }

    pub fn lerp(self, rhs: Self, t: f32) -> Self {
        self + (rhs - self) * t
    }

    pub fn clamp(self, min: f32, max: f32) -> Self {
        Self::new(
            self.x.max(min).min(max),
            self.y.max(min).min(max),
            self.z.max(min).min(max),
        )
    }

    pub fn reflect(self, normal: Self) -> Self {
        self - normal * (2.0 * self.dot(normal))
    }

    pub fn refract(self, normal: Self, eta_ratio: f32) -> Option<Self> {
        let unit_direction = self.normalize_or(Vec3::new(0.0, 0.0, -1.0));
        let cos_theta = (unit_direction * -1.0).dot(normal).min(1.0);
        let r_out_perp = (unit_direction + normal * cos_theta) * eta_ratio;
        let parallel_len = 1.0 - r_out_perp.length_squared();
        (parallel_len >= 0.0).then(|| r_out_perp - normal * parallel_len.sqrt())
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl std::ops::Mul for Vec3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}

impl std::ops::Div<f32> for Vec3 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub t_min: f32,
    pub t_max: f32,
}

impl Ray {
    pub const fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction,
            t_min: 0.001,
            t_max: f32::INFINITY,
        }
    }

    pub fn normalized(origin: Vec3, direction: Vec3) -> Self {
        Self::new(origin, direction.normalize_or(Vec3::new(0.0, 0.0, -1.0)))
    }

    pub const fn with_bounds(mut self, t_min: f32, t_max: f32) -> Self {
        self.t_min = t_min;
        self.t_max = t_max;
        self
    }

    pub fn point_at(self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn has_valid_bounds(self) -> bool {
        self.t_min.is_finite() && self.t_max > self.t_min
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub origin: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub vertical_fov_degrees: f32,
    pub aspect_ratio: f32,
    pub aperture_radius: f32,
    pub focus_distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            origin: Vec3::ZERO,
            forward: Vec3::new(0.0, 0.0, -1.0),
            right: Vec3::new(1.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            vertical_fov_degrees: 45.0,
            aspect_ratio: 1.0,
            aperture_radius: 0.0,
            focus_distance: 1.0,
        }
    }
}

impl Camera {
    pub fn look_at(
        origin: Vec3,
        target: Vec3,
        world_up: Vec3,
        vertical_fov_degrees: f32,
        aspect_ratio: f32,
    ) -> Self {
        let forward = (target - origin).normalize_or(Vec3::new(0.0, 0.0, -1.0));
        let mut right = forward.cross(world_up);
        if right.length_squared() <= f32::EPSILON {
            let fallback_up = if forward.dot(Vec3::Y).abs() > 0.99 {
                Vec3::Z
            } else {
                Vec3::Y
            };
            right = forward.cross(fallback_up);
        }
        let right = right.normalize_or(Vec3::X);
        let up = right.cross(forward).normalize_or(Vec3::Y);

        Self {
            origin,
            forward,
            right,
            up,
            vertical_fov_degrees,
            aspect_ratio,
            aperture_radius: 0.0,
            focus_distance: 1.0,
        }
    }

    pub fn ray_for_uv(self, u: f32, v: f32) -> Ray {
        let fov_radians = self.vertical_fov_degrees.to_radians();
        let viewport_height = (fov_radians * 0.5).tan() * 2.0;
        let viewport_width = viewport_height * self.aspect_ratio.max(f32::EPSILON);
        let sensor_x = (u * 2.0 - 1.0) * viewport_width * 0.5;
        let sensor_y = (1.0 - v * 2.0) * viewport_height * 0.5;
        let direction =
            (self.forward * self.focus_distance + self.right * sensor_x + self.up * sensor_y)
                .normalize_or(self.forward);

        Ray::normalized(self.origin, direction)
    }

    pub fn ray_for_pixel(
        self,
        pixel_x: u32,
        pixel_y: u32,
        image_width: u32,
        image_height: u32,
        sample_index: u32,
        settings: PathTraceSettings,
    ) -> Ray {
        let (jitter_x, jitter_y) = settings.sample_jitter(pixel_x, pixel_y, sample_index);
        let width = image_width.max(1) as f32;
        let height = image_height.max(1) as f32;
        self.ray_for_uv(
            (pixel_x as f32 + jitter_x) / width,
            (pixel_y as f32 + jitter_y) / height,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathTraceSettings {
    pub max_bounces: u32,
    pub samples_per_pixel: u32,
    pub jitter_seed: u64,
    pub seeded_jitter: bool,
    pub enable_reflections: bool,
    pub enable_refractions: bool,
    pub atmosphere_samples: u32,
    pub tile_width: u32,
    pub tile_height: u32,
}

impl Default for PathTraceSettings {
    fn default() -> Self {
        Self {
            max_bounces: 6,
            samples_per_pixel: 64,
            jitter_seed: 0,
            seeded_jitter: true,
            enable_reflections: true,
            enable_refractions: true,
            atmosphere_samples: 8,
            tile_width: 32,
            tile_height: 32,
        }
    }
}

impl PathTraceSettings {
    pub const MAX_BOUNCES: u32 = 128;
    pub const MAX_SAMPLES_PER_PIXEL: u32 = 4096;
    pub const MAX_ATMOSPHERE_SAMPLES: u32 = 256;
    pub const MAX_TILE_EDGE: u32 = 4096;

    pub const fn preview() -> Self {
        Self {
            max_bounces: 2,
            samples_per_pixel: 1,
            jitter_seed: 0,
            seeded_jitter: true,
            enable_reflections: true,
            enable_refractions: true,
            atmosphere_samples: 2,
            tile_width: 16,
            tile_height: 16,
        }
    }

    pub fn validate(self) -> Result<Self, PathTraceSettingsError> {
        if self.samples_per_pixel == 0 {
            return Err(PathTraceSettingsError::ZeroSamplesPerPixel);
        }
        if self.samples_per_pixel > Self::MAX_SAMPLES_PER_PIXEL {
            return Err(PathTraceSettingsError::SamplesPerPixelTooHigh {
                value: self.samples_per_pixel,
                max: Self::MAX_SAMPLES_PER_PIXEL,
            });
        }
        if self.max_bounces > Self::MAX_BOUNCES {
            return Err(PathTraceSettingsError::MaxBouncesTooHigh {
                value: self.max_bounces,
                max: Self::MAX_BOUNCES,
            });
        }
        if self.atmosphere_samples > Self::MAX_ATMOSPHERE_SAMPLES {
            return Err(PathTraceSettingsError::AtmosphereSamplesTooHigh {
                value: self.atmosphere_samples,
                max: Self::MAX_ATMOSPHERE_SAMPLES,
            });
        }
        if self.tile_width == 0 {
            return Err(PathTraceSettingsError::ZeroTileWidth);
        }
        if self.tile_height == 0 {
            return Err(PathTraceSettingsError::ZeroTileHeight);
        }
        if self.tile_width > Self::MAX_TILE_EDGE {
            return Err(PathTraceSettingsError::TileWidthTooHigh {
                value: self.tile_width,
                max: Self::MAX_TILE_EDGE,
            });
        }
        if self.tile_height > Self::MAX_TILE_EDGE {
            return Err(PathTraceSettingsError::TileHeightTooHigh {
                value: self.tile_height,
                max: Self::MAX_TILE_EDGE,
            });
        }

        Ok(self)
    }

    pub fn sample_jitter(self, pixel_x: u32, pixel_y: u32, sample_index: u32) -> (f32, f32) {
        if !self.seeded_jitter {
            return (0.5, 0.5);
        }

        let stream = self
            .jitter_seed
            .wrapping_add((pixel_x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add((pixel_y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
            .wrapping_add((sample_index as u64).wrapping_mul(0x94D0_49BB_1331_11EB));
        let mut rng = SplitMix64::new(stream);
        (rng.next_unit_f32(), rng.next_unit_f32())
    }

    pub fn tiles_for_image(
        self,
        image_width: u32,
        image_height: u32,
    ) -> Result<Vec<Tile>, PathTraceSettingsError> {
        let settings = self.validate()?;
        let mut tiles = Vec::new();
        let mut y = 0;
        while y < image_height {
            let mut x = 0;
            while x < image_width {
                let width = settings.tile_width.min(image_width - x);
                let height = settings.tile_height.min(image_height - y);
                tiles.push(Tile::new(x, y, width, height));
                x = x.saturating_add(settings.tile_width);
            }
            y = y.saturating_add(settings.tile_height);
        }
        Ok(tiles)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TracePlan {
    pub image_width: u32,
    pub image_height: u32,
    pub settings: PathTraceSettings,
    pub tiles: Vec<Tile>,
    pub total_pixels: u64,
    pub total_samples: u64,
}

impl TracePlan {
    pub fn new(
        image_width: u32,
        image_height: u32,
        settings: PathTraceSettings,
    ) -> Result<Self, TracePlanError> {
        if image_width == 0 || image_height == 0 {
            return Err(TracePlanError::EmptyImage {
                width: image_width,
                height: image_height,
            });
        }

        let settings = settings.validate().map_err(TracePlanError::Settings)?;
        let total_pixels = u64::from(image_width) * u64::from(image_height);
        let total_samples = total_pixels
            .checked_mul(u64::from(settings.samples_per_pixel))
            .ok_or(TracePlanError::SampleCountOverflow {
                pixels: total_pixels,
                samples_per_pixel: settings.samples_per_pixel,
            })?;
        let tiles = settings
            .tiles_for_image(image_width, image_height)
            .map_err(TracePlanError::Settings)?;

        Ok(Self {
            image_width,
            image_height,
            settings,
            tiles,
            total_pixels,
            total_samples,
        })
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TracePlanError {
    EmptyImage { width: u32, height: u32 },
    Settings(PathTraceSettingsError),
    SampleCountOverflow { pixels: u64, samples_per_pixel: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTraceSettingsError {
    ZeroSamplesPerPixel,
    SamplesPerPixelTooHigh { value: u32, max: u32 },
    MaxBouncesTooHigh { value: u32, max: u32 },
    AtmosphereSamplesTooHigh { value: u32, max: u32 },
    ZeroTileWidth,
    ZeroTileHeight,
    TileWidthTooHigh { value: u32, max: u32 },
    TileHeightTooHigh { value: u32, max: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Tile {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn x_end(self) -> u32 {
        self.x.saturating_add(self.width)
    }

    pub const fn y_end(self) -> u32 {
        self.y.saturating_add(self.height)
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn pixel_count(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn contains(self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.x_end() && y >= self.y && y < self.y_end()
    }

    pub fn clipped_to(self, image_width: u32, image_height: u32) -> Option<Self> {
        if self.x >= image_width || self.y >= image_height {
            return None;
        }

        let x_end = self.x_end().min(image_width);
        let y_end = self.y_end().min(image_height);
        let clipped = Self::new(self.x, self.y, x_end - self.x, y_end - self.y);
        (!clipped.is_empty()).then_some(clipped)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleAccumulator {
    pub width: u32,
    pub height: u32,
    pub color_sum: Vec<Vec3>,
    pub sample_count: Vec<u32>,
}

impl SampleAccumulator {
    pub fn empty(width: u32, height: u32) -> Self {
        let len = checked_pixel_len(width, height).expect("accumulator dimensions overflow usize");
        Self {
            width,
            height,
            color_sum: vec![Vec3::ZERO; len],
            sample_count: vec![0; len],
        }
    }

    pub fn try_empty(width: u32, height: u32) -> Result<Self, AccumulatorError> {
        let len = checked_pixel_len(width, height)
            .ok_or(AccumulatorError::DimensionOverflow { width, height })?;
        Ok(Self {
            width,
            height,
            color_sum: vec![Vec3::ZERO; len],
            sample_count: vec![0; len],
        })
    }

    pub fn pixel_len(&self) -> usize {
        self.color_sum.len()
    }

    pub fn is_empty(&self) -> bool {
        self.color_sum.is_empty()
    }

    pub fn add_sample(&mut self, x: u32, y: u32, color: Vec3) -> Result<u32, AccumulatorError> {
        let index = self.index_of(x, y)?;
        self.color_sum[index] += color;
        self.sample_count[index] = self.sample_count[index].saturating_add(1);
        Ok(self.sample_count[index])
    }

    pub fn sample_count_at(&self, x: u32, y: u32) -> Result<u32, AccumulatorError> {
        Ok(self.sample_count[self.index_of(x, y)?])
    }

    pub fn color_sum_at(&self, x: u32, y: u32) -> Result<Vec3, AccumulatorError> {
        Ok(self.color_sum[self.index_of(x, y)?])
    }

    pub fn resolved_pixel(&self, x: u32, y: u32) -> Result<Vec3, AccumulatorError> {
        let index = self.index_of(x, y)?;
        let samples = self.sample_count[index];
        if samples == 0 {
            Ok(Vec3::ZERO)
        } else {
            Ok(self.color_sum[index] / samples as f32)
        }
    }

    pub fn clear(&mut self) {
        self.color_sum.fill(Vec3::ZERO);
        self.sample_count.fill(0);
    }

    fn index_of(&self, x: u32, y: u32) -> Result<usize, AccumulatorError> {
        if x >= self.width || y >= self.height {
            return Err(AccumulatorError::OutOfBounds {
                x,
                y,
                width: self.width,
                height: self.height,
            });
        }

        Ok((y as usize * self.width as usize) + x as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccumulatorError {
    DimensionOverflow {
        width: u32,
        height: u32,
    },
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceBackendKind {
    Cpu,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceBackendCapabilities {
    pub kind: TraceBackendKind,
    pub available: bool,
    pub hardware_accelerated: bool,
    pub deterministic: bool,
    pub supports_tiling: bool,
    pub supports_ray_sphere_intersection: bool,
    pub supports_reflections: bool,
    pub supports_refractions: bool,
    pub supports_atmosphere_sampling: bool,
    pub unavailable_reason: Option<&'static str>,
}

impl TraceBackendCapabilities {
    pub const fn cpu() -> Self {
        Self {
            kind: TraceBackendKind::Cpu,
            available: true,
            hardware_accelerated: false,
            deterministic: true,
            supports_tiling: true,
            supports_ray_sphere_intersection: true,
            supports_reflections: true,
            supports_refractions: true,
            supports_atmosphere_sampling: true,
            unavailable_reason: None,
        }
    }

    pub const fn gpu_unavailable(reason: &'static str) -> Self {
        Self {
            kind: TraceBackendKind::Gpu,
            available: false,
            hardware_accelerated: true,
            deterministic: false,
            supports_tiling: false,
            supports_ray_sphere_intersection: false,
            supports_reflections: false,
            supports_refractions: false,
            supports_atmosphere_sampling: false,
            unavailable_reason: Some(reason),
        }
    }
}

pub trait PathTraceBackend {
    fn trace_capabilities(&self) -> TraceBackendCapabilities;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialSample {
    pub albedo: Vec3,
    pub emission: Vec3,
    pub normal: Vec3,
    pub roughness: f32,
    pub metallic: f32,
    pub transmission: f32,
    pub index_of_refraction: f32,
    pub opacity: f32,
}

impl Default for MaterialSample {
    fn default() -> Self {
        Self {
            albedo: Vec3::new(1.0, 1.0, 1.0),
            emission: Vec3::ZERO,
            normal: Vec3::new(0.0, 1.0, 0.0),
            roughness: 0.5,
            metallic: 0.0,
            transmission: 0.0,
            index_of_refraction: 1.5,
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
    pub material: MaterialSample,
}

impl Sphere {
    pub const fn new(center: Vec3, radius: f32, material: MaterialSample) -> Self {
        Self {
            center,
            radius,
            material,
        }
    }

    pub fn is_valid(self) -> bool {
        self.center.is_finite() && self.radius.is_finite() && self.radius > 0.0
    }
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 1.0,
            material: MaterialSample {
                albedo: Vec3::new(0.32, 0.52, 0.92),
                roughness: 0.38,
                metallic: 0.18,
                transmission: 0.0,
                opacity: 1.0,
                ..MaterialSample::default()
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaySphereHit {
    pub t: f32,
    pub position: Vec3,
    pub normal: Vec3,
    pub front_face: bool,
    pub material: MaterialSample,
}

pub fn intersect_ray_sphere(ray: Ray, sphere: Sphere) -> Option<RaySphereHit> {
    if !sphere.is_valid() || !ray.has_valid_bounds() {
        return None;
    }

    let (near_t, far_t) = intersect_ray_sphere_interval(ray, sphere)?;
    let t = if near_t >= ray.t_min { near_t } else { far_t };
    if t < ray.t_min || t > ray.t_max {
        return None;
    }

    let position = ray.point_at(t);
    let outward_normal = (position - sphere.center) / sphere.radius;
    let front_face = ray.direction.dot(outward_normal) < 0.0;
    let normal = if front_face {
        outward_normal
    } else {
        outward_normal * -1.0
    };

    Some(RaySphereHit {
        t,
        position,
        normal,
        front_face,
        material: sphere.material,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceSurfaceModel {
    Smooth,
    #[default]
    Terrestrial,
    Ocean,
    BandedGasGiant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceSurfaceControls {
    pub seed: u64,
    pub time_days: f32,
    pub surface_model: TraceSurfaceModel,
    pub ocean_fraction: f32,
    pub band_frequency: f32,
    pub band_contrast: f32,
    pub cloud_coverage: f32,
    pub cloud_opacity: f32,
    pub atmosphere_color: Vec3,
    pub atmosphere_strength: f32,
}

impl Default for TraceSurfaceControls {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TraceSurfaceControls {
    pub const DEFAULT: Self = Self {
        seed: 0,
        time_days: 0.0,
        surface_model: TraceSurfaceModel::Terrestrial,
        ocean_fraction: 0.54,
        band_frequency: 9.0,
        band_contrast: 0.38,
        cloud_coverage: 0.34,
        cloud_opacity: 0.42,
        atmosphere_color: Vec3::new(0.42, 0.58, 0.92),
        atmosphere_strength: 1.0,
    };

    pub const fn smooth(seed: u64) -> Self {
        Self {
            seed,
            time_days: 0.0,
            surface_model: TraceSurfaceModel::Smooth,
            ocean_fraction: 0.0,
            band_frequency: 0.0,
            band_contrast: 0.0,
            cloud_coverage: 0.0,
            cloud_opacity: 0.0,
            atmosphere_color: Vec3::new(0.42, 0.58, 0.92),
            atmosphere_strength: 1.0,
        }
    }

    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub const fn with_surface_model(mut self, surface_model: TraceSurfaceModel) -> Self {
        self.surface_model = surface_model;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceScene {
    pub planet: Sphere,
    pub atmosphere_radius: f32,
    pub atmosphere_density: f32,
    pub light_direction: Vec3,
    pub sky_color: Vec3,
    pub horizon_color: Vec3,
}

impl Default for TraceScene {
    fn default() -> Self {
        Self {
            planet: Sphere::default(),
            atmosphere_radius: 1.08,
            atmosphere_density: 0.55,
            light_direction: Vec3::new(-0.48, 0.66, 0.58).normalize(),
            sky_color: Vec3::new(0.014, 0.026, 0.065),
            horizon_color: Vec3::new(0.42, 0.58, 0.92),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DistantSolarLight {
    direction: Vec3,
    color: Vec3,
    intensity: f32,
    angular_radius: f32,
    diffusion: f32,
}

impl TraceScene {
    fn distant_solar_light(&self) -> DistantSolarLight {
        let default_direction = Vec3::new(-0.48, 0.66, 0.58).normalize_or(Vec3::Y);
        let horizon_tint = self.horizon_color.clamp(0.0, 4.0);

        DistantSolarLight {
            direction: self.light_direction.normalize_or(default_direction),
            color: Vec3::new(1.0, 0.92, 0.76)
                .lerp(horizon_tint, 0.10)
                .clamp(0.0, 4.0),
            intensity: 1.0,
            angular_radius: 0.006_80,
            diffusion: (0.085 + self.scene_density_for_light() * 0.13).min(0.38),
        }
    }

    fn scene_density_for_light(&self) -> f32 {
        self.atmosphere_density.max(0.0).min(2.0)
    }
}

impl DistantSolarLight {
    fn environment_lobe(self, direction: Vec3) -> f32 {
        let alignment = direction
            .normalize_or(self.direction)
            .dot(self.direction)
            .max(0.0);
        let core_width = (self.angular_radius * 6.0 + self.diffusion * 0.035)
            .max(0.006)
            .min(0.080);
        let core = smoothstep(1.0 - core_width, 1.0, alignment);
        let glow_power = lerp_f32(96.0, 48.0, self.diffusion);
        let glow = alignment.powf(glow_power) * (0.72 + self.diffusion * 0.48);
        let shoulder_width = (self.angular_radius * 16.0 + self.diffusion * 0.16)
            .max(0.035)
            .min(0.18);
        let shoulder =
            smoothstep(1.0 - shoulder_width, 1.0, alignment) * (0.10 + self.diffusion * 0.24);

        (core * 0.38 + glow + shoulder).min(1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtmosphereSample {
    pub samples: u32,
    pub optical_depth: f32,
    pub color: Vec3,
    pub limb_factor: f32,
    pub refraction_bend: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceSample {
    pub color: Vec3,
    pub hit: Option<RaySphereHit>,
    pub stats: TraceStats,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceTileOutput {
    pub tile: Tile,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Vec3>,
    pub stats: TraceStats,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Vec3>,
    pub stats: TraceStats,
    pub plan: TracePlan,
}

impl TraceImage {
    pub const MAX_PIXELS: u64 = 16_777_216;

    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn pixel_len(&self) -> usize {
        self.pixels.len()
    }

    pub fn pixels(&self) -> &[Vec3] {
        &self.pixels
    }

    pub fn pixel_at(&self, x: u32, y: u32) -> Option<Vec3> {
        if x >= self.width || y >= self.height {
            return None;
        }

        let index = y as usize * self.width as usize + x as usize;
        self.pixels.get(index).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceError {
    EmptyImage {
        width: u32,
        height: u32,
    },
    Settings(PathTraceSettingsError),
    SampleCountOverflow {
        pixels: u64,
        samples_per_pixel: u32,
    },
    ImageTooLarge {
        pixels: u64,
        max_pixels: u64,
    },
    TileOutsideImage {
        tile: Tile,
        image_width: u32,
        image_height: u32,
    },
    DimensionOverflow {
        width: u32,
        height: u32,
    },
}

impl From<TracePlanError> for TraceError {
    fn from(error: TracePlanError) -> Self {
        match error {
            TracePlanError::EmptyImage { width, height } => Self::EmptyImage { width, height },
            TracePlanError::Settings(error) => Self::Settings(error),
            TracePlanError::SampleCountOverflow {
                pixels,
                samples_per_pixel,
            } => Self::SampleCountOverflow {
                pixels,
                samples_per_pixel,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ProceduralSurfaceSample {
    material: MaterialSample,
    normal: Vec3,
    specular_strength: f32,
    cloud_mask: f32,
    cloud_depth: f32,
    water_depth: f32,
    ambient_occlusion: f32,
    contact_shadow: f32,
    atmosphere_color: Vec3,
    atmosphere_strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CpuTraceKernel {
    pub scene: TraceScene,
    pub surface: TraceSurfaceControls,
}

impl CpuTraceKernel {
    pub const fn new(scene: TraceScene) -> Self {
        Self {
            scene,
            surface: TraceSurfaceControls::DEFAULT,
        }
    }

    pub const fn new_with_surface(scene: TraceScene, surface: TraceSurfaceControls) -> Self {
        Self { scene, surface }
    }

    pub const fn with_surface_controls(mut self, surface: TraceSurfaceControls) -> Self {
        self.surface = surface;
        self
    }

    pub fn plan(
        &self,
        image_width: u32,
        image_height: u32,
        settings: PathTraceSettings,
    ) -> Result<TracePlan, TracePlanError> {
        TracePlan::new(image_width, image_height, settings)
    }

    pub fn trace_ray(
        &self,
        ray: Ray,
        settings: PathTraceSettings,
    ) -> Result<TraceSample, PathTraceSettingsError> {
        let settings = settings.validate()?;
        let ray = Ray::normalized(ray.origin, ray.direction).with_bounds(ray.t_min, ray.t_max);
        let hit = intersect_ray_sphere(ray, self.scene.planet);
        let atmosphere = self.sample_atmosphere(ray, settings, hit.map(|hit| hit.t));

        let mut stats = TraceStats::default();
        stats.record_primary_rays(1);
        stats.record_atmosphere_samples(u64::from(atmosphere.samples));

        let color = if let Some(hit) = hit {
            stats.record_shadow_rays(1);
            stats.record_bounce_depth(1);
            self.shade_hit(ray, hit, atmosphere, settings, &mut stats)
        } else {
            let sky_dir = atmosphere_refracted_direction(ray, self.scene.planet.center, atmosphere);
            sample_environment(sky_dir, &self.scene)
                + atmosphere.color
                    * (atmosphere.optical_depth * (0.86 + atmosphere.limb_factor * 0.42))
        };

        Ok(TraceSample {
            color: color.clamp(0.0, 16.0),
            hit,
            stats,
        })
    }

    pub fn trace_pixel(
        &self,
        camera: Camera,
        pixel_x: u32,
        pixel_y: u32,
        image_width: u32,
        image_height: u32,
        settings: PathTraceSettings,
    ) -> Result<TraceSample, PathTraceSettingsError> {
        let settings = settings.validate()?;
        let mut color = Vec3::ZERO;
        let mut stats = TraceStats::default();
        let mut first_hit = None;

        for sample_index in 0..settings.samples_per_pixel {
            let ray = camera.ray_for_pixel(
                pixel_x,
                pixel_y,
                image_width,
                image_height,
                sample_index,
                settings,
            );
            let sample = self.trace_ray(ray, settings)?;
            if first_hit.is_none() {
                first_hit = sample.hit;
            }
            color += sample.color;
            stats.merge(sample.stats);
            stats.record_samples_completed(1);
        }

        Ok(TraceSample {
            color: apply_trace_dither(
                color / settings.samples_per_pixel as f32,
                pixel_x,
                pixel_y,
                settings,
            ),
            hit: first_hit,
            stats,
        })
    }

    pub fn trace_tile(
        &self,
        camera: Camera,
        image_width: u32,
        image_height: u32,
        tile: Tile,
        settings: PathTraceSettings,
    ) -> Result<TraceTileOutput, TraceError> {
        if image_width == 0 || image_height == 0 {
            return Err(TraceError::EmptyImage {
                width: image_width,
                height: image_height,
            });
        }

        let settings = settings.validate().map_err(TraceError::Settings)?;
        let tile =
            tile.clipped_to(image_width, image_height)
                .ok_or(TraceError::TileOutsideImage {
                    tile,
                    image_width,
                    image_height,
                })?;
        let pixel_len =
            checked_pixel_len(tile.width, tile.height).ok_or(TraceError::DimensionOverflow {
                width: tile.width,
                height: tile.height,
            })?;
        let mut pixels = vec![Vec3::ZERO; pixel_len];
        let mut stats = TraceStats::default();

        for local_y in 0..tile.height {
            for local_x in 0..tile.width {
                let pixel_x = tile.x + local_x;
                let pixel_y = tile.y + local_y;
                let sample = self
                    .trace_pixel(
                        camera,
                        pixel_x,
                        pixel_y,
                        image_width,
                        image_height,
                        settings,
                    )
                    .map_err(TraceError::Settings)?;
                let index = local_y as usize * tile.width as usize + local_x as usize;
                pixels[index] = sample.color;
                stats.merge(sample.stats);
            }
        }

        stats.record_tile_completed();

        Ok(TraceTileOutput {
            tile,
            width: tile.width,
            height: tile.height,
            pixels,
            stats,
        })
    }

    pub fn trace_image(
        &self,
        camera: Camera,
        image_width: u32,
        image_height: u32,
        settings: PathTraceSettings,
    ) -> Result<TraceImage, TraceError> {
        if image_width == 0 || image_height == 0 {
            return Err(TraceError::EmptyImage {
                width: image_width,
                height: image_height,
            });
        }

        let settings = settings.validate().map_err(TraceError::Settings)?;
        let total_pixels = u64::from(image_width) * u64::from(image_height);
        let _total_samples = total_pixels
            .checked_mul(u64::from(settings.samples_per_pixel))
            .ok_or(TraceError::SampleCountOverflow {
                pixels: total_pixels,
                samples_per_pixel: settings.samples_per_pixel,
            })?;
        if total_pixels > TraceImage::MAX_PIXELS {
            return Err(TraceError::ImageTooLarge {
                pixels: total_pixels,
                max_pixels: TraceImage::MAX_PIXELS,
            });
        }

        let plan = TracePlan::new(image_width, image_height, settings).map_err(TraceError::from)?;
        let pixel_len = checked_pixel_len(plan.image_width, plan.image_height).ok_or(
            TraceError::DimensionOverflow {
                width: plan.image_width,
                height: plan.image_height,
            },
        )?;
        let mut pixels = vec![Vec3::ZERO; pixel_len];
        let mut stats = TraceStats::default();

        for tile in plan.tiles.iter().copied() {
            let output = self.trace_tile(
                camera,
                plan.image_width,
                plan.image_height,
                tile,
                plan.settings,
            )?;

            for local_y in 0..output.height {
                for local_x in 0..output.width {
                    let src_index = local_y as usize * output.width as usize + local_x as usize;
                    let dst_x = output.tile.x + local_x;
                    let dst_y = output.tile.y + local_y;
                    let dst_index = dst_y as usize * plan.image_width as usize + dst_x as usize;
                    pixels[dst_index] = output.pixels[src_index];
                }
            }

            stats.merge(output.stats);
        }

        Ok(TraceImage {
            width: plan.image_width,
            height: plan.image_height,
            pixels,
            stats,
            plan,
        })
    }

    pub fn sample_atmosphere(
        &self,
        ray: Ray,
        settings: PathTraceSettings,
        max_t: Option<f32>,
    ) -> AtmosphereSample {
        let samples = settings.atmosphere_samples;
        if samples == 0
            || self.scene.atmosphere_density <= 0.0
            || self.scene.atmosphere_radius <= self.scene.planet.radius
        {
            return AtmosphereSample {
                samples: 0,
                optical_depth: 0.0,
                color: Vec3::ZERO,
                limb_factor: 0.0,
                refraction_bend: 0.0,
            };
        }

        let atmosphere = Sphere::new(
            self.scene.planet.center,
            self.scene.atmosphere_radius,
            MaterialSample::default(),
        );
        let Some((mut near_t, mut far_t)) = intersect_ray_sphere_interval(ray, atmosphere) else {
            return AtmosphereSample {
                samples: 0,
                optical_depth: 0.0,
                color: Vec3::ZERO,
                limb_factor: 0.0,
                refraction_bend: 0.0,
            };
        };

        near_t = near_t.max(ray.t_min);
        if let Some(max_t) = max_t {
            far_t = far_t.min(max_t);
        }
        far_t = far_t.min(ray.t_max);
        if far_t <= near_t {
            return AtmosphereSample {
                samples: 0,
                optical_depth: 0.0,
                color: Vec3::ZERO,
                limb_factor: 0.0,
                refraction_bend: 0.0,
            };
        }

        let shell_depth = (self.scene.atmosphere_radius - self.scene.planet.radius).max(0.000_1);
        let step = (far_t - near_t) / samples as f32;
        let mut optical_depth = 0.0;
        let mut density_sum = 0.0;
        let mut weighted_altitude = 0.0;

        for index in 0..samples {
            let t = near_t + (index as f32 + 0.5) * step;
            let altitude =
                (ray.point_at(t) - self.scene.planet.center).length() - self.scene.planet.radius;
            let altitude_unit = saturate(altitude / shell_depth);
            let lower_air = 1.0 - altitude_unit;
            let density = lower_air * lower_air * (1.45 - lower_air * 0.45);
            optical_depth += density * self.scene.atmosphere_density * step;
            density_sum += density;
            weighted_altitude += altitude_unit * density;
        }

        optical_depth = optical_depth.max(0.0).min(2.6);
        let mean_altitude = if density_sum <= f32::EPSILON {
            1.0
        } else {
            weighted_altitude / density_sum
        };
        let view_dir = ray.direction.normalize_or(Vec3::Z);
        let impact_radius = ray_impact_radius(ray, self.scene.planet.center);
        let limb_start = self.scene.planet.radius * 0.78;
        let grazing_airmass = smoothstep(
            self.scene.planet.radius * 0.58,
            self.scene.atmosphere_radius,
            impact_radius,
        );
        let limb_factor = smoothstep(limb_start, self.scene.atmosphere_radius, impact_radius)
            * smoothstep(0.010, 0.28, optical_depth)
            * (1.0 - mean_altitude * 0.34).max(0.42);
        optical_depth =
            (optical_depth * (1.0 + limb_factor * 0.78 + grazing_airmass * 0.16)).min(3.6);
        let refraction_bend =
            (limb_factor * self.scene.atmosphere_density.max(0.0) * optical_depth * 0.024)
                .min(0.052);
        let light = self.scene.distant_solar_light();
        let light_dir = light.direction;
        let phase = view_dir.dot(light_dir).max(-1.0).min(1.0);
        let rayleigh_phase = 0.62 + phase * phase * 0.38;
        let forward_mie = smoothstep(0.58 - light.diffusion * 0.20, 1.0, phase);
        let side_scatter = (1.0 - phase.abs()).max(0.0).powf(0.75);
        let back_scatter = smoothstep(0.16, 1.0, -phase);
        let horizon_bias = smoothstep(0.07, 1.05, optical_depth) * (1.0 - mean_altitude * 0.58)
            + limb_factor * 0.25
            + grazing_airmass * 0.06;
        let upper_sky = self
            .scene
            .sky_color
            .lerp(self.scene.horizon_color, 0.46)
            .clamp(0.0, 4.0);
        let low_haze = self.scene.horizon_color.clamp(0.0, 4.0);
        let solar_mie = (light.color * Vec3::new(1.10, 0.82, 0.50)).clamp(0.0, 4.0);
        let warm_mie = solar_mie.lerp(low_haze, 0.16) * light.intensity;
        let rayleigh_blue = Vec3::new(0.42, 0.66, 1.0).lerp(upper_sky, 0.16);
        let back_blue = upper_sky.lerp(rayleigh_blue, 0.58);
        let limb_scatter = rayleigh_blue.lerp(solar_mie, forward_mie * 0.46)
            * (limb_factor * (0.25 + optical_depth * 0.13));
        let extinction = atmosphere_extinction(optical_depth * (0.34 + limb_factor * 0.72));
        let scatter = (upper_sky.lerp(low_haze, horizon_bias) * (0.42 + rayleigh_phase * 0.58)
            + warm_mie * (forward_mie * 0.39 * (0.45 + horizon_bias * 0.55))
            + rayleigh_blue * (side_scatter * 0.065 * (0.50 + optical_depth * 0.28))
            + back_blue * (back_scatter * 0.082 * (0.42 + limb_factor * 0.62)))
            * extinction
            + limb_scatter;
        AtmosphereSample {
            samples,
            optical_depth,
            color: scatter.clamp(0.0, 6.0),
            limb_factor,
            refraction_bend,
        }
    }

    fn sample_surface(&self, hit: RaySphereHit) -> ProceduralSurfaceSample {
        let outward = (hit.position - self.scene.planet.center).normalize_or(hit.normal);
        let normal = orient_to_hit(outward, hit.normal);
        let mut sample = ProceduralSurfaceSample {
            material: hit.material,
            normal,
            specular_strength: (0.04 + hit.material.metallic * 0.30).max(0.02),
            cloud_mask: 0.0,
            cloud_depth: 0.0,
            water_depth: 0.0,
            ambient_occlusion: 1.0,
            contact_shadow: 1.0,
            atmosphere_color: self.surface.atmosphere_color.clamp(0.0, 4.0),
            atmosphere_strength: saturate(self.surface.atmosphere_strength),
        };

        sample = match self.surface.surface_model {
            TraceSurfaceModel::Smooth => sample,
            TraceSurfaceModel::Terrestrial => {
                terrestrial_surface_sample(sample, outward, normal, self.surface)
            }
            TraceSurfaceModel::Ocean => ocean_surface_sample(sample, outward, normal, self.surface),
            TraceSurfaceModel::BandedGasGiant => {
                gas_giant_surface_sample(sample, outward, normal, self.surface)
            }
        };

        if self.surface.surface_model != TraceSurfaceModel::Smooth {
            sample = apply_cloud_layer(sample, outward, self.surface);
        }

        sample
    }

    fn shade_hit(
        &self,
        ray: Ray,
        hit: RaySphereHit,
        atmosphere: AtmosphereSample,
        settings: PathTraceSettings,
        stats: &mut TraceStats,
    ) -> Vec3 {
        let surface = self.sample_surface(hit);
        let material = surface.material;
        let outward = (hit.position - self.scene.planet.center).normalize_or(surface.normal);
        let light = self.scene.distant_solar_light();
        let light_dir = light.direction;
        let view_dir = (ray.direction * -1.0).normalize_or(Vec3::Z);
        let half_vec = (light_dir + view_dir).normalize_or(light_dir);
        let signed_n_dot_l = surface.normal.dot(light_dir);
        let sun_softness = (light.diffusion * 0.62 + light.angular_radius * 14.0)
            .max(0.035)
            .min(0.34);
        let soft_sun = smoothstep(-sun_softness, sun_softness * 1.55, signed_n_dot_l);
        let terminator_factor = smoothstep(
            -sun_softness * 1.20,
            0.28 + sun_softness * 0.32,
            signed_n_dot_l,
        );
        let n_dot_l = signed_n_dot_l.max(0.0);
        let soft_n_dot_l = saturate(n_dot_l * (0.78 + soft_sun * 0.22) + soft_sun * 0.055);
        let n_dot_v = surface.normal.dot(view_dir).max(0.0);
        let roughness = saturate(material.roughness).max(0.02);
        let transmission = saturate(material.transmission);
        let metallic = saturate(material.metallic);
        let opacity = saturate(material.opacity);
        let dielectric_f0 = dielectric_f0(material.index_of_refraction);
        let fresnel = schlick_fresnel(n_dot_v, dielectric_f0);
        let rough_fresnel = lerp_f32(fresnel, dielectric_f0, roughness * 0.38);
        let specular_power = lerp_f32(18.0, 190.0, (1.0 - roughness).powf(1.6))
            / (1.0 + light.diffusion * 1.25 + light.angular_radius * 12.0);
        let specular = surface.normal.dot(half_vec).max(0.0).powf(specular_power)
            * (n_dot_l * 0.74 + soft_sun * 0.11).max(0.025);
        let rim = (1.0 - n_dot_v).powf(2.0);
        let cloud_density = saturate(surface.cloud_mask * (0.55 + surface.cloud_depth * 0.65));
        let water_depth = saturate(surface.water_depth);
        let ambient_occlusion = saturate(surface.ambient_occlusion);
        let contact_shadow = saturate(surface.contact_shadow);
        let directional_contact = procedural_contact_shadow(
            outward,
            surface.normal,
            light_dir,
            self.surface.seed,
            water_depth,
            cloud_density,
        );
        stats.record_ambient_occlusion_samples(1);
        if cloud_density > 0.001 || water_depth > 0.001 {
            stats.record_cloud_depth_samples(1);
        }
        let cloud_water_shadow = water_depth * cloud_density * (0.16 + surface.cloud_depth * 0.22);
        let atmosphere_strength = surface.atmosphere_strength * self.scene.atmosphere_density;
        let cloud_shadow = (1.0 - cloud_density * 0.30 - cloud_water_shadow * 1.18).max(0.38);
        let horizon_shadow = 1.0
            - rim * (0.10 + water_depth * 0.12 + cloud_density * 0.05)
            - atmosphere.limb_factor * (0.035 + water_depth * 0.035);
        let terminator_shadow =
            lerp_f32(0.22 + atmosphere_strength * 0.045, 1.0, terminator_factor);
        let direct_shadow = (contact_shadow
            * directional_contact
            * cloud_shadow
            * horizon_shadow.max(0.50)
            * terminator_shadow)
            .max(0.12 + atmosphere_strength * 0.045 + transmission * 0.035);
        let atmosphere_tint = surface
            .atmosphere_color
            .lerp(self.scene.horizon_color, 0.35);
        let diffuse_wrap =
            (0.11 * atmosphere_strength + transmission * 0.065) * (0.45 + soft_sun * 0.55);
        let diffuse = saturate((soft_n_dot_l + diffuse_wrap) / (1.0 + diffuse_wrap))
            * (0.42 + terminator_factor * 0.58);
        let depth_path =
            (0.20 + water_depth * 5.9 + cloud_density * water_depth * 0.82) / n_dot_v.max(0.16);
        let water_filter = water_transmittance(depth_path);
        let shaded_albedo = material.albedo.lerp(
            material.albedo * water_filter,
            water_depth * (0.36 + transmission * 0.46),
        );
        let sun_color = light.color * light.intensity;
        let sky_bounce = atmosphere_tint
            * ((0.024 + atmosphere.optical_depth * 0.018)
                * atmosphere_strength
                * (0.48 + rim * 0.52)
                * (0.54 + terminator_factor * 0.46)
                * ambient_occlusion);
        let night_air = atmosphere_tint
            * (0.018
                * atmosphere_strength
                * (1.0 - terminator_factor * 0.72)
                * (0.40 + rim * 0.60)
                * ambient_occlusion);
        let caustic = if water_depth > 0.001 && transmission > 0.0 {
            ridge_value(fbm_noise(
                outward,
                self.surface.seed ^ 0xCA75_710C_5EED,
                42.0,
                3,
            )) * water_depth
                * transmission
                * diffuse
                * direct_shadow
                * 0.10
        } else {
            0.0
        };
        let specular_gain = surface.specular_strength.max(0.0).min(1.5)
            * (0.55 + rough_fresnel * 3.1 + metallic * 0.85)
            * (1.0 - cloud_density * 0.42)
            * (0.62 + direct_shadow * 0.38);
        let cloud_forward = smoothstep(0.08, 1.0, soft_n_dot_l)
            * cloud_density
            * (0.08 + surface.cloud_depth * 0.14 + soft_sun * 0.045);
        let cloud_limb = rim
            * cloud_density
            * (0.08 + atmosphere_strength * 0.10 + atmosphere.limb_factor * 0.08);
        let limb_haze = atmosphere_tint
            * (atmosphere.limb_factor
                * (0.060 + atmosphere_strength * 0.095)
                * (0.34 + rim * 0.66));
        let refractive_edge = Vec3::new(0.48, 0.68, 1.0)
            * (atmosphere.refraction_bend * (7.8 + water_depth * 2.2) * (0.32 + rim * 0.68));
        let terminator_haze = atmosphere_tint.lerp(sun_color, 0.18)
            * ((1.0 - terminator_factor)
                * soft_sun
                * atmosphere_strength
                * (0.020 + rim * 0.048 + atmosphere.limb_factor * 0.030));

        let mut color = shaded_albedo
            * (0.044 * ambient_occlusion + diffuse * 0.90 * direct_shadow)
            + shaded_albedo * (transmission * water_depth * diffuse * 0.11 * direct_shadow)
            + sun_color * (specular * specular_gain)
            + material.emission
            + sky_bounce
            + night_air
            + Vec3::new(0.24, 0.72, 0.95) * caustic
            + atmosphere.color * (atmosphere.optical_depth * 0.065 * atmosphere_strength)
            + Vec3::new(0.98, 0.96, 0.90) * cloud_forward
            + atmosphere_tint * cloud_limb
            + limb_haze
            + refractive_edge
            + terminator_haze;

        if settings.enable_reflections && settings.max_bounces > 0 {
            let dielectric_reflection = rough_fresnel
                * (1.0 - roughness * 0.55)
                * (0.22 + transmission * 0.95 + water_depth * 0.55);
            let reflection_weight =
                saturate(metallic * (1.0 - roughness * 0.35) + dielectric_reflection);
            if reflection_weight > 0.0 {
                stats.record_reflection_rays(1);
                stats.record_bounce_depth(2);
                let reflect_dir = ray
                    .direction
                    .reflect(surface.normal)
                    .normalize_or(surface.normal);
                let horizon_reflection = smoothstep(0.18, 0.96, 1.0 - reflect_dir.y.abs());
                let reflected = sample_environment(reflect_dir, &self.scene).lerp(
                    atmosphere_tint,
                    (horizon_reflection * (0.22 + water_depth * 0.38)
                        + atmosphere.limb_factor * 0.16
                        + rim * water_depth * 0.10)
                        * atmosphere_strength,
                );
                let reflection_mix = (reflection_weight
                    * lerp_f32(0.24, 0.66, water_depth.max(metallic))
                    * (0.58 + direct_shadow * 0.22 + soft_sun * 0.20))
                    .min(0.78);
                color = color.lerp(reflected, reflection_mix);
            }
        }

        if settings.enable_refractions && settings.max_bounces > 0 {
            let refraction_weight = transmission
                * (1.0 - fresnel)
                * (1.0 - opacity * 0.42)
                * (1.0 - cloud_density * 0.55);
            if refraction_weight > 0.0 {
                stats.record_refraction_rays(1);
                stats.record_bounce_depth(2);
                let eta = if hit.front_face {
                    1.0 / material.index_of_refraction.max(1.0)
                } else {
                    material.index_of_refraction.max(1.0)
                };
                let refracted = ray
                    .direction
                    .refract(surface.normal, eta)
                    .unwrap_or_else(|| ray.direction.reflect(surface.normal))
                    .normalize_or(ray.direction);
                let path_depth = (0.22 + water_depth * 2.70 + cloud_density * water_depth * 1.05)
                    / n_dot_v.max(0.16);
                let water_filter = Vec3::ONE.lerp(
                    water_transmittance(path_depth * (0.20 + water_depth * 1.04)),
                    water_depth.max(0.18),
                );
                let refracted_horizon = smoothstep(0.10, 0.92, 1.0 - refracted.y.abs());
                let refracted_sky = sample_environment(refracted, &self.scene).lerp(
                    atmosphere_tint,
                    (refracted_horizon * (0.14 + water_depth * 0.24)
                        + atmosphere.limb_factor * 0.12
                        + rim * water_depth * 0.08)
                        * atmosphere_strength,
                );
                let submerged = shaded_albedo * (0.14 + diffuse * 0.22 * direct_shadow)
                    + Vec3::new(0.025, 0.20, 0.34) * (water_depth * (0.22 + cloud_density * 0.18));
                let transmitted = refracted_sky
                    * water_filter
                    * atmosphere_extinction(atmosphere.optical_depth * atmosphere_strength * 0.10)
                    * (0.32 + roughness * 0.18)
                    + submerged * 0.60;
                color = color.lerp(
                    transmitted,
                    (refraction_weight * lerp_f32(0.32, 0.62, water_depth)).min(0.72),
                );
            }
        }

        color
            * atmosphere_extinction(
                atmosphere.optical_depth
                    * atmosphere_strength
                    * (0.090 + rim * 0.150 + water_depth * 0.040 + cloud_density * 0.035),
            )
    }
}

fn terrestrial_surface_sample(
    mut sample: ProceduralSurfaceSample,
    outward: Vec3,
    normal: Vec3,
    controls: TraceSurfaceControls,
) -> ProceduralSurfaceSample {
    let uv = surface_uv(outward);
    let ocean_fraction = saturate(controls.ocean_fraction);
    let continental = fbm_noise(outward, controls.seed ^ 0xC011_71E5_5EED, 2.15, 5);
    let detail = fbm_noise(outward, controls.seed ^ 0xD37A_11ED_5EED, 9.25, 4);
    let moisture = fbm_noise(outward, controls.seed ^ 0xA011_5EA5_4AA1, 5.70, 4);
    let height = saturate(continental * 0.72 + detail * 0.22 + uv.latitude_abs * 0.06 - 0.03);
    let water_level = ocean_fraction;
    let ocean_mask = 1.0 - smoothstep(water_level - 0.045, water_level + 0.045, height);
    let shore = 1.0 - smoothstep(0.0, 0.095, (height - water_level).abs());
    let polar_ice = smoothstep(0.67, 0.93, uv.latitude_abs);
    let mountain = smoothstep(0.62, 0.95, height);
    let vegetation = smoothstep(0.42, 0.75, moisture) * (1.0 - polar_ice) * (1.0 - ocean_mask);
    let wave = fbm_noise(outward, controls.seed ^ 0x0CEA_1185_5EED, 25.0, 3);
    let ocean_flow =
        advected_surface_vector(outward, controls.time_days, controls.seed, 0.0020, 0.0007);
    let ocean_waves = ocean_trace_wave_spectrum(ocean_flow, controls.seed ^ 0x0CEA_733A_5EED);
    let bathymetry = fbm_noise(
        Vec3::new(
            outward.x * 0.82 - outward.z * 0.18,
            outward.y * 0.36,
            outward.z * 0.82 + outward.x * 0.18,
        ),
        controls.seed ^ 0x0CEA_BA7A_5EED,
        7.0,
        4,
    );
    let water_depth = saturate(
        ocean_mask
            * (0.30
                + (water_level - height).max(0.0) * 2.8
                + shore * 0.08
                + bathymetry * 0.20
                + ocean_waves.current * 0.10),
    );
    let terrain_crease = ridge_value(detail) * smoothstep(0.30, 0.86, height);
    let relief_occlusion = (1.0 - ocean_mask) * (mountain * 0.18 + terrain_crease * 0.09);
    let shoreline_occlusion = shore * (0.04 + ocean_mask * 0.06);
    let water_occlusion = ocean_mask
        * (water_depth * 0.12 + ridge_value(wave) * 0.030 + ocean_waves.interference * 0.030);

    let dry_land = Vec3::new(0.58, 0.43, 0.25);
    let green_land = Vec3::new(0.19, 0.39, 0.20);
    let highland = Vec3::new(0.51, 0.48, 0.40);
    let snow = Vec3::new(0.86, 0.90, 0.91);
    let deep_ocean = Vec3::new(0.018, 0.105, 0.34);
    let shelf_ocean = Vec3::new(0.04, 0.31, 0.52);

    let land = dry_land
        .lerp(green_land, vegetation)
        .lerp(highland, mountain * 0.55)
        .lerp(snow, polar_ice.max(mountain * polar_ice));
    let ocean = deep_ocean
        .lerp(
            shelf_ocean,
            (shore * 0.60 + wave * 0.12 + ocean_waves.current * 0.16).min(1.0),
        )
        .lerp(
            Vec3::new(0.08, 0.39, 0.52),
            ocean_waves.eddy * ocean_mask * 0.12,
        )
        .lerp(
            Vec3::new(0.34, 0.62, 0.70),
            ocean_waves.foam * ocean_mask * 0.12,
        )
        .lerp(snow, polar_ice * 0.28);
    let procedural = land.lerp(ocean, ocean_mask);

    sample.material.albedo = sample
        .material
        .albedo
        .lerp(procedural, 0.82)
        .clamp(0.0, 2.0);
    sample.material.roughness = lerp_f32(
        sample.material.roughness,
        lerp_f32(
            0.54 + detail * 0.22,
            0.050 + ocean_waves.chop * 0.052 + ocean_waves.ripple * 0.036 + shore * 0.08,
            ocean_mask,
        ),
        0.78,
    )
    .max(0.02)
    .min(1.0);
    sample.material.metallic = sample.material.metallic.max(ocean_mask * 0.09).min(1.0);
    sample.material.transmission = sample
        .material
        .transmission
        .max(ocean_mask * (0.16 + water_depth * 0.18))
        .min(0.52);
    sample.material.opacity = sample.material.opacity.min(lerp_f32(1.0, 0.74, ocean_mask));
    sample.specular_strength =
        (0.055 + ocean_mask * 0.32 + shore * 0.08 + sample.material.metallic * 0.25).min(1.0);
    sample.water_depth = sample.water_depth.max(water_depth);
    sample.ambient_occlusion = (sample.ambient_occlusion
        * (1.0 - relief_occlusion - shoreline_occlusion - water_occlusion))
        .max(0.54);
    sample.contact_shadow = (sample.contact_shadow
        * (1.0 - relief_occlusion * 0.72 - shoreline_occlusion * 0.80 - water_occlusion * 0.55))
        .max(0.58);
    let terrain_normal = perturb_surface_normal(
        normal,
        outward,
        controls.seed ^ 0xB10C_5EA5_5EED,
        lerp_f32(0.085, 0.040, ocean_mask),
    );
    let water_normal = ocean_trace_normal(normal, ocean_flow, ocean_waves, controls.seed);
    sample.normal = terrain_normal
        .lerp(water_normal, ocean_mask * 0.78)
        .normalize_or(terrain_normal);
    sample
}

fn ocean_surface_sample(
    mut sample: ProceduralSurfaceSample,
    outward: Vec3,
    normal: Vec3,
    controls: TraceSurfaceControls,
) -> ProceduralSurfaceSample {
    let uv = surface_uv(outward);
    let wave_outward =
        advected_surface_vector(outward, controls.time_days, controls.seed, 0.0022, 0.0008);
    let current_outward = advected_surface_vector(
        outward,
        controls.time_days,
        controls.seed ^ 0x0CEA_C012_5EED,
        0.0014,
        0.0005,
    );
    let waves = ocean_trace_wave_spectrum(wave_outward, controls.seed);
    let polar = smoothstep(0.72, 0.96, uv.latitude_abs);
    let ocean_extent = saturate(controls.ocean_fraction);
    let basin = fbm_noise(
        Vec3::new(
            outward.x * 0.72 + outward.z * 0.16,
            outward.y * 0.38,
            outward.z * 0.72 - outward.x * 0.16,
        ),
        controls.seed ^ 0x0CEA_BA51_5EED,
        3.1,
        5,
    );
    let current = fbm_noise(
        Vec3::new(
            current_outward.z * 1.45 - current_outward.x * 0.28,
            current_outward.y * 0.24 + current_outward.x * 0.18,
            current_outward.x * 1.45 + current_outward.z * 0.28,
        ),
        controls.seed ^ 0x0CEA_C012_5EED,
        12.0,
        4,
    );
    let depth_breakup = fbm_noise(
        Vec3::new(
            outward.x * 0.58 - outward.z * 0.12,
            outward.y * 0.22 + current_outward.x * 0.08,
            outward.z * 0.58 + outward.x * 0.12,
        ),
        controls.seed ^ 0x0CEA_DEE9_5EED,
        6.2,
        4,
    );
    let abyssal_ridge = ridge_value(fbm_noise(
        Vec3::new(
            current_outward.z * 0.88 + current_outward.x * 0.22,
            current_outward.y * 0.28,
            current_outward.x * 0.88 - current_outward.z * 0.22,
        ),
        controls.seed ^ 0x0CEA_7E4C_5EED,
        10.5,
        3,
    ));
    let shelf = smoothstep(
        0.50,
        0.92,
        basin * 0.46
            + current * 0.14
            + waves.swell * 0.12
            + waves.current * 0.12
            + (1.0 - ocean_extent) * 0.16,
    );
    let trench = smoothstep(
        0.55,
        0.96,
        (1.0 - shelf) * 0.34 + abyssal_ridge * 0.28 + depth_breakup * 0.20 + waves.eddy * 0.18,
    );
    let depth_variation = saturate(
        depth_breakup * 0.32
            + basin * 0.22
            + (1.0 - shelf) * 0.26
            + current * 0.10
            + waves.current * 0.10,
    );
    let depth = saturate(
        0.22 + depth_variation * 0.84 + trench * 0.23 + ocean_extent * 0.12
            - shelf * 0.34
            - polar * 0.10,
    );
    let foam_lace = ridge_value(fbm_noise(
        Vec3::new(
            outward.z * 2.25 - outward.x * 0.36,
            outward.y * 0.27,
            outward.x * 2.25 + outward.z * 0.36,
        ),
        controls.seed ^ 0x0CEA_F0A5_5EED,
        37.0,
        3,
    ));
    let whitewater = saturate(
        (waves.foam * 0.66 + smoothstep(0.44, 0.92, foam_lace) * 0.20 + waves.interference * 0.14)
            * (1.0 - polar * 0.55),
    );
    let glint_tint = waves.glint * (1.0 - whitewater * 0.42);
    let shallow_scatter = smoothstep(0.35, 0.90, shelf) * (1.0 - polar * 0.40);

    let trench_color = Vec3::new(0.002, 0.026, 0.12);
    let abyss = Vec3::new(0.004, 0.050, 0.20);
    let deep = Vec3::new(0.010, 0.105, 0.32);
    let mid = Vec3::new(0.020, 0.19, 0.40);
    let shelf_color = Vec3::new(0.050, 0.36, 0.53);
    let sunlit_shallows = Vec3::new(0.10, 0.50, 0.62);
    let crest = Vec3::new(0.36, 0.66, 0.75);
    let ice = Vec3::new(0.77, 0.88, 0.93);
    let current_tint = Vec3::new(0.014, 0.17, 0.34).lerp(Vec3::new(0.040, 0.28, 0.38), current);
    let procedural = shelf_color
        .lerp(mid, saturate(depth * 0.54 + waves.current * 0.15))
        .lerp(deep, smoothstep(0.30, 0.82, depth) * 0.72)
        .lerp(
            abyss,
            saturate(smoothstep(0.58, 0.98, depth) * 0.48 + (1.0 - shelf) * 0.18),
        )
        .lerp(trench_color, trench * 0.34)
        .lerp(
            current_tint,
            saturate(waves.eddy * 0.10 + waves.interference * 0.07),
        )
        .lerp(shelf_color, shelf * 0.34)
        .lerp(sunlit_shallows, shallow_scatter * 0.24)
        .lerp(crest, saturate(whitewater * 0.36 + glint_tint * 0.08))
        .lerp(ice, polar * 0.24);

    sample.material.albedo = sample
        .material
        .albedo
        .lerp(procedural, 0.92)
        .clamp(0.0, 2.0);
    let slick = smoothstep(0.55, 0.96, waves.glint) * (1.0 - whitewater * 0.65);
    let target_roughness = (0.024
        + waves.chop * 0.044
        + waves.ripple * 0.026
        + waves.roughness_breakup * 0.026
        + whitewater * 0.082
        + polar * 0.045
        - slick * 0.018)
        .max(0.018)
        .min(0.22);
    sample.material.roughness = lerp_f32(sample.material.roughness, target_roughness, 0.90)
        .max(0.02)
        .min(1.0);
    sample.material.metallic = (sample.material.metallic * 0.08).min(0.035);
    let water_depth = saturate(depth * (1.0 - shelf * 0.20) + (1.0 - shelf) * 0.15 + trench * 0.12);
    sample.material.transmission = sample
        .material
        .transmission
        .max(
            (0.18 + shelf * 0.22 + (1.0 - depth) * 0.10 - whitewater * 0.14 - polar * 0.12)
                .max(0.06),
        )
        .min(0.54);
    sample.material.index_of_refraction = 1.333;
    sample.material.opacity = sample.material.opacity.min(
        (0.80 - shelf * 0.14 + depth * 0.04 + trench * 0.04 + polar * 0.08)
            .max(0.56)
            .min(0.92),
    );
    sample.specular_strength =
        (0.38 + waves.glint * 0.34 + (1.0 - sample.material.roughness) * 0.18).min(1.15);
    sample.water_depth = sample.water_depth.max(water_depth);
    let wave_self_shadow =
        waves.chop * 0.055 + waves.foam * 0.060 + waves.roughness_breakup * 0.035;
    let trench_shadow = ((1.0 - shelf) * water_depth * 0.105 + trench * 0.052).min(0.18);
    let polar_shadow = polar * 0.035;
    sample.ambient_occlusion = (sample.ambient_occlusion
        * (1.0 - wave_self_shadow - trench_shadow - polar_shadow))
        .max(0.58);
    sample.contact_shadow =
        (sample.contact_shadow * (1.0 - wave_self_shadow * 0.86 - trench_shadow * 0.54)).max(0.62);
    sample.normal = ocean_trace_normal(normal, wave_outward, waves, controls.seed);
    sample
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OceanTraceWaves {
    swell: f32,
    chop: f32,
    ripple: f32,
    foam: f32,
    glint: f32,
    roughness_breakup: f32,
    current: f32,
    eddy: f32,
    interference: f32,
    slope: Vec3,
}

fn ocean_trace_wave_spectrum(outward: Vec3, seed: u64) -> OceanTraceWaves {
    let uv = surface_uv(outward);
    let broad_warp = fbm_noise(
        Vec3::new(
            outward.x * 0.82 + outward.z * 0.21,
            outward.y * 0.44,
            outward.z * 0.82 - outward.x * 0.21,
        ),
        seed ^ 0x7A1E_0CEA_5EED,
        3.7,
        4,
    );
    let shear_warp = fbm_noise(
        Vec3::new(
            outward.z * 1.32 - outward.y * 0.18,
            outward.x * 0.26 + outward.y * 0.54,
            outward.x * 1.32 + outward.z * 0.18,
        ),
        seed ^ 0x7A1E_0CEA_5EA2,
        9.5,
        4,
    );
    let patch = fbm_noise(
        Vec3::new(
            outward.x * 1.75 + outward.y * 0.19,
            outward.y * 0.48 + outward.z * 0.11,
            outward.z * 1.75 - outward.x * 0.19,
        ),
        seed ^ 0x7A1E_0CEA_5EA3,
        21.0,
        3,
    );
    let current_cells = fbm_noise(
        Vec3::new(
            outward.z * 0.96 - outward.x * 0.24,
            outward.y * 0.31 + outward.z * 0.10,
            outward.x * 0.96 + outward.z * 0.24,
        ),
        seed ^ 0x7A1E_0CEA_C012,
        5.8,
        4,
    );
    let eddy = ridge_value(fbm_noise(
        Vec3::new(
            outward.x * 2.65 - outward.z * 0.46,
            outward.y * 0.40 + outward.x * 0.12,
            outward.z * 2.65 + outward.x * 0.46,
        ),
        seed ^ 0x7A1E_0CEA_EDD1,
        15.5,
        3,
    ));
    let interference = saturate(
        ridge_value(shear_warp) * 0.40
            + ridge_value(patch) * 0.30
            + current_cells * 0.18
            + eddy * 0.12,
    );

    let u = (uv.u
        + (broad_warp - 0.5) * 0.054
        + (shear_warp - 0.5) * 0.018
        + (current_cells - 0.5) * 0.026
        + (eddy - 0.5) * 0.011)
        .rem_euclid(1.0);
    let v = (uv.v + (broad_warp - 0.5) * 0.030 - (shear_warp - 0.5) * 0.016
        + (patch - 0.5) * 0.010
        - (current_cells - 0.5) * 0.014)
        .max(0.0)
        .min(1.0);
    let turn = hash_unit_f32(seed ^ 0x71DE_0CEA_5EED) * std::f32::consts::PI * 2.0;
    let (s0, c0, dx0, dy0) =
        directional_ocean_wave(u, v, turn + 0.19, 7.5, hash_unit_f32(seed ^ 0x0CEA_1001));
    let (s1, c1, dx1, dy1) =
        directional_ocean_wave(u, v, turn + 1.64, 12.5, hash_unit_f32(seed ^ 0x0CEA_1002));
    let (s2, c2, dx2, dy2) =
        directional_ocean_wave(u, v, turn - 1.13, 22.0, hash_unit_f32(seed ^ 0x0CEA_1003));
    let (s3, c3, dx3, dy3) =
        directional_ocean_wave(u, v, turn + 2.55, 42.0, hash_unit_f32(seed ^ 0x0CEA_1004));
    let (s4, c4, dx4, dy4) =
        directional_ocean_wave(u, v, turn - 2.33, 78.0, hash_unit_f32(seed ^ 0x0CEA_1005));
    let (s5, c5, dx5, dy5) =
        directional_ocean_wave(u, v, turn + 0.88, 138.0, hash_unit_f32(seed ^ 0x0CEA_1006));

    let swell = saturate(0.5 + (s0 * 0.48 + s1 * 0.32 + s2 * 0.20) * 0.5);
    let chop =
        saturate(0.5 + (s2 * 0.28 + s3 * 0.40 + s4 * 0.22 + (shear_warp - 0.5) * 0.28) * 0.5);
    let ripple = saturate(0.5 + (s4 * 0.34 + s5 * 0.42 + (patch - 0.5) * 0.36) * 0.5);
    let ridged_patch = ridge_value(patch);
    let foam = smoothstep(
        0.60,
        0.98,
        chop * 0.50 + ripple * 0.30 + ridged_patch * 0.10 + interference * 0.10,
    ) * (0.30 + ridged_patch * 0.50 + eddy * 0.20);
    let glint = saturate(
        smoothstep(0.50, 0.96, chop) * 0.34
            + smoothstep(0.57, 0.98, ripple) * 0.28
            + ridged_patch * 0.19
            + interference * 0.11
            + smoothstep(0.58, 0.94, swell) * 0.14,
    );
    let roughness_breakup = saturate(
        fbm_noise(
            Vec3::new(
                outward.z * 2.4 + outward.x * 0.31,
                outward.y * 0.62,
                outward.x * 2.4 - outward.z * 0.31,
            ),
            seed ^ 0x7A1E_0CEA_5EA4,
            31.0,
            3,
        ) * 0.70
            + ridged_patch * 0.30,
    );
    let slope = Vec3::new(
        dx0 * c0 * 0.060
            + dx1 * c1 * 0.044
            + dx2 * c2 * 0.032
            + dx3 * c3 * 0.020
            + dx4 * c4 * 0.013
            + dx5 * c5 * 0.007,
        dy0 * c0 * 0.060
            + dy1 * c1 * 0.044
            + dy2 * c2 * 0.032
            + dy3 * c3 * 0.020
            + dy4 * c4 * 0.013
            + dy5 * c5 * 0.007,
        0.0,
    ) + Vec3::new(
        (current_cells - 0.5) * 0.026 + (patch - 0.5) * 0.012,
        (shear_warp - 0.5) * 0.021 + (eddy - 0.5) * 0.015,
        0.0,
    ) * (0.70 + interference * 0.55);

    OceanTraceWaves {
        swell,
        chop,
        ripple,
        foam,
        glint,
        roughness_breakup,
        current: current_cells,
        eddy,
        interference,
        slope,
    }
}

fn directional_ocean_wave(
    u: f32,
    v: f32,
    angle: f32,
    frequency: f32,
    phase_seed: f32,
) -> (f32, f32, f32, f32) {
    let dx = angle.cos();
    let dy = angle.sin();
    let phase =
        (u * dx + v * dy) * std::f32::consts::PI * 2.0 * frequency + phase_seed * 6.283_185_5;
    (phase.sin(), phase.cos(), dx, dy)
}

fn ocean_trace_normal(hit_normal: Vec3, outward: Vec3, waves: OceanTraceWaves, seed: u64) -> Vec3 {
    let (tangent, bitangent) = surface_tangent_basis(outward);
    let micro_x = fbm_noise(
        Vec3::new(
            outward.x * 1.66 + outward.z * 0.37,
            outward.y * 0.29,
            outward.z * 1.66 - outward.x * 0.37,
        ),
        seed ^ 0x5EA0_CEAA_5EED,
        64.0,
        3,
    ) - 0.5;
    let micro_y = fbm_noise(
        Vec3::new(
            outward.z * 1.58 - outward.x * 0.31,
            outward.y * 0.33,
            outward.x * 1.58 + outward.z * 0.31,
        ),
        seed ^ 0x5EA0_CEAA_5EA2,
        83.0,
        3,
    ) - 0.5;
    let cross_x = fbm_noise(
        Vec3::new(
            outward.x * 2.20 - outward.z * 0.52,
            outward.y * 0.42 + outward.x * 0.09,
            outward.z * 2.20 + outward.x * 0.52,
        ),
        seed ^ 0x5EA0_CEAA_C012,
        33.0,
        3,
    ) - 0.5;
    let cross_y = fbm_noise(
        Vec3::new(
            outward.z * 2.05 + outward.x * 0.44,
            outward.y * 0.38 - outward.z * 0.07,
            outward.x * 2.05 - outward.z * 0.44,
        ),
        seed ^ 0x5EA0_CEAA_C013,
        47.0,
        3,
    ) - 0.5;
    let polar_damping = lerp_f32(1.0, 0.58, smoothstep(0.74, 0.97, outward.y.abs()));
    let foam_damping = 1.0 - waves.foam * 0.32;
    let slope = waves.slope * (polar_damping * foam_damping)
        + Vec3::new(micro_x, micro_y, 0.0) * (0.007 + waves.ripple * 0.017)
        + Vec3::new(cross_x, cross_y, 0.0)
            * (0.005 + waves.interference * 0.015 + waves.eddy * 0.008);
    let perturbed = (outward + tangent * slope.x + bitangent * slope.y).normalize_or(outward);
    orient_to_hit(perturbed, hit_normal)
}

fn gas_giant_surface_sample(
    mut sample: ProceduralSurfaceSample,
    outward: Vec3,
    normal: Vec3,
    controls: TraceSurfaceControls,
) -> ProceduralSurfaceSample {
    let band_outward = advected_surface_vector(
        outward,
        controls.time_days,
        controls.seed ^ 0x6A55_6A55_5EED,
        0.0038,
        0.0007,
    );
    let uv = surface_uv(band_outward);
    let frequency = finite_or(controls.band_frequency, 9.0).max(1.0);
    let contrast = saturate(controls.band_contrast);
    let turbulence = fbm_noise(band_outward, controls.seed ^ 0x6A55_6A55_5EED, 5.0, 5) - 0.5;
    let phase = hash_unit_f32(controls.seed ^ 0xBADA_5515_5EED) * 6.283_185_5;
    let broad = ((uv.v * frequency + turbulence * 0.62) * 6.283_185_5 + phase).sin() * 0.5 + 0.5;
    let fine = ((uv.v * frequency * 2.45 - turbulence * 0.35) * 6.283_185_5 + phase * 0.37).sin()
        * 0.5
        + 0.5;
    let band = saturate(broad * 0.72 + fine * 0.28);
    let storm = smoothstep(
        0.76,
        0.98,
        fbm_noise(
            Vec3::new(
                band_outward.x * 1.8,
                band_outward.y * 0.35,
                band_outward.z * 1.8,
            ),
            controls.seed ^ 0x5702_1115_5EED,
            8.0,
            4,
        ),
    );

    let light_band = Vec3::new(0.92, 0.78, 0.52);
    let warm_band = Vec3::new(0.74, 0.48, 0.29);
    let dark_band = Vec3::new(0.42, 0.30, 0.24);
    let storm_band = Vec3::new(0.94, 0.88, 0.72);
    let procedural = light_band
        .lerp(warm_band, band * contrast)
        .lerp(dark_band, (1.0 - band) * contrast * 0.42)
        .lerp(storm_band, storm * 0.24);

    sample.material.albedo = sample
        .material
        .albedo
        .lerp(procedural, 0.86)
        .clamp(0.0, 2.0);
    sample.material.roughness = lerp_f32(
        sample.material.roughness,
        0.68 + turbulence.abs() * 0.16,
        0.80,
    )
    .max(0.02)
    .min(1.0);
    sample.material.metallic = sample.material.metallic.min(0.08);
    sample.material.transmission *= 0.25;
    sample.specular_strength = 0.06 + storm * 0.08;
    sample.normal =
        perturb_surface_normal(normal, outward, controls.seed ^ 0x6A55_BA11_5EED, 0.020);
    sample
}

fn apply_cloud_layer(
    mut sample: ProceduralSurfaceSample,
    outward: Vec3,
    controls: TraceSurfaceControls,
) -> ProceduralSurfaceSample {
    let coverage = saturate(controls.cloud_coverage);
    let opacity = saturate(controls.cloud_opacity);
    if coverage <= 0.0 || opacity <= 0.0 {
        return sample;
    }

    let cloud_outward = advected_surface_vector(
        outward,
        controls.time_days,
        controls.seed ^ 0xC10D_C10D_5EED,
        0.0045,
        0.0012,
    );
    let latitude_stretch = Vec3::new(
        cloud_outward.x * 1.35,
        cloud_outward.y * 0.48,
        cloud_outward.z * 1.35,
    );
    let cellular = fbm_noise(latitude_stretch, controls.seed ^ 0xC10D_C10D_5EED, 7.0, 5);
    let streaks = fbm_noise(latitude_stretch, controls.seed ^ 0x57EA_C10D_5EED, 18.0, 3);
    let anvils = fbm_noise(
        Vec3::new(
            cloud_outward.z * 1.18 + cloud_outward.x * 0.22,
            cloud_outward.y * 0.40,
            cloud_outward.x * 1.18 - cloud_outward.z * 0.22,
        ),
        controls.seed ^ 0xA11C_10D5_5EED,
        4.4,
        4,
    );
    let wisps = fbm_noise(
        Vec3::new(
            cloud_outward.x * 1.82 - cloud_outward.z * 0.18,
            cloud_outward.y * 0.30 + cloud_outward.z * 0.08,
            cloud_outward.z * 1.82 + cloud_outward.x * 0.18,
        ),
        controls.seed ^ 0xC10D_5157_5EED,
        34.0,
        2,
    );
    let cloud_noise = saturate(cellular * 0.56 + streaks * 0.22 + anvils * 0.16 + wisps * 0.06);
    let threshold = 1.0 - coverage * 0.86;
    let veil = smoothstep(threshold - 0.10, threshold + 0.18, cloud_noise);
    let core = smoothstep(threshold + 0.04, threshold + 0.26, cloud_noise);
    let depth_noise = saturate(ridge_value(wisps) * 0.45 + anvils * 0.55);
    let mask = saturate((veil * 0.42 + core * 0.58) * opacity);
    if mask <= 0.0 {
        return sample;
    }
    let depth = saturate((core * 0.74 + veil * 0.26) * (0.58 + depth_noise * 0.42) * opacity);

    let ocean_underlay = smoothstep(0.08, 0.88, sample.water_depth);
    let undercloud_current = fbm_noise(
        Vec3::new(
            cloud_outward.z * 1.52 - cloud_outward.x * 0.20,
            cloud_outward.y * 0.26 + cloud_outward.z * 0.08,
            cloud_outward.x * 1.52 + cloud_outward.z * 0.20,
        ),
        controls.seed ^ 0xC10D_0CEA_5EED,
        16.0,
        3,
    );
    let cloud_color =
        Vec3::new(0.90, 0.93, 0.97).lerp(controls.atmosphere_color.clamp(0.0, 2.0), 0.12);
    let cloud_base = cloud_color.lerp(
        Vec3::new(0.55, 0.66, 0.80),
        depth * (0.15 + ocean_underlay * 0.24 + undercloud_current * ocean_underlay * 0.08),
    );
    sample.material.albedo = sample
        .material
        .albedo
        .lerp(cloud_base, (mask * 0.82 + depth * 0.10).min(0.94));
    sample.material.roughness =
        lerp_f32(sample.material.roughness, 0.88, mask * 0.36 + depth * 0.18).min(1.0);
    let water_shadow = ocean_underlay * mask * depth * (0.82 + undercloud_current * 0.34);
    sample.material.transmission *= 1.0 - mask * (0.42 + ocean_underlay * 0.24);
    sample.material.opacity = sample.material.opacity.max(mask * 0.32).min(1.0);
    sample.specular_strength = lerp_f32(sample.specular_strength, 0.035, mask * 0.46);
    sample.ambient_occlusion = (sample.ambient_occlusion * (1.0 - water_shadow * 0.16)).max(0.50);
    sample.contact_shadow = (sample.contact_shadow * (1.0 - water_shadow * 0.24)).max(0.54);
    sample.cloud_mask = mask;
    sample.cloud_depth = sample
        .cloud_depth
        .max(saturate(depth * (1.0 + ocean_underlay * 0.14)));
    sample
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SurfaceUv {
    u: f32,
    v: f32,
    latitude_abs: f32,
}

fn surface_uv(normal: Vec3) -> SurfaceUv {
    let normal = normal.normalize_or(Vec3::Y);
    let longitude = normal.z.atan2(normal.x);
    let latitude = normal.y.max(-1.0).min(1.0).asin();
    SurfaceUv {
        u: (longitude / (std::f32::consts::PI * 2.0) + 0.5).rem_euclid(1.0),
        v: 0.5 - latitude / std::f32::consts::PI,
        latitude_abs: normal.y.abs(),
    }
}

fn advected_surface_vector(
    outward: Vec3,
    time_days: f32,
    seed: u64,
    zonal_rate: f32,
    meridional_rate: f32,
) -> Vec3 {
    let time_days = finite_or(time_days, 0.0)
        .max(-10_000_000.0)
        .min(10_000_000.0);
    if time_days.abs() <= f32::EPSILON {
        return outward;
    }

    let uv = surface_uv(outward);
    let lat_signed = (0.5 - uv.v) * 2.0;
    let coriolis_turn = lat_signed.signum() * uv.latitude_abs * time_days * zonal_rate * 0.38;
    let jitter = hash_unit_f32(seed ^ 0x71A1_71A1_5EED) - 0.5;
    let u = (uv.u
        + time_days * zonal_rate * (0.58 + uv.latitude_abs * 0.42)
        + coriolis_turn
        + jitter * 0.011)
        .rem_euclid(1.0);
    let seasonal = (time_days / 365.2422) * std::f32::consts::PI * 2.0;
    let v = (uv.v
        + (seasonal + uv.u * std::f32::consts::PI * 2.0).sin()
            * meridional_rate
            * time_days.abs().min(365.2422).sqrt()
        + jitter * 0.004)
        .max(0.0)
        .min(1.0);

    surface_vector_from_uv(u, v)
}

fn surface_vector_from_uv(u: f32, v: f32) -> Vec3 {
    let longitude = (u.rem_euclid(1.0) - 0.5) * std::f32::consts::PI * 2.0;
    let latitude = (0.5 - v.max(0.0).min(1.0)) * std::f32::consts::PI;
    let cos_lat = latitude.cos();
    Vec3::new(
        cos_lat * longitude.cos(),
        latitude.sin(),
        cos_lat * longitude.sin(),
    )
    .normalize_or(Vec3::Y)
}

fn perturb_surface_normal(normal: Vec3, outward: Vec3, seed: u64, amount: f32) -> Vec3 {
    if amount <= 0.0 {
        return normal;
    }

    let (tangent, bitangent) = surface_tangent_basis(outward);
    let dx = fbm_noise(outward, seed ^ 0x517E_0001_5EED, 17.0, 3) - 0.5;
    let dy = fbm_noise(outward, seed ^ 0x517E_0002_5EED, 19.5, 3) - 0.5;
    let perturbed =
        (outward + tangent * (dx * amount) + bitangent * (dy * amount)).normalize_or(outward);
    orient_to_hit(perturbed, normal)
}

fn surface_tangent_basis(outward: Vec3) -> (Vec3, Vec3) {
    let helper = if outward.y.abs() > 0.86 {
        Vec3::X
    } else {
        Vec3::Y
    };
    let tangent = helper.cross(outward).normalize_or(Vec3::X);
    let bitangent = outward.cross(tangent).normalize_or(Vec3::Z);
    (tangent, bitangent)
}

fn orient_to_hit(normal: Vec3, hit_normal: Vec3) -> Vec3 {
    if normal.dot(hit_normal) < 0.0 {
        normal * -1.0
    } else {
        normal
    }
}

fn fbm_noise(point: Vec3, seed: u64, scale: f32, octaves: u32) -> f32 {
    let mut amplitude = 0.5;
    let mut frequency = scale.max(0.001);
    let mut sum = 0.0;
    let mut weight = 0.0;
    for octave in 0..octaves.max(1) {
        let sample_point = Vec3::new(
            point.x * frequency + octave as f32 * 11.17,
            point.y * frequency - octave as f32 * 7.31,
            point.z * frequency + octave as f32 * 5.43,
        );
        sum += value_noise3(
            sample_point,
            seed.wrapping_add(u64::from(octave) * 0x9E37_79B9),
        ) * amplitude;
        weight += amplitude;
        amplitude *= 0.5;
        frequency *= 2.03;
    }

    if weight <= f32::EPSILON {
        0.0
    } else {
        saturate(sum / weight)
    }
}

fn value_noise3(point: Vec3, seed: u64) -> f32 {
    let x0 = point.x.floor() as i32;
    let y0 = point.y.floor() as i32;
    let z0 = point.z.floor() as i32;
    let tx = smooth_noise_weight(point.x - x0 as f32);
    let ty = smooth_noise_weight(point.y - y0 as f32);
    let tz = smooth_noise_weight(point.z - z0 as f32);

    let c000 = hash_grid_unit(seed, x0, y0, z0);
    let c100 = hash_grid_unit(seed, x0 + 1, y0, z0);
    let c010 = hash_grid_unit(seed, x0, y0 + 1, z0);
    let c110 = hash_grid_unit(seed, x0 + 1, y0 + 1, z0);
    let c001 = hash_grid_unit(seed, x0, y0, z0 + 1);
    let c101 = hash_grid_unit(seed, x0 + 1, y0, z0 + 1);
    let c011 = hash_grid_unit(seed, x0, y0 + 1, z0 + 1);
    let c111 = hash_grid_unit(seed, x0 + 1, y0 + 1, z0 + 1);

    let x00 = lerp_f32(c000, c100, tx);
    let x10 = lerp_f32(c010, c110, tx);
    let x01 = lerp_f32(c001, c101, tx);
    let x11 = lerp_f32(c011, c111, tx);
    let y0 = lerp_f32(x00, x10, ty);
    let y1 = lerp_f32(x01, x11, ty);
    lerp_f32(y0, y1, tz)
}

fn hash_grid_unit(seed: u64, x: i32, y: i32, z: i32) -> f32 {
    let mixed = seed
        ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ (z as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    hash_unit_f32(mixed)
}

fn hash_unit_f32(mut value: u64) -> f32 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    ((value >> 40) as f32) / ((1_u64 << 24) as f32)
}

fn dielectric_f0(index_of_refraction: f32) -> f32 {
    let ior = finite_or(index_of_refraction, 1.5).max(1.0);
    let ratio = (ior - 1.0) / (ior + 1.0);
    ratio * ratio
}

fn schlick_fresnel(cosine: f32, f0: f32) -> f32 {
    let cosine = saturate(cosine);
    let f0 = saturate(f0);
    f0 + (1.0 - f0) * (1.0 - cosine).powf(5.0)
}

fn ridge_value(value: f32) -> f32 {
    1.0 - (saturate(value) * 2.0 - 1.0).abs()
}

fn smooth_noise_weight(value: f32) -> f32 {
    let value = saturate(value);
    value * value * (3.0 - 2.0 * value)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value >= edge1 { 1.0 } else { 0.0 };
    }

    let t = saturate((value - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

fn lerp_f32(left: f32, right: f32, t: f32) -> f32 {
    left + (right - left) * saturate(t)
}

fn water_transmittance(path_depth: f32) -> Vec3 {
    let depth = path_depth.max(0.0).min(8.0);
    Vec3::new(
        (-depth * 1.75).exp(),
        (-depth * 0.52).exp(),
        (-depth * 0.16).exp(),
    )
    .clamp(0.0, 1.0)
}

fn atmosphere_extinction(optical_depth: f32) -> Vec3 {
    let depth = optical_depth.max(0.0).min(4.0);
    Vec3::new(
        (-depth * 0.42).exp(),
        (-depth * 0.24).exp(),
        (-depth * 0.10).exp(),
    )
    .clamp(0.0, 1.0)
}

fn atmosphere_refracted_direction(ray: Ray, center: Vec3, atmosphere: AtmosphereSample) -> Vec3 {
    let direction = ray.direction.normalize_or(Vec3::Z);
    if atmosphere.refraction_bend <= 0.0 {
        return direction;
    }

    let to_center = center - ray.origin;
    let closest_t = to_center.dot(direction).max(0.0);
    let closest = ray.origin + direction * closest_t;
    let toward_center = (center - closest).normalize_or(Vec3::ZERO);
    let lateral =
        (toward_center - direction * toward_center.dot(direction)).normalize_or(Vec3::ZERO);
    if lateral.length_squared() <= f32::EPSILON {
        direction
    } else {
        (direction + lateral * atmosphere.refraction_bend).normalize_or(direction)
    }
}

fn ray_impact_radius(ray: Ray, center: Vec3) -> f32 {
    let direction = ray.direction.normalize_or(Vec3::Z);
    let to_center = center - ray.origin;
    let closest_t = to_center.dot(direction).max(0.0);
    (ray.origin + direction * closest_t - center).length()
}

fn procedural_contact_shadow(
    outward: Vec3,
    normal: Vec3,
    light_dir: Vec3,
    seed: u64,
    water_depth: f32,
    cloud_density: f32,
) -> f32 {
    let outward = outward.normalize_or(Vec3::Y);
    let normal = normal.normalize_or(outward);
    let light_dir = light_dir.normalize_or(Vec3::Y);
    let grazing = smoothstep(0.18, 0.82, 1.0 - outward.dot(light_dir).max(0.0));
    let normal_fold = smoothstep(0.010, 0.115, (normal - outward).length());
    let light_probe = (outward + light_dir * 0.09).normalize_or(outward);
    let lee_probe = (outward - light_dir * 0.13).normalize_or(outward);
    let ridge = ridge_value(fbm_noise(light_probe, seed ^ 0xA0C0_5EED_1111, 24.0, 3));
    let lee = fbm_noise(lee_probe, seed ^ 0xA0C0_5EED_2222, 38.0, 2);
    let directional = smoothstep(0.46, 0.96, ridge * 0.42 + lee * 0.24 + normal_fold * 0.34);
    let water_contact = water_depth
        * (0.045 + ridge_value(fbm_noise(outward, seed ^ 0xA0C0_5EED_3333, 58.0, 2)) * 0.055);
    let cloud_contact = cloud_density * (0.035 + grazing * 0.045);
    let occlusion = directional * (0.055 + grazing * 0.120 + normal_fold * 0.075)
        + water_contact
        + cloud_contact;

    (1.0 - occlusion).max(0.56).min(1.0)
}

fn apply_trace_dither(
    color: Vec3,
    pixel_x: u32,
    pixel_y: u32,
    settings: PathTraceSettings,
) -> Vec3 {
    if !color.is_finite() {
        return color;
    }

    let base_seed = if settings.seeded_jitter {
        settings.jitter_seed
    } else {
        0
    };
    let seed = base_seed
        .wrapping_add((pixel_x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add((pixel_y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9))
        .wrapping_add((settings.samples_per_pixel as u64).wrapping_mul(0x94D0_49BB_1331_11EB))
        .wrapping_add((settings.max_bounces as u64).wrapping_mul(0xD6E8_FD9A_5EED_1208));
    let sample_scale = (settings.samples_per_pixel.max(1) as f32).sqrt();
    let amplitude = 1.0 / (384.0 * sample_scale);
    let dither = Vec3::new(
        triangular_dither(seed ^ 0xD17E_0001),
        triangular_dither(seed ^ 0xD17E_0002),
        triangular_dither(seed ^ 0xD17E_0003),
    );

    (color + dither * amplitude).clamp(0.0, 16.0)
}

fn triangular_dither(seed: u64) -> f32 {
    hash_unit_f32(seed) + hash_unit_f32(seed ^ 0x517C_C1B1_D17E_5EED) - 1.0
}

fn saturate(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0).min(1.0)
    } else {
        0.0
    }
}

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TraceStats {
    pub tiles_completed: u64,
    pub samples_completed: u64,
    pub rays_traced: u64,
    pub primary_rays: u64,
    pub shadow_rays: u64,
    pub reflection_rays: u64,
    pub refraction_rays: u64,
    pub atmosphere_samples: u64,
    pub ambient_occlusion_samples: u64,
    pub cloud_depth_samples: u64,
    pub max_bounce_depth: u32,
    pub elapsed_millis: u64,
}

impl TraceStats {
    pub fn record_tile_completed(&mut self) {
        self.tiles_completed = self.tiles_completed.saturating_add(1);
    }

    pub fn record_samples_completed(&mut self, count: u64) {
        self.samples_completed = self.samples_completed.saturating_add(count);
    }

    pub fn record_primary_rays(&mut self, count: u64) {
        self.primary_rays = self.primary_rays.saturating_add(count);
        self.rays_traced = self.rays_traced.saturating_add(count);
    }

    pub fn record_shadow_rays(&mut self, count: u64) {
        self.shadow_rays = self.shadow_rays.saturating_add(count);
        self.rays_traced = self.rays_traced.saturating_add(count);
    }

    pub fn record_reflection_rays(&mut self, count: u64) {
        self.reflection_rays = self.reflection_rays.saturating_add(count);
        self.rays_traced = self.rays_traced.saturating_add(count);
    }

    pub fn record_refraction_rays(&mut self, count: u64) {
        self.refraction_rays = self.refraction_rays.saturating_add(count);
        self.rays_traced = self.rays_traced.saturating_add(count);
    }

    pub fn record_atmosphere_samples(&mut self, count: u64) {
        self.atmosphere_samples = self.atmosphere_samples.saturating_add(count);
    }

    pub fn record_ambient_occlusion_samples(&mut self, count: u64) {
        self.ambient_occlusion_samples = self.ambient_occlusion_samples.saturating_add(count);
    }

    pub fn record_cloud_depth_samples(&mut self, count: u64) {
        self.cloud_depth_samples = self.cloud_depth_samples.saturating_add(count);
    }

    pub fn record_bounce_depth(&mut self, depth: u32) {
        self.max_bounce_depth = self.max_bounce_depth.max(depth);
    }

    pub fn record_elapsed_millis(&mut self, elapsed_millis: u64) {
        self.elapsed_millis = self.elapsed_millis.saturating_add(elapsed_millis);
    }

    pub fn merge(&mut self, rhs: Self) {
        self.tiles_completed = self.tiles_completed.saturating_add(rhs.tiles_completed);
        self.samples_completed = self.samples_completed.saturating_add(rhs.samples_completed);
        self.rays_traced = self.rays_traced.saturating_add(rhs.rays_traced);
        self.primary_rays = self.primary_rays.saturating_add(rhs.primary_rays);
        self.shadow_rays = self.shadow_rays.saturating_add(rhs.shadow_rays);
        self.reflection_rays = self.reflection_rays.saturating_add(rhs.reflection_rays);
        self.refraction_rays = self.refraction_rays.saturating_add(rhs.refraction_rays);
        self.atmosphere_samples = self
            .atmosphere_samples
            .saturating_add(rhs.atmosphere_samples);
        self.ambient_occlusion_samples = self
            .ambient_occlusion_samples
            .saturating_add(rhs.ambient_occlusion_samples);
        self.cloud_depth_samples = self
            .cloud_depth_samples
            .saturating_add(rhs.cloud_depth_samples);
        self.max_bounce_depth = self.max_bounce_depth.max(rhs.max_bounce_depth);
        self.elapsed_millis = self.elapsed_millis.saturating_add(rhs.elapsed_millis);
    }
}

fn checked_pixel_len(width: u32, height: u32) -> Option<usize> {
    let len = (width as usize).checked_mul(height as usize)?;
    (len <= isize::MAX as usize).then_some(len)
}

fn intersect_ray_sphere_interval(ray: Ray, sphere: Sphere) -> Option<(f32, f32)> {
    if !sphere.is_valid() || !ray.has_valid_bounds() {
        return None;
    }

    let oc = ray.origin - sphere.center;
    let a = ray.direction.length_squared();
    if a <= f32::EPSILON || !a.is_finite() {
        return None;
    }
    let half_b = oc.dot(ray.direction);
    let c = oc.length_squared() - sphere.radius * sphere.radius;
    let discriminant = half_b * half_b - a * c;
    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let near_t = (-half_b - sqrt_discriminant) / a;
    let far_t = (-half_b + sqrt_discriminant) / a;
    if far_t < ray.t_min || near_t > ray.t_max {
        return None;
    }

    Some((near_t, far_t))
}

fn sample_environment(direction: Vec3, scene: &TraceScene) -> Vec3 {
    let direction = direction.normalize_or(Vec3::new(0.0, 0.0, 1.0));
    let light = scene.distant_solar_light();
    let horizon = (1.0 - direction.y.abs()).max(0.0).min(1.0);
    let horizon_mix = horizon * (0.40 + light.diffusion * 0.45);

    scene
        .sky_color
        .clamp(0.0, 4.0)
        .lerp(scene.horizon_color.clamp(0.0, 4.0), horizon_mix)
        + light.color * (light.environment_lobe(direction) * 2.4 * light.intensity)
}

#[derive(Debug, Clone)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_unit_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1_u64 << 24) as f32)
    }
}
