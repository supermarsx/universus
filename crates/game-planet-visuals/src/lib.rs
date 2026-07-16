#![forbid(unsafe_code)]

use image::RgbaImage;
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::{f32::consts::PI, sync::mpsc, thread};

pub mod backend;
pub mod catalog;
pub mod evolution;
pub mod geology;
pub mod gpu;
pub mod modifiers;
pub mod pathtrace;
pub mod profile;

pub use backend::{
    BackendCapabilities, ConfiguredRenderBackend, CpuBackend, RenderBackend,
    RenderBackendConfiguration, RenderBackendConfigurationReport, RenderBackendPreference,
    RenderOutputKind, RenderQuality, RenderRequest,
};
pub use gpu::{
    CudaBackend, GpuBackend, GpuBackendEnvironment, GpuBackendFamily, GpuBackendKind,
    GpuBackendReadiness, GpuBackendReport, GpuBackendStatus, GpuRenderError, RocmBackend,
    WgpuBackend,
};
pub use profile::{
    generate_planet_profile, known_planet_archetype_keys, GeneratedPlanetProfile, ProfileSeedInput,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistantLight {
    pub direction: [f32; 3],
    pub angular_radius_rad: f32,
    pub intensity: f32,
    pub color_temperature_k: f32,
    pub color_linear: [f32; 3],
    pub diffusion_scale: f32,
}

impl DistantLight {
    pub fn solar_default() -> Self {
        Self::new([-0.48, 0.66, 0.58], 0.004_650_47, 1.0, 5_780.0, 1.0)
    }

    pub fn night_fill() -> Self {
        Self::new([0.26, 0.54, 0.80], 0.004_650_47, 0.18, 7_100.0, 1.18)
    }

    pub fn new(
        direction: [f32; 3],
        angular_radius_rad: f32,
        intensity: f32,
        color_temperature_k: f32,
        diffusion_scale: f32,
    ) -> Self {
        let direction = normalized_direction_array(direction);
        Self {
            direction,
            angular_radius_rad: angular_radius_rad.clamp(0.000_1, 0.12),
            intensity: intensity.max(0.0),
            color_temperature_k: color_temperature_k.clamp(1_000.0, 40_000.0),
            color_linear: color_temperature_to_linear_rgb(color_temperature_k),
            diffusion_scale: diffusion_scale.clamp(0.05, 8.0),
        }
    }

    pub fn sun_disk_cosine_threshold(self) -> f32 {
        self.angular_radius_rad.cos()
    }

    pub fn projected_overview_screen(self, aspect: f32) -> [f32; 2] {
        let aspect = aspect.max(0.1);
        [
            (0.5 + self.direction[0] * 0.333 / aspect.sqrt()).clamp(0.06, 0.94),
            (0.5 - self.direction[1] * 0.43).clamp(0.055, 0.86),
        ]
    }

    pub fn medium_diffusion(self, distance: f32, density: f32, roughness: f32) -> f32 {
        let d = distance.max(0.0);
        let spread =
            0.34 + density.max(0.0) * 0.42 * self.diffusion_scale + roughness.max(0.0) * 0.30;
        1.0 / (1.0 + d * d * spread)
    }
}

fn normalized_direction_array(direction: [f32; 3]) -> [f32; 3] {
    let len =
        (direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2])
            .sqrt();
    if len <= f32::EPSILON {
        [0.0, 1.0, 0.0]
    } else {
        [direction[0] / len, direction[1] / len, direction[2] / len]
    }
}

fn color_temperature_to_linear_rgb(kelvin: f32) -> [f32; 3] {
    let temperature = (kelvin.clamp(1_000.0, 40_000.0) / 100.0).max(1.0);
    let red = if temperature <= 66.0 {
        1.0
    } else {
        (1.292_936_2 * (temperature - 60.0).powf(-0.133_204_76)).clamp(0.0, 1.0)
    };
    let green = if temperature <= 66.0 {
        (0.390_081_58 * temperature.ln() - 0.631_841_4).clamp(0.0, 1.0)
    } else {
        (1.129_890_9 * (temperature - 60.0).powf(-0.075_514_846)).clamp(0.0, 1.0)
    };
    let blue = if temperature >= 66.0 {
        1.0
    } else if temperature <= 19.0 {
        0.0
    } else {
        (0.543_206_8 * (temperature - 10.0).ln() - 1.196_254_1).clamp(0.0, 1.0)
    };
    [red, green, blue]
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetVisualProfile {
    pub seed: u64,
    #[serde(default)]
    pub snapshot_time_days: f32,
    pub algorithm: String,
    #[serde(default)]
    pub catalog_version: String,
    #[serde(default)]
    pub archetype_key: String,
    #[serde(default)]
    pub class_key: String,
    #[serde(default)]
    pub size_class: String,
    #[serde(default)]
    pub size_key: String,
    #[serde(default)]
    pub scale_band: String,
    #[serde(default)]
    pub scale_key: String,
    #[serde(default)]
    pub radius_scale: f32,
    pub planet_class: String,
    pub radius_km: i32,
    #[serde(default)]
    pub radius_earth: f32,
    #[serde(default)]
    pub density_earth: f32,
    #[serde(default)]
    pub gravity_g: f32,
    pub temperature_c: i32,
    pub ocean_fraction: f32,
    pub ice_fraction: f32,
    pub cloud_density: f32,
    pub atmosphere_density: f32,
    pub volcanic_activity: f32,
    pub ringed: bool,
    #[serde(default)]
    pub modifier_keys: Vec<String>,
    pub palette: String,
    pub render_model: String,
}

impl PlanetVisualProfile {
    pub fn from_seed(seed: u64) -> Self {
        GeneratedPlanetProfile::from_seed(seed).into()
    }

    pub fn from_seed_input(input: impl Into<ProfileSeedInput>) -> Self {
        generate_planet_profile(input).into()
    }

    pub fn size_class(&self) -> &str {
        if self.size_class.is_empty() {
            planet_size_class_for_radius_km(self.radius_km)
        } else {
            self.size_class.as_str()
        }
    }

    pub fn scale_key(&self) -> &str {
        if self.scale_key.is_empty() {
            planet_scale_key_for_radius_km(self.radius_km)
        } else {
            self.scale_key.as_str()
        }
    }

    pub fn scale_band(&self) -> &str {
        if self.scale_band.is_empty() {
            self.scale_key()
        } else {
            self.scale_band.as_str()
        }
    }

    pub fn radius_scale(&self) -> f32 {
        if self.radius_scale > 0.0 {
            self.radius_scale
        } else {
            1.0
        }
    }

    pub fn with_snapshot_time_days(mut self, days: f32) -> Self {
        self.snapshot_time_days = sanitize_snapshot_time_days(days);
        self
    }

    pub fn set_snapshot_time_days(&mut self, days: f32) {
        self.snapshot_time_days = sanitize_snapshot_time_days(days);
    }
}

fn sanitize_snapshot_time_days(days: f32) -> f32 {
    if days.is_finite() {
        days.clamp(-10_000_000.0, 10_000_000.0)
    } else {
        0.0
    }
}

impl From<GeneratedPlanetProfile> for PlanetVisualProfile {
    fn from(generated: GeneratedPlanetProfile) -> Self {
        let planet_class = generated.legacy_planet_class_label();
        let size_key = generated.size_key.clone();
        let scale_key = generated.scale_key.clone();
        let modifier_keys = generated
            .modifiers
            .iter()
            .map(|modifier| modifier.key.clone())
            .collect();
        Self {
            seed: generated.seed,
            snapshot_time_days: 0.0,
            algorithm: generated.algorithm,
            catalog_version: generated.catalog_version,
            archetype_key: generated.archetype_key,
            class_key: generated.class_key,
            size_class: size_key.clone(),
            size_key,
            scale_band: scale_key.clone(),
            scale_key,
            radius_scale: generated.radius_scale,
            planet_class,
            radius_km: generated.radius_km,
            radius_earth: generated.radius_earth,
            density_earth: generated.density_earth,
            gravity_g: generated.gravity_g,
            temperature_c: generated.temperature_c,
            ocean_fraction: generated.hydrosphere.ocean_fraction,
            ice_fraction: generated.hydrosphere.ice_fraction,
            cloud_density: generated.atmosphere.cloud_density,
            atmosphere_density: generated.atmosphere.density,
            volcanic_activity: generated.surface.volcanic_activity,
            ringed: generated.rings.present,
            modifier_keys,
            palette: generated.palette_key,
            render_model: generated.render_model_key,
        }
    }
}

impl Serialize for PlanetVisualProfile {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("PlanetVisualProfile", 26)?;
        state.serialize_field("seed", &self.seed)?;
        state.serialize_field("snapshotTimeDays", &self.snapshot_time_days)?;
        state.serialize_field("algorithm", &self.algorithm)?;
        state.serialize_field("catalogVersion", &self.catalog_version)?;
        state.serialize_field("archetypeKey", &self.archetype_key)?;
        state.serialize_field("classKey", &self.class_key)?;
        state.serialize_field("planetClass", &self.planet_class)?;
        state.serialize_field("radiusKm", &self.radius_km)?;
        state.serialize_field("radiusEarth", &self.radius_earth)?;
        state.serialize_field("densityEarth", &self.density_earth)?;
        state.serialize_field("gravityG", &self.gravity_g)?;
        state.serialize_field("sizeClass", self.size_class())?;
        state.serialize_field("sizeKey", &self.size_key)?;
        state.serialize_field("scaleBand", self.scale_band())?;
        state.serialize_field("scaleKey", self.scale_key())?;
        state.serialize_field("radiusScale", &self.radius_scale())?;
        state.serialize_field("temperatureC", &self.temperature_c)?;
        state.serialize_field("oceanFraction", &self.ocean_fraction)?;
        state.serialize_field("iceFraction", &self.ice_fraction)?;
        state.serialize_field("cloudDensity", &self.cloud_density)?;
        state.serialize_field("atmosphereDensity", &self.atmosphere_density)?;
        state.serialize_field("volcanicActivity", &self.volcanic_activity)?;
        state.serialize_field("ringed", &self.ringed)?;
        state.serialize_field("modifierKeys", &self.modifier_keys)?;
        state.serialize_field("palette", &self.palette)?;
        state.serialize_field("renderModel", &self.render_model)?;
        state.end()
    }
}

pub const fn planet_size_class_for_radius_km(radius_km: i32) -> &'static str {
    if radius_km < 4_500 {
        "small"
    } else if radius_km < 13_000 {
        "medium"
    } else {
        "large"
    }
}

pub const fn planet_scale_key_for_radius_km(radius_km: i32) -> &'static str {
    if radius_km < 500 {
        "tiny"
    } else if radius_km < 1_500 {
        "dwarf"
    } else if radius_km < 4_500 {
        "small"
    } else if radius_km < 7_500 {
        "medium"
    } else if radius_km < 10_000 {
        "large"
    } else if radius_km < 13_000 {
        "super-earth"
    } else if radius_km < 24_000 {
        "sub-neptune"
    } else if radius_km < 35_000 {
        "gas-dwarf"
    } else if radius_km < 100_000 {
        "giant"
    } else {
        "super-jovian"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsVector2 {
    pub x: f32,
    pub y: f32,
}

impl PhysicsVector2 {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            Self::ZERO
        } else {
            Self::new(self.x / len, self.y / len)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetPhysicsSample {
    pub ocean_current_mps: PhysicsVector2,
    pub cloud_flow_mps: PhysicsVector2,
    pub current_speed_mps: f32,
    pub current_shear: f32,
    pub water_density_kg_m3: f32,
    pub atmosphere_density_kg_m3: f32,
    pub surface_pressure_bar: f32,
    pub humidity: f32,
    pub cloud_lift: f32,
    pub coriolis: f32,
    pub magnetic_field_microtesla: f32,
    pub aurora_power: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetPhysicsSummary {
    pub snapshot_time_days: f32,
    pub rotation_period_hours: f32,
    pub angular_velocity_rad_s: f32,
    pub surface_gravity_m_s2: f32,
    pub water_density_kg_m3: f32,
    pub atmosphere_density_kg_m3: f32,
    pub surface_pressure_bar: f32,
    pub current_velocity_scale_mps: f32,
    pub cloud_velocity_scale_mps: f32,
    pub magnetic_field_microtesla: f32,
    pub magnetosphere_strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetPhysicsModel {
    pub seed: u64,
    pub snapshot_time_days: f32,
    pub radius_m: f32,
    pub radius_earth: f32,
    pub density_earth: f32,
    pub surface_gravity_m_s2: f32,
    pub rotation_period_hours: f32,
    pub angular_velocity_rad_s: f32,
    pub ocean_fraction: f32,
    pub water_density_kg_m3: f32,
    pub atmosphere_density_kg_m3: f32,
    pub surface_pressure_bar: f32,
    pub current_velocity_scale_mps: f32,
    pub cloud_velocity_scale_mps: f32,
    pub thermal_contrast: f32,
    pub coriolis_strength: f32,
    pub tidal_mix: f32,
    pub magnetic_field_microtesla: f32,
    pub magnetosphere_strength: f32,
}

impl PlanetPhysicsModel {
    pub fn from_profile(profile: &PlanetVisualProfile) -> Self {
        let radius_earth = if profile.radius_earth > 0.0 {
            profile.radius_earth
        } else {
            profile.radius_km.max(1) as f32 / 6_371.0
        };
        let density_earth = if profile.density_earth > 0.0 {
            profile.density_earth
        } else {
            clamp(
                0.62 + profile.gravity_g * 0.38 / radius_earth.max(0.20),
                0.35,
                2.2,
            )
        };
        let gravity_g = if profile.gravity_g > 0.0 {
            profile.gravity_g
        } else {
            clamp(density_earth * radius_earth, 0.03, 4.5)
        };
        let spin_jitter = hash2(311, 907, profile.seed);
        let ocean_world = is_ocean_world_profile(profile);
        let gas_giant = is_banded_gas_giant_profile(profile);
        let hot_world = smoothstep(22.0, 180.0, profile.temperature_c as f32);
        let icy_world =
            smoothstep(5.0, -80.0, profile.temperature_c as f32) + profile.ice_fraction * 0.35;
        let rotation_period_hours = if gas_giant {
            clamp(
                8.0 + spin_jitter * 11.0 + radius_earth.sqrt() * 0.9,
                7.0,
                22.0,
            )
        } else {
            clamp(
                12.0 + spin_jitter * 34.0 + radius_earth.max(0.5) * 2.2 - gravity_g * 2.0,
                7.5,
                96.0,
            )
        };
        let angular_velocity_rad_s = PI * 2.0 / (rotation_period_hours * 3_600.0);
        let salinity = clamp01(
            0.48 + profile.ocean_fraction * 0.22
                + profile.volcanic_activity * 0.13
                + hot_world * 0.12
                - profile.ice_fraction * 0.10,
        );
        let water_density_kg_m3 = clamp(
            990.0 + salinity * 48.0 + icy_world * 18.0 - hot_world * 22.0 + gravity_g * 8.0,
            930.0,
            1_180.0,
        );
        let atmosphere_density_kg_m3 = clamp(
            1.225
                * profile.atmosphere_density.max(0.02)
                * (0.62 + gravity_g * 0.34)
                * (1.0 - hot_world * 0.20 + icy_world * 0.10),
            0.004,
            8.5,
        );
        let surface_pressure_bar = clamp(
            profile.atmosphere_density * (0.58 + gravity_g * 0.42) + hot_world * 0.20,
            0.002,
            16.0,
        );
        let current_velocity_scale_mps = clamp(
            0.18 + profile.ocean_fraction * 1.25
                + profile.atmosphere_density * 0.42
                + (24.0 / rotation_period_hours).sqrt() * 0.62
                + profile.volcanic_activity * 0.18
                - profile.ice_fraction * 0.38,
            0.03,
            if ocean_world { 4.4 } else { 2.8 },
        );
        let cloud_velocity_scale_mps = clamp(
            6.0 + profile.atmosphere_density * 28.0
                + hot_world * 16.0
                + (24.0 / rotation_period_hours) * 10.0
                + profile.cloud_density * 12.0,
            0.5,
            96.0,
        );
        let thermal_contrast = clamp01(
            (profile.temperature_c as f32).abs() / 160.0
                + profile.atmosphere_density * 0.18
                + profile.ice_fraction * 0.22
                + hot_world * 0.20,
        );
        let coriolis_strength = clamp(
            (24.0 / rotation_period_hours)
                * (0.72 + gravity_g.sqrt() * 0.28)
                * (0.70 + profile.atmosphere_density * 0.18),
            0.06,
            4.8,
        );
        let tidal_mix = clamp01(
            0.16 + profile.ocean_fraction * 0.32
                + profile.ringed as u8 as f32 * 0.18
                + profile.volcanic_activity * 0.14
                + hash2(719, 191, profile.seed) * 0.18,
        );
        let iron_core = if profile_key_contains_any(profile, &["iron", "metal", "mercury"]) {
            0.28
        } else {
            0.0
        };
        let dynamo = clamp01(
            density_earth * 0.34
                + gravity_g * 0.22
                + (24.0 / rotation_period_hours) * 0.26
                + profile.volcanic_activity * 0.10
                + iron_core
                - hot_world * 0.12,
        );
        let magnetic_field_microtesla = clamp(
            4.0 + dynamo * 72.0 + hash2(431, 617, profile.seed) * 8.0,
            1.0,
            120.0,
        );
        let magnetosphere_strength = clamp01(magnetic_field_microtesla / 70.0 + dynamo * 0.18);

        Self {
            seed: profile.seed,
            snapshot_time_days: sanitize_snapshot_time_days(profile.snapshot_time_days),
            radius_m: profile.radius_km.max(1) as f32 * 1_000.0,
            radius_earth,
            density_earth,
            surface_gravity_m_s2: gravity_g * 9.80665,
            rotation_period_hours,
            angular_velocity_rad_s,
            ocean_fraction: profile.ocean_fraction.clamp(0.0, 1.0),
            water_density_kg_m3,
            atmosphere_density_kg_m3,
            surface_pressure_bar,
            current_velocity_scale_mps,
            cloud_velocity_scale_mps,
            thermal_contrast,
            coriolis_strength,
            tidal_mix,
            magnetic_field_microtesla,
            magnetosphere_strength,
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> PlanetPhysicsSample {
        let u = u.rem_euclid(1.0);
        let v = clamp01(v);
        let lat_signed = (v - 0.5) * 2.0;
        let lat = lat_signed.abs();
        let lat_rad = lat_signed * PI * 0.5;
        let coriolis = lat_rad.sin() * self.coriolis_strength;
        let time_days = self.snapshot_time_days;
        let cloud_phase = time_days * (0.0025 + self.cloud_velocity_scale_mps * 0.000_018)
            + self.angular_velocity_rad_s * time_days * 16_500.0;
        let ocean_phase = time_days * (0.0012 + self.current_velocity_scale_mps * 0.000_22);
        let seasonal_phase = (time_days / 365.2422) * PI * 2.0;
        let magnetic_phase = time_days * 0.000_014;
        let atmosphere_u =
            u + cloud_phase + lat_rad.cos() * time_days * 0.000_19 + self.thermal_contrast * 0.013;
        let atmosphere_v = clamp01(v + seasonal_phase.sin() * 0.018 * (1.0 - lat * 0.55));
        let ocean_u = u + ocean_phase + coriolis.signum() * lat * time_days * 0.000_11;
        let ocean_v =
            clamp01(v + (seasonal_phase + lat_rad).sin() * 0.006 + self.tidal_mix * 0.003);
        let pressure = self.pressure_field(atmosphere_u, atmosphere_v);
        let pressure_e = self.pressure_field(atmosphere_u + 0.004, atmosphere_v);
        let pressure_w = self.pressure_field(atmosphere_u - 0.004, atmosphere_v);
        let pressure_n = self.pressure_field(atmosphere_u, atmosphere_v - 0.004);
        let pressure_s = self.pressure_field(atmosphere_u, atmosphere_v + 0.004);
        let dpdu = pressure_e - pressure_w;
        let dpdv = pressure_s - pressure_n;
        let thermal = self.thermal_field(atmosphere_u + seasonal_phase.cos() * 0.018, atmosphere_v);
        let salinity = self.salinity_field(ocean_u, ocean_v);
        let stream = self.stream_field(ocean_u, ocean_v);
        let stream_e = self.stream_field(ocean_u + 0.006, ocean_v);
        let stream_n = self.stream_field(ocean_u, ocean_v - 0.006);
        let shear = clamp01((stream - stream_e).abs() * 2.6 + (stream - stream_n).abs() * 2.2);
        let gyre = PhysicsVector2::new(-dpdv * coriolis.signum(), dpdu * coriolis.signum());
        let trade = PhysicsVector2::new(
            (lat_rad * 2.0).cos() * 0.34 + (stream - 0.5) * 0.30,
            -lat_signed * 0.16 + (thermal - 0.5) * 0.24,
        );
        let eddy = PhysicsVector2::new(
            (stream - 0.5) * 0.74 + (pressure - 0.5) * 0.18,
            (thermal - 0.5) * 0.44 - dpdu * 0.9,
        );
        let current_dir = PhysicsVector2::new(
            gyre.x * 1.8 + trade.x + eddy.x,
            gyre.y * 1.8 + trade.y + eddy.y,
        )
        .normalize_or_zero();
        let current_speed_mps = clamp(
            self.current_velocity_scale_mps
                * (0.28 + shear * 0.52 + stream * 0.18 + self.tidal_mix * 0.20)
                * (1.0 - smoothstep(0.78, 1.0, lat) * 0.34),
            0.0,
            self.current_velocity_scale_mps * 1.45,
        );
        let ocean_current_mps = PhysicsVector2::new(
            current_dir.x * current_speed_mps,
            current_dir.y * current_speed_mps,
        );
        let jet = (lat_rad * 4.0 + pressure * 1.8).sin() * 0.5 + 0.5;
        let cloud_dir = PhysicsVector2::new(
            trade.x * 0.7 + dpdv * 1.2 + (jet - 0.5) * 0.9,
            trade.y * 0.5 - dpdu * 1.2 + (thermal - 0.5) * 0.5,
        )
        .normalize_or_zero();
        let cloud_speed =
            self.cloud_velocity_scale_mps * (0.24 + jet * 0.36 + pressure * 0.16 + shear * 0.16);
        let cloud_flow_mps =
            PhysicsVector2::new(cloud_dir.x * cloud_speed, cloud_dir.y * cloud_speed);
        let water_density_kg_m3 = clamp(
            self.water_density_kg_m3
                + (salinity - 0.5) * 38.0
                + (0.5 - thermal) * 22.0
                + self.surface_gravity_m_s2 * 0.35
                + pressure * 8.0,
            920.0,
            1_220.0,
        );
        let humidity = clamp01(
            self.ocean_fraction * 0.38
                + pressure * 0.20
                + thermal * 0.18
                + self.atmosphere_density_kg_m3 / 8.5 * 0.22
                - lat * 0.12,
        );
        let convergence = clamp01(
            shear * 0.46 + (pressure - 0.5).abs() * 0.22 + dpdu.abs() * 1.8 + dpdv.abs() * 1.8,
        );
        let cloud_lift = clamp01(
            humidity * (0.22 + convergence * 0.52 + thermal * 0.20 + self.tidal_mix * 0.08),
        );
        let atmosphere_density_kg_m3 = clamp(
            self.atmosphere_density_kg_m3 * (0.78 + pressure * 0.36 - lat * 0.08),
            0.001,
            10.0,
        );
        let surface_pressure_bar = clamp(
            self.surface_pressure_bar * (0.80 + pressure * 0.34 + atmosphere_density_kg_m3 * 0.018),
            0.001,
            20.0,
        );
        let anomaly = self.magnetic_anomaly_field(
            u + magnetic_phase + seasonal_phase.cos() * 0.004,
            v + seasonal_phase.sin() * 0.002,
        );
        let magnetic_field_microtesla = clamp(
            self.magnetic_field_microtesla * (0.58 + lat * 0.80 + anomaly * 0.28),
            0.5,
            160.0,
        );
        let aurora_power = clamp01(
            smoothstep(0.54, 0.97, lat)
                * smoothstep(10.0, 85.0, magnetic_field_microtesla)
                * self.magnetosphere_strength
                * (0.52
                    + self.solar_wind_field(
                        atmosphere_u + time_days * 0.009,
                        atmosphere_v + seasonal_phase.cos() * 0.006,
                    ) * 0.48)
                * clamp01(atmosphere_density_kg_m3 / 2.2),
        );

        PlanetPhysicsSample {
            ocean_current_mps,
            cloud_flow_mps,
            current_speed_mps,
            current_shear: shear,
            water_density_kg_m3,
            atmosphere_density_kg_m3,
            surface_pressure_bar,
            humidity,
            cloud_lift,
            coriolis,
            magnetic_field_microtesla,
            aurora_power,
        }
    }

    pub fn summary(self) -> PlanetPhysicsSummary {
        PlanetPhysicsSummary {
            snapshot_time_days: self.snapshot_time_days,
            rotation_period_hours: self.rotation_period_hours,
            angular_velocity_rad_s: self.angular_velocity_rad_s,
            surface_gravity_m_s2: self.surface_gravity_m_s2,
            water_density_kg_m3: self.water_density_kg_m3,
            atmosphere_density_kg_m3: self.atmosphere_density_kg_m3,
            surface_pressure_bar: self.surface_pressure_bar,
            current_velocity_scale_mps: self.current_velocity_scale_mps,
            cloud_velocity_scale_mps: self.cloud_velocity_scale_mps,
            magnetic_field_microtesla: self.magnetic_field_microtesla,
            magnetosphere_strength: self.magnetosphere_strength,
        }
    }

    pub fn at_time_days(mut self, days: f32) -> Self {
        self.snapshot_time_days = sanitize_snapshot_time_days(days);
        self
    }

    pub fn sample_at_time_days(&self, u: f32, v: f32, days: f32) -> PlanetPhysicsSample {
        self.at_time_days(days).sample(u, v)
    }

    fn pressure_field(self, u: f32, v: f32) -> f32 {
        fbm_tiled(
            u * 0.72 + self.thermal_contrast * 0.08,
            v * 0.58 - self.tidal_mix * 0.06,
            601,
            5,
            self.seed + 41_101,
            0.58,
        )
    }

    fn thermal_field(self, u: f32, v: f32) -> f32 {
        let lat = ((v - 0.5) * 2.0).abs();
        let insolation = 1.0 - smoothstep(0.18, 1.0, lat) * 0.72;
        let cells = fbm_tiled(
            u * 1.05 + insolation * 0.08,
            v * 0.84 - self.thermal_contrast * 0.05,
            607,
            4,
            self.seed + 41_207,
            0.54,
        );
        clamp01(insolation * 0.62 + cells * 0.30 + self.thermal_contrast * 0.08)
    }

    fn salinity_field(self, u: f32, v: f32) -> f32 {
        fbm_tiled(
            u * 1.6 + self.tidal_mix * 0.11,
            v * 1.2 - self.ocean_fraction * 0.07,
            613,
            4,
            self.seed + 41_313,
            0.52,
        )
    }

    fn stream_field(self, u: f32, v: f32) -> f32 {
        fbm_tiled(
            u * 2.3 + self.coriolis_strength * 0.04,
            v * 1.7 - self.thermal_contrast * 0.05,
            619,
            4,
            self.seed + 41_419,
            0.50,
        )
    }

    fn magnetic_anomaly_field(self, u: f32, v: f32) -> f32 {
        fbm_tiled(
            u * 1.9 + self.density_earth * 0.04,
            v * 1.3 - self.radius_earth * 0.03,
            631,
            4,
            self.seed + 41_521,
            0.52,
        )
    }

    fn solar_wind_field(self, u: f32, v: f32) -> f32 {
        fbm_tiled(
            u * 0.90 + self.angular_velocity_rad_s * 180_000.0,
            v * 0.64 + self.magnetosphere_strength * 0.08,
            641,
            3,
            self.seed + 41_617,
            0.55,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderExecutionMode {
    Serial,
    Automatic,
    MultiThreaded { threads: usize },
}

impl RenderExecutionMode {
    pub fn resolved_worker_threads(self) -> usize {
        match self {
            Self::Serial => 1,
            Self::Automatic => available_render_threads(),
            Self::MultiThreaded { threads } => threads.max(1),
        }
    }
}

pub fn available_render_threads() -> usize {
    thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .max(1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPhase {
    Planning,
    Background,
    Planet,
    Rings,
    Moon,
    TerrainOverview,
    SurfaceMap,
    ReflectionMap,
    NormalMap,
    HeightMap,
    VegetationMap,
    RoughnessMap,
    AmbientOcclusionMap,
    HorizonOcclusionMap,
    PhysicsMap,
    DensityMap,
    Sharpen,
    Downscale,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderTile {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RenderTile {
    pub const fn pixel_count(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub const fn x_end(self) -> u32 {
        self.x + self.width
    }

    pub const fn y_end(self) -> u32 {
        self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilePlan {
    pub size: RenderSize,
    pub tile_width: u32,
    pub tile_height: u32,
    pub tiles: Vec<RenderTile>,
    pub execution_mode: RenderExecutionMode,
    pub worker_threads: usize,
}

impl TilePlan {
    pub fn for_size(size: RenderSize, execution_mode: RenderExecutionMode) -> Self {
        Self::with_tile_size(size, 128, 128, execution_mode)
    }

    pub fn with_tile_size(
        size: RenderSize,
        tile_width: u32,
        tile_height: u32,
        execution_mode: RenderExecutionMode,
    ) -> Self {
        let tile_width = tile_width.max(1);
        let tile_height = tile_height.max(1);
        let mut tiles = Vec::new();

        let mut y = 0;
        while y < size.height {
            let height = tile_height.min(size.height - y);
            let mut x = 0;
            while x < size.width {
                let width = tile_width.min(size.width - x);
                tiles.push(RenderTile {
                    x,
                    y,
                    width,
                    height,
                });
                x += tile_width;
            }
            y += tile_height;
        }

        let worker_threads =
            if matches!(execution_mode, RenderExecutionMode::Serial) || tiles.len() < 2 {
                1
            } else {
                execution_mode
                    .resolved_worker_threads()
                    .min(tiles.len())
                    .max(1)
            };

        Self {
            size,
            tile_width,
            tile_height,
            tiles,
            execution_mode,
            worker_threads,
        }
    }

    pub fn total_tiles(&self) -> usize {
        self.tiles.len()
    }

    pub fn total_pixels(&self) -> u64 {
        self.tiles.iter().map(|tile| tile.pixel_count()).sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderProgress {
    pub phase: RenderPhase,
    pub completed_tiles: u32,
    pub total_tiles: u32,
    pub completed_pixels: u64,
    pub total_pixels: u64,
}

impl RenderProgress {
    pub fn fraction(self) -> f32 {
        if self.total_pixels == 0 {
            1.0
        } else {
            self.completed_pixels as f32 / self.total_pixels as f32
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderProgressEvent {
    pub progress: RenderProgress,
    pub tile: Option<RenderTile>,
    pub execution_mode: RenderExecutionMode,
    pub worker_threads: usize,
}

/// Maximum native long edge for renderer output. This matches UHD 8K width.
pub const MAX_NATIVE_RENDER_LONG_EDGE: u32 = 7_680;
/// Maximum native short edge for renderer output. This matches UHD 8K height.
pub const MAX_NATIVE_RENDER_SHORT_EDGE: u32 = 4_320;
/// Maximum native pixel budget for a single renderer output.
pub const MAX_NATIVE_RENDER_PIXELS: u64 =
    MAX_NATIVE_RENDER_LONG_EDGE as u64 * MAX_NATIVE_RENDER_SHORT_EDGE as u64;
/// Alias for landscape UHD 8K native width.
pub const MAX_NATIVE_RENDER_WIDTH: u32 = MAX_NATIVE_RENDER_LONG_EDGE;
/// Alias for landscape UHD 8K native height.
pub const MAX_NATIVE_RENDER_HEIGHT: u32 = MAX_NATIVE_RENDER_SHORT_EDGE;
/// Maximum supported native landscape render size.
pub const MAX_NATIVE_RENDER_SIZE: RenderSize = RenderSize {
    width: MAX_NATIVE_RENDER_WIDTH,
    height: MAX_NATIVE_RENDER_HEIGHT,
};

/// Native 16:9 render preset for 480p output.
pub const RENDER_SIZE_480P: RenderSize = RenderSize {
    width: 854,
    height: 480,
};
/// Native 16:9 render preset for 720p output.
pub const RENDER_SIZE_720P: RenderSize = RenderSize {
    width: 1_280,
    height: 720,
};
/// Native 16:9 render preset for 1080p output.
pub const RENDER_SIZE_1080P: RenderSize = RenderSize {
    width: 1_920,
    height: 1_080,
};
/// Native 16:9 render preset for 4K UHD output.
pub const RENDER_SIZE_4K: RenderSize = RenderSize {
    width: 3_840,
    height: 2_160,
};
/// Native 16:9 render preset for 8K UHD output.
pub const RENDER_SIZE_8K: RenderSize = MAX_NATIVE_RENDER_SIZE;

/// Native square render preset for 512px output.
pub const RENDER_SIZE_SQUARE_512: RenderSize = RenderSize {
    width: 512,
    height: 512,
};
/// Native square render preset for 1024px output.
pub const RENDER_SIZE_SQUARE_1024: RenderSize = RenderSize {
    width: 1_024,
    height: 1_024,
};
/// Native square render preset for 2048px output.
pub const RENDER_SIZE_SQUARE_2048: RenderSize = RenderSize {
    width: 2_048,
    height: 2_048,
};
/// Native square render preset for 4096px output.
pub const RENDER_SIZE_SQUARE_4096: RenderSize = RenderSize {
    width: 4_096,
    height: 4_096,
};

/// Native 9:16 portrait render preset for 720p output.
pub const RENDER_SIZE_PORTRAIT_720P: RenderSize = RenderSize {
    width: 720,
    height: 1_280,
};
/// Native 9:16 portrait render preset for 1080p output.
pub const RENDER_SIZE_PORTRAIT_1080P: RenderSize = RenderSize {
    width: 1_080,
    height: 1_920,
};
/// Native 9:16 portrait render preset for 4K UHD output.
pub const RENDER_SIZE_PORTRAIT_4K: RenderSize = RenderSize {
    width: 2_160,
    height: 3_840,
};

/// Named native render size presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSizePreset {
    P480,
    P720,
    P1080,
    Uhd4K,
    Uhd8K,
    Square512,
    Square1024,
    Square2048,
    Square4096,
    Portrait720P,
    Portrait1080P,
    Portrait4K,
}

impl RenderSizePreset {
    pub const fn size(self) -> RenderSize {
        match self {
            Self::P480 => RENDER_SIZE_480P,
            Self::P720 => RENDER_SIZE_720P,
            Self::P1080 => RENDER_SIZE_1080P,
            Self::Uhd4K => RENDER_SIZE_4K,
            Self::Uhd8K => RENDER_SIZE_8K,
            Self::Square512 => RENDER_SIZE_SQUARE_512,
            Self::Square1024 => RENDER_SIZE_SQUARE_1024,
            Self::Square2048 => RENDER_SIZE_SQUARE_2048,
            Self::Square4096 => RENDER_SIZE_SQUARE_4096,
            Self::Portrait720P => RENDER_SIZE_PORTRAIT_720P,
            Self::Portrait1080P => RENDER_SIZE_PORTRAIT_1080P,
            Self::Portrait4K => RENDER_SIZE_PORTRAIT_4K,
        }
    }
}

/// Validation failures for native render output dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSizeValidationError {
    Empty { width: u32, height: u32 },
    LongEdgeTooLarge { value: u32, maximum: u32 },
    ShortEdgeTooLarge { value: u32, maximum: u32 },
    PixelCountTooLarge { value: u64, maximum: u64 },
}

impl RenderSize {
    pub const fn pixel_count(self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn validate_native(self) -> Result<(), RenderSizeValidationError> {
        validate_render_size(self)
    }

    fn scaled_by(self, scale: u32) -> Option<Self> {
        Some(Self {
            width: self.width.checked_mul(scale)?,
            height: self.height.checked_mul(scale)?,
        })
    }
}

/// Validate a native output size against the renderer's 8K policy.
pub fn validate_render_size(size: RenderSize) -> Result<(), RenderSizeValidationError> {
    if size.width == 0 || size.height == 0 {
        return Err(RenderSizeValidationError::Empty {
            width: size.width,
            height: size.height,
        });
    }

    let long_edge = size.width.max(size.height);
    if long_edge > MAX_NATIVE_RENDER_LONG_EDGE {
        return Err(RenderSizeValidationError::LongEdgeTooLarge {
            value: long_edge,
            maximum: MAX_NATIVE_RENDER_LONG_EDGE,
        });
    }

    let short_edge = size.width.min(size.height);
    if short_edge > MAX_NATIVE_RENDER_SHORT_EDGE {
        return Err(RenderSizeValidationError::ShortEdgeTooLarge {
            value: short_edge,
            maximum: MAX_NATIVE_RENDER_SHORT_EDGE,
        });
    }

    let pixels = size.pixel_count();
    if pixels > MAX_NATIVE_RENDER_PIXELS {
        return Err(RenderSizeValidationError::PixelCountTooLarge {
            value: pixels,
            maximum: MAX_NATIVE_RENDER_PIXELS,
        });
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    pub supersample: u32,
    pub atmosphere_samples: u32,
    pub shadow_samples: u32,
    pub ray_traced_reflections: bool,
}

impl RenderOptions {
    pub const fn preview() -> Self {
        Self {
            supersample: 1,
            atmosphere_samples: 4,
            shadow_samples: 1,
            ray_traced_reflections: true,
        }
    }

    pub const fn standard() -> Self {
        Self {
            supersample: 1,
            atmosphere_samples: 8,
            shadow_samples: 2,
            ray_traced_reflections: true,
        }
    }

    pub const fn ultra() -> Self {
        Self {
            supersample: 2,
            atmosphere_samples: 16,
            shadow_samples: 4,
            ray_traced_reflections: true,
        }
    }

    pub fn native_supersample_for_size(self, size: RenderSize) -> u32 {
        let requested = self.supersample.clamp(1, 4);
        requested.min(max_native_supersample_for_size(size)).max(1)
    }

    pub fn icon_supersample_for_size(self, size: RenderSize) -> u32 {
        icon_supersample_for_size(self, size)
    }

    fn visual_quality_grade(self) -> f32 {
        let sample_grade = smoothstep(4.0, 16.0, self.atmosphere_samples as f32);
        let shadow_grade = smoothstep(1.0, 4.0, self.shadow_samples as f32);
        let supersample_grade = smoothstep(1.0, 2.0, self.supersample as f32);
        clamp01(sample_grade * 0.46 + shadow_grade * 0.34 + supersample_grade * 0.20)
    }
}

fn max_native_supersample_for_size(size: RenderSize) -> u32 {
    let mut maximum = 1;
    for scale in 2..=4 {
        let Some(scaled) = size.scaled_by(scale) else {
            break;
        };
        if validate_render_size(scaled).is_err() {
            break;
        }
        maximum = scale;
    }
    maximum
}

fn icon_supersample_for_size(options: RenderOptions, size: RenderSize) -> u32 {
    let quality_floor = if options.atmosphere_samples >= 16 || options.shadow_samples >= 4 {
        4
    } else if options.atmosphere_samples >= 8 || options.shadow_samples >= 2 {
        3
    } else {
        3
    };
    options
        .supersample
        .clamp(1, 4)
        .max(quality_floor)
        .min(max_native_supersample_for_size(size))
        .max(1)
}

fn pathtrace_icon_supersample_for_size(options: RenderOptions, size: RenderSize) -> u32 {
    let mut scale = icon_supersample_for_size(options, size);
    while scale > 1 {
        let Some(scaled) = size.scaled_by(scale) else {
            scale -= 1;
            continue;
        };
        if scaled.pixel_count() <= pathtrace::TraceImage::MAX_PIXELS {
            break;
        }
        scale -= 1;
    }
    scale.max(1)
}

#[derive(Debug, Clone)]
pub struct PlanetRenderer {
    pub profile: PlanetVisualProfile,
    maps: PlanetMaps,
    stable_terrain: Option<StableTerrainContext>,
}

const DEFAULT_PLANET_MAP_WIDTH: usize = 768;
const DEFAULT_PLANET_MAP_HEIGHT: usize = 384;

impl PlanetRenderer {
    pub fn new(profile: PlanetVisualProfile) -> Self {
        let maps = PlanetMaps::generate(
            &profile,
            DEFAULT_PLANET_MAP_WIDTH,
            DEFAULT_PLANET_MAP_HEIGHT,
        );
        let stable_terrain = if matches!(
            planet_render_style(&profile),
            PlanetRenderStyle::Terrestrial
        ) {
            let (anchor_u, anchor_v) = select_overview_anchor(&maps);
            let frame = StableTerrainFrame::for_anchor(&maps, anchor_u, anchor_v, profile.seed);
            Some(StableTerrainContext {
                frame,
                tile: LocalTerrainTile::generate(frame, &maps, &profile),
            })
        } else {
            None
        };
        Self {
            profile,
            maps,
            stable_terrain,
        }
    }

    pub fn render_icon(&self, size: u32) -> RgbaImage {
        self.render_icon_with_options(size, RenderOptions::standard())
    }

    pub fn render_icon_with_options(&self, size: u32, options: RenderOptions) -> RgbaImage {
        self.render_icon_with_progress(size, options, RenderExecutionMode::Serial, noop_progress)
    }

    pub fn render_icon_with_progress<F>(
        &self,
        size: u32,
        options: RenderOptions,
        _execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_icon_lighting_with_progress(size, options, LightingMode::Day, &mut progress)
    }

    pub fn render_night_icon_with_options(&self, size: u32, options: RenderOptions) -> RgbaImage {
        self.render_night_icon_with_progress(
            size,
            options,
            RenderExecutionMode::Serial,
            noop_progress,
        )
    }

    pub fn render_night_icon_with_progress<F>(
        &self,
        size: u32,
        options: RenderOptions,
        _execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_icon_lighting_with_progress(size, options, LightingMode::Night, &mut progress)
    }

    pub fn try_render_raytraced_icon_with_options(
        &self,
        size: u32,
        options: RenderOptions,
    ) -> Result<RgbaImage, pathtrace::TraceError> {
        self.try_render_raytraced_icon_with_progress(
            size,
            options,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn try_render_raytraced_icon_with_progress<F>(
        &self,
        size: u32,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> Result<RgbaImage, pathtrace::TraceError>
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_raytraced_icon_lighting_with_progress(
            size,
            options,
            execution_mode,
            LightingMode::Day,
            &mut progress,
        )
    }

    pub fn try_render_raytraced_night_icon_with_progress<F>(
        &self,
        size: u32,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> Result<RgbaImage, pathtrace::TraceError>
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_raytraced_icon_lighting_with_progress(
            size,
            options,
            execution_mode,
            LightingMode::Night,
            &mut progress,
        )
    }

    fn render_raytraced_icon_lighting_with_progress<F>(
        &self,
        size: u32,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        lighting_mode: LightingMode,
        progress: &mut F,
    ) -> Result<RgbaImage, pathtrace::TraceError>
    where
        F: FnMut(RenderProgressEvent),
    {
        let size = RenderSize {
            width: size,
            height: size,
        };
        let scale = pathtrace_icon_supersample_for_size(options, size);
        let render_size = size.scaled_by(scale).unwrap_or(size);
        let total_pixels = render_size.pixel_count();
        if total_pixels > pathtrace::TraceImage::MAX_PIXELS {
            return Err(pathtrace::TraceError::ImageTooLarge {
                pixels: total_pixels,
                max_pixels: pathtrace::TraceImage::MAX_PIXELS,
            });
        }

        let settings = pathtrace_settings_for_render(&self.profile, options)
            .validate()
            .map_err(pathtrace::TraceError::Settings)?;
        let plan = TilePlan::with_tile_size(
            render_size,
            settings.tile_width,
            settings.tile_height,
            execution_mode,
        );
        emit_plan_progress(progress, &plan);

        let (scene, surface) = pathtrace_scene_for_profile(&self.profile, lighting_mode);
        let kernel = pathtrace::CpuTraceKernel::new_with_surface(scene, surface);
        let camera = pathtrace_icon_camera(render_size);
        let mut canvas = Canvas::transparent(render_size.width, render_size.height);
        render_tiled_into_canvas(&mut canvas, &plan, RenderPhase::Planet, progress, |x, y| {
            let sample = kernel
                .trace_pixel(
                    camera,
                    x,
                    y,
                    render_size.width,
                    render_size.height,
                    settings,
                )
                .expect("validated CPU pathtrace settings should trace each pixel");
            pathtrace_color_to_rgba(
                sample.color,
                pathtrace_icon_alpha(camera, scene, x, y, render_size),
            )
        });

        emit_phase_start(progress, &plan, RenderPhase::Sharpen);
        canvas.sharpen_opaque(profile_sharpen_amount(&self.profile, scale, 0.020, 0.050));
        emit_phase_complete(progress, &plan, RenderPhase::Sharpen);

        let image = canvas.into_image();
        let mut image = if scale > 1 {
            emit_phase_start(progress, &plan, RenderPhase::Downscale);
            downscale_lanczos3_premul(&image, size.width, size.height)
        } else {
            image
        };
        apply_visual_quality_postprocess(&mut image, &self.profile, options);
        polish_icon_alpha_edge(&mut image);
        if scale > 1 {
            emit_phase_complete(progress, &plan, RenderPhase::Downscale);
        }
        emit_phase_complete(progress, &plan, RenderPhase::Complete);

        Ok(image)
    }

    fn render_icon_lighting_with_progress<F>(
        &self,
        size: u32,
        options: RenderOptions,
        lighting_mode: LightingMode,
        progress: &mut F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let size = RenderSize {
            width: size,
            height: size,
        };
        let scale = icon_supersample_for_size(options, size);
        let render_size = size.scaled_by(scale).unwrap_or(size);
        let plan = TilePlan::for_size(render_size, RenderExecutionMode::Serial);
        emit_plan_progress(progress, &plan);
        let mut canvas = Canvas::transparent(render_size.width, render_size.height);
        emit_phase_start(progress, &plan, RenderPhase::Planet);
        render_planet_with_lighting(
            &mut canvas,
            &self.maps,
            &self.profile,
            render_size.width as f32 * 0.5,
            render_size.height as f32 * 0.5,
            render_size.width.min(render_size.height) as f32 * 0.414,
            lighting_mode,
        );
        emit_phase_complete(progress, &plan, RenderPhase::Planet);
        emit_phase_start(progress, &plan, RenderPhase::Sharpen);
        canvas.sharpen_opaque(profile_sharpen_amount(&self.profile, scale, 0.060, 0.18));
        emit_phase_complete(progress, &plan, RenderPhase::Sharpen);
        let image = canvas.into_image();
        let mut image = if scale > 1 {
            emit_phase_start(progress, &plan, RenderPhase::Downscale);
            downscale_lanczos3_premul(&image, size.width, size.height)
        } else {
            image
        };
        apply_visual_quality_postprocess(&mut image, &self.profile, options);
        polish_icon_alpha_edge(&mut image);
        if scale > 1 {
            emit_phase_complete(progress, &plan, RenderPhase::Downscale);
        }
        emit_phase_complete(progress, &plan, RenderPhase::Complete);
        image
    }

    pub fn render_banner(&self, size: RenderSize) -> RgbaImage {
        self.render_banner_with_options(size, RenderOptions::standard())
    }

    pub fn render_banner_with_options(
        &self,
        size: RenderSize,
        options: RenderOptions,
    ) -> RgbaImage {
        self.render_banner_with_progress(
            size,
            options,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_banner_with_progress<F>(
        &self,
        size: RenderSize,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_terrain_overview_with_progress(size, options, execution_mode, progress)
    }

    pub fn render_night_banner_with_progress<F>(
        &self,
        size: RenderSize,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_night_terrain_overview_with_progress(size, options, execution_mode, progress)
    }

    pub fn render_orbital_banner_with_options(
        &self,
        size: RenderSize,
        options: RenderOptions,
    ) -> RgbaImage {
        self.render_orbital_banner_with_progress(
            size,
            options,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_orbital_banner_with_progress<F>(
        &self,
        size: RenderSize,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let scale = options.native_supersample_for_size(size);
        let render_size = size.scaled_by(scale).unwrap_or(size);
        let plan = TilePlan::for_size(render_size, execution_mode);
        let serial_plan = TilePlan::for_size(render_size, RenderExecutionMode::Serial);
        emit_plan_progress(&mut progress, &plan);
        let mut canvas = Canvas::transparent(render_size.width, render_size.height);
        render_space_background_with_progress(
            &mut canvas,
            self.profile.seed,
            execution_mode,
            &mut progress,
        );

        let cx = render_size.width as f32 * 0.755;
        let cy = render_size.height as f32 * 0.61;
        let radius = render_size.height as f32 * 0.67;

        if self.profile.ringed {
            emit_phase_start(&mut progress, &serial_plan, RenderPhase::Rings);
            draw_rings(&mut canvas, cx, cy, radius, false);
            emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Rings);
        }
        emit_phase_start(&mut progress, &serial_plan, RenderPhase::Planet);
        render_planet(&mut canvas, &self.maps, &self.profile, cx, cy, radius);
        emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Planet);
        if self.profile.ringed {
            emit_phase_start(&mut progress, &serial_plan, RenderPhase::Rings);
            draw_rings(&mut canvas, cx, cy, radius, true);
            emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Rings);
        }
        emit_phase_start(&mut progress, &serial_plan, RenderPhase::Moon);
        render_moon(
            &mut canvas,
            render_size.width as f32 * 0.225,
            render_size.height as f32 * 0.25,
            21.0 * scale as f32,
            self.profile.seed,
        );
        emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Moon);
        emit_phase_start(&mut progress, &serial_plan, RenderPhase::Sharpen);
        canvas.sharpen_opaque(profile_sharpen_amount(&self.profile, scale, 0.08, 0.20));
        emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Sharpen);
        let image = canvas.into_image();
        let image = if scale > 1 {
            emit_phase_start(&mut progress, &serial_plan, RenderPhase::Downscale);
            downscale_lanczos3_premul(&image, size.width, size.height)
        } else {
            image
        };
        if scale > 1 {
            emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Downscale);
        }
        emit_phase_complete(&mut progress, &serial_plan, RenderPhase::Complete);
        image
    }

    pub fn render_terrain_overview_with_options(
        &self,
        size: RenderSize,
        options: RenderOptions,
    ) -> RgbaImage {
        self.render_terrain_overview_with_progress(
            size,
            options,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_terrain_overview_with_progress<F>(
        &self,
        size: RenderSize,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_terrain_overview_lighting_with_progress(
            size,
            options,
            execution_mode,
            LightingMode::Day,
            &mut progress,
        )
    }

    pub fn render_night_terrain_overview_with_options(
        &self,
        size: RenderSize,
        options: RenderOptions,
    ) -> RgbaImage {
        self.render_night_terrain_overview_with_progress(
            size,
            options,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_night_terrain_overview_with_progress<F>(
        &self,
        size: RenderSize,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_terrain_overview_lighting_with_progress(
            size,
            options,
            execution_mode,
            LightingMode::Night,
            &mut progress,
        )
    }

    fn render_terrain_overview_lighting_with_progress<F>(
        &self,
        size: RenderSize,
        options: RenderOptions,
        execution_mode: RenderExecutionMode,
        lighting_mode: LightingMode,
        progress: &mut F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let scale = options.native_supersample_for_size(size);
        let render_size = size.scaled_by(scale).unwrap_or(size);
        let plan = TilePlan::for_size(render_size, execution_mode);
        emit_plan_progress(progress, &plan);
        let mut canvas = Canvas::transparent(render_size.width, render_size.height);
        render_terrain_overview_with_progress(
            &mut canvas,
            &self.maps,
            &self.profile,
            self.stable_terrain.as_ref(),
            execution_mode,
            lighting_mode,
            progress,
        );
        emit_phase_start(progress, &plan, RenderPhase::Sharpen);
        canvas.sharpen_opaque(profile_sharpen_amount(&self.profile, scale, 0.06, 0.14));
        emit_phase_complete(progress, &plan, RenderPhase::Sharpen);
        let image = canvas.into_image();
        let image = if scale > 1 {
            emit_phase_start(progress, &plan, RenderPhase::Downscale);
            downscale_lanczos3_premul(&image, size.width, size.height)
        } else {
            image
        };
        if scale > 1 {
            emit_phase_complete(progress, &plan, RenderPhase::Downscale);
        }
        emit_phase_complete(progress, &plan, RenderPhase::Complete);
        image
    }

    pub fn render_surface_map(&self, size: RenderSize) -> RgbaImage {
        self.render_surface_map_with_progress(size, RenderExecutionMode::Automatic, noop_progress)
    }

    pub fn render_surface_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::SurfaceMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let u = x as f32 / size.width as f32;
                let v = y as f32 / (size.height.saturating_sub(1).max(1)) as f32;
                let sample = maps.sample(u, v);
                let rgb = tone_map(sample.albedo * 1.18);
                rgba(rgb, 255)
            },
        );
        canvas.into_image()
    }

    pub fn render_reflection_map(&self, size: RenderSize) -> RgbaImage {
        self.render_reflection_map_with_progress(
            size,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_reflection_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let seed = self.profile.seed;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::ReflectionMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let v = y as f32 / size.height.saturating_sub(1).max(1) as f32;
                let lat = (0.5 - v) * PI;
                let u = x as f32 / size.width as f32;
                let lon = (u - 0.5) * PI * 2.0;
                let dir = Vec3::new(lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin());
                rgba(
                    tone_map(sample_environment(dir, seed, DistantLight::solar_default())),
                    255,
                )
            },
        );
        canvas.into_image()
    }

    pub fn render_normal_map(&self, size: RenderSize) -> RgbaImage {
        self.render_normal_map_with_progress(size, RenderExecutionMode::Automatic, noop_progress)
    }

    pub fn render_normal_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::NormalMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let n = maps.tangent_space_normal(u, v, 5.5);
                rgba(
                    Vec3::new(n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5),
                    255,
                )
            },
        );
        canvas.into_image()
    }

    pub fn render_height_map(&self, size: RenderSize) -> RgbaImage {
        self.render_height_map_with_progress(size, RenderExecutionMode::Automatic, noop_progress)
    }

    pub fn render_height_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::HeightMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let sample = maps.sample(u, v);
                let height = (sample.height * 255.0) as u8;
                [height, height, height, 255]
            },
        );
        canvas.into_image()
    }

    pub fn render_vegetation_map(&self, size: RenderSize) -> RgbaImage {
        self.render_vegetation_map_with_progress(
            size,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_vegetation_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::VegetationMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let sample = maps.sample(u, v);
                rgba(
                    Vec3::new(sample.biome, sample.vegetation, sample.water),
                    255,
                )
            },
        );
        canvas.into_image()
    }

    pub fn render_roughness_map(&self, size: RenderSize) -> RgbaImage {
        self.render_roughness_map_with_progress(size, RenderExecutionMode::Automatic, noop_progress)
    }

    pub fn render_roughness_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::RoughnessMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let sample = maps.sample(u, v);
                let roughness = (sample.roughness * 255.0) as u8;
                let wetness = (sample.wetness * 255.0) as u8;
                [roughness, wetness, 0, 255]
            },
        );
        canvas.into_image()
    }

    /// Renders baked multi-scale terrain ambient occlusion.
    ///
    /// The map is linear grayscale: black is open terrain and white is fully
    /// occluded. It is derived once from the deterministic global heightfield
    /// by integrating logarithmically spaced horizon probes in eight azimuths.
    pub fn render_ambient_occlusion_map(&self, size: RenderSize) -> RgbaImage {
        self.render_ambient_occlusion_map_with_progress(
            size,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_ambient_occlusion_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        render_tiled_canvas(
            size,
            RenderPhase::AmbientOcclusionMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let value = (maps.sample(u, v).ambient_occlusion * 255.0).round() as u8;
                [value, value, value, 255]
            },
        )
        .into_image()
    }

    /// Renders the distant terrain horizon/sky occlusion product.
    ///
    /// This is intentionally separate from local AO: it answers how much of
    /// the upper hemisphere is blocked by ridges and crater walls, which lets
    /// surface cameras attenuate diffuse sky light without baking a light
    /// direction into the material maps.
    pub fn render_horizon_occlusion_map(&self, size: RenderSize) -> RgbaImage {
        self.render_horizon_occlusion_map_with_progress(
            size,
            RenderExecutionMode::Automatic,
            noop_progress,
        )
    }

    pub fn render_horizon_occlusion_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let maps = &self.maps;
        render_tiled_canvas(
            size,
            RenderPhase::HorizonOcclusionMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let value = (maps.sample(u, v).horizon_occlusion * 255.0).round() as u8;
                [value, value, value, 255]
            },
        )
        .into_image()
    }

    pub fn physics_model(&self) -> PlanetPhysicsModel {
        PlanetPhysicsModel::from_profile(&self.profile)
    }

    pub fn sample_physics(&self, u: f32, v: f32) -> PlanetPhysicsSample {
        self.physics_model().sample(u, v)
    }

    pub fn evolution_model(&self) -> evolution::PlanetEvolutionModel {
        evolution::PlanetEvolutionModel::from_profile(self.profile.clone())
    }

    pub fn climate_snapshot(&self) -> evolution::PlanetClimateSnapshot {
        self.evolution_model()
            .snapshot_at(evolution::EvolutionTime::from_days(
                self.profile.snapshot_time_days as f64,
            ))
    }

    pub fn sample_climate(&self, u: f32, v: f32) -> evolution::PlanetClimateSample {
        self.climate_snapshot().sample(u, v)
    }

    pub fn geology_model(&self) -> geology::GeologyModel {
        geology::GeologyModel::from_profile(&self.profile)
    }

    pub fn sample_geology(&self, u: f32, v: f32) -> geology::GeologySample {
        self.geology_model().sample(u, v)
    }

    pub fn render_physics_map(&self, size: RenderSize) -> RgbaImage {
        self.render_physics_map_with_progress(size, RenderExecutionMode::Automatic, noop_progress)
    }

    pub fn render_physics_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let physics = self.physics_model();
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::PhysicsMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let sample = maps.sample(u, v);
                let physics_sample = physics.sample(u, v);
                let current = clamp01(
                    physics_sample.current_speed_mps
                        / (physics.current_velocity_scale_mps * 1.45).max(0.001),
                );
                let clouds = clamp01(physics_sample.cloud_lift * 0.72 + sample.cloud * 0.42);
                let magnetic = clamp01(
                    physics_sample.magnetic_field_microtesla / 120.0
                        + physics_sample.aurora_power * 0.20,
                );
                rgba(Vec3::new(current, clouds, magnetic), 255)
            },
        );
        canvas.into_image()
    }

    pub fn render_density_map(&self, size: RenderSize) -> RgbaImage {
        self.render_density_map_with_progress(size, RenderExecutionMode::Automatic, noop_progress)
    }

    pub fn render_density_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        mut progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        let plan = TilePlan::for_size(size, execution_mode);
        emit_plan_progress(&mut progress, &plan);
        let physics = self.physics_model();
        let maps = &self.maps;
        let canvas = render_tiled_canvas(
            size,
            RenderPhase::DensityMap,
            execution_mode,
            &mut progress,
            |x, y| {
                let (u, v) = material_map_uv(size, x, y);
                let sample = maps.sample(u, v);
                let physics_sample = physics.sample(u, v);
                let water_density = clamp01((physics_sample.water_density_kg_m3 - 920.0) / 300.0);
                let atmosphere_density = clamp01(physics_sample.atmosphere_density_kg_m3 / 8.5);
                let pressure = clamp01(
                    (physics_sample.surface_pressure_bar - physics.surface_pressure_bar * 0.72)
                        / (physics.surface_pressure_bar * 0.66).max(0.015),
                );
                rgba(
                    Vec3::new(
                        water_density * smoothstep(0.05, 0.70, sample.water),
                        atmosphere_density,
                        pressure,
                    ),
                    255,
                )
            },
        );
        canvas.into_image()
    }

    pub fn render_surface_normal_map(&self, size: RenderSize) -> RgbaImage {
        self.render_normal_map(size)
    }

    pub fn render_surface_normal_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_normal_map_with_progress(size, execution_mode, progress)
    }

    pub fn render_surface_height_map(&self, size: RenderSize) -> RgbaImage {
        self.render_height_map(size)
    }

    pub fn render_surface_height_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_height_map_with_progress(size, execution_mode, progress)
    }

    pub fn render_surface_biome_map(&self, size: RenderSize) -> RgbaImage {
        self.render_vegetation_map(size)
    }

    pub fn render_surface_biome_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_vegetation_map_with_progress(size, execution_mode, progress)
    }

    pub fn render_surface_roughness_wetness_map(&self, size: RenderSize) -> RgbaImage {
        self.render_roughness_map(size)
    }

    pub fn render_surface_roughness_wetness_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_roughness_map_with_progress(size, execution_mode, progress)
    }

    pub fn render_surface_ao_map(&self, size: RenderSize) -> RgbaImage {
        self.render_ambient_occlusion_map(size)
    }

    pub fn render_surface_ao_map_with_progress<F>(
        &self,
        size: RenderSize,
        execution_mode: RenderExecutionMode,
        progress: F,
    ) -> RgbaImage
    where
        F: FnMut(RenderProgressEvent),
    {
        self.render_ambient_occlusion_map_with_progress(size, execution_mode, progress)
    }
}

#[derive(Debug, Clone)]
struct PlanetMaps {
    width: usize,
    height: usize,
    albedo: Vec<Vec3>,
    height_map: Vec<f32>,
    water: Vec<f32>,
    clouds: Vec<f32>,
    cities: Vec<f32>,
    vegetation: Vec<f32>,
    biome: Vec<f32>,
    roughness: Vec<f32>,
    wetness: Vec<f32>,
    ambient_occlusion: Vec<f32>,
    horizon_occlusion: Vec<f32>,
}

#[derive(Debug, Clone, Copy)]
struct SurfaceSample {
    albedo: Vec3,
    height: f32,
    water: f32,
    cloud: f32,
    city: f32,
    vegetation: f32,
    biome: f32,
    roughness: f32,
    wetness: f32,
    ambient_occlusion: f32,
    horizon_occlusion: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanetRenderStyle {
    Terrestrial,
    OceanWorld,
    RockyWorld,
    VolcanicWorld,
    GasGiant,
}

fn planet_render_style(profile: &PlanetVisualProfile) -> PlanetRenderStyle {
    if is_banded_gas_giant_profile(profile) {
        PlanetRenderStyle::GasGiant
    } else if is_ocean_world_profile(profile) {
        PlanetRenderStyle::OceanWorld
    } else if is_volcanic_world_profile(profile) {
        PlanetRenderStyle::VolcanicWorld
    } else if is_rocky_world_profile(profile) {
        PlanetRenderStyle::RockyWorld
    } else {
        PlanetRenderStyle::Terrestrial
    }
}

fn is_banded_gas_giant_profile(profile: &PlanetVisualProfile) -> bool {
    key_is_any(
        profile.archetype_key.as_str(),
        &[
            "catalog.archetype.banded-gas-giant",
            "catalog.archetype.gas-giant",
            "catalog.archetype.storm-gas-giant",
            "catalog.archetype.sulfur-gas-world",
            "catalog.archetype.hot-jupiter",
            "catalog.archetype.cold-jupiter",
            "catalog.archetype.saturn-like",
            "catalog.archetype.helium-giant",
            "catalog.archetype.puffy-giant",
            "catalog.archetype.ice-giant",
            "catalog.archetype.blue-ice-giant",
            "catalog.archetype.methane-ice-giant",
            "catalog.archetype.uranus-like-ice-giant",
            "catalog.archetype.neptune-like-storm-giant",
            "catalog.archetype.mini-neptune",
            "catalog.archetype.sub-neptune",
            "catalog.archetype.sub-neptune-haze",
            "catalog.archetype.gas-dwarf",
            "catalog.archetype.hot-neptune",
            "catalog.archetype.rogue-gas-giant",
            "catalog.archetype.diamond-rain-gas-giant",
        ],
    ) || key_is_any(
        profile.class_key.as_str(),
        &[
            "gas-giant",
            "hot-jupiter",
            "cold-jupiter",
            "saturn-like",
            "helium-giant",
            "puffy-giant",
            "ice-giant",
            "mini-neptune",
            "sub-neptune",
            "gas-dwarf",
        ],
    ) || profile.render_model == "render.gas-bands-storms-rings"
        || profile.render_model == "render.ice-giant-cloud-gradient"
        || profile_key_contains_any(
            profile,
            &[
                "gas-giant",
                "storm-gas",
                "sulfur-gas",
                "ice-giant",
                "mini-neptune",
                "sub-neptune",
                "gas-dwarf",
                "hot-neptune",
                "rogue-gas",
                "diamond-rain-gas",
            ],
        )
}

fn is_ocean_world_profile(profile: &PlanetVisualProfile) -> bool {
    profile.ocean_fraction >= 0.88
        || key_is_any(
            profile.archetype_key.as_str(),
            &[
                "catalog.archetype.global-ocean",
                "catalog.archetype.ocean",
                "catalog.archetype.water-steam-hycean",
            ],
        )
        || profile.class_key == "ocean-world"
        || profile.render_model == "render.ocean-cloud-glint-atmosphere"
}

fn is_volcanic_world_profile(profile: &PlanetVisualProfile) -> bool {
    if profile_key_contains_any(profile, &["carbon", "diamond"])
        && !profile_key_contains_any(profile, &["diamond-rain-gas", "sulfur", "lava", "magma"])
    {
        return false;
    }

    profile.volcanic_activity >= 0.62
        || key_is_any(
            profile.archetype_key.as_str(),
            &[
                "catalog.archetype.active-volcanic",
                "catalog.archetype.sulfur-volcanic",
                "catalog.archetype.lava-magma",
                "catalog.archetype.sulfur-io-like",
            ],
        )
        || profile_key_contains_any(
            profile,
            &[
                "active-volcanic",
                "volcanic",
                "lava",
                "magma",
                "sulfur",
                "io-like",
            ],
        )
        || profile.render_model.contains("lava")
        || profile.render_model.contains("volcanic")
}

fn is_rocky_world_profile(profile: &PlanetVisualProfile) -> bool {
    if profile.ocean_fraction > 0.18 || profile.cloud_density > 0.82 {
        return false;
    }

    key_is_any(
        profile.archetype_key.as_str(),
        &[
            "catalog.archetype.barren-basalt",
            "catalog.archetype.mercury-like",
            "catalog.archetype.mars-like",
            "catalog.archetype.red-dune-desert",
            "catalog.archetype.desert-dune",
            "catalog.archetype.carbon",
            "catalog.archetype.iron",
            "catalog.archetype.chthonian",
            "catalog.archetype.proto-planet",
            "catalog.archetype.dwarf-asteroid-like",
        ],
    ) || profile_key_contains_any(
        profile,
        &[
            "barren",
            "airless",
            "basalt",
            "mercury",
            "mars",
            "asteroid",
            "dwarf",
            "desert",
            "dune",
            "iron",
            "carbon",
            "chthonian",
            "rocky",
        ],
    )
}

#[derive(Debug, Clone, Copy)]
struct RockySurfaceMaterial {
    rocky: f32,
    barren: f32,
    volcanic: f32,
    basalt: f32,
    oxide: f32,
    ash: f32,
    sulfur: f32,
    lava: f32,
    relief: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RockyPaletteKind {
    IronOxide,
    PaleSilicate,
    Basaltic,
    Carbonaceous,
    DustyRegolith,
}

#[derive(Debug, Clone, Copy)]
struct RockySurfacePalette {
    kind: RockyPaletteKind,
    low: Vec3,
    mid: Vec3,
    high: Vec3,
    shadow: Vec3,
    mineral: Vec3,
    haze: Vec3,
    atmosphere_boost: f32,
    relief_boost: f32,
}

fn rocky_surface_material(profile: &PlanetVisualProfile) -> RockySurfaceMaterial {
    let volcanic_key = profile_key_contains_any(
        profile,
        &[
            "volcanic",
            "lava",
            "magma",
            "sulfur",
            "io-like",
            "active-volcanic",
            "caldera",
        ],
    );
    let barren_key = profile_key_contains_any(
        profile,
        &[
            "barren",
            "airless",
            "basalt",
            "mercury",
            "mars",
            "asteroid",
            "dwarf-asteroid",
            "desert",
            "dune",
            "chthonian",
            "iron",
            "carbon",
            "rocky",
        ],
    );
    let basalt_key =
        profile_key_contains_any(profile, &["basalt", "slag", "chthonian", "magma", "iron"]);
    let oxide_key =
        profile_key_contains_any(profile, &["oxide", "rust", "hematite", "mars", "red-dune"]);
    let ash_key = profile_key_contains_any(profile, &["ash", "soot", "acid-cloud"]);
    let sulfur_key = profile_key_contains_any(profile, &["sulfur", "io-like"]);
    let lava_key = profile_key_contains_any(profile, &["lava", "magma", "incandescent"]);

    let temperature = profile.temperature_c as f32;
    let ocean_dryness = 1.0 - smoothstep(0.04, 0.58, profile.ocean_fraction);
    let sparse_cloud = 1.0 - smoothstep(0.10, 0.72, profile.cloud_density);
    let thin_air = 1.0 - smoothstep(0.04, 0.85, profile.atmosphere_density);
    let hot = smoothstep(180.0, 920.0, temperature);
    let cold_airless = smoothstep(0.25, 0.85, thin_air) * smoothstep(120.0, -120.0, temperature);
    let key_barren = if barren_key { 1.0 } else { 0.0 };
    let key_volcanic = if volcanic_key { 1.0 } else { 0.0 };

    let barren = clamp01(
        ocean_dryness * 0.58
            + sparse_cloud * 0.14
            + thin_air * 0.16
            + cold_airless * 0.10
            + key_barren * 0.40
            - profile.cloud_density * 0.08,
    );
    let volcanic = clamp01(profile.volcanic_activity * 1.10 + key_volcanic * 0.34 + hot * 0.12);
    let basalt = clamp01(
        volcanic * 0.52
            + barren * 0.24
            + if basalt_key { 0.42 } else { 0.0 }
            + if lava_key { 0.18 } else { 0.0 },
    );
    let oxide = clamp01(
        barren * 0.34
            + volcanic * 0.08
            + smoothstep(-90.0, 260.0, temperature) * 0.14
            + if oxide_key { 0.54 } else { 0.0 },
    );
    let ash = clamp01(
        volcanic * 0.38
            + profile.atmosphere_density * 0.12
            + sparse_cloud * 0.08
            + if ash_key { 0.52 } else { 0.0 },
    );
    let sulfur = clamp01(if sulfur_key { 0.78 } else { 0.0 } + volcanic * 0.18);
    let lava = clamp01(
        volcanic * (0.18 + hot * 0.34)
            + if lava_key { 0.62 } else { 0.0 }
            + if profile.render_model.contains("emissive") {
                0.18
            } else {
                0.0
            },
    );
    let rocky = clamp01(
        barren * 0.52
            + volcanic * 0.38
            + basalt * 0.18
            + if barren_key || volcanic_key {
                0.18
            } else {
                0.0
            },
    );
    let relief = clamp01(rocky * 0.58 + barren * 0.22 + volcanic * 0.24 + basalt * 0.10);

    RockySurfaceMaterial {
        rocky,
        barren,
        volcanic,
        basalt,
        oxide,
        ash,
        sulfur,
        lava,
        relief,
    }
}

fn rocky_surface_palette(
    profile: &PlanetVisualProfile,
    material: RockySurfaceMaterial,
) -> RockySurfacePalette {
    let explicit_iron_key = key_is_any(profile.archetype_key.as_str(), &["catalog.archetype.iron"])
        || profile_key_contains_any(profile, &["iron-world", "metallic-iron", "iron_core"]);
    let pale_key = profile_key_contains_any(
        profile,
        &[
            "mercury",
            "silicate",
            "proto-planet",
            "dwarf",
            "asteroid",
            "white",
        ],
    );
    let basalt_key =
        profile_key_contains_any(profile, &["basalt", "chthonian", "slag", "lava", "magma"]);
    let oxide_key = profile_key_contains_any(profile, &["oxide", "hematite", "mars", "red-dune"]);
    let rust_accent_key = profile_key_contains(profile, "rust") && !pale_key && !basalt_key;
    let iron_key = explicit_iron_key || oxide_key || rust_accent_key;
    let carbon_key = profile_key_contains_any(profile, &["carbon", "soot", "graphite"]);
    let roll = hash2(503, 1_019, profile.seed);

    let kind = if carbon_key {
        RockyPaletteKind::Carbonaceous
    } else if pale_key && !explicit_iron_key && !oxide_key {
        RockyPaletteKind::PaleSilicate
    } else if basalt_key && !explicit_iron_key && !oxide_key {
        RockyPaletteKind::Basaltic
    } else if oxide_key || (iron_key && roll < 0.88) {
        RockyPaletteKind::IronOxide
    } else if pale_key || (iron_key && roll >= 0.78) {
        RockyPaletteKind::PaleSilicate
    } else if basalt_key {
        RockyPaletteKind::Basaltic
    } else if roll < 0.26 {
        RockyPaletteKind::PaleSilicate
    } else if roll < 0.54 {
        RockyPaletteKind::DustyRegolith
    } else if roll < 0.78 {
        RockyPaletteKind::IronOxide
    } else {
        RockyPaletteKind::Basaltic
    };

    let tint = hash2(1_121, 1_307, profile.seed);
    let atmosphere = smoothstep(0.10, 0.74, profile.atmosphere_density);
    let hot_bleach = smoothstep(160.0, 720.0, profile.temperature_c as f32) * 0.10;
    let cold_bleach = smoothstep(-180.0, -40.0, profile.temperature_c as f32) * 0.08;

    let mut palette = match kind {
        RockyPaletteKind::IronOxide => RockySurfacePalette {
            kind,
            low: Vec3::new(0.30, 0.105, 0.048).lerp(Vec3::new(0.42, 0.15, 0.070), tint),
            mid: Vec3::new(0.66, 0.255, 0.105).lerp(Vec3::new(0.82, 0.36, 0.145), tint),
            high: Vec3::new(0.94, 0.50, 0.215).lerp(Vec3::new(1.02, 0.64, 0.32), tint),
            shadow: Vec3::new(0.125, 0.052, 0.036),
            mineral: Vec3::new(1.00, 0.285, 0.070).lerp(Vec3::new(1.06, 0.46, 0.13), tint),
            haze: Vec3::new(0.82, 0.48, 0.32),
            atmosphere_boost: 0.070,
            relief_boost: 0.090,
        },
        RockyPaletteKind::PaleSilicate => RockySurfacePalette {
            kind,
            low: Vec3::new(0.31, 0.295, 0.255),
            mid: Vec3::new(0.56, 0.525, 0.440).lerp(Vec3::new(0.66, 0.62, 0.52), tint),
            high: Vec3::new(0.82, 0.78, 0.66).lerp(Vec3::new(0.92, 0.88, 0.76), tint),
            shadow: Vec3::new(0.120, 0.116, 0.105),
            mineral: Vec3::new(0.68, 0.56, 0.40).lerp(Vec3::new(0.88, 0.78, 0.60), tint),
            haze: Vec3::new(0.74, 0.76, 0.72),
            atmosphere_boost: 0.040,
            relief_boost: 0.115,
        },
        RockyPaletteKind::Basaltic => RockySurfacePalette {
            kind,
            low: Vec3::new(0.075, 0.072, 0.066),
            mid: Vec3::new(0.20, 0.19, 0.165).lerp(Vec3::new(0.30, 0.275, 0.225), tint),
            high: Vec3::new(0.48, 0.44, 0.36).lerp(Vec3::new(0.62, 0.57, 0.45), tint),
            shadow: Vec3::new(0.032, 0.031, 0.030),
            mineral: Vec3::new(0.44, 0.265, 0.155).lerp(Vec3::new(0.54, 0.40, 0.28), tint),
            haze: Vec3::new(0.52, 0.55, 0.56),
            atmosphere_boost: 0.030,
            relief_boost: 0.150,
        },
        RockyPaletteKind::Carbonaceous => RockySurfacePalette {
            kind,
            low: Vec3::new(0.050, 0.048, 0.044),
            mid: Vec3::new(0.145, 0.135, 0.120).lerp(Vec3::new(0.22, 0.205, 0.175), tint),
            high: Vec3::new(0.40, 0.38, 0.32).lerp(Vec3::new(0.55, 0.52, 0.43), tint),
            shadow: Vec3::new(0.020, 0.020, 0.019),
            mineral: Vec3::new(0.34, 0.30, 0.24).lerp(Vec3::new(0.52, 0.42, 0.30), tint),
            haze: Vec3::new(0.48, 0.50, 0.50),
            atmosphere_boost: 0.020,
            relief_boost: 0.130,
        },
        RockyPaletteKind::DustyRegolith => RockySurfacePalette {
            kind,
            low: Vec3::new(0.28, 0.235, 0.175),
            mid: Vec3::new(0.50, 0.405, 0.285).lerp(Vec3::new(0.62, 0.50, 0.36), tint),
            high: Vec3::new(0.78, 0.66, 0.48).lerp(Vec3::new(0.88, 0.76, 0.56), tint),
            shadow: Vec3::new(0.105, 0.082, 0.060),
            mineral: Vec3::new(0.62, 0.36, 0.17).lerp(Vec3::new(0.74, 0.50, 0.28), tint),
            haze: Vec3::new(0.78, 0.66, 0.50),
            atmosphere_boost: 0.085,
            relief_boost: 0.100,
        },
    };

    let bleach = clamp01(hot_bleach + cold_bleach + material.ash * 0.055);
    palette.mid = palette.mid.lerp(palette.high, bleach);
    palette.high = palette
        .high
        .lerp(Vec3::new(0.94, 0.91, 0.82), bleach * 0.46);
    palette.haze = palette
        .haze
        .lerp(Vec3::new(0.68, 0.78, 0.88), atmosphere * 0.26);
    palette.atmosphere_boost = clamp01(palette.atmosphere_boost + atmosphere * 0.055);
    palette.relief_boost = clamp01(palette.relief_boost + material.relief * 0.090);

    palette
}

fn rocky_patch_field(u: f32, v: f32, seed: u64) -> f32 {
    let broad = fbm_periodic(u * 0.82 + 0.13, v * 0.68 - 0.07, 5, 5, seed + 34_101, 0.58);
    let lobe = fbm_periodic(
        u * 1.65 + broad * 0.12,
        v * 1.28 - broad * 0.09,
        11,
        4,
        seed + 34_219,
        0.54,
    );
    let mottled = fbm_periodic(
        u * 3.2 + lobe * 0.06,
        v * 2.4 - broad * 0.05,
        29,
        3,
        seed + 34_337,
        0.48,
    );
    clamp01(broad * 0.50 + lobe * 0.34 + mottled * 0.16)
}

fn rocky_patch_height(u: f32, v: f32, seed: u64) -> f32 {
    let lobes = rocky_patch_field(u, v, seed);
    let broken = fbm_periodic(
        u * 5.8 + lobes * 0.07,
        v * 4.4 - lobes * 0.05,
        43,
        3,
        seed + 34_509,
        0.47,
    );
    let grain = terrain_micro_detail(u * 1.7 + lobes * 0.035, v * 1.4 - broken * 0.025, seed);
    clamp01(lobes * 0.56 + broken * 0.30 + grain * 0.14)
}

fn landform_exposure_mask(mountain_chain: f32, uplift: f32, plate: f32) -> f32 {
    smoothstep(
        0.18,
        0.86,
        mountain_chain * 0.46 + uplift * 0.30 + ridge(plate) * 0.24,
    )
}

fn crater_like_patch(value: f32) -> f32 {
    let basin = smoothstep(0.38, 0.05, value);
    let low_scarp = smoothstep(0.16, 0.40, value) * smoothstep(0.70, 0.46, value);
    clamp01(basin * 0.62 + low_scarp * 0.22)
}

#[derive(Debug, Clone, Copy)]
struct MacroLandform {
    uplift: f32,
    basin: f32,
    undulation: f32,
}

#[derive(Debug, Clone, Copy)]
struct TectonicRidgeSystem {
    primary: f32,
    secondary: f32,
    mountain_band: f32,
    compression: f32,
}

#[derive(Debug, Clone, Copy)]
struct ValleyNetwork {
    trunk: f32,
    tributary: f32,
    floor: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct CraterAging {
    floor: f32,
    rim: f32,
    ejecta: f32,
    aged_fill: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct CachedGeologySample {
    terrain_elevation_m: f32,
    ocean_depth_m: f32,
    rift: f32,
    oceanic_rift: f32,
    continental_rift: f32,
    basin: f32,
    trench: f32,
    uplift: f32,
    volcanic_heat: f32,
    surface_heat_flow_mw_m2: f32,
}

impl CachedGeologySample {
    fn from_sample(sample: geology::GeologySample) -> Self {
        Self {
            terrain_elevation_m: sample.terrain_elevation_m,
            ocean_depth_m: sample.ocean_depth_m,
            rift: sample.rift,
            oceanic_rift: sample.oceanic_rift,
            continental_rift: sample.continental_rift,
            basin: sample.basin,
            trench: sample.trench,
            uplift: sample.uplift,
            volcanic_heat: sample.volcanic_heat,
            surface_heat_flow_mw_m2: sample.surface_heat_flow_mw_m2,
        }
    }

    fn lerp(self, rhs: Self, t: f32) -> Self {
        Self {
            terrain_elevation_m: lerp_f32(self.terrain_elevation_m, rhs.terrain_elevation_m, t),
            ocean_depth_m: lerp_f32(self.ocean_depth_m, rhs.ocean_depth_m, t),
            rift: lerp_f32(self.rift, rhs.rift, t),
            oceanic_rift: lerp_f32(self.oceanic_rift, rhs.oceanic_rift, t),
            continental_rift: lerp_f32(self.continental_rift, rhs.continental_rift, t),
            basin: lerp_f32(self.basin, rhs.basin, t),
            trench: lerp_f32(self.trench, rhs.trench, t),
            uplift: lerp_f32(self.uplift, rhs.uplift, t),
            volcanic_heat: lerp_f32(self.volcanic_heat, rhs.volcanic_heat, t),
            surface_heat_flow_mw_m2: lerp_f32(
                self.surface_heat_flow_mw_m2,
                rhs.surface_heat_flow_mw_m2,
                t,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct MapGeologyCache {
    width: usize,
    height: usize,
    samples: Vec<CachedGeologySample>,
}

impl MapGeologyCache {
    fn build(model: &geology::GeologyModel, map_width: usize, map_height: usize) -> Self {
        let width = cache_axis_size(map_width, 32, 384);
        let height = cache_axis_size(map_height, 16, 192);
        let mut samples = Vec::with_capacity(width * height);

        for y in 0..height {
            let v = y as f32 / height.saturating_sub(1).max(1) as f32;
            for x in 0..width {
                let u = x as f32 / width.max(1) as f32;
                samples.push(CachedGeologySample::from_sample(model.sample(u, v)));
            }
        }

        Self {
            width,
            height,
            samples,
        }
    }

    fn sample(&self, u: f32, v: f32) -> CachedGeologySample {
        let x = u.rem_euclid(1.0) * self.width as f32;
        let y = clamp(v, 0.0, 0.999_999) * self.height.saturating_sub(1).max(1) as f32;
        let x0 = x.floor() as usize % self.width;
        let x1 = (x0 + 1) % self.width;
        let y0 = y.floor() as usize;
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = x - x.floor();
        let ty = y - y.floor();

        let i00 = y0 * self.width + x0;
        let i10 = y0 * self.width + x1;
        let i01 = y1 * self.width + x0;
        let i11 = y1 * self.width + x1;

        self.samples[i00]
            .lerp(self.samples[i10], tx)
            .lerp(self.samples[i01].lerp(self.samples[i11], tx), ty)
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * clamp01(t)
}

fn cache_axis_size(map_len: usize, minimum: usize, maximum: usize) -> usize {
    let map_len = map_len.max(1);
    let lower = minimum.min(map_len);
    let upper = maximum.min(map_len).max(lower);
    (map_len / 4).max(1).clamp(lower, upper)
}

fn macro_landform_field(u: f32, v: f32, seed: u64) -> MacroLandform {
    let warp_a = fbm_periodic(u * 0.64 + 0.11, v * 0.50 - 0.07, 5, 5, seed + 35_101, 0.58);
    let warp_b = fbm_periodic(u * 0.58 - 0.09, v * 0.56 + 0.13, 7, 5, seed + 35_219, 0.56);
    let curve_u = u + (warp_a - 0.5) * 0.16 + (warp_b - 0.5) * 0.045 * (v - 0.5);
    let curve_v = v + (warp_b - 0.5) * 0.12 - (warp_a - 0.5) * 0.040 * (u - 0.5);
    let broad = fbm_periodic(
        curve_u * 0.82 + 0.04,
        curve_v * 0.68 - 0.02,
        4,
        5,
        seed + 35_337,
        0.58,
    );
    let folded = fbm_periodic(
        curve_u * 1.36 + broad * 0.18,
        curve_v * 1.10 - broad * 0.13,
        9,
        4,
        seed + 35_509,
        0.53,
    );
    let lobe = fbm_periodic(
        curve_u * 2.25 + folded * 0.11,
        curve_v * 1.75 - broad * 0.08,
        17,
        3,
        seed + 35_617,
        0.49,
    );

    MacroLandform {
        uplift: clamp01(broad * 0.54 + folded * 0.32 + ridge(lobe) * 0.14),
        basin: clamp01((1.0 - broad) * 0.50 + (1.0 - folded) * 0.32 + lobe * 0.18),
        undulation: clamp01(folded * 0.52 + lobe * 0.30 + ridge(warp_a) * 0.18),
    }
}

fn curved_directional_band(
    u: f32,
    v: f32,
    seed: u64,
    angle: f32,
    frequency: f32,
    bend: f32,
) -> f32 {
    let warp_a = fbm_periodic(u * 0.92 + 0.05, v * 0.74 - 0.03, 7, 4, seed + 36_101, 0.55);
    let warp_b = fbm_periodic(u * 0.76 - 0.08, v * 0.88 + 0.04, 11, 4, seed + 36_211, 0.53);
    let uu = u + (warp_a - 0.5) * 0.12;
    let vv = v + (warp_b - 0.5) * 0.10;
    let dx = angle.cos();
    let dy = angle.sin();
    let lon = uu.rem_euclid(1.0) * PI * 2.0;
    let ring_x = lon.cos();
    let ring_y = lon.sin();
    let lat_axis = (clamp01(vv) - 0.5) * 2.0;
    let along = (ring_x * dx + ring_y * dy) * 0.44 + lat_axis * (0.54 + dx.abs() * 0.10);
    let across = (ring_x * -dy + ring_y * dx) * 0.52 + lat_axis * dy * 0.36;
    let meander = (fbm_periodic(
        uu * 1.35 + along * 0.16,
        vv * 1.15 - across * 0.12,
        13,
        4,
        seed + 36_347,
        0.52,
    ) - 0.5)
        * bend;
    let pinch = fbm_periodic(
        uu * 2.30 + meander * 0.18,
        vv * 1.85 - along * 0.08,
        23,
        3,
        seed + 36_503,
        0.49,
    );
    let phase = (across + meander + (pinch - 0.5) * 0.040) * PI * 2.0 * frequency
        + along * (0.70 + bend * 0.28);
    let band = ridge(phase.sin() * 0.5 + 0.5);
    let broken = band * (0.54 + pinch * 0.24)
        + ridge(pinch) * 0.17
        + pinch * 0.08
        + (pinch - 0.5).abs() * 0.07;
    clamp01(smoothstep(0.08, 0.96, broken) * (0.58 + ridge(pinch) * 0.28 + pinch * 0.14))
}

fn tectonic_ridge_system(
    u: f32,
    v: f32,
    seed: u64,
    plate: f32,
    uplift: f32,
) -> TectonicRidgeSystem {
    let angle = hash2(127, 311, seed) * PI * 2.0;
    let primary_band = curved_directional_band(
        u,
        v,
        seed + 36_701,
        angle,
        4.4 + hash2(17, 97, seed) * 2.2,
        0.34,
    );
    let secondary_band = curved_directional_band(
        u,
        v,
        seed + 36_907,
        angle + 0.92 + hash2(41, 157, seed) * 0.58,
        7.0 + hash2(23, 131, seed) * 3.0,
        0.24,
    );
    let compression = fbm_periodic(
        u * 1.18 + primary_band * 0.12,
        v * 0.96 - secondary_band * 0.08,
        11,
        4,
        seed + 37_013,
        0.52,
    );
    let mountain_band = smoothstep(
        0.52,
        0.96,
        primary_band * 0.38 + plate * 0.31 + uplift * 0.21 + compression * 0.10,
    ) * smoothstep(0.18, 0.90, compression);

    TectonicRidgeSystem {
        primary: smoothstep(
            0.58,
            0.985,
            primary_band * 0.50 + plate * 0.28 + compression * 0.17 + uplift * 0.05,
        ),
        secondary: smoothstep(
            0.66,
            0.99,
            secondary_band * 0.50 + ridge(compression) * 0.26 + plate * 0.16 + uplift * 0.08,
        ),
        mountain_band,
        compression,
    }
}

fn valley_network_field(u: f32, v: f32, seed: u64, basin: f32, uplift: f32) -> ValleyNetwork {
    let angle = hash2(503, 761, seed) * PI * 2.0;
    let trunk_band = curved_directional_band(
        u,
        v,
        seed + 37_229,
        angle + PI * 0.5,
        3.6 + hash2(67, 173, seed) * 1.8,
        0.42,
    );
    let drainage = fbm_periodic(
        u * 1.95 + trunk_band * 0.15,
        v * 1.60 - basin * 0.10,
        17,
        4,
        seed + 37_337,
        0.52,
    );
    let tributary_noise = fbm_periodic(
        u * 3.55 + drainage * 0.11,
        v * 2.90 - trunk_band * 0.09,
        31,
        3,
        seed + 37_451,
        0.48,
    );
    let trunk = smoothstep(
        0.58,
        0.96,
        trunk_band * 0.44 + ridge(drainage) * 0.34 + basin * 0.18 + (1.0 - uplift) * 0.04,
    );
    let tributary = smoothstep(
        0.61,
        0.985,
        ridge(tributary_noise) * 0.46 + trunk * 0.22 + basin * 0.18 + drainage * 0.14,
    );
    let floor = clamp01((trunk * 0.72 + tributary * 0.44) * (0.58 + basin * 0.34));

    ValleyNetwork {
        trunk,
        tributary,
        floor,
    }
}

fn crater_aging_field(u: f32, v: f32, seed: u64, erosion: f32) -> CraterAging {
    let mut craters = CraterAging::default();
    for layer in 0..3 {
        let (cells_x, cells_y, radius, density) = match layer {
            0 => (8, 4, 0.46, 0.44),
            1 => (15, 8, 0.40, 0.34),
            _ => (29, 14, 0.34, 0.22),
        };
        let layer_seed = seed + 38_003 + layer as u64 * 547;
        let grid_x = u.rem_euclid(1.0) * cells_x as f32;
        let grid_y = clamp01(v) * cells_y as f32;
        let ix = grid_x.floor() as i32;
        let iy = grid_y.floor() as i32;

        for oy in -1..=1 {
            let cy = iy + oy;
            if cy < 0 || cy >= cells_y {
                continue;
            }
            for ox in -1..=1 {
                let cx = (ix + ox).rem_euclid(cells_x);
                let present = hash2(cx * 73 + layer * 11, cy * 97 - layer * 7, layer_seed);
                if present > density {
                    continue;
                }

                let center_u =
                    (cx as f32 + 0.5 + (hash2(cx * 31 + 5, cy * 41 + 7, layer_seed) - 0.5) * 0.72)
                        / cells_x as f32;
                let center_v = (cy as f32
                    + 0.5
                    + (hash2(cx * 43 + 13, cy * 37 + 17, layer_seed) - 0.5) * 0.56)
                    / cells_y as f32;
                let local_u = ((u - center_u + 0.5).rem_euclid(1.0) - 0.5) * cells_x as f32;
                let local_v = (v - center_v) * cells_y as f32;
                let angle = hash2(cx * 59 + 19, cy * 67 + 23, layer_seed) * PI * 2.0;
                let aspect = 0.82 + hash2(cx * 71 + 29, cy * 79 + 31, layer_seed) * 0.34;
                let rx = (local_u * angle.cos() + local_v * angle.sin()) / (radius * aspect);
                let ry = (-local_u * angle.sin() + local_v * angle.cos()) / radius;
                let distance = (rx * rx + ry * ry).sqrt();
                let age =
                    clamp01(hash2(cx * 83 + 37, cy * 89 + 41, layer_seed) * 0.72 + erosion * 0.28);
                let rim_variation = 0.76 + hash2(cx * 97 + 43, cy * 101 + 47, layer_seed) * 0.42;
                let floor = smoothstep(0.84, 0.12, distance);
                let rim = smoothstep(1.16, 0.86, distance) * smoothstep(0.62, 1.0, distance);
                let ejecta = smoothstep(1.78, 0.96, distance) * smoothstep(0.88, 1.24, distance);
                let old = smoothstep(0.42, 0.94, age);
                let fresh = 1.0 - old * 0.72;

                craters.floor = craters.floor.max(floor * (0.54 + fresh * 0.46));
                craters.rim = craters.rim.max(rim * rim_variation * (0.30 + fresh * 0.70));
                craters.ejecta = craters.ejecta.max(ejecta * (0.20 + fresh * 0.80));
                craters.aged_fill = craters.aged_fill.max(floor * old);
            }
        }
    }
    craters.floor = clamp01(craters.floor);
    craters.rim = clamp01(craters.rim);
    craters.ejecta = clamp01(craters.ejecta);
    craters.aged_fill = clamp01(craters.aged_fill);
    craters
}

fn volcanic_lava_flow_path(
    u: f32,
    v: f32,
    seed: u64,
    hotspot: f32,
    ridge_mask: f32,
    valley_floor: f32,
    caldera_floor: f32,
) -> f32 {
    let angle = hash2(907, 1_019, seed) * PI * 2.0;
    let primary = curved_directional_band(
        u + hotspot * 0.018,
        v - caldera_floor * 0.015,
        seed + 39_101,
        angle,
        9.5 + hash2(109, 211, seed) * 4.0,
        0.18,
    );
    let branching = curved_directional_band(
        u - valley_floor * 0.012,
        v + hotspot * 0.014,
        seed + 39_313,
        angle + 0.58,
        15.0 + hash2(127, 223, seed) * 6.0,
        0.12,
    );
    let cooled_edges = fbm_periodic(
        u * 5.0 + primary * 0.10,
        v * 4.2 - branching * 0.07,
        53,
        3,
        seed + 39_467,
        0.47,
    );
    smoothstep(
        0.60,
        0.985,
        primary * 0.34
            + branching * 0.18
            + hotspot * 0.24
            + ridge_mask * 0.11
            + valley_floor * 0.09
            + caldera_floor * 0.04
            + ridge(cooled_edges) * 0.08,
    )
}

fn sediment_basin_field(
    u: f32,
    v: f32,
    seed: u64,
    basin: f32,
    valley_floor: f32,
    shoreline_transition: f32,
) -> f32 {
    let alluvial = fbm_periodic(
        u * 2.25 + valley_floor * 0.13,
        v * 1.75 - basin * 0.09,
        19,
        4,
        seed + 40_101,
        0.51,
    );
    let fan = fbm_periodic(
        u * 4.4 + alluvial * 0.08,
        v * 3.3 - valley_floor * 0.06,
        41,
        3,
        seed + 40_257,
        0.48,
    );
    clamp01(
        smoothstep(
            0.42,
            0.94,
            basin * 0.36
                + valley_floor * 0.28
                + shoreline_transition * 0.22
                + alluvial * 0.10
                + (1.0 - ridge(fan)) * 0.04,
        ) * (0.74 + alluvial * 0.20 + fan * 0.06),
    )
}

fn profile_key_contains_any(profile: &PlanetVisualProfile, fragments: &[&str]) -> bool {
    fragments
        .iter()
        .any(|fragment| profile_key_contains(profile, fragment))
}

fn profile_key_contains(profile: &PlanetVisualProfile, fragment: &str) -> bool {
    profile.archetype_key.contains(fragment)
        || profile.class_key.contains(fragment)
        || profile.planet_class.contains(fragment)
        || profile.palette.contains(fragment)
        || profile.render_model.contains(fragment)
        || profile
            .modifier_keys
            .iter()
            .any(|modifier| modifier.contains(fragment))
}

fn key_is_any(value: &str, keys: &[&str]) -> bool {
    keys.iter().any(|key| value == *key)
}

fn profile_sharpen_amount(
    profile: &PlanetVisualProfile,
    scale: u32,
    supersampled: f32,
    native: f32,
) -> f32 {
    let base = if scale > 1 { supersampled } else { native };
    let ocean_softening = if is_ocean_world_profile(profile) {
        0.60
    } else {
        smoothstep(0.62, 0.94, profile.ocean_fraction) * 0.28
    };
    let rocky_softening = if is_rocky_world_profile(profile) {
        0.48
    } else {
        0.0
    };
    base * (1.0 - ocean_softening.max(rocky_softening) * (1.0 - profile.ice_fraction * 0.18))
}

#[derive(Debug, Clone, Copy)]
struct GasGiantMaterial {
    zone: Vec3,
    zone_light: Vec3,
    belt: Vec3,
    belt_dark: Vec3,
    plume: Vec3,
    polar_haze: Vec3,
    storm: Vec3,
    storm_eye: Vec3,
    limb_haze: Vec3,
    lightning: Vec3,
    band_count: f32,
    band_jitter: f32,
    shear_strength: f32,
    filament_strength: f32,
    storm_strength: f32,
    storm_count: i32,
    storm_radius_scale: f32,
    cloud_floor: f32,
    cloud_ceiling: f32,
    high_cloud_strength: f32,
    contrast: f32,
    specular_strength: f32,
    thermal_glow: f32,
    methane_absorption: f32,
    diamond_sparkle: f32,
}

fn gas_giant_material(profile: &PlanetVisualProfile) -> GasGiantMaterial {
    let sulfur = profile_key_contains_any(profile, &["sulfur-gas", "sulfur", "acid-cloud"]);
    let hot = profile.temperature_c > 260
        || profile_key_contains_any(
            profile,
            &["hot-jupiter", "hot-neptune", "puffy", "close-in"],
        );
    let methane_ice = profile_key_contains_any(
        profile,
        &[
            "ice-giant",
            "blue-ice",
            "methane",
            "neptune",
            "uranus",
            "mini-neptune",
            "sub-neptune",
            "gas-dwarf",
            "hot-neptune",
        ],
    );
    let helium = profile_key_contains_any(profile, &["helium"]);
    let puffy = profile_key_contains_any(profile, &["puffy", "low-density"]);
    let saturn = profile_key_contains_any(profile, &["saturn"]);
    let cold_jupiter = profile_key_contains_any(profile, &["cold-jupiter", "cold-ammonia"]);
    let rogue = profile_key_contains_any(profile, &["rogue-gas", "rogue"]);
    let diamond = profile_key_contains_any(profile, &["diamond-rain", "diamond", "carbon-rain"]);
    let stormy = profile_key_contains_any(profile, &["storm", "jupiter", "lightning", "vortex"])
        || profile.cloud_density > 0.94;

    let mut material = if sulfur {
        GasGiantMaterial {
            zone: Vec3::new(1.04, 0.86, 0.28),
            zone_light: Vec3::new(1.25, 1.08, 0.54),
            belt: Vec3::new(0.74, 0.34, 0.070),
            belt_dark: Vec3::new(0.24, 0.090, 0.020),
            plume: Vec3::new(1.32, 1.16, 0.62),
            polar_haze: Vec3::new(0.90, 0.66, 0.28),
            storm: Vec3::new(1.02, 0.44, 0.055),
            storm_eye: Vec3::new(1.38, 1.08, 0.42),
            limb_haze: Vec3::new(1.0, 0.66, 0.18),
            lightning: Vec3::new(1.0, 0.95, 0.56),
            band_count: 18.0,
            band_jitter: 7.0,
            shear_strength: 1.25,
            filament_strength: 1.28,
            storm_strength: 1.38,
            storm_count: 6,
            storm_radius_scale: 1.08,
            cloud_floor: 0.12,
            cloud_ceiling: 0.62,
            high_cloud_strength: 1.25,
            contrast: 1.34,
            specular_strength: 1.12,
            thermal_glow: if hot { 0.20 } else { 0.04 },
            methane_absorption: 0.0,
            diamond_sparkle: 0.0,
        }
    } else if methane_ice {
        GasGiantMaterial {
            zone: Vec3::new(0.20, 0.58, 0.88),
            zone_light: Vec3::new(0.58, 0.96, 1.20),
            belt: Vec3::new(0.070, 0.25, 0.58),
            belt_dark: Vec3::new(0.010, 0.045, 0.18),
            plume: Vec3::new(0.78, 1.05, 1.22),
            polar_haze: Vec3::new(0.46, 0.84, 1.02),
            storm: Vec3::new(0.030, 0.075, 0.25),
            storm_eye: Vec3::new(0.60, 0.94, 1.15),
            limb_haze: Vec3::new(0.20, 0.64, 1.12),
            lightning: Vec3::new(0.72, 0.92, 1.0),
            band_count: if profile_key_contains_any(profile, &["uranus"]) {
                9.0
            } else {
                13.0
            },
            band_jitter: if stormy { 6.0 } else { 3.0 },
            shear_strength: if stormy { 1.24 } else { 0.70 },
            filament_strength: if stormy { 1.20 } else { 0.72 },
            storm_strength: if stormy { 1.30 } else { 0.64 },
            storm_count: if stormy { 5 } else { 2 },
            storm_radius_scale: if stormy { 1.05 } else { 0.82 },
            cloud_floor: 0.08,
            cloud_ceiling: if stormy { 0.50 } else { 0.34 },
            high_cloud_strength: if stormy { 1.10 } else { 0.72 },
            contrast: if stormy { 1.18 } else { 0.82 },
            specular_strength: 1.05,
            thermal_glow: if hot { 0.16 } else { 0.0 },
            methane_absorption: 0.42,
            diamond_sparkle: if diamond { 0.32 } else { 0.0 },
        }
    } else if saturn {
        GasGiantMaterial {
            zone: Vec3::new(0.98, 0.80, 0.42),
            zone_light: Vec3::new(1.28, 1.08, 0.66),
            belt: Vec3::new(0.78, 0.52, 0.20),
            belt_dark: Vec3::new(0.20, 0.12, 0.050),
            plume: Vec3::new(1.18, 1.05, 0.82),
            polar_haze: Vec3::new(0.78, 0.70, 0.56),
            storm: Vec3::new(0.54, 0.39, 0.24),
            storm_eye: Vec3::new(1.04, 0.92, 0.68),
            limb_haze: Vec3::new(0.96, 0.82, 0.54),
            lightning: Vec3::new(1.0, 0.88, 0.60),
            band_count: 10.0,
            band_jitter: 3.0,
            shear_strength: 0.54,
            filament_strength: 0.50,
            storm_strength: 0.46,
            storm_count: 1,
            storm_radius_scale: 0.72,
            cloud_floor: 0.045,
            cloud_ceiling: 0.24,
            high_cloud_strength: 0.46,
            contrast: 0.98,
            specular_strength: 0.72,
            thermal_glow: 0.0,
            methane_absorption: 0.0,
            diamond_sparkle: 0.0,
        }
    } else if cold_jupiter {
        GasGiantMaterial {
            zone: Vec3::new(0.78, 0.84, 0.86),
            zone_light: Vec3::new(1.05, 1.06, 0.92),
            belt: Vec3::new(0.50, 0.58, 0.70),
            belt_dark: Vec3::new(0.16, 0.24, 0.38),
            plume: Vec3::new(1.05, 1.05, 0.94),
            polar_haze: Vec3::new(0.56, 0.66, 0.82),
            storm: Vec3::new(0.28, 0.40, 0.62),
            storm_eye: Vec3::new(0.92, 0.96, 0.92),
            limb_haze: Vec3::new(0.52, 0.70, 0.98),
            lightning: Vec3::new(0.76, 0.88, 1.0),
            band_count: 17.0,
            band_jitter: 5.0,
            shear_strength: 1.10,
            filament_strength: 0.92,
            storm_strength: 1.06,
            storm_count: 4,
            storm_radius_scale: 0.92,
            cloud_floor: 0.08,
            cloud_ceiling: 0.42,
            high_cloud_strength: 0.94,
            contrast: 1.04,
            specular_strength: 0.95,
            thermal_glow: 0.0,
            methane_absorption: 0.14,
            diamond_sparkle: 0.0,
        }
    } else if helium {
        GasGiantMaterial {
            zone: Vec3::new(0.82, 0.82, 0.78),
            zone_light: Vec3::new(1.10, 1.08, 0.98),
            belt: Vec3::new(0.54, 0.50, 0.43),
            belt_dark: Vec3::new(0.22, 0.21, 0.19),
            plume: Vec3::new(1.12, 1.10, 1.02),
            polar_haze: Vec3::new(0.70, 0.73, 0.76),
            storm: Vec3::new(0.62, 0.52, 0.42),
            storm_eye: Vec3::new(1.0, 0.96, 0.82),
            limb_haze: Vec3::new(0.60, 0.72, 0.92),
            lightning: Vec3::new(0.86, 0.92, 1.0),
            band_count: 12.0,
            band_jitter: 4.0,
            shear_strength: 0.72,
            filament_strength: 0.62,
            storm_strength: 0.62,
            storm_count: 2,
            storm_radius_scale: 0.82,
            cloud_floor: 0.05,
            cloud_ceiling: 0.26,
            high_cloud_strength: 0.55,
            contrast: 0.78,
            specular_strength: 0.80,
            thermal_glow: 0.0,
            methane_absorption: 0.0,
            diamond_sparkle: 0.0,
        }
    } else if puffy {
        GasGiantMaterial {
            zone: Vec3::new(0.96, 0.58, 0.48),
            zone_light: Vec3::new(1.28, 0.90, 0.72),
            belt: Vec3::new(0.62, 0.24, 0.25),
            belt_dark: Vec3::new(0.20, 0.070, 0.12),
            plume: Vec3::new(1.28, 0.98, 0.80),
            polar_haze: Vec3::new(0.92, 0.54, 0.66),
            storm: Vec3::new(0.98, 0.30, 0.24),
            storm_eye: Vec3::new(1.24, 0.82, 0.58),
            limb_haze: Vec3::new(1.0, 0.46, 0.34),
            lightning: Vec3::new(1.0, 0.72, 0.55),
            band_count: 8.0,
            band_jitter: 4.0,
            shear_strength: 0.78,
            filament_strength: 0.64,
            storm_strength: 0.72,
            storm_count: 2,
            storm_radius_scale: 1.18,
            cloud_floor: 0.14,
            cloud_ceiling: 0.50,
            high_cloud_strength: 0.72,
            contrast: 0.88,
            specular_strength: 0.82,
            thermal_glow: 0.34,
            methane_absorption: 0.0,
            diamond_sparkle: if diamond { 0.36 } else { 0.0 },
        }
    } else if hot {
        GasGiantMaterial {
            zone: Vec3::new(0.80, 0.42, 0.31),
            zone_light: Vec3::new(1.12, 0.78, 0.52),
            belt: Vec3::new(0.45, 0.105, 0.090),
            belt_dark: Vec3::new(0.095, 0.028, 0.060),
            plume: Vec3::new(1.25, 0.82, 0.48),
            polar_haze: Vec3::new(0.52, 0.28, 0.32),
            storm: Vec3::new(1.20, 0.28, 0.10),
            storm_eye: Vec3::new(1.24, 0.86, 0.44),
            limb_haze: Vec3::new(1.0, 0.30, 0.11),
            lightning: Vec3::new(1.0, 0.72, 0.42),
            band_count: 20.0,
            band_jitter: 8.0,
            shear_strength: 1.46,
            filament_strength: 1.18,
            storm_strength: 1.06,
            storm_count: 4,
            storm_radius_scale: 0.96,
            cloud_floor: 0.08,
            cloud_ceiling: 0.46,
            high_cloud_strength: 0.90,
            contrast: 1.26,
            specular_strength: 0.92,
            thermal_glow: 0.50,
            methane_absorption: 0.0,
            diamond_sparkle: if diamond { 0.36 } else { 0.0 },
        }
    } else if rogue {
        GasGiantMaterial {
            zone: Vec3::new(0.16, 0.12, 0.25),
            zone_light: Vec3::new(0.38, 0.28, 0.46),
            belt: Vec3::new(0.20, 0.095, 0.14),
            belt_dark: Vec3::new(0.035, 0.020, 0.055),
            plume: Vec3::new(0.42, 0.32, 0.52),
            polar_haze: Vec3::new(0.14, 0.16, 0.28),
            storm: Vec3::new(0.42, 0.20, 0.18),
            storm_eye: Vec3::new(0.62, 0.42, 0.38),
            limb_haze: Vec3::new(0.28, 0.34, 0.76),
            lightning: Vec3::new(0.60, 0.74, 1.0),
            band_count: 14.0,
            band_jitter: 5.0,
            shear_strength: 0.98,
            filament_strength: 0.92,
            storm_strength: 0.88,
            storm_count: 3,
            storm_radius_scale: 0.92,
            cloud_floor: 0.04,
            cloud_ceiling: 0.34,
            high_cloud_strength: 0.78,
            contrast: 1.18,
            specular_strength: 0.70,
            thermal_glow: 0.28,
            methane_absorption: 0.18,
            diamond_sparkle: 0.0,
        }
    } else {
        GasGiantMaterial {
            zone: Vec3::new(0.88, 0.76, 0.50),
            zone_light: Vec3::new(1.12, 1.02, 0.80),
            belt: Vec3::new(0.70, 0.39, 0.20),
            belt_dark: Vec3::new(0.27, 0.14, 0.080),
            plume: Vec3::new(1.0, 0.93, 0.78),
            polar_haze: Vec3::new(0.58, 0.50, 0.42),
            storm: Vec3::new(0.80, 0.36, 0.20),
            storm_eye: Vec3::new(0.98, 0.84, 0.60),
            limb_haze: Vec3::new(0.92, 0.78, 0.54),
            lightning: Vec3::new(1.0, 0.88, 0.62),
            band_count: 15.0,
            band_jitter: 6.0,
            shear_strength: 1.0,
            filament_strength: 1.0,
            storm_strength: if stormy { 1.18 } else { 0.92 },
            storm_count: if stormy { 5 } else { 3 },
            storm_radius_scale: if stormy { 1.04 } else { 0.94 },
            cloud_floor: 0.04,
            cloud_ceiling: if stormy { 0.44 } else { 0.30 },
            high_cloud_strength: if stormy { 1.16 } else { 0.90 },
            contrast: if stormy { 1.20 } else { 1.0 },
            specular_strength: 1.0,
            thermal_glow: 0.0,
            methane_absorption: 0.0,
            diamond_sparkle: if diamond { 0.30 } else { 0.0 },
        }
    };

    material.zone = gas_color_variant(material.zone, profile.seed, 711, 0.12);
    material.zone_light = gas_color_variant(material.zone_light, profile.seed, 719, 0.10);
    material.belt = gas_color_variant(material.belt, profile.seed, 727, 0.16);
    material.belt_dark = gas_color_variant(material.belt_dark, profile.seed, 733, 0.14);
    material.plume = gas_color_variant(material.plume, profile.seed, 739, 0.08);
    material.polar_haze = gas_color_variant(material.polar_haze, profile.seed, 743, 0.10);
    material.storm = gas_color_variant(material.storm, profile.seed, 751, 0.14);
    material.storm_eye = gas_color_variant(material.storm_eye, profile.seed, 757, 0.10);

    if stormy {
        material.storm_strength *= 1.14;
        material.high_cloud_strength *= 1.10;
        material.contrast *= 1.08;
    }
    if diamond {
        material.diamond_sparkle = material.diamond_sparkle.max(0.28);
        material.specular_strength *= 1.16;
    }

    material
}

fn gas_color_variant(color: Vec3, seed: u64, salt: i32, amount: f32) -> Vec3 {
    Vec3::new(
        color.x * (1.0 + (hash2(salt, 11, seed) - 0.5) * amount),
        color.y * (1.0 + (hash2(salt, 23, seed) - 0.5) * amount),
        color.z * (1.0 + (hash2(salt, 37, seed) - 0.5) * amount),
    )
}

fn gas_giant_map_sample(
    profile: &PlanetVisualProfile,
    material: GasGiantMaterial,
    u: f32,
    v: f32,
) -> SurfaceSample {
    let lat_signed = (v - 0.5) * 2.0;
    let lat = lat_signed.abs();
    let seed_jitter = hash2(11, 23, profile.seed);
    let shear = fbm_periodic(
        u + lat_signed * 0.035 * material.shear_strength,
        v * 0.82 + seed_jitter * 0.07,
        9,
        4,
        profile.seed + 7_101,
        0.54,
    );
    let wave_u =
        (u + (shear - 0.5) * (0.045 + lat * 0.030) * material.shear_strength).rem_euclid(1.0);
    let rolling = fbm_periodic(wave_u, v, 24, 5, profile.seed + 7_203, 0.50);
    let fine = fbm_periodic(
        (wave_u + rolling * 0.030).rem_euclid(1.0),
        v,
        74,
        3,
        profile.seed + 7_307,
        0.48,
    );
    let micro = fbm_periodic(
        (wave_u + fine * 0.035).rem_euclid(1.0),
        v * 1.15 + rolling * 0.020,
        132,
        3,
        profile.seed + 7_409,
        0.44,
    );

    let band_count = material.band_count + seed_jitter * material.band_jitter;
    let phase = v * PI * band_count
        + (rolling - 0.5) * 2.6 * material.shear_strength
        + lat_signed * shear * 0.55;
    let broad = phase.sin() * 0.5 + 0.5;
    let narrow = (phase * 2.85 + fine * 2.2).sin() * 0.5 + 0.5;
    let filament = ((u * 34.0 + v * 21.0 + rolling * 4.2).sin() * 0.5 + 0.5)
        * smoothstep(0.22, 0.92, narrow)
        * material.filament_strength;
    let belt = clamp01(
        broad * (0.52 + material.contrast * 0.040)
            + narrow * 0.30
            + rolling * 0.12
            + (micro - 0.5) * 0.060,
    );

    let zone = material.zone.lerp(material.zone_light, broad);
    let belt_color = material.belt_dark.lerp(material.belt, rolling);
    let storm = gas_giant_vortex_mask(
        u,
        v,
        profile.seed,
        material.storm_count,
        material.storm_radius_scale,
    );
    let storm_eye = gas_giant_vortex_mask(
        (u + 0.018 * (phase * 0.7).sin()).rem_euclid(1.0),
        v,
        profile.seed + 17,
        (material.storm_count - 1).max(1),
        material.storm_radius_scale * 0.70,
    );
    let storm_cells = gas_giant_storm_cell_mask(
        (wave_u + shear * 0.020).rem_euclid(1.0),
        v,
        profile.seed + 29,
        material.storm_strength,
    );

    let belt_mix = smoothstep(0.30, 0.86, belt);
    let mut albedo = zone
        .lerp(
            belt_color,
            clamp01(belt_mix * (0.80 + material.contrast * 0.16)),
        )
        .lerp(
            material.plume,
            clamp01(filament * 0.20 + storm_cells * 0.18),
        )
        .lerp(material.polar_haze, smoothstep(0.74, 1.0, lat) * 0.35);
    albedo = albedo
        .lerp(
            material.storm,
            clamp01(storm * 0.50 * material.storm_strength),
        )
        .lerp(
            material.storm_eye,
            clamp01(storm_eye * 0.28 * material.storm_strength),
        );

    if material.methane_absorption > 0.0 {
        let absorption =
            smoothstep(0.44, 0.96, belt * 0.72 + narrow * 0.28) * material.methane_absorption;
        albedo = albedo.lerp(Vec3::new(0.010, 0.055, 0.22), absorption * 0.24);
    }
    if material.thermal_glow > 0.0 {
        let equatorial_heat =
            (1.0 - smoothstep(0.18, 0.88, lat)) * smoothstep(0.48, 0.94, belt * 0.54 + fine * 0.46);
        albedo += Vec3::new(1.18, 0.23, 0.055) * equatorial_heat * material.thermal_glow * 0.28;
    }
    if material.diamond_sparkle > 0.0 {
        let sparkle = smoothstep(0.965, 0.998, micro)
            * smoothstep(0.20, 0.92, narrow)
            * (1.0 - smoothstep(0.76, 1.0, lat))
            * material.diamond_sparkle;
        albedo += Vec3::new(1.15, 1.25, 1.38) * sparkle;
    }
    if storm_cells > 0.12 {
        albedo += material.lightning * storm_cells * material.storm_strength * 0.085;
    }

    let band_relief = (belt - 0.5) * (0.085 + material.contrast * 0.030)
        + (filament - 0.5) * 0.030
        + storm * 0.060 * material.storm_strength
        + storm_cells * 0.040;
    let high_cloud = (smoothstep(0.68, 0.94, fine) * 0.12 + storm * 0.14 + storm_cells * 0.16)
        * material.high_cloud_strength;

    SurfaceSample {
        albedo,
        height: clamp01(0.52 + band_relief),
        water: 0.0,
        cloud: clamp(
            material.cloud_floor + profile.cloud_density * 0.10 + high_cloud,
            material.cloud_floor,
            material.cloud_ceiling,
        ),
        city: 0.0,
        vegetation: 0.0,
        biome: 0.0,
        roughness: clamp(
            0.14 + material.storm_strength * 0.025 + material.contrast * 0.020,
            0.12,
            0.32,
        ),
        wetness: 0.0,
        ambient_occlusion: 0.0,
        horizon_occlusion: 0.0,
    }
}

fn gas_giant_vortex_mask(u: f32, v: f32, seed: u64, count: i32, radius_scale: f32) -> f32 {
    let mut mask: f32 = 0.0;
    for index in 0..count.clamp(1, 8) {
        let center_u = hash2(71 + index * 17, 83 + index * 11, seed);
        let hemisphere = if hash2(19 + index, 41 + index, seed) > 0.5 {
            1.0
        } else {
            -1.0
        };
        let center_v = 0.5 + hemisphere * (0.10 + hash2(37 + index, 97, seed) * 0.23);
        let radius_u = (0.060 + hash2(101 + index, 7, seed) * 0.055) * radius_scale;
        let radius_v = (0.020 + hash2(3, 131 + index, seed) * 0.025) * radius_scale;
        let dx = ((u - center_u + 0.5).rem_euclid(1.0) - 0.5) / radius_u;
        let dy = (v - center_v) / radius_v;
        let d = dx * dx + dy * dy;
        let oval = smoothstep(1.35, 0.12, d);
        let swirl = ((dx.atan2(dy) * 3.0 + d.sqrt() * 8.0).sin() * 0.5 + 0.5) * oval;
        mask = mask.max(oval * 0.72 + swirl * 0.28);
    }
    mask
}

fn gas_giant_storm_cell_mask(u: f32, v: f32, seed: u64, intensity: f32) -> f32 {
    let shear = fbm_periodic(u * 1.6, v * 0.92, 17, 4, seed + 8_101, 0.52);
    let cell = fbm_periodic(
        (u + shear * 0.026).rem_euclid(1.0),
        v * 1.10 - shear * 0.020,
        86,
        3,
        seed + 8_211,
        0.46,
    );
    let tower = fbm_periodic(
        (u - shear * 0.018).rem_euclid(1.0),
        v * 1.24 + cell * 0.020,
        144,
        2,
        seed + 8_307,
        0.42,
    );
    smoothstep(0.70, 0.985, cell * 0.72 + tower * 0.28) * clamp01(0.44 + intensity * 0.42)
}

fn ocean_cyclone_cloud_mask(u: f32, v: f32, seed: u64) -> f32 {
    let mut mask: f32 = 0.0;
    for index in 0..4 {
        let ii = index as i32;
        let center_u = hash2(113 + ii * 29, 191 + ii * 17, seed);
        let hemisphere = if hash2(223 + ii, 97 + ii * 3, seed) > 0.5 {
            1.0
        } else {
            -1.0
        };
        let center_v = 0.5 + hemisphere * (0.08 + hash2(31 + ii * 7, 151, seed) * 0.34);
        let radius_u = 0.050 + hash2(47 + ii, 61 + ii * 11, seed) * 0.070;
        let radius_v = 0.034 + hash2(71 + ii * 13, 83 + ii, seed) * 0.052;
        let dx = ((u - center_u + 0.5).rem_euclid(1.0) - 0.5) / radius_u;
        let dy = (v - center_v) / radius_v;
        let d = (dx * dx + dy * dy).sqrt();
        let core = smoothstep(1.28, 0.18, d);
        let angle = dy.atan2(dx);
        let spiral = ((angle * 3.0 + d * 10.0 + hash2(ii * 13, ii * 17, seed) * PI).sin() * 0.5
            + 0.5)
            * smoothstep(1.22, 0.20, d);
        let eye = 1.0 - smoothstep(0.10, 0.30, d);
        mask = mask.max((core * 0.48 + spiral * 0.52) * (1.0 - eye * 0.72));
    }
    clamp01(mask)
}

impl PlanetMaps {
    fn generate(profile: &PlanetVisualProfile, width: usize, height: usize) -> Self {
        let style = planet_render_style(profile);
        let rocky_material = rocky_surface_material(profile);
        let rocky_palette = rocky_surface_palette(profile, rocky_material);
        let gas_material = if matches!(style, PlanetRenderStyle::GasGiant) {
            Some(gas_giant_material(profile))
        } else {
            None
        };
        let physics = PlanetPhysicsModel::from_profile(profile);
        let geology_model = geology::GeologyModel::from_profile(profile);
        let geology_cache = MapGeologyCache::build(&geology_model, width, height);
        let len = width * height;
        let mut albedo = vec![Vec3::ZERO; len];
        let mut height_map = vec![0.0; len];
        let mut water = vec![0.0; len];
        let mut clouds = vec![0.0; len];
        let mut cities = vec![0.0; len];
        let mut vegetation = vec![0.0; len];
        let mut biome = vec![0.0; len];
        let mut roughness = vec![0.0; len];
        let mut wetness = vec![0.0; len];

        for y in 0..height {
            let v = y as f32 / (height - 1) as f32;
            let lat_signed = (v - 0.5) * 2.0;
            let lat = lat_signed.abs();
            let polar = smoothstep(0.62, 0.95, lat);
            let temp = clamp01(1.0 - lat * 1.25);
            let wind = (v * PI * 13.0 + (v * PI * 3.0).sin() * 0.8).sin();

            for x in 0..width {
                let u = x as f32 / width as f32;
                let i = y * width + x;

                let continents = fbm_periodic(u, v, 6, 7, profile.seed + 101, 0.54);
                let tectonic = fbm_periodic(u, v, 20, 6, profile.seed + 211, 0.50);
                let moisture = fbm_periodic(u, v, 10, 5, profile.seed + 307, 0.55);
                let cloud_base = fbm_periodic(u, v, 12, 6, profile.seed + 409, 0.56);
                let cloud_detail = fbm_periodic(u, v, 42, 4, profile.seed + 503, 0.48);
                let city_noise = fbm_periodic(u, v, 54, 4, profile.seed + 601, 0.47);

                if matches!(style, PlanetRenderStyle::GasGiant) {
                    let sample = gas_giant_map_sample(
                        profile,
                        gas_material.expect("gas material exists for gas giant map generation"),
                        u,
                        v,
                    );
                    albedo[i] = sample.albedo;
                    height_map[i] = sample.height;
                    water[i] = sample.water;
                    clouds[i] = sample.cloud;
                    cities[i] = sample.city;
                    vegetation[i] = sample.vegetation;
                    biome[i] = sample.biome;
                    roughness[i] = sample.roughness;
                    wetness[i] = sample.wetness;
                    continue;
                }

                let ocean_world = matches!(style, PlanetRenderStyle::OceanWorld);
                let rocky_world = matches!(style, PlanetRenderStyle::RockyWorld);
                let volcanic_world = matches!(style, PlanetRenderStyle::VolcanicWorld);
                let dry_world = rocky_world || volcanic_world || profile.ocean_fraction < 0.10;
                let geology_sample = geology_cache.sample(u, v);
                let geologic_relief = clamp01(
                    geology_sample.uplift * 0.40
                        + geology_sample.rift * 0.24
                        + geology_sample.trench * 0.20
                        + geology_sample.volcanic_heat * 0.16,
                );
                let geologic_basin = clamp01(
                    geology_sample.basin * 0.58
                        + geology_sample.oceanic_rift * 0.28
                        + geology_sample.trench * 0.14,
                );
                let macro_landform = macro_landform_field(u, v, profile.seed);
                let continental_mass = clamp01(
                    continents * 0.68 + macro_landform.uplift * 0.25 - macro_landform.basin * 0.08
                        + geology_sample.uplift * 0.050
                        - geologic_basin * 0.040
                        + macro_landform.undulation * 0.15,
                );
                let plate = ridge(tectonic);
                let tectonic_system =
                    tectonic_ridge_system(u, v, profile.seed, plate, macro_landform.uplift);
                let mountain_chain = tectonic_system.mountain_band
                    * if ocean_world {
                        0.22
                    } else if volcanic_world {
                        1.26
                    } else {
                        1.08
                    }
                    + geology_sample.continental_rift * if ocean_world { 0.040 } else { 0.12 }
                    + geology_sample.uplift * if ocean_world { 0.035 } else { 0.16 };
                let mountain = smoothstep(
                    0.50,
                    0.94,
                    plate * 0.38
                        + mountain_chain * 0.40
                        + macro_landform.uplift * 0.18
                        + tectonic_system.compression * 0.12
                        + geologic_relief * 0.10,
                ) * (0.32 + tectonic * 0.28 + tectonic_system.compression * 0.22);
                let rocky_relief = if ocean_world {
                    0.0
                } else {
                    clamp01(
                        rocky_material.relief
                            + if rocky_world { 0.10 } else { 0.0 }
                            + if volcanic_world { 0.16 } else { 0.0 },
                    )
                };
                let volcanic_bias = if ocean_world {
                    0.0
                } else {
                    clamp01(rocky_material.volcanic + if volcanic_world { 0.20 } else { 0.0 })
                };
                let highland_noise = if rocky_relief > 0.01 {
                    fbm_periodic(
                        u * 1.55 + tectonic * 0.050 + macro_landform.undulation * 0.060,
                        v * 1.24 - continental_mass * 0.042 + macro_landform.uplift * 0.035,
                        13,
                        4,
                        profile.seed + 1_109,
                        0.52,
                    )
                } else {
                    0.5
                };
                let valley_noise = if rocky_relief > 0.01 {
                    fbm_periodic(
                        u * 2.25 - tectonic * 0.060 + macro_landform.basin * 0.060,
                        v * 1.95 + continental_mass * 0.045 - macro_landform.undulation * 0.035,
                        31,
                        3,
                        profile.seed + 1_241,
                        0.48,
                    )
                } else {
                    0.5
                };
                let valley_network = valley_network_field(
                    u,
                    v,
                    profile.seed,
                    macro_landform.basin,
                    macro_landform.uplift,
                );
                let raw_fault_ridge = if rocky_relief > 0.01 {
                    clamp01(
                        tectonic_system.primary * 0.70
                            + mountain_chain * 0.22
                            + geology_sample.continental_rift * 0.16
                            + smoothstep(0.72, 0.98, plate) * 0.08,
                    )
                } else {
                    0.0
                };
                let raw_secondary_fault = if rocky_relief > 0.01 {
                    clamp01(
                        tectonic_system.secondary * 0.68
                            + ridge(highland_noise) * 0.16
                            + valley_network.tributary * 0.10
                            + geology_sample.rift * 0.10
                            + tectonic_system.compression * 0.06,
                    )
                } else {
                    0.0
                };
                let raw_valley_system = if rocky_relief > 0.01 {
                    clamp01(
                        smoothstep(0.52, 0.94, ridge(valley_noise))
                            * (0.44 + smoothstep(0.30, 0.86, highland_noise) * 0.30)
                            + valley_network.floor * 0.52
                            + valley_network.trunk * 0.18
                            + geology_sample.basin * 0.12,
                    )
                } else {
                    0.0
                };
                let rocky_fault_soften = if rocky_world && !volcanic_world {
                    0.36
                } else {
                    1.0
                };
                let rocky_valley_soften = if rocky_world && !volcanic_world {
                    0.44
                } else {
                    1.0
                };
                let fault_ridge = raw_fault_ridge * rocky_fault_soften;
                let secondary_fault = raw_secondary_fault * rocky_fault_soften;
                let valley_system = raw_valley_system * rocky_valley_soften;
                let crater_field = if rocky_relief > 0.01 {
                    crater_aging_field(
                        u + highland_noise * 0.010,
                        v - valley_noise * 0.008,
                        profile.seed,
                        clamp01(
                            moisture * 0.24
                                + profile.ocean_fraction * 0.18
                                + (1.0 - rocky_material.barren) * 0.22,
                        ),
                    )
                } else {
                    CraterAging::default()
                };
                let rocky_patch = if rocky_world {
                    rocky_patch_field(
                        u + highland_noise * 0.030,
                        v - valley_noise * 0.024,
                        profile.seed,
                    )
                } else {
                    0.5
                };
                let rocky_patch_relief = if rocky_world {
                    rocky_patch_height(u + tectonic * 0.018, v - continents * 0.014, profile.seed)
                } else {
                    0.5
                };
                let harsh_land = (rocky_world || volcanic_world) && !ocean_world;
                let monolith = if harsh_land {
                    smoothstep(
                        0.60,
                        0.96,
                        rocky_patch_relief * 0.32
                            + ridge(highland_noise) * 0.24
                            + mountain_chain * 0.22
                            + plate * 0.12
                            + crater_field.rim * 0.10,
                    ) * landform_exposure_mask(mountain_chain, macro_landform.uplift, plate)
                } else {
                    0.0
                };
                let fracture_detail = if harsh_land {
                    clamp01(
                        ridge(highland_noise) * 0.28
                            + ridge(valley_noise) * 0.26
                            + secondary_fault * 0.22
                            + fault_ridge * 0.14
                            + crater_field.ejecta * 0.10,
                    )
                } else {
                    0.5
                };
                let rift_floor = if ocean_world {
                    0.0
                } else {
                    clamp01(
                        geology_sample.continental_rift * 0.58
                            + geology_sample.rift * 0.16
                            + valley_network.trunk * 0.14
                            + secondary_fault * 0.08
                            + fault_ridge * 0.04,
                    ) * smoothstep(0.12, 0.86, rocky_relief + volcanic_bias * 0.35)
                };
                let volcanic_noise = if volcanic_bias > 0.01 {
                    fbm_periodic(
                        u * 2.0 + plate * 0.090,
                        v * 1.8 - highland_noise * 0.060,
                        27,
                        3,
                        profile.seed + 1_397,
                        0.47,
                    )
                } else {
                    0.5
                };
                let volcanic_hotspot = if volcanic_bias > 0.01 {
                    smoothstep(
                        0.58,
                        0.96,
                        volcanic_noise * 0.50
                            + fault_ridge * 0.22
                            + plate * 0.15
                            + geology_sample.volcanic_heat * 0.13,
                    )
                } else {
                    0.0
                };
                let caldera_floor =
                    volcanic_hotspot * smoothstep(0.50, 0.90, ridge(volcanic_noise));
                let caldera_rim =
                    volcanic_hotspot * smoothstep(0.48, 0.96, plate) * (1.0 - caldera_floor * 0.55);
                let geologic_height = if ocean_world {
                    0.0
                } else {
                    let macro_lift =
                        (macro_landform.uplift - macro_landform.basin) * 0.052 * rocky_relief;
                    let highland_lift = (highland_noise - 0.5) * 0.105 * rocky_relief;
                    let ridge_lift = (fault_ridge * 0.078
                        + secondary_fault * 0.038
                        + mountain_chain * 0.050
                        + monolith * 0.052)
                        * rocky_relief;
                    let valley_cut = (valley_system * (0.047 + rocky_material.barren * 0.030)
                        + valley_network.trunk * 0.020
                        + rift_floor * (0.038 + volcanic_bias * 0.016))
                        * rocky_relief;
                    let crater_cut = crater_field.floor
                        * (0.036 + rocky_material.barren * 0.034 + crater_field.aged_fill * 0.010)
                        * rocky_relief;
                    let crater_lift =
                        (crater_field.rim * 0.032 + crater_field.ejecta * 0.012) * rocky_relief;
                    let caldera_cut = caldera_floor * volcanic_bias * 0.052;
                    let caldera_lift = caldera_rim * volcanic_bias * 0.038;
                    let geology_lift = (geology_sample.terrain_elevation_m / 9_000.0)
                        .clamp(-0.12, 0.16)
                        * rocky_relief
                        * 0.62;
                    let geology_cut = (geology_sample.continental_rift * 0.030
                        + geology_sample.basin * 0.022)
                        * rocky_relief;
                    let patch_lift = if rocky_world && !volcanic_world {
                        ((rocky_patch_relief - 0.5) * 0.112
                            + smoothstep(0.62, 0.94, rocky_patch_relief) * 0.032
                            - smoothstep(0.02, 0.32, rocky_patch_relief) * 0.024)
                            * rocky_relief
                    } else {
                        0.0
                    };
                    let monolith_lift = monolith
                        * (0.042 + rocky_material.barren * 0.032 + volcanic_bias * 0.026)
                        * rocky_relief;
                    let fracture_lift =
                        (fracture_detail - 0.5) * (0.026 + monolith * 0.020) * rocky_relief;
                    macro_lift
                        + highland_lift
                        + ridge_lift
                        + crater_lift
                        + caldera_lift
                        + geology_lift
                        + patch_lift
                        + monolith_lift
                        + fracture_lift
                        - valley_cut
                        - crater_cut
                        - caldera_cut
                        - geology_cut
                };
                let h = if ocean_world {
                    continental_mass * 0.34 + mountain * 0.060
                        - macro_landform.basin * 0.120
                        - geologic_basin * 0.060
                        - geology_sample.trench * 0.042
                        + geology_sample.uplift * 0.018
                        - polar * 0.035
                        + (macro_landform.undulation - 0.5) * 0.010
                } else {
                    let dry_basin_breakup = if dry_world {
                        (highland_noise - 0.5) * 0.080
                            + fault_ridge * 0.040
                            + secondary_fault * 0.020
                            + monolith * 0.040
                            + (fracture_detail - 0.5) * 0.030
                            + crater_field.ejecta * 0.012
                            - valley_system * 0.046
                            - rift_floor * 0.030
                            - crater_field.floor * 0.022
                    } else {
                        0.0
                    } * rocky_relief;
                    let volcanic_massif = if volcanic_world {
                        volcanic_hotspot * 0.076
                            + caldera_rim * 0.060
                            + fault_ridge * 0.040
                            + monolith * 0.030
                            - caldera_floor * 0.068
                            - rift_floor * 0.020
                    } else {
                        0.0
                    } * volcanic_bias;
                    continental_mass * if dry_world { 0.91 } else { 1.04 }
                        + mountain * if volcanic_world { 0.31 } else { 0.22 }
                        - if dry_world { 0.46 } else { 0.53 }
                        - polar * if dry_world { 0.040 } else { 0.055 }
                        + (macro_landform.undulation - 0.5) * if dry_world { 0.026 } else { 0.018 }
                        + geologic_height
                        + dry_basin_breakup
                        + volcanic_massif
                };
                let ocean_level = if ocean_world {
                    0.54 + smoothstep(0.70, 1.0, profile.ocean_fraction) * 0.18
                } else if dry_world {
                    -0.34 + profile.ocean_fraction * 0.20
                } else {
                    -0.18 + profile.ocean_fraction * 0.36
                };
                let shoreline_width = if ocean_world {
                    0.030
                } else if dry_world {
                    0.024
                } else {
                    0.045
                };
                let mut water_level = clamp01(smoothstep(
                    ocean_level + 0.012,
                    ocean_level - shoreline_width,
                    h,
                ));
                if ocean_world {
                    let island_seed = smoothstep(
                        0.93,
                        0.995,
                        continental_mass * 0.64
                            + mountain * 0.22
                            + tectonic_system.primary * 0.10
                            + macro_landform.uplift * 0.04,
                    );
                    water_level = water_level.max(0.972 - island_seed * 0.24);
                }
                if dry_world {
                    water_level *= smoothstep(0.035, 0.32, profile.ocean_fraction);
                }
                let land = 1.0 - water_level;
                let coast = 1.0 - smoothstep(0.002, 0.060, (h - ocean_level).abs());
                let shoreline_transition = clamp01(
                    coast * 0.62
                        + (1.0
                            - smoothstep(
                                shoreline_width * 0.45,
                                shoreline_width * 3.60,
                                (h - ocean_level).abs(),
                            ))
                            * 0.38,
                );
                let sediment_basin = sediment_basin_field(
                    u,
                    v,
                    profile.seed,
                    macro_landform.basin,
                    valley_network.floor,
                    shoreline_transition,
                ) * (0.82 + geologic_basin * 0.28)
                    * if volcanic_world {
                        0.42
                    } else if rocky_world {
                        0.64
                    } else {
                        1.0
                    };
                let geologic_ocean_depth = clamp01(geology_sample.ocean_depth_m / 9_200.0);
                let depth = clamp01(
                    ((ocean_level - h) * if ocean_world { 3.8 } else { 5.5 })
                        .max(geologic_ocean_depth * water_level * 0.92),
                );
                let shallow = 1.0 - depth;
                let ocean_regions = ocean_regional_cues(u, v, profile.seed);
                let physics_sample = physics.sample(u, v);
                let physics_current_norm = clamp01(
                    physics_sample.current_speed_mps
                        / (physics.current_velocity_scale_mps * 1.45).max(0.001),
                );
                let density_front = clamp01(
                    (physics_sample.water_density_kg_m3 - physics.water_density_kg_m3).abs() / 48.0,
                );
                let current_u = u
                    + wind * 0.002
                    + (moisture - 0.5) * 0.0015
                    + (ocean_regions.current - 0.5) * 0.009
                    + (ocean_regions.gyre - 0.5) * 0.004
                    + physics_sample.ocean_current_mps.x * 0.0034;
                let current_v = v
                    + wind * 0.004
                    + (tectonic - 0.5) * 0.0012
                    + (ocean_regions.gyre - 0.5) * 0.007
                    - (ocean_regions.current - 0.5) * 0.003
                    + physics_sample.ocean_current_mps.y * 0.0028;
                let wave_spectrum = ocean_wave_spectrum(current_u, current_v, profile.seed + 8_113);
                let bathymetry = ocean_depth_cues(u, v, profile.seed);
                let seabed_mottle = fbm_tiled(
                    u * 3.0 + moisture * 0.08,
                    v * 2.0 - tectonic * 0.06,
                    24,
                    3,
                    profile.seed + 8_517,
                    0.50,
                );
                let capillary_mottle = fbm_tiled(
                    u * 3.8 + v * 0.31 + wave_spectrum.chop * 0.025,
                    v * 3.2 - u * 0.27 + wave_spectrum.swell * 0.020,
                    72,
                    2,
                    profile.seed + 8_641,
                    0.46,
                );
                let depth_mottle = fbm_tiled(
                    u * 1.7 + bathymetry.basin * 0.070 - wave_spectrum.swell * 0.020
                        + ocean_regions.depth_patch * 0.035,
                    v * 1.4 - bathymetry.shelf * 0.055 + tectonic * 0.030
                        - ocean_regions.gyre * 0.030,
                    17,
                    4,
                    profile.seed + 8_733,
                    0.51,
                );
                let seabed_slope = clamp01(
                    plate * 0.32
                        + mountain * 0.26
                        + mountain_chain * 0.13
                        + coast * 0.18
                        + rift_floor * 0.14
                        + geology_sample.oceanic_rift * 0.18
                        + geology_sample.trench * 0.22
                        - sediment_basin * 0.10,
                );
                let abyssal_shadow = water_level
                    * smoothstep(0.34, 0.98, depth)
                    * (bathymetry.trench * 0.12
                        + geology_sample.trench * 0.12
                        + geology_sample.oceanic_rift * 0.055
                        + (1.0 - bathymetry.basin) * 0.055
                        + bathymetry.ridge * 0.035
                        + ocean_regions.depth_patch * 0.035
                        + density_front * 0.030);
                let water_texture = clamp01(
                    wave_spectrum.swell * 0.42
                        + wave_spectrum.chop * 0.34
                        + wave_spectrum.ripple * 0.18
                        + (seabed_mottle - 0.5) * 0.10
                        + (capillary_mottle - 0.5) * 0.16
                        + (bathymetry.micro - 0.5) * 0.10
                        + (depth_mottle - 0.5) * 0.060
                        + (ocean_regions.current - 0.5) * 0.070
                        + ocean_regions.current_edge * 0.050
                        + geology_sample.oceanic_rift * water_level * 0.050
                        + geology_sample.trench * water_level * 0.035
                        + physics_current_norm * 0.040
                        + physics_sample.current_shear * 0.060
                        + density_front * 0.030,
                );
                let shore_shelf = shoreline_transition
                    * smoothstep(0.06, 0.92, water_level)
                    * (0.016 + shallow * 0.026 + sediment_basin * 0.010 + seabed_slope * 0.008);
                let coastal_bluff = coast
                    * land
                    * (0.020 + mountain * 0.034 + mountain_chain * 0.016 + rift_floor * 0.010)
                    * (1.0 - sediment_basin * 0.35);
                let coastal_scarp = shoreline_transition
                    * (0.008
                        + seabed_slope * 0.018
                        + mountain_chain * land * 0.012
                        + rift_floor * land * 0.010)
                    * (1.0 - sediment_basin * 0.42);
                let arid = clamp01(
                    (1.0 - moisture) * 1.15 + temp * 0.18 - land * 0.06 - sediment_basin * 0.10,
                );
                let forest = clamp01(
                    (moisture - 0.28) * 1.35 * temp
                        + sediment_basin * land * temp * 0.10
                        + valley_network.floor * land * moisture * 0.05,
                );
                let high = smoothstep(ocean_level + 0.16, ocean_level + 0.42, h);
                let ice = clamp01(
                    smoothstep(0.68, 0.98, lat + (0.50 - moisture) * 0.16)
                        * profile.ice_fraction
                        * 4.5,
                );
                let wave_visibility = smoothstep(0.10, 0.72, water_level) * (1.0 - ice * 0.70);
                let bare_rock = high * land * (1.0 - ice);
                let vegetation_suppression = if dry_world {
                    1.0 - clamp01(
                        rocky_material.barren * 0.86
                            + rocky_material.volcanic * 0.72
                            + rocky_material.basalt * 0.22
                            + rocky_material.sulfur * 0.42,
                    )
                } else {
                    1.0
                };
                let vegetation_signal =
                    forest * land * (1.0 - ice) * (1.0 - high * 0.62) * vegetation_suppression;
                let biome_signal = clamp01(
                    vegetation_signal * 0.72 + arid * land * 0.45 + ice * 0.86 + high * 0.30,
                );
                let wetness_signal = clamp01(
                    (water_level * 0.90
                        + moisture * land * 0.34
                        + shoreline_transition * land * 0.28
                        + sediment_basin * land * 0.18
                        + valley_network.floor * land * 0.075
                        + physics_sample.humidity * land * 0.085)
                        * if dry_world {
                            1.0 - clamp01(rocky_material.barren * 0.62 + volcanic_bias * 0.34)
                        } else {
                            1.0
                        },
                );
                let water_roughness = 0.044
                    + water_texture * 0.058
                    + wave_spectrum.foam * 0.056
                    + ridge(seabed_mottle) * 0.038
                    + ridge(wave_spectrum.chop) * 0.020
                    + bathymetry.ridge * water_level * 0.018
                    + geology_sample.oceanic_rift * water_level * 0.030
                    + geology_sample.trench * water_level * 0.018
                    + ridge(depth_mottle) * water_level * smoothstep(0.14, 0.76, depth) * 0.024
                    + ocean_regions.current_edge * water_level * 0.030
                    + physics_sample.current_shear * water_level * 0.044
                    + physics_current_norm * water_level * 0.030
                    + density_front * water_level * 0.018
                    + smoothstep(0.72, 0.06, depth) * 0.050
                    + shoreline_transition * 0.024
                    + coastal_scarp * 0.16
                    + seabed_slope * water_level * 0.024
                    - sediment_basin * water_level * 0.010;
                let terrain_micro = terrain_micro_detail(u, v, profile.seed);
                let terrain_grain = ridge(terrain_micro);
                let rocky_land = if ocean_world {
                    0.0
                } else {
                    land * (1.0 - ice * 0.86) * rocky_material.rocky
                };
                let active_volcanic = rocky_land * rocky_material.volcanic;
                let basalt_noise = if rocky_land > 0.001 {
                    fbm_periodic(
                        u * 3.2 + highland_noise * 0.055,
                        v * 2.8 - valley_noise * 0.045,
                        46,
                        3,
                        profile.seed + 1_619,
                        0.47,
                    )
                } else {
                    0.5
                };
                let ash_noise = if active_volcanic > 0.001 {
                    fbm_periodic(
                        u * 4.6 - fault_ridge * 0.040,
                        v * 4.0 + volcanic_hotspot * 0.052,
                        69,
                        2,
                        profile.seed + 1_733,
                        0.45,
                    )
                } else {
                    0.5
                };
                let oxide_noise = if rocky_land > 0.001 {
                    fbm_periodic(
                        u * 2.8 + v * 0.20 + terrain_micro * 0.035,
                        v * 2.3 - u * 0.16 + highland_noise * 0.050,
                        38,
                        3,
                        profile.seed + 1_861,
                        0.50,
                    )
                } else {
                    0.5
                };
                let basalt_flow = smoothstep(
                    0.52,
                    0.94,
                    basalt_noise * 0.46
                        + fault_ridge * 0.22
                        + volcanic_hotspot * 0.24
                        + geology_sample.volcanic_heat * 0.08
                        + crater_field.ejecta * 0.08,
                ) * rocky_land
                    * (0.30 + rocky_material.basalt * 0.70);
                let ash_fall = smoothstep(
                    0.46,
                    0.90,
                    ash_noise * 0.54
                        + volcanic_hotspot * 0.22
                        + (1.0 - highland_noise) * 0.16
                        + crater_field.floor * 0.08,
                ) * active_volcanic
                    * (0.28 + rocky_material.ash * 0.72);
                let oxide_stain = smoothstep(
                    0.48,
                    0.92,
                    oxide_noise * 0.52
                        + arid * 0.22
                        + high * 0.10
                        + valley_system * 0.08
                        + sediment_basin * 0.08,
                ) * rocky_land
                    * rocky_material.oxide;
                let sulfur_deposit = smoothstep(
                    0.42,
                    0.90,
                    volcanic_noise * 0.42
                        + ash_noise * 0.18
                        + caldera_rim * 0.24
                        + active_volcanic * 0.16,
                ) * active_volcanic
                    * rocky_material.sulfur;
                let lava_flow_path = if active_volcanic > 0.001 {
                    volcanic_lava_flow_path(
                        u,
                        v,
                        profile.seed,
                        volcanic_hotspot,
                        fault_ridge,
                        valley_system,
                        caldera_floor,
                    )
                } else {
                    0.0
                };
                let lava_channel = smoothstep(
                    0.62,
                    0.985,
                    fault_ridge * 0.30
                        + volcanic_hotspot * 0.24
                        + lava_flow_path * 0.30
                        + geology_sample.volcanic_heat * 0.12
                        + ridge(valley_noise) * 0.10
                        + secondary_fault * 0.06
                        + rift_floor * 0.08
                        + monolith * 0.04,
                ) * active_volcanic
                    * rocky_material.lava
                    * (1.0 - ice * 0.96);
                let rock_exposure = rocky_land
                    * clamp01(
                        high * 0.34
                            + fault_ridge * 0.22
                            + mountain_chain * 0.18
                            + valley_system * 0.16
                            + rift_floor * 0.12
                            + crater_field.rim * 0.10
                            + monolith * 0.30
                            + plate * 0.12,
                    );
                let roughness_signal = clamp01(
                    water_level * water_roughness
                        + land * (0.44 + bare_rock * 0.34 + arid * 0.10 + terrain_grain * 0.10)
                        + mountain * 0.24
                        + mountain_chain * land * 0.110
                        + coastal_scarp * land * 0.16
                        + monolith * land * (0.16 + rocky_material.barren * 0.10)
                        + ridge(fracture_detail) * rocky_land * 0.060
                        + rocky_land
                            * (0.08
                                + rocky_material.barren * 0.08
                                + rocky_material.basalt * 0.06
                                + fault_ridge * 0.070
                                + valley_system * 0.045
                                + rift_floor * 0.050
                                + crater_field.rim * 0.050
                                + crater_field.ejecta * 0.032)
                        + ash_fall * 0.18
                        + oxide_stain * 0.06
                        + rocky_land * if rocky_world { 0.10 } else { 0.0 }
                        + active_volcanic * if volcanic_world { 0.14 } else { 0.0 }
                        + geology_sample.rift * land * 0.060
                        + geology_sample.surface_heat_flow_mw_m2.max(0.0).min(180.0) / 180.0
                            * active_volcanic
                            * 0.070
                        + ice * 0.08
                        - lava_channel * 0.18
                        - sediment_basin * land * 0.050
                        - shoreline_transition * land * 0.030
                        - crater_field.aged_fill * rocky_land * 0.025
                        - vegetation_signal * 0.16,
                );
                let roughness_signal = if volcanic_world {
                    clamp(
                        roughness_signal * 0.74
                            + active_volcanic * 0.060
                            + (basalt_noise - 0.5) * active_volcanic * 0.060
                            + (ash_noise - 0.5) * active_volcanic * 0.050
                            + caldera_rim * 0.030
                            + crater_field.rim * active_volcanic * 0.020
                            - lava_channel * 0.240
                            - lava_flow_path * active_volcanic * 0.060
                            - caldera_floor * active_volcanic * 0.045,
                        0.26,
                        0.91,
                    )
                } else {
                    roughness_signal
                };

                let color = if water_level > 0.5 {
                    let deep_ocean = if ocean_world {
                        Vec3::new(0.006, 0.036, 0.150)
                    } else {
                        Vec3::new(0.012, 0.045, 0.140)
                    };
                    let trench_color = if ocean_world {
                        Vec3::new(0.0015, 0.014, 0.070)
                    } else {
                        Vec3::new(0.004, 0.022, 0.082)
                    };
                    let mid_ocean = if ocean_world {
                        Vec3::new(0.010, 0.070, 0.205)
                    } else {
                        Vec3::new(0.018, 0.092, 0.200)
                    };
                    let shelf = if ocean_world {
                        Vec3::new(0.030, 0.210, 0.365)
                    } else {
                        Vec3::new(0.045, 0.330, 0.430)
                    };
                    let sunlit_skin = if ocean_world {
                        Vec3::new(0.050, 0.245, 0.410)
                    } else {
                        Vec3::new(0.080, 0.340, 0.440)
                    };
                    let reef = Vec3::new(0.110, 0.520, 0.560);
                    let sediment = Vec3::new(0.055, 0.205, 0.255)
                        .lerp(Vec3::new(0.105, 0.310, 0.300), bathymetry.shelf);
                    let basin = Vec3::new(0.002, 0.022, 0.110);
                    let color_depth = clamp01(
                        depth * (0.88 + ocean_regions.depth_patch * 0.18)
                            + (depth_mottle - 0.5) * 0.105
                            + (bathymetry.micro - 0.5) * 0.052
                            + ocean_regions.current_edge * 0.038
                            + seabed_slope * 0.050
                            + geology_sample.trench * water_level * 0.044
                            + geology_sample.oceanic_rift * water_level * 0.026
                            + density_front * 0.050
                            + physics_sample.current_shear * 0.030
                            - coast * 0.035
                            - smoothstep(0.62, 0.96, ocean_regions.warm) * shallow * 0.040,
                    );
                    let depth_mix = smoothstep(0.06, 0.86, color_depth);
                    let shelf_mix = clamp01(
                        shallow * (0.74 - seabed_slope * 0.28)
                            + shoreline_transition * 0.18
                            + coastal_scarp * 0.18
                            + bathymetry.shelf * shallow * 0.16
                            + sediment_basin * shallow * 0.07
                            + ocean_regions.reef * shallow * 0.055,
                    );
                    let trench_mix = smoothstep(0.48, 0.99, color_depth)
                        * (0.18
                            + bathymetry.trench * if ocean_world { 0.70 } else { 0.42 }
                            + geology_sample.trench * if ocean_world { 0.30 } else { 0.18 }
                            + geology_sample.oceanic_rift * 0.12
                            + ocean_regions.depth_patch * 0.12);
                    let sediment_mix = bathymetry.turbidity
                        * shallow
                        * water_level
                        * (0.09 + shoreline_transition * if ocean_world { 0.18 } else { 0.30 })
                        + ocean_regions.sediment
                            * shallow
                            * water_level
                            * shoreline_transition
                            * 0.12
                        + sediment_basin
                            * shallow
                            * water_level
                            * (0.10 + shoreline_transition * 0.18);
                    let reef_mix = if ocean_world {
                        shoreline_transition
                            * shallow
                            * (0.18 + ridge(seabed_mottle) * 0.10 + ocean_regions.reef * 0.20)
                    } else {
                        shoreline_transition * shallow * (0.48 + ridge(seabed_mottle) * 0.20)
                    };
                    let slope_shadow =
                        seabed_slope * depth_mix * 0.20 + abyssal_shadow + coastal_scarp * 0.06;
                    let capillary_tint = (capillary_mottle - 0.5) * (0.060 + shallow * 0.025)
                        + ridge(capillary_mottle) * wave_spectrum.foam * 0.018;
                    let subsurface_layer = water_level
                        * smoothstep(0.14, 0.60, color_depth)
                        * (1.0 - smoothstep(0.72, 0.99, color_depth))
                        * (0.20
                            + depth_mottle * 0.20
                            + bathymetry.basin * 0.12
                            + ocean_regions.gyre * 0.08);
                    let bottom_glow = water_level
                        * (1.0 - depth_mix)
                        * (0.10 + shoreline_transition * 0.18)
                        * (0.12 + ridge(seabed_mottle) * 0.10 + bathymetry.shelf * 0.16);
                    let wind_sheen = wave_visibility
                        * (0.04 + wave_spectrum.swell * 0.045 + ridge(capillary_mottle) * 0.030)
                        * (1.0 - depth_mix * 0.35);
                    let warm_current = smoothstep(0.54, 0.94, ocean_regions.warm)
                        * water_level
                        * (1.0 - color_depth * 0.38);
                    let cool_current = smoothstep(0.58, 0.96, 1.0 - ocean_regions.warm)
                        * water_level
                        * color_depth
                        * 0.55;
                    let current_line = clamp01(
                        ocean_regions.current_edge * water_level * (0.26 + shallow * 0.14)
                            + physics_sample.current_shear * water_level * 0.22
                            + physics_current_norm * water_level * 0.08
                            + density_front * water_level * 0.06,
                    );
                    let current_color = Vec3::new(0.008, 0.080, 0.170)
                        .lerp(Vec3::new(0.030, 0.180, 0.240), ocean_regions.warm);
                    let wave_tint = 0.88
                        + water_texture * 0.095
                        + wave_spectrum.foam * 0.052
                        + capillary_tint
                        + (ocean_regions.current - 0.5) * 0.045;
                    let mut ocean_color = basin
                        .lerp(mid_ocean, 0.36 + subsurface_layer)
                        .lerp(deep_ocean, 1.0 - depth_mix * 0.42)
                        .lerp(trench_color, trench_mix)
                        .lerp(shelf, shelf_mix)
                        .lerp(reef, reef_mix)
                        .lerp(sediment, sediment_mix)
                        .lerp(current_color, current_line)
                        .lerp(Vec3::new(0.060, 0.290, 0.350), warm_current * 0.16)
                        .lerp(Vec3::new(0.0015, 0.020, 0.092), cool_current * 0.10)
                        .lerp(sunlit_skin, wind_sheen + bottom_glow)
                        * (wave_tint - slope_shadow);
                    ocean_color += Vec3::new(0.020, 0.100, 0.130)
                        * (ocean_regions.current_edge + physics_sample.current_shear * 0.60)
                        * water_level
                        * (0.05 + wave_spectrum.foam * 0.055 + density_front * 0.030);
                    ocean_color
                } else if ocean_world {
                    let basalt =
                        Vec3::new(0.13, 0.14, 0.13).lerp(Vec3::new(0.26, 0.25, 0.22), high);
                    let wet_green =
                        Vec3::new(0.10, 0.25, 0.18).lerp(Vec3::new(0.17, 0.38, 0.23), forest);
                    let silt = Vec3::new(0.36, 0.34, 0.25)
                        .lerp(Vec3::new(0.27, 0.40, 0.32), wetness_signal);
                    basalt
                        .lerp(
                            silt,
                            sediment_basin * land * (0.18 + shoreline_transition * 0.24),
                        )
                        .lerp(wet_green, forest * 0.38)
                        .lerp(Vec3::new(0.84, 0.89, 0.88), ice)
                } else {
                    let soil = Vec3::new(0.42, 0.32, 0.20).lerp(Vec3::new(0.70, 0.56, 0.34), arid);
                    let veg = Vec3::new(0.12, 0.32, 0.21).lerp(Vec3::new(0.18, 0.50, 0.28), forest);
                    let oxide = Vec3::new(0.40, 0.23, 0.16)
                        .lerp(Vec3::new(0.66, 0.38, 0.22), arid * 0.55 + high * 0.18);
                    let rock = Vec3::new(0.38, 0.36, 0.32).lerp(Vec3::new(0.72, 0.68, 0.56), high);
                    let basalt = Vec3::new(0.045, 0.048, 0.050)
                        .lerp(Vec3::new(0.18, 0.17, 0.15), basalt_noise);
                    let ash =
                        Vec3::new(0.20, 0.19, 0.17).lerp(Vec3::new(0.47, 0.45, 0.40), ash_noise);
                    let rusty_oxide =
                        Vec3::new(0.50, 0.19, 0.10).lerp(Vec3::new(0.82, 0.45, 0.20), oxide_noise);
                    let sulfur = Vec3::new(0.72, 0.57, 0.16)
                        .lerp(Vec3::new(0.95, 0.82, 0.34), sulfur_deposit);
                    let alluvial_silt = Vec3::new(0.46, 0.39, 0.27)
                        .lerp(Vec3::new(0.30, 0.39, 0.31), wetness_signal)
                        .lerp(Vec3::new(0.58, 0.47, 0.32), arid * 0.26);
                    let exposed = rock.lerp(
                        Vec3::new(0.78, 0.75, 0.65),
                        fault_ridge * 0.40
                            + mountain_chain * 0.16
                            + crater_field.rim * 0.10
                            + monolith * 0.20,
                    );
                    let valley_shadow = Vec3::new(0.20, 0.18, 0.16)
                        .lerp(Vec3::new(0.34, 0.28, 0.22), oxide_stain)
                        .lerp(Vec3::new(0.10, 0.095, 0.090), rift_floor * 0.30);
                    let crater_shadow = Vec3::new(0.18, 0.16, 0.14)
                        .lerp(rock, crater_field.aged_fill * 0.36)
                        .lerp(alluvial_silt, sediment_basin * 0.18);
                    let mut terrain = soil
                        .lerp(
                            alluvial_silt,
                            sediment_basin * land * (0.22 + shoreline_transition * 0.28),
                        )
                        .lerp(veg, forest * 0.78)
                        .lerp(oxide, arid * 0.44)
                        .lerp(rock, high * 0.58)
                        .lerp(exposed, rock_exposure * 0.50)
                        .lerp(
                            basalt,
                            basalt_flow * 0.70 + rocky_land * rocky_material.basalt * 0.18,
                        )
                        .lerp(rusty_oxide, oxide_stain * (0.45 + arid * 0.18))
                        .lerp(ash, ash_fall * 0.64)
                        .lerp(sulfur, sulfur_deposit * 0.70)
                        .lerp(valley_shadow, valley_system * rocky_land * 0.25)
                        .lerp(
                            crater_shadow,
                            crater_field.floor
                                * rocky_land
                                * (0.16 + crater_field.aged_fill * 0.12),
                        );
                    if rocky_world {
                        let mineral_patch = smoothstep(0.24, 0.86, rocky_patch);
                        let light_patch = smoothstep(0.50, 0.94, rocky_patch_relief);
                        let dark_patch = smoothstep(0.48, 0.12, rocky_patch);
                        let rough_patch = smoothstep(0.40, 0.92, terrain_micro);
                        let iron_palette =
                            matches!(rocky_palette.kind, RockyPaletteKind::IronOxide);
                        let highland_cap = clamp01(high * 0.28 + light_patch * 0.48)
                            * if iron_palette { 0.68 } else { 1.0 };
                        let basaltic_shadow = clamp01(
                            rocky_material.basalt * (0.18 + dark_patch * 0.18)
                                + valley_system * 0.065
                                + rift_floor * 0.050
                                + raw_secondary_fault * 0.024
                                + crater_field.floor * 0.030,
                        );
                        let oxide_mix = clamp01(
                            rocky_material.oxide * (0.22 + mineral_patch * 0.34)
                                + sediment_basin * 0.050
                                + if iron_palette {
                                    0.18 + 0.24 * mineral_patch
                                } else {
                                    0.0
                                },
                        );
                        let mut rocky_tone = rocky_palette
                            .mid
                            .lerp(rocky_palette.high, highland_cap + monolith * 0.18)
                            .lerp(rocky_palette.low, dark_patch * 0.24 + basaltic_shadow)
                            .lerp(rocky_palette.mineral, oxide_mix)
                            .lerp(
                                rocky_palette.shadow,
                                valley_system * 0.035
                                    + rift_floor * 0.026
                                    + crater_like_patch(rocky_patch_relief) * 0.032
                                    + crater_field.floor * 0.055,
                            );
                        rocky_tone = rocky_tone.lerp(Vec3::new(0.84, 0.80, 0.70), ice * 0.88)
                            * (0.86
                                + (rocky_patch - 0.5) * 0.18
                                + (rocky_patch_relief - 0.5) * 0.16
                                + (fracture_detail - 0.5) * 0.070
                                + monolith * 0.065
                                + crater_field.rim * 0.026
                                + crater_field.ejecta * 0.018
                                + (rough_patch - 0.5) * 0.050
                                - valley_system * 0.028
                                - rift_floor * 0.022
                                - crater_field.aged_fill * 0.018
                                - fault_ridge * 0.018);
                        terrain = terrain.lerp(
                            rocky_tone,
                            clamp01(
                                rocky_material.rocky * (0.80 + rocky_material.barren * 0.22)
                                    + if iron_palette { 0.10 } else { 0.0 },
                            ),
                        );
                        if profile_key_contains_any(profile, &["carbon", "diamond"]) {
                            let carbon_plate = smoothstep(
                                0.26,
                                0.88,
                                rocky_patch * 0.42
                                    + rocky_patch_relief * 0.34
                                    + terrain_micro * 0.16
                                    + crater_field.floor * 0.08,
                            );
                            let graphite = Vec3::new(0.002, 0.002, 0.002)
                                .lerp(Vec3::new(0.045, 0.042, 0.038), carbon_plate);
                            let carbide = Vec3::new(0.055, 0.052, 0.048)
                                .lerp(Vec3::new(0.18, 0.17, 0.15), rocky_patch_relief);
                            let diamond_glint = smoothstep(
                                0.955,
                                0.998,
                                terrain_micro * 0.44
                                    + rocky_patch_relief * 0.36
                                    + ridge(fracture_detail) * 0.20,
                            );
                            let carbon_crust =
                                graphite.lerp(carbide, smoothstep(0.58, 0.94, carbon_plate));
                            terrain = terrain
                                .lerp(carbon_crust, 0.86 + carbon_plate * 0.12)
                                .lerp(Vec3::new(0.88, 0.86, 0.76), diamond_glint * 0.14);
                        }
                    }
                    terrain += Vec3::new(1.45, 0.38, 0.060)
                        * lava_channel
                        * (0.32 + profile.volcanic_activity * 0.22);
                    if volcanic_world && rocky_material.sulfur > 0.08 {
                        let sulfur_wash = clamp01(
                            rocky_material.sulfur
                                * (0.18
                                    + active_volcanic * 0.18
                                    + sulfur_deposit * 0.74
                                    + ash_fall * 0.10),
                        );
                        let sulfur_skin = Vec3::new(0.86, 0.66, 0.12)
                            .lerp(Vec3::new(1.04, 0.88, 0.26), sulfur_deposit)
                            .lerp(Vec3::new(0.76, 0.38, 0.08), lava_channel * 0.28);
                        terrain = terrain.lerp(sulfur_skin, sulfur_wash * 0.72);
                    }
                    terrain.lerp(Vec3::new(0.84, 0.89, 0.88), ice)
                };

                let cloud_warp = fbm_tiled(
                    u * 0.86 + physics_sample.cloud_flow_mps.x * 0.00035,
                    v * 0.64 + physics_sample.cloud_flow_mps.y * 0.00025,
                    24,
                    4,
                    profile.seed + 9_011,
                    0.54,
                );
                let cloud_u =
                    u + (cloud_warp - 0.5) * 0.045 + physics_sample.cloud_flow_mps.x * 0.00055;
                let cloud_v = v + (cloud_base - 0.5) * 0.026 - wind * 0.006
                    + physics_sample.cloud_flow_mps.y * 0.00045;
                let cloud_cell = fbm_tiled(
                    cloud_u * 2.0 + cloud_v * 0.22,
                    cloud_v * 1.48 - cloud_u * 0.14,
                    28,
                    3,
                    profile.seed + 9_043,
                    0.50,
                );
                let cloud_breakup = fbm_tiled(
                    cloud_u * 4.1 - cloud_cell * 0.035 + cloud_v * 0.18,
                    cloud_v * 2.8 + cloud_base * 0.040 - cloud_u * 0.12,
                    71,
                    2,
                    profile.seed + 9_157,
                    0.46,
                );
                let storm_flow = fbm_tiled(
                    cloud_u * 1.35
                        + wind * 0.038
                        + cloud_base * 0.030
                        + physics_sample.cloud_flow_mps.x * 0.0009,
                    cloud_v * 1.05 - wind * 0.024
                        + cloud_cell * 0.040
                        + physics_sample.cloud_flow_mps.y * 0.0007,
                    19,
                    4,
                    profile.seed + 9_241,
                    0.52,
                );
                let marine_sheet = if ocean_world {
                    smoothstep(
                        0.44,
                        0.90,
                        moisture * 0.28
                            + cloud_cell * 0.20
                            + ridge(cloud_breakup) * 0.18
                            + ocean_regions.current_edge * 0.24
                            + ocean_regions.gyre * 0.10,
                    ) * water_level
                        * (0.070
                            + profile.atmosphere_density * 0.035
                            + physics_sample.cloud_lift * 0.040)
                } else {
                    0.0
                };
                let cyclone_cloud = if ocean_world {
                    ocean_cyclone_cloud_mask(
                        cloud_u
                            + (storm_flow - 0.5) * 0.010
                            + physics_sample.cloud_flow_mps.x * 0.00045,
                        cloud_v
                            + (cloud_base - 0.5) * 0.008
                            + physics_sample.cloud_flow_mps.y * 0.00035,
                        profile.seed,
                    ) * water_level
                } else {
                    0.0
                };
                let marine_convergence = if ocean_world {
                    smoothstep(
                        0.45,
                        0.92,
                        ocean_regions.current_edge * 0.58
                            + ridge(storm_flow) * 0.18
                            + (ocean_regions.current - 0.5).abs() * 0.12
                            + physics_sample.current_shear * 0.10
                            + physics_sample.cloud_lift * 0.10
                            + (1.0 - polar) * 0.08,
                    ) * water_level
                        * 0.085
                } else {
                    0.0
                };
                let storm_band = smoothstep(
                    0.48,
                    0.92,
                    storm_flow * 0.54 + ridge(cloud_breakup) * 0.18 + (wind * 0.5 + 0.5) * 0.28,
                ) * if ocean_world { 0.165 } else { 0.130 };
                let volcanic_plume = if volcanic_world {
                    smoothstep(
                        0.58,
                        0.96,
                        volcanic_hotspot * 0.42
                            + ash_noise * 0.26
                            + ridge(cloud_breakup) * 0.20
                            + fault_ridge * 0.12,
                    ) * (0.10 + rocky_material.ash * 0.14 + rocky_material.sulfur * 0.08)
                } else {
                    0.0
                };
                let cloud_field = cloud_base * 0.60
                    + cloud_detail * 0.14
                    + cloud_cell * 0.15
                    + ridge(cloud_breakup) * 0.040
                    + (storm_flow - 0.5) * 0.038
                    + storm_band
                    + marine_sheet
                    + marine_convergence
                    + cyclone_cloud * 0.22
                    + physics_sample.cloud_lift * 0.12
                    + physics_sample.humidity * 0.045
                    + volcanic_plume
                    - lat * 0.05;
                let cloud = clamp01(
                    smoothstep(
                        if ocean_world { 0.60 } else { 0.62 },
                        if ocean_world { 0.90 } else { 0.91 },
                        cloud_field,
                    ) * profile.cloud_density
                        * if ocean_world {
                            0.94 + moisture * 0.42
                                + ocean_regions.current_edge * 0.12
                                + physics_sample.cloud_lift * 0.10
                        } else {
                            0.78 + moisture * 0.36
                        }
                        + cyclone_cloud * profile.cloud_density * 0.22
                        + marine_sheet * profile.cloud_density * 0.36
                        + physics_sample.cloud_lift * profile.cloud_density * 0.08
                        + volcanic_plume * profile.atmosphere_density.min(1.0),
                );
                let settlement = smoothstep(0.78, 0.96, city_noise);
                let habitable = smoothstep(0.10, 0.34, temp) * smoothstep(0.85, 0.42, lat);
                let city =
                    settlement * land * habitable * smoothstep(0.01, 0.11, shoreline_transition);

                albedo[i] = color;
                let wave_height = ((wave_spectrum.swell - 0.5) * 0.022
                    + (wave_spectrum.chop - 0.5) * 0.011
                    + (wave_spectrum.ripple - 0.5) * 0.0035
                    + (seabed_mottle - 0.5) * 0.0025
                    + (ocean_regions.current - 0.5) * 0.0045
                    + ocean_regions.current_edge * 0.0035
                    + (physics_current_norm - 0.5) * 0.0040
                    + physics_sample.current_shear * 0.0035
                    + density_front * 0.0022)
                    * wave_visibility;
                let micro_height = (terrain_micro - 0.5)
                    * land
                    * (1.0 - ice * 0.72)
                    * (0.014 + mountain * 0.026 + mountain_chain * 0.012 + bare_rock * 0.012);
                let dry_height_gain = 1.0
                    + if rocky_world { 0.42 } else { 0.0 }
                    + if volcanic_world { 0.62 } else { 0.0 };
                let rocky_height = (rocky_land
                    * ((highland_noise - 0.5) * 0.030
                        + fault_ridge * 0.036
                        + secondary_fault * 0.016
                        + mountain_chain * 0.022
                        + monolith * 0.040
                        + (fracture_detail - 0.5) * 0.018
                        + crater_field.rim * 0.022
                        + crater_field.ejecta * 0.006
                        + if rocky_world {
                            (rocky_patch_relief - 0.5) * 0.050
                                + smoothstep(0.68, 0.95, rocky_patch_relief) * 0.018
                                - smoothstep(0.02, 0.28, rocky_patch_relief) * 0.014
                        } else {
                            0.0
                        }
                        + basalt_flow * 0.012
                        + lava_flow_path * volcanic_bias * 0.006
                        - valley_system * (0.028 + rocky_material.barren * 0.013)
                        - rift_floor * (0.024 + volcanic_bias * 0.012)
                        - crater_field.floor * (0.022 + rocky_material.barren * 0.011)
                        - crater_field.aged_fill * 0.006
                        - sediment_basin * 0.005
                        - ash_fall * 0.010)
                    + active_volcanic
                        * (caldera_rim * 0.024 - caldera_floor * 0.034
                            + lava_channel * 0.016
                            + lava_flow_path * 0.010))
                    * dry_height_gain;
                let abyssal_height = water_level
                    * smoothstep(0.30, 0.96, depth)
                    * (bathymetry.trench * 0.032 + (1.0 - bathymetry.basin) * 0.014);
                let ridge_height =
                    water_level * smoothstep(0.20, 0.88, depth) * bathymetry.ridge * 0.012;
                let ocean_floor_height = if ocean_world {
                    ridge_height - abyssal_height
                } else {
                    (ridge_height - abyssal_height) * 0.40
                };
                let sediment_fill_height = sediment_basin
                    * (land * (0.004 + shoreline_transition * 0.010)
                        + water_level * shallow * 0.006);
                let shelf_height = shore_shelf
                    + coastal_bluff
                    + coastal_scarp
                    + ocean_floor_height
                    + sediment_fill_height;
                height_map[i] = if ocean_world {
                    clamp01(
                        0.34 + (h - ocean_level) * 0.42
                            + mountain * 0.10
                            + mountain_chain * 0.018
                            + shelf_height
                            + micro_height
                            + rocky_height
                            + wave_height,
                    )
                } else {
                    clamp01(
                        (h + 0.24) / 0.72
                            + shelf_height
                            + micro_height
                            + rocky_height
                            + wave_height,
                    )
                };
                water[i] = water_level;
                clouds[i] = cloud;
                cities[i] = city;
                vegetation[i] = vegetation_signal;
                biome[i] = biome_signal;
                roughness[i] = roughness_signal;
                wetness[i] = wetness_signal;
            }
        }

        let (ambient_occlusion, horizon_occlusion) = if matches!(style, PlanetRenderStyle::GasGiant)
        {
            (vec![0.0; len], vec![0.0; len])
        } else {
            bake_terrain_occlusion_maps(width, height, &height_map, &water)
        };

        Self {
            width,
            height,
            albedo,
            height_map,
            water,
            clouds,
            cities,
            vegetation,
            biome,
            roughness,
            wetness,
            ambient_occlusion,
            horizon_occlusion,
        }
    }

    fn sample(&self, u: f32, v: f32) -> SurfaceSample {
        let x = (u.rem_euclid(1.0)) * self.width as f32;
        let y = clamp(v, 0.0, 0.999_999) * (self.height - 1) as f32;
        let x0 = x.floor() as usize % self.width;
        let x1 = (x0 + 1) % self.width;
        let y0 = y.floor() as usize;
        let y1 = (y0 + 1).min(self.height - 1);
        let tx = x - x.floor();
        let ty = y - y.floor();

        let i00 = y0 * self.width + x0;
        let i10 = y0 * self.width + x1;
        let i01 = y1 * self.width + x0;
        let i11 = y1 * self.width + x1;

        SurfaceSample {
            albedo: bilerp_vec3(
                self.albedo[i00],
                self.albedo[i10],
                self.albedo[i01],
                self.albedo[i11],
                tx,
                ty,
            ),
            height: bilerp(
                self.height_map[i00],
                self.height_map[i10],
                self.height_map[i01],
                self.height_map[i11],
                tx,
                ty,
            ),
            water: bilerp(
                self.water[i00],
                self.water[i10],
                self.water[i01],
                self.water[i11],
                tx,
                ty,
            ),
            cloud: bilerp(
                self.clouds[i00],
                self.clouds[i10],
                self.clouds[i01],
                self.clouds[i11],
                tx,
                ty,
            ),
            city: bilerp(
                self.cities[i00],
                self.cities[i10],
                self.cities[i01],
                self.cities[i11],
                tx,
                ty,
            ),
            vegetation: bilerp(
                self.vegetation[i00],
                self.vegetation[i10],
                self.vegetation[i01],
                self.vegetation[i11],
                tx,
                ty,
            ),
            biome: bilerp(
                self.biome[i00],
                self.biome[i10],
                self.biome[i01],
                self.biome[i11],
                tx,
                ty,
            ),
            roughness: bilerp(
                self.roughness[i00],
                self.roughness[i10],
                self.roughness[i01],
                self.roughness[i11],
                tx,
                ty,
            ),
            wetness: bilerp(
                self.wetness[i00],
                self.wetness[i10],
                self.wetness[i01],
                self.wetness[i11],
                tx,
                ty,
            ),
            ambient_occlusion: bilerp(
                self.ambient_occlusion[i00],
                self.ambient_occlusion[i10],
                self.ambient_occlusion[i01],
                self.ambient_occlusion[i11],
                tx,
                ty,
            ),
            horizon_occlusion: bilerp(
                self.horizon_occlusion[i00],
                self.horizon_occlusion[i10],
                self.horizon_occlusion[i01],
                self.horizon_occlusion[i11],
                tx,
                ty,
            ),
        }
    }

    fn tangent_space_normal(&self, u: f32, v: f32, strength: f32) -> Vec3 {
        let eps_u = 1.0 / self.width.max(1) as f32;
        let eps_v = 1.0 / self.height.max(1) as f32;
        let water = clamp01(self.sample(u, v).water);
        let strength = strength * (1.0 - smoothstep(0.38, 0.92, water) * 0.38);
        let left = self.sample(u - eps_u, v).height;
        let right = self.sample(u + eps_u, v).height;
        let up = self.sample(u, v - eps_v).height;
        let down = self.sample(u, v + eps_v).height;

        Vec3::new((left - right) * strength, (up - down) * strength, 1.0).normalize()
    }
}

/// Integrates a compact heightfield horizon in eight azimuths. Distances are
/// logarithmic so the first probes capture contact/cavity occlusion while the
/// last probes capture ridges and crater walls. The calculation happens once
/// at the renderer's canonical map resolution; every output then bilinearly
/// samples the same deterministic products.
fn bake_terrain_occlusion_maps(
    width: usize,
    height: usize,
    height_map: &[f32],
    water: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    const DIRECTIONS: [(isize, isize); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    const DISTANCES: [isize; 6] = [1, 2, 4, 8, 16, 28];

    let len = width.saturating_mul(height);
    let mut ambient = vec![0.0; len];
    let mut horizon = vec![0.0; len];
    if width == 0 || height == 0 || height_map.len() != len || water.len() != len {
        return (ambient, horizon);
    }

    for y in 0..height {
        let latitude = (0.5 - y as f32 / height.saturating_sub(1).max(1) as f32) * PI;
        let longitude_scale = latitude.cos().abs().max(0.16);
        for x in 0..width {
            let index = y * width + x;
            let center = height_map[index];
            let land = 1.0 - smoothstep(0.34, 0.88, water[index]);
            if land <= 0.002 {
                continue;
            }

            let mut directional_occlusion = 0.0;
            let mut local_neighbor_height = 0.0;
            for (direction_index, (dx, dy)) in DIRECTIONS.iter().copied().enumerate() {
                let mut max_horizon_slope = 0.0_f32;
                for distance in DISTANCES {
                    let sample_x = (x as isize + dx * distance).rem_euclid(width as isize) as usize;
                    let sample_y = (y as isize + dy * distance)
                        .clamp(0, height.saturating_sub(1) as isize)
                        as usize;
                    let sample_height = height_map[sample_y * width + sample_x];
                    if distance == 1 {
                        local_neighbor_height += sample_height;
                    }

                    let world_dx = dx as f32 * longitude_scale;
                    let world_dy = dy as f32;
                    let horizontal_distance =
                        (world_dx * world_dx + world_dy * world_dy).sqrt() * distance as f32;
                    let elevation_delta =
                        sample_height - center - (0.0015 + horizontal_distance * 0.000_08);
                    let slope = elevation_delta.max(0.0) * 34.0 / horizontal_distance.max(0.25);
                    max_horizon_slope = max_horizon_slope.max(slope);
                }

                // sin(atan(slope)) is the blocked fraction of the upper
                // hemisphere in this azimuth for a heightfield horizon.
                let blocked =
                    max_horizon_slope / (1.0 + max_horizon_slope * max_horizon_slope).sqrt();
                let azimuth_weight = if direction_index % 2 == 0 { 1.0 } else { 0.86 };
                directional_occlusion += blocked * azimuth_weight;
            }

            let horizon_occ = clamp01(directional_occlusion / 7.44) * land;
            let local_average = local_neighbor_height / DIRECTIONS.len() as f32;
            let cavity = clamp01((local_average - center - 0.001) * 18.0) * land;
            let curvature = clamp01(
                (height_map[y * width + (x + 1) % width]
                    + height_map[y * width + (x + width - 1) % width]
                    + height_map[y.saturating_sub(1) * width + x]
                    + height_map[(y + 1).min(height - 1) * width + x]
                    - center * 4.0)
                    * 11.0,
            ) * land;

            horizon[index] = horizon_occ;
            ambient[index] = clamp01(horizon_occ * 0.68 + cavity * 0.22 + curvature * 0.10);
        }
    }

    (ambient, horizon)
}

fn material_map_uv(size: RenderSize, x: u32, y: u32) -> (f32, f32) {
    (
        x as f32 / size.width.max(1) as f32,
        y as f32 / size.height.saturating_sub(1).max(1) as f32,
    )
}

#[derive(Debug, Clone)]
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn transparent(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
        }
    }

    fn blend(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let sa = rgba[3] as u32;
        if sa == 0 {
            return;
        }
        let i = ((y as u32 * self.width + x as u32) * 4) as usize;
        if sa == 255 {
            self.pixels[i..i + 4].copy_from_slice(&rgba);
            return;
        }

        let da = self.pixels[i + 3] as u32;
        let inv = 255 - sa;
        let out_a = sa + (da * inv + 127) / 255;
        if out_a == 0 {
            return;
        }

        for (channel, src) in rgba.iter().take(3).enumerate() {
            let dst = self.pixels[i + channel] as u32;
            self.pixels[i + channel] = ((*src as u32 * sa + dst * da * inv / 255) / out_a) as u8;
        }
        self.pixels[i + 3] = out_a as u8;
    }

    fn blit_tile(&mut self, tile: RenderTile, pixels: &[u8]) {
        debug_assert_eq!(pixels.len(), tile.pixel_count() as usize * 4);
        for row in 0..tile.height {
            let src = (row * tile.width * 4) as usize;
            let dst = (((tile.y + row) * self.width + tile.x) * 4) as usize;
            let len = (tile.width * 4) as usize;
            self.pixels[dst..dst + len].copy_from_slice(&pixels[src..src + len]);
        }
    }

    fn sharpen_opaque(&mut self, amount: f32) {
        let src = self.pixels.clone();
        let width = self.width as usize;
        let height = self.height as usize;
        if width < 3 || height < 3 {
            return;
        }

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let i = (y * width + x) * 4;
                if src[i + 3] < 210 {
                    continue;
                }
                for channel in 0..3 {
                    let center = src[i + channel] as f32;
                    let blur = (center * 4.0
                        + src[i - 4 + channel] as f32
                        + src[i + 4 + channel] as f32
                        + src[i - width * 4 + channel] as f32
                        + src[i + width * 4 + channel] as f32)
                        / 8.0;
                    self.pixels[i + channel] =
                        clamp(center + (center - blur) * amount, 0.0, 255.0) as u8;
                }
            }
        }
    }

    fn into_image(self) -> RgbaImage {
        RgbaImage::from_raw(self.width, self.height, self.pixels)
            .expect("canvas buffer size must match image dimensions")
    }
}

struct TileRenderResult {
    tile: RenderTile,
    pixels: Vec<u8>,
}

fn noop_progress(_: RenderProgressEvent) {}

fn emit_plan_progress<F>(progress: &mut F, plan: &TilePlan)
where
    F: FnMut(RenderProgressEvent),
{
    emit_progress(progress, plan, RenderPhase::Planning, 0, 0, None);
    emit_progress(
        progress,
        plan,
        RenderPhase::Planning,
        plan.total_tiles(),
        plan.total_pixels(),
        None,
    );
}

fn emit_phase_start<F>(progress: &mut F, plan: &TilePlan, phase: RenderPhase)
where
    F: FnMut(RenderProgressEvent),
{
    emit_progress(progress, plan, phase, 0, 0, None);
}

fn emit_phase_complete<F>(progress: &mut F, plan: &TilePlan, phase: RenderPhase)
where
    F: FnMut(RenderProgressEvent),
{
    emit_progress(
        progress,
        plan,
        phase,
        plan.total_tiles(),
        plan.total_pixels(),
        None,
    );
}

fn emit_progress<F>(
    progress: &mut F,
    plan: &TilePlan,
    phase: RenderPhase,
    completed_tiles: usize,
    completed_pixels: u64,
    tile: Option<RenderTile>,
) where
    F: FnMut(RenderProgressEvent),
{
    progress(RenderProgressEvent {
        progress: RenderProgress {
            phase,
            completed_tiles: completed_tiles.min(u32::MAX as usize) as u32,
            total_tiles: plan.total_tiles().min(u32::MAX as usize) as u32,
            completed_pixels,
            total_pixels: plan.total_pixels(),
        },
        tile,
        execution_mode: plan.execution_mode,
        worker_threads: plan.worker_threads,
    });
}

fn render_tiled_canvas<F, P>(
    size: RenderSize,
    phase: RenderPhase,
    execution_mode: RenderExecutionMode,
    progress: &mut P,
    render_pixel: F,
) -> Canvas
where
    F: Fn(u32, u32) -> [u8; 4] + Sync,
    P: FnMut(RenderProgressEvent),
{
    let plan = TilePlan::for_size(size, execution_mode);
    let mut canvas = Canvas::transparent(size.width, size.height);
    render_tiled_into_canvas(&mut canvas, &plan, phase, progress, render_pixel);
    canvas
}

fn render_tiled_into_canvas<F, P>(
    canvas: &mut Canvas,
    plan: &TilePlan,
    phase: RenderPhase,
    progress: &mut P,
    render_pixel: F,
) where
    F: Fn(u32, u32) -> [u8; 4] + Sync,
    P: FnMut(RenderProgressEvent),
{
    emit_phase_start(progress, plan, phase);

    if plan.tiles.is_empty() {
        emit_phase_complete(progress, plan, phase);
        return;
    }

    let mut completed_tiles = 0usize;
    let mut completed_pixels = 0u64;

    if plan.worker_threads <= 1 {
        for tile in plan.tiles.iter().copied() {
            let pixels = render_tile_pixels(tile, &render_pixel);
            canvas.blit_tile(tile, &pixels);
            completed_tiles += 1;
            completed_pixels += tile.pixel_count();
            emit_progress(
                progress,
                plan,
                phase,
                completed_tiles,
                completed_pixels,
                Some(tile),
            );
        }
        return;
    }

    let (tx, rx) = mpsc::channel::<TileRenderResult>();
    let worker_threads = plan.worker_threads;

    thread::scope(|scope| {
        for worker_index in 0..worker_threads {
            let tx = tx.clone();
            let tiles = &plan.tiles;
            let render_pixel = &render_pixel;
            scope.spawn(move || {
                for tile in tiles
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(tile_index, _)| tile_index % worker_threads == worker_index)
                    .map(|(_, tile)| tile)
                {
                    let pixels = render_tile_pixels(tile, render_pixel);
                    if tx.send(TileRenderResult { tile, pixels }).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);

        for result in rx {
            canvas.blit_tile(result.tile, &result.pixels);
            completed_tiles += 1;
            completed_pixels += result.tile.pixel_count();
            emit_progress(
                progress,
                plan,
                phase,
                completed_tiles,
                completed_pixels,
                Some(result.tile),
            );
        }
    });
}

fn render_tile_pixels<F>(tile: RenderTile, render_pixel: &F) -> Vec<u8>
where
    F: Fn(u32, u32) -> [u8; 4],
{
    let mut pixels = Vec::with_capacity(tile.pixel_count() as usize * 4);
    for y in tile.y..tile.y_end() {
        for x in tile.x..tile.x_end() {
            pixels.extend_from_slice(&render_pixel(x, y));
        }
    }
    pixels
}

fn downscale_lanczos3_premul(
    source: &RgbaImage,
    target_width: u32,
    target_height: u32,
) -> RgbaImage {
    let src_width = source.width() as i32;
    let src_height = source.height() as i32;
    let scale_x = source.width() as f32 / target_width as f32;
    let scale_y = source.height() as f32 / target_height as f32;
    let src = source.as_raw();
    let mut out = vec![0_u8; target_width as usize * target_height as usize * 4];

    for y in 0..target_height {
        let src_y = (y as f32 + 0.5) * scale_y - 0.5;
        let y_start = (src_y - 3.0).floor() as i32;
        let y_end = (src_y + 3.0).ceil() as i32;

        for x in 0..target_width {
            let src_x = (x as f32 + 0.5) * scale_x - 0.5;
            let x_start = (src_x - 3.0).floor() as i32;
            let x_end = (src_x + 3.0).ceil() as i32;

            let mut sum = Vec3::ZERO;
            let mut sum_a = 0.0;
            let mut sum_w = 0.0;

            for sy in y_start..=y_end {
                if sy < 0 || sy >= src_height {
                    continue;
                }
                let wy = lanczos3((src_y - sy as f32) / scale_y.max(1.0));
                for sx in x_start..=x_end {
                    if sx < 0 || sx >= src_width {
                        continue;
                    }
                    let wx = lanczos3((src_x - sx as f32) / scale_x.max(1.0));
                    let w = wx * wy;
                    if w.abs() <= 0.000_001 {
                        continue;
                    }
                    let i = ((sy as u32 * source.width() + sx as u32) * 4) as usize;
                    let a = src[i + 3] as f32 / 255.0;
                    let rgb = Vec3::new(
                        src[i] as f32 / 255.0,
                        src[i + 1] as f32 / 255.0,
                        src[i + 2] as f32 / 255.0,
                    );
                    sum += rgb * a * w;
                    sum_a += a * w;
                    sum_w += w;
                }
            }

            let out_i = ((y * target_width + x) * 4) as usize;
            let alpha = clamp01(if sum_w.abs() > f32::EPSILON {
                sum_a / sum_w
            } else {
                0.0
            });
            let rgb = if alpha > 0.000_1 {
                sum / sum_a.max(0.000_1)
            } else {
                Vec3::ZERO
            };
            out[out_i] = (clamp01(rgb.x) * 255.0) as u8;
            out[out_i + 1] = (clamp01(rgb.y) * 255.0) as u8;
            out[out_i + 2] = (clamp01(rgb.z) * 255.0) as u8;
            out[out_i + 3] = (alpha * 255.0) as u8;
        }
    }

    RgbaImage::from_raw(target_width, target_height, out)
        .expect("downscaled buffer size must match image dimensions")
}

fn apply_visual_quality_postprocess(
    image: &mut RgbaImage,
    profile: &PlanetVisualProfile,
    options: RenderOptions,
) {
    let grade = options.visual_quality_grade();
    if grade <= 0.025 {
        return;
    }

    let gas = is_banded_gas_giant_profile(profile);
    let ocean = is_ocean_world_profile(profile);
    let rocky = is_rocky_world_profile(profile) || is_volcanic_world_profile(profile);
    let ice = profile.ice_fraction > 0.42 || profile_key_contains_any(profile, &["ice", "frozen"]);
    let saturation = 1.0
        + grade
            * if gas {
                0.34
            } else if ocean {
                0.24
            } else if ice {
                0.18
            } else {
                0.16
            };
    let contrast = 1.0
        + grade
            * if rocky {
                0.16
            } else if gas {
                0.14
            } else {
                0.10
            };
    let exposure = 1.0 + grade * if gas { 0.055 } else { 0.035 };
    let pivot = if gas { 0.43 } else { 0.46 };

    for pixel in image.pixels_mut() {
        if pixel[3] == 0 {
            continue;
        }

        let color = Vec3::new(
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
        );
        let luma = color_luma(color);
        let chroma = color - Vec3::splat(luma);
        let mut adjusted = Vec3::splat(luma) + chroma * saturation;
        adjusted = (adjusted - Vec3::splat(pivot)) * contrast + Vec3::splat(pivot);
        adjusted = adjusted * exposure;

        pixel[0] = (clamp01(adjusted.x) * 255.0).round() as u8;
        pixel[1] = (clamp01(adjusted.y) * 255.0).round() as u8;
        pixel[2] = (clamp01(adjusted.z) * 255.0).round() as u8;
    }
}

fn polish_icon_alpha_edge(image: &mut RgbaImage) {
    let width = image.width() as i32;
    let height = image.height() as i32;
    if width < 3 || height < 3 {
        return;
    }

    let src = image.as_raw().clone();
    let mut out = src.clone();

    for y in 0..height {
        for x in 0..width {
            let i = ((y as u32 * image.width() + x as u32) * 4) as usize;
            let center_alpha = src[i + 3] as f32;
            let mut min_alpha = 255_u8;
            let mut max_alpha = 0_u8;
            let mut weighted_alpha = 0.0;
            let mut weighted_rgb = Vec3::ZERO;
            let mut weight_sum = 0.0;

            for oy in -1..=1 {
                let sy = (y + oy).clamp(0, height - 1);
                for ox in -1..=1 {
                    let sx = (x + ox).clamp(0, width - 1);
                    let si = ((sy as u32 * image.width() + sx as u32) * 4) as usize;
                    let alpha = src[si + 3];
                    min_alpha = min_alpha.min(alpha);
                    max_alpha = max_alpha.max(alpha);

                    let weight = match (ox.abs(), oy.abs()) {
                        (0, 0) => 4.0,
                        (0, _) | (_, 0) => 2.0,
                        _ => 1.0,
                    };
                    let alpha_f = alpha as f32 / 255.0;
                    weighted_alpha += alpha as f32 * weight;
                    weighted_rgb += Vec3::new(
                        src[si] as f32 / 255.0,
                        src[si + 1] as f32 / 255.0,
                        src[si + 2] as f32 / 255.0,
                    ) * alpha_f
                        * weight;
                    weight_sum += weight;
                }
            }

            if max_alpha.saturating_sub(min_alpha) < 10 {
                continue;
            }

            let coverage_alpha = weighted_alpha / weight_sum.max(1.0);
            let mix = if center_alpha < 32.0 {
                0.70
            } else if center_alpha > 224.0 {
                0.22
            } else {
                0.48
            };
            let polished_alpha = clamp(
                center_alpha * (1.0 - mix) + coverage_alpha * mix,
                0.0,
                255.0,
            );
            out[i + 3] = polished_alpha.round() as u8;

            if out[i + 3] > 0 {
                let rgb = weighted_rgb / (weighted_alpha / 255.0).max(0.000_1);
                out[i] = (clamp01(rgb.x) * 255.0).round() as u8;
                out[i + 1] = (clamp01(rgb.y) * 255.0).round() as u8;
                out[i + 2] = (clamp01(rgb.z) * 255.0).round() as u8;
            } else {
                out[i] = 0;
                out[i + 1] = 0;
                out[i + 2] = 0;
            }
        }
    }

    *image = RgbaImage::from_raw(image.width(), image.height(), out)
        .expect("polished icon buffer size must match image dimensions");
}

fn lanczos3(x: f32) -> f32 {
    let x = x.abs();
    if x < 0.000_001 {
        1.0
    } else if x >= 3.0 {
        0.0
    } else {
        sinc(x) * sinc(x / 3.0)
    }
}

fn sinc(x: f32) -> f32 {
    let pix = PI * x;
    pix.sin() / pix
}

fn render_planet(
    canvas: &mut Canvas,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    cx: f32,
    cy: f32,
    radius: f32,
) {
    render_planet_with_lighting(canvas, maps, profile, cx, cy, radius, LightingMode::Day);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LightingMode {
    Day,
    Night,
}

impl LightingMode {
    const fn is_night(self) -> bool {
        matches!(self, Self::Night)
    }
}

fn pathtrace_settings_for_render(
    profile: &PlanetVisualProfile,
    options: RenderOptions,
) -> pathtrace::PathTraceSettings {
    let mut settings = pathtrace::PathTraceSettings::preview();
    settings.jitter_seed =
        profile.seed ^ u64::from(profile.snapshot_time_days.to_bits()).rotate_left(17);
    settings.samples_per_pixel = options.shadow_samples.clamp(1, 16);
    settings.max_bounces = if options.ray_traced_reflections { 4 } else { 2 };
    settings.enable_reflections = options.ray_traced_reflections;
    settings.enable_refractions = options.ray_traced_reflections;
    settings.atmosphere_samples = options.atmosphere_samples.clamp(2, 64);
    settings.tile_width = 32;
    settings.tile_height = 32;
    settings
}

fn pathtrace_icon_camera(size: RenderSize) -> pathtrace::Camera {
    let aspect = if size.height == 0 {
        1.0
    } else {
        size.width.max(1) as f32 / size.height.max(1) as f32
    };
    pathtrace::Camera::look_at(
        pathtrace::Vec3::new(0.0, 0.02, 3.12),
        pathtrace::Vec3::ZERO,
        pathtrace::Vec3::Y,
        41.0,
        aspect,
    )
}

fn pathtrace_scene_for_profile(
    profile: &PlanetVisualProfile,
    lighting_mode: LightingMode,
) -> (pathtrace::TraceScene, pathtrace::TraceSurfaceControls) {
    let gas = is_banded_gas_giant_profile(profile);
    let ocean = is_ocean_world_profile(profile) || profile.ocean_fraction > 0.70;
    let volcanic = is_volcanic_world_profile(profile);
    let rocky = is_rocky_world_profile(profile);
    let atmosphere_density = profile
        .atmosphere_density
        .max(if rocky { 0.01 } else { 0.04 });
    let albedo = if gas {
        pathtrace::Vec3::new(0.90, 0.74, 0.48)
    } else if ocean {
        pathtrace::Vec3::new(0.035, 0.18, 0.46)
    } else if volcanic {
        pathtrace::Vec3::new(0.34, 0.22, 0.14)
    } else if rocky {
        pathtrace::Vec3::new(0.46, 0.34, 0.24)
    } else {
        pathtrace::Vec3::new(0.26, 0.39, 0.24).lerp(
            pathtrace::Vec3::new(0.44, 0.33, 0.20),
            1.0 - profile.ocean_fraction,
        )
    };
    let roughness = if ocean {
        0.055
    } else if gas {
        0.74
    } else if volcanic {
        0.58
    } else if rocky {
        0.68
    } else {
        0.46
    };
    let metallic = if ocean {
        0.05
    } else if profile_key_contains_any(profile, &["iron", "metal", "mercury"]) {
        0.08
    } else {
        0.0
    };
    let transmission = if ocean { 0.36 } else { 0.0 };
    let opacity = if ocean { 0.78 } else { 1.0 };
    let horizon_color = if gas {
        pathtrace::Vec3::new(0.90, 0.72, 0.44)
    } else if volcanic {
        pathtrace::Vec3::new(0.86, 0.50, 0.27)
    } else if rocky && atmosphere_density < 0.12 {
        pathtrace::Vec3::new(0.42, 0.45, 0.50)
    } else {
        pathtrace::Vec3::new(0.34, 0.55, 0.95)
    };
    let light = distant_light_for_mode(lighting_mode);
    let night = lighting_mode.is_night();
    let scene = pathtrace::TraceScene {
        planet: pathtrace::Sphere::new(
            pathtrace::Vec3::ZERO,
            1.0,
            pathtrace::MaterialSample {
                albedo,
                roughness,
                metallic,
                transmission,
                opacity,
                index_of_refraction: if ocean { 1.333 } else { 1.47 },
                ..pathtrace::MaterialSample::default()
            },
        ),
        atmosphere_radius: 1.045 + atmosphere_density.min(1.8) * 0.060,
        atmosphere_density,
        light_direction: pathtrace::Vec3::new(
            light.direction[0],
            light.direction[1],
            light.direction[2],
        ),
        sky_color: if night {
            pathtrace::Vec3::new(0.002, 0.006, 0.022)
        } else {
            pathtrace::Vec3::new(0.008, 0.014, 0.040)
        },
        horizon_color,
    };
    let surface_model = if gas {
        pathtrace::TraceSurfaceModel::BandedGasGiant
    } else if ocean {
        pathtrace::TraceSurfaceModel::Ocean
    } else {
        pathtrace::TraceSurfaceModel::Terrestrial
    };
    let surface = pathtrace::TraceSurfaceControls {
        seed: profile.seed,
        time_days: sanitize_snapshot_time_days(profile.snapshot_time_days),
        surface_model,
        ocean_fraction: profile.ocean_fraction.clamp(0.0, 1.0),
        band_frequency: if gas { 18.0 } else { 8.0 },
        band_contrast: if gas {
            0.95
        } else if volcanic {
            0.54
        } else {
            0.34
        },
        cloud_coverage: if gas {
            0.18
        } else {
            profile.cloud_density.clamp(0.0, 1.0)
        },
        cloud_opacity: if gas {
            0.12
        } else {
            (0.18 + profile.cloud_density * 0.50 + atmosphere_density * 0.04).clamp(0.0, 0.86)
        },
        atmosphere_color: scene.horizon_color,
        atmosphere_strength: atmosphere_density.clamp(0.0, 1.0),
    };

    (scene, surface)
}

fn pathtrace_color_to_rgba(color: pathtrace::Vec3, alpha: f32) -> [u8; 4] {
    let mapped = pathtrace::Vec3::new(
        color.x.max(0.0) / (1.0 + color.x.max(0.0)),
        color.y.max(0.0) / (1.0 + color.y.max(0.0)),
        color.z.max(0.0) / (1.0 + color.z.max(0.0)),
    );
    [
        (mapped.x.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8,
        (mapped.y.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8,
        (mapped.z.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8,
        (clamp01(alpha) * 255.0).round() as u8,
    ]
}

fn pathtrace_icon_alpha(
    camera: pathtrace::Camera,
    scene: pathtrace::TraceScene,
    x: u32,
    y: u32,
    size: RenderSize,
) -> f32 {
    let width = size.width.max(1) as f32;
    let height = size.height.max(1) as f32;
    let ray = camera.ray_for_uv((x as f32 + 0.5) / width, (y as f32 + 0.5) / height);
    let to_center = ray.origin - scene.planet.center;
    let along_ray = to_center.dot(ray.direction);
    let closest_d2 = (to_center.length_squared() - along_ray * along_ray).max(0.0);
    let closest = closest_d2.sqrt();
    let pixel_world = scene.atmosphere_radius * (2.35 / width.min(height).max(1.0));
    let body_alpha = 1.0
        - smoothstep(
            scene.planet.radius - pixel_world * 0.65,
            scene.planet.radius + pixel_world * 1.15,
            closest,
        );
    let atmosphere_alpha =
        (1.0 - smoothstep(
            scene.atmosphere_radius - pixel_world * 0.75,
            scene.atmosphere_radius + pixel_world * 1.75,
            closest,
        )) * smoothstep(
            scene.planet.radius - pixel_world,
            scene.atmosphere_radius - pixel_world,
            closest,
        ) * (0.16 + scene.atmosphere_density.clamp(0.0, 1.0) * 0.30);

    body_alpha.max(atmosphere_alpha).clamp(0.0, 1.0)
}

fn render_planet_with_lighting(
    canvas: &mut Canvas,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    cx: f32,
    cy: f32,
    radius: f32,
    lighting_mode: LightingMode,
) {
    let style = planet_render_style(profile);
    let pad = (radius * 0.10) as i32;
    let x0 = ((cx - radius) as i32 - pad).max(0);
    let x1 = ((cx + radius) as i32 + pad).min(canvas.width as i32 - 1);
    let y0 = ((cy - radius) as i32 - pad).max(0);
    let y1 = ((cy + radius) as i32 + pad).min(canvas.height as i32 - 1);
    let rocky_limb = matches!(style, PlanetRenderStyle::RockyWorld);

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5 - cy) / radius;
        for x in x0..=x1 {
            let dx = (x as f32 + 0.5 - cx) / radius;
            let d2 = dx * dx + dy * dy;
            let d = d2.sqrt();
            let limb_offset = if rocky_limb && d > 0.935 {
                let angle_u = (dy.atan2(dx) / (PI * 2.0) + 0.5).rem_euclid(1.0);
                let broad = fbm_periodic(
                    angle_u * 1.7 + 0.13,
                    0.41,
                    17,
                    4,
                    profile.seed + 36_101,
                    0.56,
                );
                let detail = fbm_periodic(
                    angle_u * 4.2 + broad * 0.10,
                    0.73,
                    29,
                    3,
                    profile.seed + 36_223,
                    0.50,
                );
                smoothstep(0.935, 1.015, d) * ((broad - 0.5) * 0.006 + (detail - 0.5) * 0.003)
            } else {
                0.0
            };
            let surface_limit = 1.0 + limb_offset;

            if d <= surface_limit {
                let surface_d = if d > 0.935 {
                    (d / surface_limit.max(0.985)).min(0.999_4)
                } else {
                    d
                };
                let z = (1.0 - surface_d * surface_d).sqrt();
                let edge = 1.0 - smoothstep(0.992, 1.0, surface_d);
                let n = Vec3::new(
                    dx * surface_d / d.max(0.000_1),
                    dy * surface_d / d.max(0.000_1),
                    z,
                )
                .normalize();
                canvas.blend(
                    x,
                    y,
                    shade_surface(n, maps, profile, style, edge, lighting_mode),
                );
            } else if d <= 1.080 {
                let n = Vec3::new(dx / d, dy / d, 0.0).normalize();
                let light_dir = planet_light_dir(lighting_mode);
                let raw_light = n.dot(light_dir);
                let light = raw_light.max(0.0);
                let inner_rim = smoothstep(1.028, 1.000, d);
                let outer_rim = smoothstep(1.080, 1.018, d);
                let shell_texture = fbm_periodic(
                    dx * 0.42 + dy * 0.08 + 0.21,
                    dy * 0.38 - dx * 0.06 + 0.37,
                    47,
                    2,
                    profile.seed + 36_801,
                    0.46,
                );
                let rim = (inner_rim * 0.72 + outer_rim * 0.28) * profile.atmosphere_density;
                let optical_depth = atmosphere_optical_depth(0.0, profile.atmosphere_density);
                let day_air = smoothstep(-0.20, 0.34, raw_light)
                    * if lighting_mode.is_night() { 0.28 } else { 1.0 };
                let sunset = if lighting_mode.is_night() {
                    0.0
                } else {
                    smoothstep(-0.04, 0.22, raw_light) * (1.0 - smoothstep(0.24, 0.58, light))
                };
                let rayleigh = Vec3::new(0.12, 0.36, 1.00) * (0.42 + optical_depth * 0.28);
                let amber = Vec3::new(1.0, 0.45, 0.17);
                let mut color = if matches!(style, PlanetRenderStyle::GasGiant) {
                    Vec3::new(0.92, 0.74, 0.46)
                        .lerp(amber, sunset * 0.40)
                        .lerp(rayleigh, day_air * 0.18)
                        .lerp(
                            Vec3::new(0.10, 0.17, 0.40),
                            if lighting_mode.is_night() { 0.52 } else { 0.0 },
                        )
                } else {
                    rayleigh.lerp(amber, sunset * 0.58).lerp(
                        Vec3::new(0.055, 0.15, 0.44),
                        if lighting_mode.is_night() { 0.46 } else { 0.0 },
                    )
                };
                let refracted_shell = inner_rim * (0.32 + optical_depth * 0.18)
                    + ridge(shell_texture) * outer_rim * 0.10;
                color = color
                    .lerp(
                        Vec3::new(0.42, 0.70, 0.90),
                        refracted_shell * day_air * 0.12,
                    )
                    .lerp(amber, sunset * inner_rim * 0.12);
                color = apply_anti_band_grain(
                    color,
                    dx * 0.5 + 0.5,
                    dy * 0.5 + 0.5,
                    profile.seed + 36_907,
                    0.0026 + outer_rim * 0.0018,
                );
                let rim_strength = if lighting_mode.is_night() { 0.42 } else { 1.0 };
                canvas.blend(
                    x,
                    y,
                    rgba(
                        tone_map(
                            color
                                * rim
                                * rim_strength
                                * (1.10 + optical_depth * 0.24 + shell_texture * 0.10),
                        ),
                        (rim * (if lighting_mode.is_night() { 24.0 } else { 38.0 }
                            + inner_rim * if lighting_mode.is_night() { 8.0 } else { 14.0 }
                            + optical_depth * 16.0)) as u8,
                    ),
                );
            }
        }
    }
}

fn render_terrain_overview_with_progress<P>(
    canvas: &mut Canvas,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    stable_terrain: Option<&StableTerrainContext>,
    execution_mode: RenderExecutionMode,
    lighting_mode: LightingMode,
    progress: &mut P,
) where
    P: FnMut(RenderProgressEvent),
{
    let size = RenderSize {
        width: canvas.width,
        height: canvas.height,
    };
    let plan = TilePlan::for_size(size, execution_mode);
    let camera = TerrainOverviewCamera::for_size(size);
    let (anchor_u, anchor_v, stable_frame, stable_tile) = if let Some(context) = stable_terrain {
        (
            context.frame.anchor_u,
            context.frame.anchor_v,
            context.frame,
            Some(&context.tile),
        )
    } else {
        let (anchor_u, anchor_v) = select_overview_anchor(maps);
        (
            anchor_u,
            anchor_v,
            StableTerrainFrame::for_anchor(maps, anchor_u, anchor_v, profile.seed),
            None,
        )
    };
    let style = planet_render_style(profile);
    let physics = PlanetPhysicsModel::from_profile(profile);

    render_tiled_into_canvas(
        canvas,
        &plan,
        RenderPhase::TerrainOverview,
        progress,
        |x, y| {
            terrain_overview_pixel(
                x,
                y,
                size,
                camera,
                anchor_u,
                anchor_v,
                stable_frame,
                stable_tile,
                maps,
                profile,
                style,
                lighting_mode,
                physics,
            )
        },
    );
}

#[derive(Debug, Clone, Copy)]
struct TerrainOverviewCamera {
    aspect: f32,
    x_scale: f32,
    base_horizon: f32,
    horizon_curve: f32,
    horizon_tilt: f32,
    distance_ground_scale: f32,
    distance_offset: f32,
    spread_scale: f32,
    forward_scale: f32,
    sun_screen: Vec3,
}

impl TerrainOverviewCamera {
    fn for_size(size: RenderSize) -> Self {
        let aspect = if size.height == 0 {
            1.0
        } else {
            size.width.max(1) as f32 / size.height.max(1) as f32
        };
        let wide = clamp01((aspect - 1.0) / 0.80);
        let tall = clamp01((1.0 - aspect) / 0.55);
        let solar_projection = DistantLight::solar_default().projected_overview_screen(aspect);

        // Aspect is part of the camera model, not a post-scale. Portrait narrows
        // horizontal world FOV and raises the sky/foreground balance; landscape
        // opens the view into a wider establishing shot.
        Self {
            aspect,
            x_scale: clamp(aspect, 0.62, 1.82),
            base_horizon: 0.500 + tall * 0.055 - wide * 0.045,
            horizon_curve: 0.020 + wide * 0.006 - tall * 0.006,
            horizon_tilt: 0.006 + wide * 0.002,
            distance_ground_scale: 6.05 + wide * 0.35 - tall * 1.10,
            distance_offset: 0.20 - wide * 0.02 + tall * 0.045,
            spread_scale: 0.94 + wide * 0.18 - tall * 0.10,
            forward_scale: 1.0 - wide * 0.05 + tall * 0.42,
            sun_screen: Vec3::new(
                clamp(
                    solar_projection[0] + tall * 0.020 - wide * 0.010,
                    0.08,
                    0.92,
                ),
                clamp(solar_projection[1] + tall * 0.020, 0.07, 0.78),
                0.0,
            ),
        }
    }

    fn screen_x(self, fx: f32) -> f32 {
        (fx - 0.5) * 2.0 * self.x_scale
    }

    fn local_horizon(self, sx: f32) -> f32 {
        clamp(
            self.base_horizon + sx * sx * self.horizon_curve - sx * self.horizon_tilt,
            0.34,
            0.66,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct TerrainOverviewProjection {
    u: f32,
    v: f32,
    distance: f32,
}

#[derive(Debug, Clone, Copy)]
struct TerrainOverviewHit {
    u: f32,
    v: f32,
    distance: f32,
    ground_v: f32,
    elevation: f32,
    silhouette: f32,
    parallax: f32,
}

#[derive(Debug, Clone, Copy)]
struct TerrainOverviewGradient {
    left: f32,
    right: f32,
    far: f32,
    near: f32,
    average: f32,
    relief: f32,
}

#[derive(Debug, Clone, Copy)]
struct StableTerrainFrame {
    anchor_u: f32,
    anchor_v: f32,
    side_u: f32,
    side_v: f32,
    forward_u: f32,
    forward_v: f32,
    reference_elevation: f32,
}

impl StableTerrainFrame {
    fn for_anchor(maps: &PlanetMaps, anchor_u: f32, anchor_v: f32, seed: u64) -> Self {
        let eps_u = 6.0 / maps.width.max(1) as f32;
        let eps_v = 6.0 / maps.height.max(1) as f32;
        let water_du = maps.sample(anchor_u + eps_u, anchor_v).water
            - maps.sample(anchor_u - eps_u, anchor_v).water;
        let water_dv = maps.sample(anchor_u, anchor_v + eps_v).water
            - maps.sample(anchor_u, anchor_v - eps_v).water;
        let gradient_length = (water_du * water_du + water_dv * water_dv).sqrt();
        let seed_angle = hash2(71, 113, seed + 40_003) * PI * 2.0;
        let (mut forward_u, mut forward_v) = if gradient_length > 0.035 {
            // Face from the water side of the selected coast toward land. The
            // camera origin sits slightly behind the anchor, so this yields a
            // readable foreground water plane, shoreline scale cue, and
            // displaced inland relief rather than staring out over empty sea.
            (-water_du / gradient_length, -water_dv / gradient_length)
        } else {
            (seed_angle.cos(), seed_angle.sin())
        };

        // Do not aim exactly along a discrete map gradient. A small stable
        // yaw gives coastlines depth while keeping the camera predominantly
        // pointed across, rather than along, the shore.
        let yaw = (hash2(127, 31, seed + 40_019) - 0.5) * 0.34;
        let cos_yaw = yaw.cos();
        let sin_yaw = yaw.sin();
        let rotated_u = forward_u * cos_yaw - forward_v * sin_yaw;
        let rotated_v = forward_u * sin_yaw + forward_v * cos_yaw;
        forward_u = rotated_u;
        forward_v = rotated_v;

        Self {
            anchor_u,
            anchor_v,
            side_u: -forward_v,
            side_v: forward_u,
            forward_u,
            forward_v,
            reference_elevation: terrain_overview_elevation(maps.sample(anchor_u, anchor_v)),
        }
    }

    fn map_uv(self, world_x: f32, world_z: f32, warp_x: f32, warp_z: f32) -> (f32, f32) {
        const MAP_SCALE: f32 = 0.029;
        (
            self.anchor_u + (self.side_u * world_x + self.forward_u * world_z) * MAP_SCALE + warp_x,
            self.anchor_v + (self.side_v * world_x + self.forward_v * world_z) * MAP_SCALE + warp_z,
        )
    }
}

#[derive(Debug, Clone)]
struct StableTerrainContext {
    frame: StableTerrainFrame,
    tile: LocalTerrainTile,
}

#[derive(Debug, Clone)]
struct LocalTerrainTile {
    width: usize,
    height: usize,
    x_min: f32,
    x_max: f32,
    z_min: f32,
    z_max: f32,
    heights: Vec<f32>,
    details: Vec<f32>,
    waters: Vec<f32>,
}

impl LocalTerrainTile {
    const WIDTH: usize = 896;
    const HEIGHT: usize = 640;
    const X_MIN: f32 = -7.0;
    const X_MAX: f32 = 7.0;
    const Z_MIN: f32 = -1.0;
    const Z_MAX: f32 = 9.0;

    fn generate(
        frame: StableTerrainFrame,
        maps: &PlanetMaps,
        profile: &PlanetVisualProfile,
    ) -> Self {
        let mut heights = vec![0.0; Self::WIDTH * Self::HEIGHT];
        let mut details = vec![0.0; Self::WIDTH * Self::HEIGHT];
        let mut waters = vec![0.0; Self::WIDTH * Self::HEIGHT];
        let phase = hash2(241, 419, profile.seed + 41_001) * PI * 2.0;
        for y in 0..Self::HEIGHT {
            let z_t = y as f32 / Self::HEIGHT.saturating_sub(1).max(1) as f32;
            let world_z = Self::Z_MIN + (Self::Z_MAX - Self::Z_MIN) * z_t;
            for x in 0..Self::WIDTH {
                let x_t = x as f32 / Self::WIDTH.saturating_sub(1).max(1) as f32;
                let world_x = Self::X_MIN + (Self::X_MAX - Self::X_MIN) * x_t;
                let warp_x = ((world_x * 0.73 + world_z * 0.29 + phase).sin()
                    + (world_x * -0.37 + world_z * 0.91 - phase * 0.7).sin() * 0.46)
                    * 0.0017;
                let warp_z = ((world_x * -0.24 + world_z * 0.68 - phase).sin()
                    + (world_x * 0.83 + world_z * 0.34 + phase * 1.3).sin() * 0.42)
                    * 0.0016;
                let (u, v) = frame.map_uv(world_x, world_z, warp_x, warp_z);
                let sample = maps.sample(u, v);
                let du = 2.6 / maps.width.max(1) as f32;
                let dv = 2.6 / maps.height.max(1) as f32;
                let filtered_water = (sample.water * 6.0
                    + maps.sample(u - du, v).water
                    + maps.sample(u + du, v).water
                    + maps.sample(u, v - dv).water
                    + maps.sample(u, v + dv).water
                    + maps.sample(u - du, v - dv).water * 0.5
                    + maps.sample(u + du, v - dv).water * 0.5
                    + maps.sample(u - du, v + dv).water * 0.5
                    + maps.sample(u + du, v + dv).water * 0.5)
                    / 12.0;
                let water = smoothstep(0.16, 0.84, filtered_water);
                let land = 1.0 - water;
                let continental = fbm_tiled(
                    world_x * 0.070 + frame.anchor_u,
                    world_z * 0.064 + frame.anchor_v,
                    13,
                    5,
                    profile.seed + 41_017,
                    0.54,
                );
                let eroded = fbm_tiled(
                    world_x * 0.145 + continental * 0.10,
                    world_z * 0.132 - continental * 0.08,
                    19,
                    4,
                    profile.seed + 41_029,
                    0.50,
                );
                let rock = fbm_tiled(
                    world_x * 0.34 - eroded * 0.07,
                    world_z * 0.31 + continental * 0.06,
                    31,
                    3,
                    profile.seed + 41_047,
                    0.46,
                );
                let fine = fbm_tiled(
                    world_x * 0.36 + rock * 0.045,
                    world_z * 0.33 - eroded * 0.035,
                    47,
                    2,
                    profile.seed + 41_063,
                    0.43,
                );
                let broad_ridge = ridge(eroded).powf(2.4);
                let rock_ridge = ridge(rock).powf(3.0);
                let drainage = ridge(clamp01(continental * 0.58 + eroded * 0.42)).powf(4.0);
                let detail =
                    clamp01(continental * 0.34 + eroded * 0.30 + rock * 0.22 + fine * 0.14);
                let macro_height =
                    (terrain_overview_elevation(sample) - frame.reference_elevation) * 0.24;
                let local_relief = (continental - 0.5) * 0.045
                    + (eroded - 0.5) * 0.035
                    + (rock - 0.5) * 0.020
                    + (broad_ridge - 0.32) * 0.045
                    + (rock_ridge - 0.24) * 0.056
                    - drainage * 0.020;
                let micro_height = (fine - 0.5) * 0.018 + (rock - 0.5) * 0.018;
                let shore_lift =
                    (1.0 - smoothstep(0.025, 0.24, (sample.water - 0.5).abs())) * land * 0.010;
                let land_height = 0.038 + macro_height + local_relief + micro_height + shore_lift;
                // Use a domain-warped, stochastic wave surface instead of a
                // small set of sinusoids. Perspective turns even oblique
                // periodic waves into implausible screen-space bands, while
                // this multi-scale surface stays irregular at every distance.
                let water_warp = fbm_tiled(
                    world_x * 0.083 + phase * 0.013,
                    world_z * 0.076 - phase * 0.011,
                    17,
                    3,
                    profile.seed + 41_071,
                    0.52,
                );
                let water_swell = fbm_tiled(
                    world_x * 0.128 + (water_warp - 0.5) * 0.18,
                    world_z * 0.112 - (water_warp - 0.5) * 0.15,
                    23,
                    4,
                    profile.seed + 41_077,
                    0.50,
                );
                let water_chop = fbm_tiled(
                    world_x * 0.315 + (water_swell - 0.5) * 0.10,
                    world_z * 0.278 + (water_warp - 0.5) * 0.09,
                    37,
                    3,
                    profile.seed + 41_083,
                    0.44,
                );
                // Keep the geometric ocean as a single stable plane. Wave
                // energy belongs in the shading normal below; displacing a
                // low-angle heightfield makes perspective turn harmless swell
                // into coherent horizontal bands and can open coast cracks.
                let water_height = 0.0;
                let index = y * Self::WIDTH + x;
                let planetary_curvature = (world_x * world_x + world_z * world_z) * 0.0032;
                let coast_grade = smoothstep(0.02, 0.98, land);
                heights[index] = (land_height * coast_grade + water_height * water
                    - planetary_curvature)
                    .clamp(-0.48, 0.34);
                details[index] = detail * land + water_chop * water;
                waters[index] = water;
            }
        }

        // Relax only the geometric coast transition. The material mask stays
        // sharp enough for a narrow wet/foam line, but the underlying land
        // must meet the ocean with a bounded slope or a low camera ray sees a
        // near-vertical dark wall that reads as an intersection hole.
        for _ in 0..14 {
            let previous = heights.clone();
            for y in 1..Self::HEIGHT - 1 {
                for x in 1..Self::WIDTH - 1 {
                    let index = y * Self::WIDTH + x;
                    let water = waters[index];
                    let coast_weight = 4.0 * water * (1.0 - water);
                    if coast_weight <= 0.012 {
                        continue;
                    }
                    let north = previous[index - Self::WIDTH];
                    let south = previous[index + Self::WIDTH];
                    let west = previous[index - 1];
                    let east = previous[index + 1];
                    let northwest = previous[index - Self::WIDTH - 1];
                    let northeast = previous[index - Self::WIDTH + 1];
                    let southwest = previous[index + Self::WIDTH - 1];
                    let southeast = previous[index + Self::WIDTH + 1];
                    let local_mean = (north + south + west + east) * 0.17
                        + (northwest + northeast + southwest + southeast) * 0.08;
                    heights[index] =
                        previous[index] + (local_mean - previous[index]) * coast_weight * 0.52;
                }
            }
        }

        Self {
            width: Self::WIDTH,
            height: Self::HEIGHT,
            x_min: Self::X_MIN,
            x_max: Self::X_MAX,
            z_min: Self::Z_MIN,
            z_max: Self::Z_MAX,
            heights,
            details,
            waters,
        }
    }

    fn sample(&self, world_x: f32, world_z: f32) -> (f32, f32, f32) {
        let tx = clamp01((world_x - self.x_min) / (self.x_max - self.x_min).max(0.001))
            * self.width.saturating_sub(1) as f32;
        let tz = clamp01((world_z - self.z_min) / (self.z_max - self.z_min).max(0.001))
            * self.height.saturating_sub(1) as f32;
        let x0 = tx.floor() as usize;
        let z0 = tz.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let z1 = (z0 + 1).min(self.height - 1);
        let fx = tx - tx.floor();
        let fz = tz - tz.floor();
        let i00 = z0 * self.width + x0;
        let i10 = z0 * self.width + x1;
        let i01 = z1 * self.width + x0;
        let i11 = z1 * self.width + x1;
        (
            bilerp(
                self.heights[i00],
                self.heights[i10],
                self.heights[i01],
                self.heights[i11],
                fx,
                fz,
            ),
            bilerp(
                self.details[i00],
                self.details[i10],
                self.details[i01],
                self.details[i11],
                fx,
                fz,
            ),
            bilerp(
                self.waters[i00],
                self.waters[i10],
                self.waters[i01],
                self.waters[i11],
                fx,
                fz,
            ),
        )
    }

    fn sample_smooth(&self, world_x: f32, world_z: f32) -> (f32, f32, f32) {
        (
            self.sample_channel_smooth(&self.heights, world_x, world_z),
            self.sample_channel_smooth(&self.details, world_x, world_z),
            clamp01(self.sample_channel_smooth(&self.waters, world_x, world_z)),
        )
    }

    fn sample_height_smooth(&self, world_x: f32, world_z: f32) -> f32 {
        self.sample_channel_smooth(&self.heights, world_x, world_z)
    }

    fn sample_channel_smooth(&self, values: &[f32], world_x: f32, world_z: f32) -> f32 {
        let tx = clamp01((world_x - self.x_min) / (self.x_max - self.x_min).max(0.001))
            * self.width.saturating_sub(1) as f32;
        let tz = clamp01((world_z - self.z_min) / (self.z_max - self.z_min).max(0.001))
            * self.height.saturating_sub(1) as f32;
        let x1 = tx.floor() as isize;
        let z1 = tz.floor() as isize;
        let fx = tx - tx.floor();
        let fz = tz - tz.floor();
        let mut rows = [0.0; 4];
        let mut neighborhood_min = f32::MAX;
        let mut neighborhood_max = f32::MIN;
        for (row_index, dz) in (-1_isize..=2).enumerate() {
            let z = (z1 + dz).clamp(0, self.height.saturating_sub(1) as isize) as usize;
            let mut samples = [0.0; 4];
            for (column_index, dx) in (-1_isize..=2).enumerate() {
                let x = (x1 + dx).clamp(0, self.width.saturating_sub(1) as isize) as usize;
                let value = values[z * self.width + x];
                neighborhood_min = neighborhood_min.min(value);
                neighborhood_max = neighborhood_max.max(value);
                samples[column_index] = value;
            }
            rows[row_index] = catmull_rom(samples[0], samples[1], samples[2], samples[3], fx);
        }
        catmull_rom(rows[0], rows[1], rows[2], rows[3], fz)
            .clamp(neighborhood_min, neighborhood_max)
    }
}

fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * (2.0 * p1
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

fn terrain_overview_project(
    camera: TerrainOverviewCamera,
    anchor_u: f32,
    anchor_v: f32,
    sx: f32,
    ground_v: f32,
) -> TerrainOverviewProjection {
    let ground_v = clamp01(ground_v);
    let distance = 1.0 / (ground_v * camera.distance_ground_scale + camera.distance_offset);
    // A production surface camera needs enough lateral footprint in the near
    // field to avoid magnifying a handful of global map texels into radial
    // streaks. The perspective term still converges at the vanishing point,
    // while this finite camera-footprint term keeps foreground materials and
    // shorelines spatially resolved.
    let spread = (0.044 + distance * 0.034) * camera.spread_scale;
    let forward = distance * 0.035 * camera.forward_scale;
    let perspective_x = sx / camera.x_scale.max(1.0).sqrt();

    TerrainOverviewProjection {
        u: anchor_u
            + sx * spread
            + ground_v * 0.012
            + perspective_x * perspective_x * distance * 0.004,
        v: anchor_v - forward + perspective_x * perspective_x * distance * 0.006,
        distance,
    }
}

fn terrain_overview_shore(sample: SurfaceSample) -> f32 {
    1.0 - smoothstep(0.030, 0.36, (clamp01(sample.water) - 0.50).abs())
}

fn terrain_overview_elevation(sample: SurfaceSample) -> f32 {
    let water = clamp01(sample.water);
    let land = 1.0 - water;
    let shore = terrain_overview_shore(sample);
    let shelf = shore * (0.030 + land * 0.060 + (1.0 - water) * 0.020);
    let land_lift = land * (0.070 + smoothstep(0.50, 0.92, sample.height) * 0.125);
    let shallow_water_lift = water * smoothstep(0.28, 0.64, sample.height) * 0.040;

    clamp01(
        sample.height * (0.58 + land * 0.38) + land_lift + shelf + shallow_water_lift
            - water * 0.050,
    )
}

fn terrain_overview_filtered_elevation(maps: &PlanetMaps, u: f32, v: f32, distance: f32) -> f32 {
    let center = terrain_overview_elevation(maps.sample(u, v));
    let filter = smoothstep(0.72, 4.65, distance);
    if filter <= 0.001 {
        return center;
    }

    let radius = 0.65 + filter * 3.2;
    let eps_u = radius / maps.width.max(1) as f32;
    let eps_v = radius / maps.height.max(1) as f32;
    let cross = (terrain_overview_elevation(maps.sample(u - eps_u, v))
        + terrain_overview_elevation(maps.sample(u + eps_u, v))
        + terrain_overview_elevation(maps.sample(u, v - eps_v))
        + terrain_overview_elevation(maps.sample(u, v + eps_v)))
        * 0.25;
    center + (cross - center) * filter * 0.72
}

fn terrain_overview_height_lift(elevation: f32, ground_v: f32, distance: f32) -> f32 {
    let horizon_gain = smoothstep(0.72, 0.0, ground_v);
    let distance_gain = smoothstep(0.42, 5.20, distance);
    let lift_scale = 0.014 + horizon_gain * 0.054 + distance_gain * 0.020;
    elevation * lift_scale * (0.72 + elevation * 0.48)
}

fn terrain_overview_trace(
    camera: TerrainOverviewCamera,
    anchor_u: f32,
    anchor_v: f32,
    sx: f32,
    pixel_ground_v: f32,
    maps: &PlanetMaps,
) -> TerrainOverviewHit {
    let screen_ground_v = clamp01(pixel_ground_v);
    let base_projection = terrain_overview_project(camera, anchor_u, anchor_v, sx, screen_ground_v);
    let base_elevation = terrain_overview_filtered_elevation(
        maps,
        base_projection.u,
        base_projection.v,
        base_projection.distance,
    );
    let mut best = TerrainOverviewHit {
        u: base_projection.u,
        v: base_projection.v,
        distance: base_projection.distance,
        ground_v: screen_ground_v,
        elevation: base_elevation,
        silhouette: if pixel_ground_v >= 0.0 { 1.0 } else { 0.0 },
        parallax: 0.0,
    };
    let mut best_score = f32::MIN;

    let horizon_bias = smoothstep(0.62, 0.0, screen_ground_v);
    let scan_window = (0.035 + base_projection.distance * 0.018 + horizon_bias * 0.055)
        * (1.0 - screen_ground_v * 0.60);
    const SAMPLES: usize = 8;

    for step in 0..SAMPLES {
        let t = step as f32 / (SAMPLES - 1) as f32;
        let eased_t = t * t * (3.0 - 2.0 * t);
        let layer_ground_v = clamp(screen_ground_v + eased_t * scan_window, 0.0, 0.985);
        let projection = terrain_overview_project(camera, anchor_u, anchor_v, sx, layer_ground_v);
        let elevation = terrain_overview_filtered_elevation(
            maps,
            projection.u,
            projection.v,
            projection.distance,
        );
        let lift = terrain_overview_height_lift(elevation, layer_ground_v, projection.distance);
        let projected_ground_v = layer_ground_v - lift;
        let coverage = pixel_ground_v - projected_ground_v;
        let tolerance = -0.004 - horizon_bias * 0.004;

        if coverage < tolerance {
            continue;
        }

        let ray_error = (projected_ground_v - pixel_ground_v).abs();
        let thickness = (0.008 + lift * 0.72 + horizon_bias * 0.008).max(0.001);
        let proximity = 1.0 - clamp01(ray_error / thickness);
        let height_score = smoothstep(0.18, 0.82, elevation);
        let score = proximity * 0.78
            + height_score * 0.16
            + t * (0.10 + screen_ground_v * 0.04)
            + coverage.max(0.0).min(0.04) * 0.35;

        if score > best_score {
            best_score = score;
            best = TerrainOverviewHit {
                u: projection.u,
                v: projection.v,
                distance: projection.distance,
                ground_v: layer_ground_v,
                elevation,
                silhouette: if pixel_ground_v < 0.0 {
                    smoothstep(-0.003, 0.017, coverage) * (0.72 + height_score * 0.28)
                } else {
                    1.0
                },
                parallax: t,
            };
        }
    }

    best
}

fn terrain_overview_gradient(
    maps: &PlanetMaps,
    u: f32,
    v: f32,
    eps_u: f32,
    eps_v: f32,
    center: f32,
) -> TerrainOverviewGradient {
    let left = terrain_overview_elevation(maps.sample(u - eps_u, v));
    let right = terrain_overview_elevation(maps.sample(u + eps_u, v));
    let far = terrain_overview_elevation(maps.sample(u, v - eps_v));
    let near = terrain_overview_elevation(maps.sample(u, v + eps_v));
    let average = (left + right + far + near) * 0.25;
    let relief = clamp01(
        ((left - right).abs() + (near - far).abs()) * 18.0 + (average - center).abs() * 5.0,
    );

    TerrainOverviewGradient {
        left,
        right,
        far,
        near,
        average,
        relief,
    }
}

#[derive(Debug, Clone, Copy)]
struct TerrainOverviewLod {
    footprint: f32,
    material_filter: f32,
    foreground_detail: f32,
    streak_weight: f32,
}

fn terrain_overview_lod(distance: f32, ground_v: f32, parallax: f32) -> TerrainOverviewLod {
    let foreground =
        smoothstep(0.34, 0.94, ground_v) * (1.0 - smoothstep(0.70, 2.35, distance) * 0.78);
    let far = smoothstep(1.05, 4.85, distance);
    let horizon = smoothstep(0.18, 0.0, ground_v);
    TerrainOverviewLod {
        footprint: clamp(
            0.30 + distance * 0.58 + parallax * 0.16 + far * 0.34 - foreground * 0.13,
            0.24,
            5.20,
        ),
        material_filter: clamp01(far * 0.66 + horizon * 0.18),
        foreground_detail: foreground,
        streak_weight: clamp01(1.0 - foreground * 0.86),
    }
}

fn terrain_overview_sample_lod(
    maps: &PlanetMaps,
    u: f32,
    v: f32,
    eps_u: f32,
    eps_v: f32,
    filter: f32,
) -> SurfaceSample {
    let center = maps.sample(u, v);
    if filter <= 0.001 {
        return center;
    }

    let spread = 0.85 + filter * 1.15;
    let east = maps.sample(u + eps_u * spread, v);
    let west = maps.sample(u - eps_u * spread, v);
    let north = maps.sample(u, v - eps_v * spread);
    let south = maps.sample(u, v + eps_v * spread);
    let averaged = weighted_surface_sample(center, east, west, north, south);

    lerp_surface_sample(center, averaged, filter * 0.56)
}

fn weighted_surface_sample(
    center: SurfaceSample,
    east: SurfaceSample,
    west: SurfaceSample,
    north: SurfaceSample,
    south: SurfaceSample,
) -> SurfaceSample {
    const CENTER: f32 = 0.44;
    const SIDE: f32 = 0.14;

    SurfaceSample {
        albedo: center.albedo * CENTER
            + (east.albedo + west.albedo + north.albedo + south.albedo) * SIDE,
        height: center.height * CENTER
            + (east.height + west.height + north.height + south.height) * SIDE,
        water: center.water * CENTER + (east.water + west.water + north.water + south.water) * SIDE,
        cloud: center.cloud * CENTER + (east.cloud + west.cloud + north.cloud + south.cloud) * SIDE,
        city: center.city * CENTER + (east.city + west.city + north.city + south.city) * SIDE,
        vegetation: center.vegetation * CENTER
            + (east.vegetation + west.vegetation + north.vegetation + south.vegetation) * SIDE,
        biome: center.biome * CENTER + (east.biome + west.biome + north.biome + south.biome) * SIDE,
        roughness: center.roughness * CENTER
            + (east.roughness + west.roughness + north.roughness + south.roughness) * SIDE,
        wetness: center.wetness * CENTER
            + (east.wetness + west.wetness + north.wetness + south.wetness) * SIDE,
        ambient_occlusion: center.ambient_occlusion * CENTER
            + (east.ambient_occlusion
                + west.ambient_occlusion
                + north.ambient_occlusion
                + south.ambient_occlusion)
                * SIDE,
        horizon_occlusion: center.horizon_occlusion * CENTER
            + (east.horizon_occlusion
                + west.horizon_occlusion
                + north.horizon_occlusion
                + south.horizon_occlusion)
                * SIDE,
    }
}

fn lerp_surface_sample(a: SurfaceSample, b: SurfaceSample, t: f32) -> SurfaceSample {
    let t = clamp01(t);
    SurfaceSample {
        albedo: a.albedo.lerp(b.albedo, t),
        height: a.height + (b.height - a.height) * t,
        water: a.water + (b.water - a.water) * t,
        cloud: a.cloud + (b.cloud - a.cloud) * t,
        city: a.city + (b.city - a.city) * t,
        vegetation: a.vegetation + (b.vegetation - a.vegetation) * t,
        biome: a.biome + (b.biome - a.biome) * t,
        roughness: a.roughness + (b.roughness - a.roughness) * t,
        wetness: a.wetness + (b.wetness - a.wetness) * t,
        ambient_occlusion: a.ambient_occlusion + (b.ambient_occlusion - a.ambient_occlusion) * t,
        horizon_occlusion: a.horizon_occlusion + (b.horizon_occlusion - a.horizon_occlusion) * t,
    }
}

#[allow(clippy::too_many_arguments)]
fn terrain_overview_pixel(
    x: u32,
    y: u32,
    size: RenderSize,
    camera: TerrainOverviewCamera,
    anchor_u: f32,
    anchor_v: f32,
    stable_frame: StableTerrainFrame,
    stable_tile: Option<&LocalTerrainTile>,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    style: PlanetRenderStyle,
    lighting_mode: LightingMode,
    physics: PlanetPhysicsModel,
) -> [u8; 4] {
    if matches!(style, PlanetRenderStyle::GasGiant) {
        return gas_giant_overview_pixel(x, y, size, anchor_u, anchor_v, maps, profile);
    }

    let night_view = lighting_mode.is_night();
    let ocean_world = matches!(style, PlanetRenderStyle::OceanWorld);
    let rocky_material = rocky_surface_material(profile);
    let rocky_world = matches!(style, PlanetRenderStyle::RockyWorld);
    let volcanic_world = matches!(style, PlanetRenderStyle::VolcanicWorld);
    let dry_world = rocky_world || volcanic_world || profile.ocean_fraction < 0.10;
    let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
    let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
    let sx = camera.screen_x(fx);
    let local_horizon = camera.local_horizon(sx);
    let atmosphere = clamp(profile.atmosphere_density, 0.0, 1.6);
    let ocean_air = if ocean_world {
        smoothstep(0.70, 1.0, profile.ocean_fraction) * (0.62 + atmosphere * 0.24)
    } else {
        0.0
    };
    let volcanic_air = if volcanic_world {
        clamp01(rocky_material.ash * 0.44 + rocky_material.sulfur * 0.34 + atmosphere * 0.20)
    } else {
        0.0
    };
    let sky_top =
        Vec3::new(0.010, 0.026, 0.078).lerp(Vec3::new(0.020, 0.044, 0.120), atmosphere * 0.30);
    let sky_mid =
        Vec3::new(0.070, 0.168, 0.330).lerp(Vec3::new(0.100, 0.220, 0.410), atmosphere * 0.26);
    let sky_horizon = Vec3::new(0.340, 0.540, 0.740)
        .lerp(Vec3::new(0.480, 0.620, 0.720), atmosphere * 0.34)
        .lerp(Vec3::new(0.450, 0.650, 0.780), ocean_air * 0.26)
        .lerp(Vec3::new(0.680, 0.540, 0.350), volcanic_air * 0.32);
    let horizon_air = Vec3::new(0.560, 0.640, 0.640)
        .lerp(Vec3::new(0.780, 0.590, 0.380), atmosphere * 0.12)
        .lerp(Vec3::new(0.540, 0.700, 0.730), ocean_air * 0.22)
        .lerp(Vec3::new(0.820, 0.550, 0.270), volcanic_air * 0.36);
    let light_dir = overview_light_dir(lighting_mode);

    let sky_color = if fy < local_horizon + 0.22 {
        // Keep one atmospheric solution alive slightly below the geometric
        // horizon. Rays in this transition can either miss the curved planet
        // or hit distant terrain; terminating the sky exactly at the horizon
        // gave those two cases different fallback colors and produced visible
        // concentric contour bands.
        let sky_v = clamp01(fy / local_horizon.max(0.001));
        let horizon_blend = smoothstep(0.50, 1.0, sky_v);
        let sky_depth = atmosphere_optical_depth(1.0 - horizon_blend * 0.92, atmosphere);
        let mut color = if night_view {
            let zenith = Vec3::new(0.002, 0.006, 0.024)
                .lerp(Vec3::new(0.006, 0.014, 0.046), atmosphere * 0.20);
            let horizon = Vec3::new(0.014, 0.034, 0.088)
                .lerp(Vec3::new(0.030, 0.065, 0.145), atmosphere * 0.28)
                .lerp(Vec3::new(0.018, 0.045, 0.094), ocean_air * 0.22)
                .lerp(Vec3::new(0.090, 0.055, 0.040), volcanic_air * 0.36);
            zenith.lerp(horizon, horizon_blend * (0.64 + sky_depth * 0.08))
        } else {
            let mut color = sky_top.lerp(sky_mid, smoothstep(0.02, 0.72, sky_v));
            color = color.lerp(sky_horizon, horizon_blend * (0.62 + sky_depth * 0.10));
            color += Vec3::new(0.18, 0.42, 1.0) * horizon_blend * sky_depth * 0.032;
            color += horizon_air * horizon_blend * sky_depth * 0.090;
            let marine_haze = ocean_air * horizon_blend * (0.045 + sky_depth * 0.075);
            color.lerp(Vec3::new(0.670, 0.800, 0.880), marine_haze)
        };
        let air_warp = fbm_tiled(
            fx * 0.30 + sky_v * 0.11 + anchor_u * 0.09,
            sky_v * 0.22 - fx * 0.04 + anchor_v * 0.07,
            31,
            3,
            profile.seed + 12_019,
            0.50,
        );
        let air_micro = fbm_tiled(
            fx * 1.10 + sky_v * 0.38 + air_warp * 0.17,
            sky_v * 0.82 - fx * 0.16 + air_warp * 0.11,
            83,
            2,
            profile.seed + 12_071,
            0.45,
        );
        let refracted_horizon = horizon_blend * (0.018 + atmosphere * 0.020 + ocean_air * 0.014);
        color = color.lerp(
            horizon_air,
            refracted_horizon * (0.52 + air_warp * 0.36 + ridge(air_micro) * 0.12),
        );
        color += Vec3::new(0.055, 0.110, 0.190)
            * (air_micro - 0.5)
            * horizon_blend
            * (0.018 + atmosphere * 0.018);
        color = apply_anti_band_grain(
            color,
            fx + anchor_u * 0.13,
            sky_v + anchor_v * 0.09,
            profile.seed + 12_113,
            if night_view { 0.0045 } else { 0.0032 } + horizon_blend * 0.0020,
        );

        let sun_dx = (fx - camera.sun_screen.x) * camera.aspect.max(0.1);
        let sun_dy = fy - camera.sun_screen.y;
        let sun_dist = (sun_dx * sun_dx + sun_dy * sun_dy).sqrt();
        if night_view {
            let moon_dx = (fx - 0.74) * camera.aspect.max(0.1);
            let moon_dy = fy - 0.185;
            let moon_dist = (moon_dx * moon_dx + moon_dy * moon_dy).sqrt();
            let moon_core = smoothstep(0.030, 0.0, moon_dist);
            let moon_glow = smoothstep(0.240, 0.0, moon_dist);
            color += Vec3::new(0.72, 0.80, 1.0) * moon_core * 0.82;
            color += Vec3::new(0.16, 0.24, 0.55) * moon_glow * (0.13 + atmosphere * 0.035);
            color += Vec3::new(0.030, 0.070, 0.180) * horizon_blend * sky_depth * 0.050;
        } else {
            let sun_core = smoothstep(0.032, 0.0, sun_dist);
            let sun_glow = smoothstep(0.410, 0.0, sun_dist);
            let sun_corona = smoothstep(0.145, 0.0, sun_dist);
            let ray_forward = sun_dy.max(0.0);
            let ray_axis = sun_dx.abs() * (0.30 + ray_forward * 0.16) + ray_forward * 0.018;
            let ray_warp = fbm_tiled(
                fx * 0.36 + sky_v * 0.10 + anchor_u * 0.11,
                sky_v * 0.30 - fx * 0.05 + anchor_v * 0.09,
                23,
                3,
                profile.seed + 12_891,
                0.50,
            );
            let ray_noise = fbm_tiled(
                fx * 0.64 + sky_v * 0.22 + ray_warp * 0.16 + anchor_u * 0.07,
                sky_v * 0.52 - fx * 0.10 + ray_warp * 0.10 + anchor_v * 0.06,
                37,
                4,
                profile.seed + 12_913,
                0.52,
            );
            let ray_body = smoothstep(
                0.32,
                0.88,
                ray_noise * 0.66 + ray_warp * 0.22 + horizon_blend * 0.12,
            );
            let crepuscular = smoothstep(0.220, 0.0, ray_axis)
                * smoothstep(0.015, 0.28, ray_forward)
                * smoothstep(camera.sun_screen.y, local_horizon + 0.060, fy)
                * smoothstep(1.0, 0.28, sky_v)
                * (0.44 + ray_body * 0.56);
            let low_sun_warmth = smoothstep(0.18, 0.72, sky_v) * (0.72 + sky_depth * 0.55);
            color += Vec3::new(6.4, 5.2, 3.4) * sun_core;
            color += Vec3::new(1.55, 1.05, 0.48) * sun_corona * (0.16 + low_sun_warmth * 0.11);
            color += Vec3::new(1.0, 0.55, 0.23) * sun_glow * (0.18 + low_sun_warmth * 0.10);
            color += Vec3::new(1.00, 0.70, 0.36) * crepuscular * (0.10 + ocean_air * 0.045);
        }

        if profile.ringed {
            let ring_alpha = terrain_sky_ring_alpha(fx, fy, camera);
            let ring_light =
                0.52 + smoothstep(-0.22, 0.78, light_dir.x * -0.5 + light_dir.y) * 0.36;
            let ring_color = if night_view {
                Vec3::new(0.26, 0.31, 0.48) * (0.42 + ring_light * 0.35)
            } else {
                Vec3::new(0.80, 0.74, 0.60) * ring_light
            };
            color = color.lerp(
                ring_color,
                ring_alpha * if night_view { 0.46 } else { 0.78 },
            );
        }

        if !night_view {
            if let Some((moon_color, moon_alpha)) =
                terrain_sky_moon(fx, fy, profile.seed, light_dir, camera.aspect)
            {
                color = color.lerp(moon_color, moon_alpha);
            }
        }

        let sky_warp = fbm_tiled(
            fx * 0.24 + anchor_u * 0.17,
            sky_v * 0.18 + anchor_v * 0.13,
            17,
            3,
            profile.seed + 12_287,
            0.50,
        );
        let cloud = fbm_tiled(
            fx * 0.46 + sky_v * 0.10 + sky_warp * 0.10 + anchor_u * 0.12,
            sky_v * 0.34 - fx * 0.05 + sky_warp * 0.08 + anchor_v * 0.10,
            19,
            5,
            profile.seed + 12_331,
            0.55,
        );
        let high_cloud = fbm_tiled(
            fx * 0.82 + sky_v * 0.18 + sky_warp * 0.16 + anchor_u * 0.16,
            sky_v * 0.52 - fx * 0.10 + cloud * 0.11 + anchor_v * 0.12,
            47,
            3,
            profile.seed + 12_377,
            0.49,
        );
        let cloud_relief = clamp01((cloud - 0.50) * 1.25 + ridge(high_cloud) * 0.36);
        let cloud_alpha = smoothstep(
            0.62,
            0.89,
            cloud * 0.74 + high_cloud * 0.18 + ridge(sky_warp) * 0.08,
        ) * smoothstep(0.04, 0.40, sky_v)
            * (0.86 + cloud_relief * 0.34);
        let cloud_alpha = cloud_alpha
            * if ocean_world {
                1.22 + ocean_air * 0.18
            } else {
                1.0
            };
        let cloud_altitude = smoothstep(0.18, 0.92, sky_v) * (0.46 + atmosphere * 0.16)
            + high_cloud * 0.22
            + ocean_air * 0.08;
        let cloud_color = if night_view {
            Vec3::new(0.040, 0.060, 0.120).lerp(Vec3::new(0.12, 0.16, 0.30), horizon_blend)
        } else {
            Vec3::new(0.82, 0.86, 0.84)
                .lerp(Vec3::new(0.98, 0.99, 0.96), cloud_altitude)
                .lerp(
                    Vec3::new(0.74, 0.83, 0.87),
                    ocean_air * horizon_blend * 0.20,
                )
        };
        let cloud_underlight = horizon_blend
            * (1.0 - cloud_altitude * 0.48)
            * cloud_alpha
            * (0.035 + ocean_air * 0.020);
        color = color.lerp(
            horizon_air,
            cloud_underlight * if night_view { 0.32 } else { 0.52 },
        );
        color = color.lerp(
            cloud_color,
            cloud_alpha * if night_view { 0.19 } else { 0.32 },
        );
        color += Vec3::new(0.82, 0.92, 1.0)
            * ridge(high_cloud)
            * cloud_alpha
            * cloud_altitude
            * if night_view { 0.010 } else { 0.026 };
        if ocean_world {
            let marine_bank_warp = fbm_tiled(
                fx * 0.34 + sky_v * 0.08 + anchor_u * 0.08,
                sky_v * 0.28 - fx * 0.04 + anchor_v * 0.07,
                29,
                3,
                profile.seed + 13_291,
                0.52,
            );
            let marine_bank_mass = fbm_tiled(
                fx * 0.72 + sky_v * 0.22 + marine_bank_warp * 0.16 + anchor_u * 0.10,
                sky_v * 0.52 - fx * 0.06 + marine_bank_warp * 0.11 + anchor_v * 0.08,
                37,
                5,
                profile.seed + 13_337,
                0.56,
            );
            let marine_bank_detail = fbm_tiled(
                fx * 1.80 + sky_v * 0.36 + marine_bank_mass * 0.18,
                sky_v * 0.92 - fx * 0.16 + marine_bank_warp * 0.12,
                67,
                3,
                profile.seed + 13_409,
                0.48,
            );
            let marine_bank = smoothstep(
                0.46,
                0.88,
                marine_bank_mass * 0.58 + ridge(marine_bank_detail) * 0.26 + ocean_air * 0.08,
            ) * smoothstep(0.48, 0.98, sky_v)
                * (1.0 - smoothstep(0.985, 1.0, sky_v))
                * profile.cloud_density;
            color = color.lerp(
                if night_view {
                    Vec3::new(0.070, 0.095, 0.180)
                } else {
                    Vec3::new(0.82, 0.89, 0.90)
                },
                marine_bank * if night_view { 0.22 } else { 0.46 },
            );
        }

        let star = hash2(x as i32, y as i32, profile.seed + 77);
        let star_threshold = if night_view { 0.9940 } else { 0.9985 };
        if sky_v < if night_view { 0.62 } else { 0.30 } && star > star_threshold {
            let intensity = (star - star_threshold) / (1.0 - star_threshold);
            color += Vec3::new(0.70, 0.78, 1.0).lerp(Vec3::new(1.0, 0.88, 0.62), star)
                * intensity.powf(1.7)
                * if night_view { 0.95 } else { 0.70 };
        }
        if night_view {
            let aurora_sample = physics.sample(
                fx + anchor_u * 0.13,
                clamp01(0.02 + sky_v * 0.18 + anchor_v * 0.05),
            );
            let aurora_warp = fbm_tiled(
                fx * 0.46 + anchor_u * 0.06,
                sky_v * 0.22 + anchor_v * 0.06,
                31,
                3,
                profile.seed + 14_037,
                0.50,
            );
            let curtain = fbm_tiled(
                fx * 0.95 + aurora_warp * 0.22 + aurora_sample.magnetic_field_microtesla * 0.0008,
                sky_v * 0.48 + aurora_warp * 0.14 + anchor_v * 0.08,
                47,
                4,
                profile.seed + 14_101,
                0.54,
            );
            let aurora = aurora_sample.aurora_power
                * smoothstep(0.08, 0.58, sky_v)
                * (1.0 - smoothstep(0.82, 1.0, sky_v))
                * smoothstep(0.50, 0.92, curtain);
            color += Vec3::new(0.10, 0.58, 0.42).lerp(Vec3::new(0.30, 0.18, 0.70), curtain)
                * aurora
                * 0.34;
        }

        Some(color * if night_view { 1.0 } else { 0.72 })
    } else {
        None
    };

    if ocean_world {
        return ocean_world_terrain_overview_pixel(
            fx,
            fy,
            sx,
            local_horizon,
            camera.aspect,
            camera.sun_screen,
            sky_color,
            profile,
            lighting_mode,
            physics,
        );
    }

    if dry_world {
        return dry_world_terrain_overview_pixel(
            fx,
            fy,
            sx,
            local_horizon,
            camera.aspect,
            sky_color,
            profile,
            rocky_material,
            volcanic_world,
            lighting_mode,
        );
    }

    if matches!(style, PlanetRenderStyle::Terrestrial) {
        if let Some(tile) = stable_tile {
            return raymarched_terrain_overview_pixel(
                fx,
                fy,
                sx,
                local_horizon,
                camera.aspect,
                camera.sun_screen,
                sky_color,
                stable_frame,
                tile,
                maps,
                profile,
                lighting_mode,
            );
        }
        return stable_terrain_overview_pixel(
            fx,
            fy,
            sx,
            local_horizon,
            camera.aspect,
            camera.sun_screen,
            sky_color,
            stable_frame,
            maps,
            profile,
            lighting_mode,
        );
    }

    let pixel_ground_v = (fy - local_horizon) / (1.0 - local_horizon);
    let pixel_ground_v = if !ocean_world && !dry_world && pixel_ground_v > 0.0 {
        let ground = clamp01(pixel_ground_v);
        0.72 * (1.0 - (1.0 - ground).powf(1.18))
    } else {
        pixel_ground_v
    };
    if let Some(sky) = sky_color {
        if pixel_ground_v < -0.125 {
            return rgba(tone_map(sky), 255);
        }
    }

    let hit = terrain_overview_trace(camera, anchor_u, anchor_v, sx, pixel_ground_v, maps);
    if let Some(sky) = sky_color {
        if hit.silhouette <= 0.002 {
            return rgba(tone_map(sky), 255);
        }
    }

    let ground_v = clamp01(hit.ground_v.max(pixel_ground_v));
    let distance = hit.distance;
    let u = hit.u;
    let v = hit.v;
    let lod = terrain_overview_lod(distance, ground_v, hit.parallax);
    let eps_u = lod.footprint / maps.width as f32;
    let eps_v = lod.footprint / maps.height as f32;
    let gradient = terrain_overview_gradient(maps, u, v, eps_u, eps_v, hit.elevation);
    let right = gradient.right;
    let left = gradient.left;
    let far = gradient.far;
    let near = gradient.near;
    let relief = gradient.relief;
    let sample = terrain_overview_sample_lod(maps, u, v, eps_u, eps_v, lod.material_filter);
    let physics_sample = physics.sample(u, v);
    let cloud_altitude = clamp01(
        0.34 + physics_sample.cloud_lift * 0.34
            + atmosphere * 0.12
            + sample.cloud * 0.10
            + smoothstep(0.30, 0.88, sample.water) * 0.04,
    );
    let raw_water = clamp01(sample.water);
    let shore = terrain_overview_shore(sample);
    let water = smoothstep(
        0.44 - lod.foreground_detail * 0.020,
        0.64 + lod.foreground_detail * 0.030,
        raw_water,
    );
    let land = 1.0 - water;
    let shore_water = smoothstep(0.34, 0.58, raw_water) * shore;
    let projection_safety = smoothstep(0.72, 0.96, ground_v);
    let mixed_projection_safety = if !ocean_world && !dry_world {
        smoothstep(0.74, 0.985, ground_v)
    } else {
        0.0
    };
    let dry_projection_safety = if dry_world {
        smoothstep(0.30, 0.78, ground_v) * land
    } else {
        0.0
    };
    let rocky_overview_relief = if ocean_world {
        0.0
    } else {
        clamp01(
            rocky_material.relief
                + if rocky_world { 0.12 } else { 0.0 }
                + if volcanic_world { 0.18 } else { 0.0 },
        )
    };
    let normal_strength = 3.2
        + land * (11.8 + lod.foreground_detail * 4.4 + rocky_overview_relief * 3.4)
        + relief * (7.0 + rocky_overview_relief * 1.8)
        + hit.parallax * 2.0;
    let normal_strength =
        normal_strength * (1.0 - projection_safety * 0.14 - dry_projection_safety * 0.36).max(0.42);
    let terrain_n = Vec3::new(
        (left - right) * normal_strength,
        1.0,
        (near - far) * normal_strength * 1.25,
    )
    .normalize();
    let view_dir = Vec3::new(-sx * 0.16, 0.38 + ground_v * 0.34, 0.88).normalize();

    let mottle = fbm_periodic(
        u * 2.7 + 0.11,
        v * 2.2 - 0.07,
        70,
        3,
        profile.seed + 17_003,
        0.52,
    );
    let fracture_noise = fbm_periodic(
        u * 7.1 + mottle * 0.10,
        v * 6.7 - mottle * 0.07,
        181,
        2,
        profile.seed + 17_291,
        0.46,
    );
    let strata_breakup = 0.52 + ridge(fracture_noise) * 0.38 + (mottle - 0.5) * 0.10;
    let primary_strata = ((u * 560.0 + v * 390.0 + mottle * 6.2).sin() * 0.5 + 0.5)
        * (0.34 + relief * 0.76)
        * strata_breakup;
    let cross_strata = ((u * 315.0 - v * 690.0 + mottle * 4.1).sin() * 0.5 + 0.5)
        * (0.24 + relief * 0.44)
        * (0.64 + fracture_noise * 0.24);
    let streak_visibility = lod.streak_weight
        * (1.0 - lod.foreground_detail * 0.64)
        * (1.0 - mixed_projection_safety * (0.52 + water * 0.28))
        * (1.0 - dry_projection_safety * 0.96);
    let strata = primary_strata * streak_visibility
        + cross_strata * (1.0 - streak_visibility) * 0.74
        + (fracture_noise - 0.5) * lod.foreground_detail * 0.16;
    let micro = terrain_micro_detail(u, v, profile.seed);
    let fine_grain = fbm_periodic(
        u * 3.6 + v * 0.21,
        v * 3.1 - u * 0.14,
        96,
        3,
        profile.seed + 18_707,
        0.48,
    );
    let foreground_grain = fbm_periodic(
        u * 5.8 - v * 0.31,
        v * 5.4 + u * 0.27,
        148,
        2,
        profile.seed + 18_931,
        0.45,
    );
    let micro_step_u = (0.38 + lod.foreground_detail * 0.24) / maps.width as f32;
    let micro_step_v = (0.38 + lod.foreground_detail * 0.24) / maps.height as f32;
    let micro_right = terrain_micro_detail(u + micro_step_u, v, profile.seed);
    let micro_left = terrain_micro_detail(u - micro_step_u, v, profile.seed);
    let micro_near = terrain_micro_detail(u, v + micro_step_v, profile.seed);
    let micro_far = terrain_micro_detail(u, v - micro_step_v, profile.seed);
    let micro_relief =
        ridge(micro) * land * (1.0 - sample.biome * 0.22) * (0.88 + lod.foreground_detail * 0.24);
    let rocky_land = if ocean_world {
        0.0
    } else {
        land * rocky_material.rocky * (1.0 - sample.vegetation * 0.22)
    };
    let volcanic_land = rocky_land * rocky_material.volcanic;
    let basalt_vein = smoothstep(
        0.48,
        0.94,
        fracture_noise * 0.34 + ridge(strata) * 0.24 + relief * 0.24 + mottle * 0.18,
    ) * rocky_land
        * (0.24 + rocky_material.basalt * 0.76);
    let ash_sheet = smoothstep(
        0.44,
        0.90,
        fine_grain * 0.42 + foreground_grain * 0.22 + (1.0 - mottle) * 0.20 + relief * 0.16,
    ) * volcanic_land
        * (0.24 + rocky_material.ash * 0.76);
    let oxide_varnish = smoothstep(
        0.42,
        0.92,
        fine_grain * 0.42 + mottle * 0.26 + sample.biome * 0.18 + hit.elevation * 0.14,
    ) * rocky_land
        * rocky_material.oxide;
    let sulfur_crust = smoothstep(
        0.55,
        0.96,
        foreground_grain * 0.38 + ridge(fracture_noise) * 0.26 + relief * 0.22 + strata * 0.14,
    ) * volcanic_land
        * rocky_material.sulfur;
    let lava_thread = smoothstep(
        0.66,
        0.985,
        ridge(fracture_noise) * 0.42
            + primary_strata * 0.22
            + smoothstep(0.58, 0.96, relief) * 0.22
            + micro_relief * 0.14,
    ) * volcanic_land
        * rocky_material.lava
        * smoothstep(0.05, 0.78, ground_v);

    let mut base = sample.albedo;
    if land > 0.08 {
        let texture = 0.82
            + (mottle - 0.5) * 0.24
            + (strata - 0.5) * (0.070 + relief * 0.030)
            + (micro - 0.5) * (0.16 + lod.foreground_detail * 0.08)
            + (fine_grain - 0.5) * (0.08 + relief * 0.06 + lod.foreground_detail * 0.035)
            + (foreground_grain - 0.5) * lod.foreground_detail * 0.070;
        base = base * texture;
        let wet_shore = Vec3::new(0.55, 0.50, 0.34).lerp(Vec3::new(0.30, 0.44, 0.35), mottle);
        base = base.lerp(
            wet_shore,
            shore
                * land
                * (0.42 + lod.foreground_detail * 0.18)
                * if dry_world { 0.24 } else { 1.0 },
        );
        base = base.lerp(
            Vec3::new(0.52, 0.50, 0.44),
            (relief * 0.24 + micro_relief * 0.12) * land,
        );
        let basalt = Vec3::new(0.048, 0.050, 0.052).lerp(Vec3::new(0.20, 0.19, 0.17), mottle);
        let ash = Vec3::new(0.18, 0.17, 0.16).lerp(Vec3::new(0.50, 0.48, 0.43), fine_grain);
        let oxide = Vec3::new(0.48, 0.18, 0.09).lerp(Vec3::new(0.86, 0.48, 0.22), foreground_grain);
        let sulfur = Vec3::new(0.70, 0.55, 0.14).lerp(Vec3::new(0.98, 0.82, 0.30), sulfur_crust);
        let ridge_highlight = Vec3::new(0.66, 0.63, 0.55).lerp(Vec3::new(0.82, 0.78, 0.66), micro);
        base = base
            .lerp(
                ridge_highlight,
                rocky_land * (relief * 0.22 + micro_relief * 0.12),
            )
            .lerp(
                basalt,
                basalt_vein * 0.62 + rocky_land * rocky_material.basalt * 0.10,
            )
            .lerp(oxide, oxide_varnish * 0.46)
            .lerp(ash, ash_sheet * 0.58)
            .lerp(sulfur, sulfur_crust * 0.66);
        base += Vec3::new(1.8, 0.46, 0.070) * lava_thread * (0.18 + lod.foreground_detail * 0.12);

        if mixed_projection_safety > 0.001 {
            let local_u = fx * 1.18 + sx * 0.10 + anchor_u * 0.11;
            let local_v = ground_v * 1.28 + fy * 0.16 + anchor_v * 0.09;
            let local_mass = fbm_periodic(
                local_u + mottle * 0.055,
                local_v - fine_grain * 0.045,
                23,
                5,
                profile.seed + 24_031,
                0.53,
            );
            let local_grain = fbm_periodic(
                local_u * 4.2 + local_mass * 0.120,
                local_v * 3.6 - local_mass * 0.085,
                89,
                3,
                profile.seed + 24_137,
                0.46,
            );
            let local_slope = fbm_periodic(
                local_u * 2.1 - local_grain * 0.080,
                local_v * 1.9 + local_mass * 0.095,
                47,
                3,
                profile.seed + 24_199,
                0.48,
            );
            let local_shore = shore * (0.62 + ridge(local_slope) * 0.20);
            let local_soil = Vec3::new(0.50, 0.45, 0.32)
                .lerp(Vec3::new(0.67, 0.61, 0.46), local_mass * 0.38)
                .lerp(Vec3::new(0.50, 0.46, 0.34), local_shore * 0.34)
                .lerp(
                    Vec3::new(0.68, 0.63, 0.50),
                    ridge(local_grain) * land * 0.12,
                )
                * (0.90 + (local_mass - 0.5) * 0.18 + (local_grain - 0.5) * 0.10);
            base = base.lerp(local_soil, mixed_projection_safety * land * 0.28);
        }

        if dry_projection_safety > 0.001 {
            let screen_u = fx * 1.35 + sx * 0.10 + anchor_u * 0.17;
            let screen_v = ground_v * 1.18 + fy * 0.18 + anchor_v * 0.13;
            let local_mass = fbm_periodic(
                screen_u + fine_grain * 0.050,
                screen_v - foreground_grain * 0.040,
                263,
                5,
                profile.seed + 23_311,
                0.54,
            );
            let local_grit = fbm_periodic(
                screen_u * 4.8 + local_mass * 0.130,
                screen_v * 4.1 - local_mass * 0.095,
                337,
                3,
                profile.seed + 23_417,
                0.47,
            );
            let local_crack_noise = fbm_periodic(
                screen_u * 2.7 - local_grit * 0.080,
                screen_v * 2.4 + local_mass * 0.110,
                401,
                3,
                profile.seed + 23_551,
                0.46,
            );
            let local_cracks =
                smoothstep(0.80, 0.992, ridge(local_crack_noise)) * (0.34 + relief * 0.42);
            let local_crater = smoothstep(
                0.58,
                0.96,
                ridge(local_mass * 0.58 + local_grit * 0.22 + mottle * 0.20),
            ) * rocky_land
                * if volcanic_world { 0.34 } else { 0.58 };
            let regolith =
                Vec3::new(0.33, 0.30, 0.25).lerp(Vec3::new(0.62, 0.56, 0.45), local_mass);
            let basalt =
                Vec3::new(0.055, 0.054, 0.050).lerp(Vec3::new(0.22, 0.20, 0.16), local_grit);
            let ash = Vec3::new(0.17, 0.16, 0.145).lerp(Vec3::new(0.55, 0.53, 0.46), local_mass);
            let oxide = Vec3::new(0.47, 0.20, 0.10).lerp(Vec3::new(0.80, 0.47, 0.23), local_grit);
            let sulfur = Vec3::new(0.73, 0.58, 0.16).lerp(Vec3::new(1.0, 0.86, 0.32), local_mass);
            let mut local_base = if volcanic_world {
                basalt
                    .lerp(
                        ash,
                        clamp01(0.30 + rocky_material.ash * 0.42 + local_mass * 0.18),
                    )
                    .lerp(sulfur, sulfur_crust * 0.44 + rocky_material.sulfur * 0.16)
            } else {
                regolith
                    .lerp(
                        basalt,
                        clamp01(rocky_material.basalt * 0.42 + local_crater * 0.26),
                    )
                    .lerp(oxide, rocky_material.oxide * 0.36)
            };
            let local_lava = if volcanic_world {
                smoothstep(
                    0.90,
                    0.997,
                    local_cracks * 0.52 + ridge(local_grit) * 0.30 + ridge(local_mass) * 0.18,
                ) * rocky_material.lava
                    * smoothstep(0.08, 0.86, ground_v)
            } else {
                0.0
            };
            let local_texture = 0.88 + (local_mass - 0.5) * 0.16 + (local_grit - 0.5) * 0.10
                - local_cracks * 0.08
                - local_crater * 0.08;
            local_base = local_base * local_texture
                + Vec3::new(1.8, 0.42, 0.055) * local_lava * (0.14 + volcanic_air * 0.10);
            base = base.lerp(
                local_base,
                clamp01(dry_projection_safety * (1.08 + rocky_overview_relief * 0.16)),
            );
        }
    }
    if water > 0.06 || shore_water > 0.08 {
        let shallow = Vec3::new(0.045, 0.320, 0.400).lerp(Vec3::new(0.100, 0.520, 0.560), shore);
        let deep = Vec3::new(0.006, 0.030, 0.105);
        let trench = Vec3::new(0.002, 0.018, 0.076);
        let bathymetry = ocean_depth_cues(u, v, profile.seed);
        let water_current = fbm_periodic(
            u * 4.8 + foreground_grain * 0.052 - fine_grain * 0.020,
            v * 3.4 - foreground_grain * 0.034 + mottle * 0.018,
            211,
            3,
            profile.seed + 21_337,
            0.49,
        );
        let water_regions = ocean_regional_cues(
            u + (water_current - 0.5) * 0.020,
            v - (fine_grain - 0.5) * 0.014,
            profile.seed + 21_733,
        );
        let capillary = fbm_periodic(
            u * 8.6 + v * 0.42 + water_current * 0.072,
            v * 5.8 - u * 0.31 + fine_grain * 0.044,
            331,
            2,
            profile.seed + 21_409,
            0.45,
        );
        let water_spectrum = ocean_wave_spectrum(
            u + (water_current - 0.5) * 0.0030 + (foreground_grain - 0.5) * 0.0015,
            v + (capillary - 0.5) * 0.0022 + (fine_grain - 0.5) * 0.0011,
            profile.seed + 21_509,
        );
        let height_depth = clamp01((0.63 - sample.height) * if ocean_world { 2.15 } else { 1.55 });
        let apparent_depth = clamp01(
            water
                * (height_depth * 0.56
                    + (1.0 - smoothstep(0.42, 0.76, hit.elevation)) * 0.30
                    + (1.0 - shore) * 0.22
                    + bathymetry.trench * if ocean_world { 0.18 } else { 0.08 }),
        );
        let mid_depth_layer = smoothstep(0.14, 0.58, apparent_depth)
            * (1.0 - smoothstep(0.72, 0.98, apparent_depth))
            * (0.34
                + bathymetry.basin * 0.24
                + ridge(water_current) * 0.12
                + water_regions.gyre * 0.08);
        let bottom_visibility = water * (1.0 - apparent_depth) * (0.34 + shore * 0.48);
        let slope_shadow = clamp01(
            relief * 0.44
                + gradient.average.max(hit.elevation) * 0.08
                + bathymetry.trench * apparent_depth * 0.18
                + bathymetry.ridge * apparent_depth * 0.08,
        );
        let shelf_mix = clamp01(
            shore * 0.68
                + shore_water * 0.22
                + (1.0 - water) * 0.16
                + smoothstep(0.36, 0.72, hit.elevation) * 0.08
                - slope_shadow * water * 0.24,
        );
        let sediment =
            Vec3::new(0.055, 0.205, 0.255).lerp(Vec3::new(0.100, 0.300, 0.300), bathymetry.shelf);
        let abyss_mix = smoothstep(0.48, 0.98, apparent_depth)
            * (0.26 + bathymetry.trench * if ocean_world { 0.50 } else { 0.28 });
        let sediment_mix =
            bathymetry.turbidity * bottom_visibility * (0.12 + shore * 0.28 + shore_water * 0.18);
        let sunlit_shelf = bottom_visibility
            * smoothstep(0.24, 0.88, bathymetry.shelf + shore * 0.45)
            * (0.16 + ridge(capillary) * 0.10);
        let broad_current_lane = smoothstep(
            0.46,
            0.93,
            water_regions.current_edge * 0.62
                + (water_regions.current - 0.5).abs() * 0.26
                + ridge(water_current) * 0.14
                + ridge(capillary) * 0.08,
        ) * water
            * smoothstep(0.04, 0.58, ground_v)
            * (1.0 - smoothstep(0.56, 0.90, ground_v));
        let mut water_color = trench
            .lerp(deep, 1.0 - apparent_depth * 0.32)
            .lerp(Vec3::new(0.010, 0.070, 0.180), mid_depth_layer * water)
            .lerp(Vec3::new(0.001, 0.014, 0.062), abyss_mix)
            .lerp(shallow, shelf_mix)
            .lerp(sediment, sediment_mix)
            .lerp(Vec3::new(0.090, 0.390, 0.470), sunlit_shelf)
            .lerp(Vec3::new(0.012, 0.055, 0.120), slope_shadow * water * 0.10)
            .lerp(
                Vec3::new(0.015, 0.105, 0.205)
                    .lerp(Vec3::new(0.035, 0.180, 0.235), water_regions.warm),
                broad_current_lane * if ocean_world { 0.26 } else { 0.16 },
            );
        if ocean_world {
            let broad_swell = fbm_periodic(
                u * 2.2 + water_current * 0.070,
                v * 1.55 - capillary * 0.050,
                541,
                4,
                profile.seed + 24_113,
                0.52,
            );
            let cross_swell = fbm_periodic(
                u * 3.4 - broad_swell * 0.060,
                v * 2.6 + water_current * 0.050,
                587,
                3,
                profile.seed + 24_211,
                0.48,
            );
            let moving_depth = clamp01(
                apparent_depth * 0.62
                    + bathymetry.trench * 0.22
                    + (1.0 - bathymetry.basin) * 0.08
                    + (1.0 - broad_swell) * 0.08,
            );
            let swell_contrast = (broad_swell - 0.5) * 0.16
                + (cross_swell - 0.5) * 0.10
                + (water_spectrum.swell - 0.5) * 0.11
                - moving_depth * 0.045;
            water_color = water_color
                .lerp(Vec3::new(0.002, 0.018, 0.082), moving_depth * 0.16)
                .lerp(
                    Vec3::new(0.030, 0.155, 0.275),
                    ridge(cross_swell) * (1.0 - apparent_depth) * 0.11,
                )
                * (0.94 + swell_contrast);
            let open_ocean_foam = smoothstep(
                0.66,
                0.985,
                water_spectrum.foam * 0.38
                    + ridge(capillary) * 0.24
                    + ridge(cross_swell) * 0.20
                    + ridge(broad_swell) * 0.18,
            ) * water
                * (0.09 + smoothstep(0.10, 0.86, ground_v) * 0.16);
            water_color += Vec3::new(0.13, 0.25, 0.28) * open_ocean_foam;
        }
        if mixed_projection_safety > 0.001 {
            let screen_depth = fbm_periodic(
                fx * 1.28 + sx * 0.12,
                ground_v * 1.72 - fx * 0.10,
                19,
                5,
                profile.seed + 24_331,
                0.53,
            );
            let screen_current = fbm_periodic(
                fx * 3.1 + screen_depth * 0.24,
                ground_v * 2.6 - sx * 0.22,
                53,
                3,
                profile.seed + 24_421,
                0.47,
            );
            let screen_chop = fbm_periodic(
                fx * 6.4 - screen_current * 0.18,
                ground_v * 5.1 + sx * 0.34,
                97,
                2,
                profile.seed + 24_539,
                0.44,
            );
            let near_depth = clamp01(
                apparent_depth * 0.58
                    + (1.0 - screen_depth) * 0.18
                    + bathymetry.trench * 0.16
                    + ground_v * 0.08,
            );
            let screen_water = Vec3::new(0.003, 0.026, 0.092)
                .lerp(Vec3::new(0.012, 0.075, 0.165), 1.0 - near_depth * 0.35)
                .lerp(
                    Vec3::new(0.055, 0.255, 0.315),
                    shore * 0.30 + bottom_visibility * 0.22,
                )
                * (0.90 + (screen_current - 0.5) * 0.11 + (screen_chop - 0.5) * 0.055);
            water_color = water_color.lerp(
                screen_water,
                mixed_projection_safety * water * (0.72 + shore_water * 0.22),
            );
        }
        base = base.lerp(water_color, clamp01(water * 0.88 + shore_water * 0.32));
        let projection_safe_detail = smoothstep(0.04, 0.58, ground_v)
            * (1.0 - smoothstep(0.56, 0.90, ground_v))
            * if ocean_world {
                1.0 - smoothstep(0.42, 0.70, ground_v)
            } else {
                1.0
            };
        let detail_visibility = water
            * projection_safe_detail
            * if ocean_world {
                0.32 + lod.foreground_detail * 0.74
            } else {
                0.16 + lod.foreground_detail * 0.44
            };
        let current_shadow =
            (water_current - 0.5) * (0.050 + apparent_depth * 0.035 + bathymetry.ridge * 0.018);
        let capillary_lift = (capillary - 0.5) * 0.032
            + ridge(capillary) * water_spectrum.foam * 0.014
            + (bathymetry.micro - 0.5) * bottom_visibility * 0.030;
        let small_crest = smoothstep(
            0.58,
            0.96,
            water_spectrum.chop * 0.46 + water_spectrum.ripple * 0.34 + ridge(capillary) * 0.20,
        );
        base = base * (1.0 + (current_shadow + capillary_lift) * detail_visibility);
        base += Vec3::new(0.020, 0.090, 0.125)
            * broad_current_lane
            * detail_visibility
            * (0.70 + water_regions.current_edge * 0.40);
        base += Vec3::new(0.015, 0.065, 0.078)
            * ridge(bathymetry.micro)
            * bottom_visibility
            * detail_visibility
            * (0.24 + bathymetry.ridge * 0.34);
        base += Vec3::new(0.030, 0.095, 0.125)
            * small_crest
            * detail_visibility
            * if ocean_world {
                0.20 + shore_water * 0.10
            } else {
                0.090 + shore_water * 0.10
            };
        let foam_trace = smoothstep(
            0.58,
            0.96,
            water_spectrum.foam * 0.44 + ridge(capillary) * 0.30 + shore_water * 0.26,
        ) * detail_visibility
            * (0.22 + shore_water * 0.46);
        base += Vec3::new(0.140, 0.300, 0.330) * foam_trace;
    }

    let micro_strength = land
        * (3.0 + lod.foreground_detail * 3.2 + relief * 0.90 + rocky_land * 1.55)
        * (1.0 - water * 0.45)
        * (1.0 - projection_safety * 0.16 - dry_projection_safety * 0.30).max(0.36);
    let micro_nudge = Vec3::new(
        (micro_left - micro_right) * micro_strength,
        0.0,
        (micro_near - micro_far) * micro_strength * 1.18,
    );
    let terrain_n = (terrain_n + micro_nudge).normalize();
    let terrain_n = terrain_n
        .lerp(
            Vec3::new(0.0, 1.0, 0.08).normalize(),
            dry_projection_safety * 0.82,
        )
        .normalize();
    let ndotl = terrain_n.dot(light_dir).max(0.0);
    let half = (light_dir + view_dir).normalize();

    let sky_visibility = 1.0 - sample.horizon_occlusion * land;
    let ambient = (if night_view {
        0.038 + profile.atmosphere_density * 0.024
    } else {
        0.18 + profile.atmosphere_density * 0.055
    }) * (0.62 + sky_visibility * 0.38);
    let grazing_fill = smoothstep(0.22, 0.0, ground_v) * if night_view { 0.025 } else { 0.06 };
    let terrain_shadow = smoothstep(0.014, 0.082, far - hit.elevation)
        * land
        * (0.24 + hit.parallax * 0.08)
        * (1.0 - dry_projection_safety * 0.86);
    let cloud_projection =
        (0.012 + distance * 0.011) * (0.82 + cloud_altitude * 0.70 + atmosphere * 0.10);
    let cast_cloud = maps
        .sample(
            u - light_dir.x * cloud_projection,
            v - light_dir.z * cloud_projection * 0.72,
        )
        .cloud;
    let cast_cloud_mid = maps
        .sample(
            u - light_dir.x * cloud_projection * 1.35,
            v - light_dir.z * cloud_projection * 0.96,
        )
        .cloud;
    let cast_cloud_far = maps
        .sample(
            u - light_dir.x * cloud_projection * 1.8,
            v - light_dir.z * cloud_projection * 1.15,
        )
        .cloud;
    let cloud_edge = (cast_cloud - cast_cloud_far).abs() + (cast_cloud_mid - cast_cloud).abs();
    let cloud_shadow = smoothstep(
        0.06,
        0.62,
        cast_cloud * 0.56 + cast_cloud_mid * 0.30 + cast_cloud_far * 0.14,
    ) * smoothstep(0.04, 0.86, ground_v)
        * (0.12 + relief * 0.045 + cloud_edge * 0.12 + cloud_altitude * 0.030)
        * (1.0 + water * (0.08 + ocean_air * 0.12));
    let local_basin = smoothstep(0.012, 0.064, (gradient.average - hit.elevation).max(0.0))
        * land
        * 0.26
        * (1.0 - dry_projection_safety * 0.78);
    let shore_contact = shore * (land * 0.11 + water * 0.055) * smoothstep(0.02, 0.52, ground_v);
    let mountain_ao = relief
        * land
        * smoothstep(0.54, 0.92, hit.elevation)
        * smoothstep(0.08, 0.82, ground_v)
        * 0.23
        * (1.0 - dry_projection_safety * 0.70);
    let horizon_ao =
        smoothstep(0.125, 0.0, ground_v) * (0.08 + relief * land * 0.15 + shore * 0.040);
    let rocky_crevice_ao = rocky_land
        * (basalt_vein * 0.035
            + ash_sheet * 0.025
            + oxide_varnish * 0.020
            + sulfur_crust * 0.018
            + (1.0 - lava_thread) * relief * 0.030)
        * smoothstep(0.08, 0.92, ground_v)
        * (1.0 - dry_projection_safety * 0.70);
    let baked_terrain_ao = sample.ambient_occlusion
        * land
        * smoothstep(0.025, 0.96, ground_v)
        * (0.22 + lod.foreground_detail * 0.18 + relief * 0.10);
    let baked_horizon_ao =
        sample.horizon_occlusion * land * smoothstep(0.16, 0.92, ground_v) * 0.12;
    let occlusion = clamp01(
        terrain_shadow
            + cloud_shadow
            + local_basin
            + shore_contact
            + mountain_ao
            + horizon_ao
            + rocky_crevice_ao
            + baked_terrain_ao
            + baked_horizon_ao,
    );
    let mut color = base * (ambient + grazing_fill + ndotl * 1.02) * (1.0 - occlusion);
    color += if night_view {
        Vec3::new(0.36, 0.48, 1.0)
    } else {
        Vec3::new(1.0, 0.72, 0.40)
    } * ndotl
        * land
        * if night_view { 0.018 } else { 0.035 };
    color += Vec3::new(2.6, 0.70, 0.12)
        * lava_thread
        * (1.0 - dry_projection_safety * 0.88)
        * if night_view {
            0.34 + (1.0 - ndotl) * 0.34 + smoothstep(0.32, 0.0, ground_v) * 0.18
        } else {
            0.12 + (1.0 - ndotl) * 0.18 + smoothstep(0.32, 0.0, ground_v) * 0.10
        };

    if water > 0.24 {
        let wave = ocean_wave_spectrum(
            u + (foreground_grain - 0.5) * lod.foreground_detail * 0.0018,
            v + (fine_grain - 0.5) * lod.foreground_detail * 0.0012,
            profile.seed + 15_991,
        );
        let wave_detail = 1.0 - smoothstep(0.72, 3.65, lod.footprint);
        let wave_strength = (1.62 + lod.foreground_detail * 0.55) * (0.58 + wave_detail * 0.42);
        let wave_n = (terrain_n
            + Vec3::new(
                wave.slope.x * wave_strength,
                0.0,
                wave.slope.y * (1.38 + lod.foreground_detail * 0.48) * (0.58 + wave_detail * 0.42),
            ))
        .normalize();
        let water_view = wave_n.dot(view_dir).max(0.0);
        let fresnel = schlick_fresnel(water_view, 0.024);
        let glint_mask =
            wave.glint * (0.48 + wave_detail * 0.52) * (0.78 + ridge(foreground_grain) * 0.22);
        let glitter_breakup = fbm_periodic(
            u * 30.0 + wave.chop * 0.110 + foreground_grain * 0.070,
            v * 18.0 - wave.swell * 0.085 + fine_grain * 0.050,
            373,
            2,
            profile.seed + 22_019,
            0.44,
        );
        let glint_islands = fbm_periodic(
            u * 17.0 + glitter_breakup * 0.10,
            v * 13.5 - wave.ripple * 0.08,
            419,
            2,
            profile.seed + 22_147,
            0.42,
        );
        let world_glint_gate = smoothstep(
            0.44,
            0.90,
            glint_islands * 0.44 + ridge(glitter_breakup) * 0.34 + wave.glint * 0.22,
        );
        let glitter_gate = smoothstep(
            0.50,
            0.94,
            wave.glint * 0.38
                + ridge(glitter_breakup) * 0.28
                + world_glint_gate * 0.22
                + wave.foam * 0.12,
        );
        let spec_core = wave_n.dot(half).max(0.0).powf(82.0)
            * ndotl
            * water
            * glint_mask
            * (0.58 + glitter_gate * 0.66);
        let spec_broad =
            wave_n.dot(half).max(0.0).powf(26.0) * ndotl * water * (0.025 + glint_mask * 0.055);
        let path_wander = ((foreground_grain - 0.5) * 0.052
            + (fine_grain - 0.5) * 0.030
            + (glint_islands - 0.5) * 0.046
            + wave.slope.x * 0.18
            + wave.slope.y * 0.12)
            * (0.36 + lod.foreground_detail * 0.64);
        let foreground_glint_suppression =
            (1.0 - lod.foreground_detail * 0.58) * (1.0 - mixed_projection_safety * 0.42);
        let reflection_x = if night_view {
            0.74
        } else {
            camera.sun_screen.x
        };
        let sun_path = smoothstep(
            0.34,
            0.0,
            ((fx - reflection_x + path_wander) * camera.aspect + sx * ground_v * 0.035).abs(),
        ) * smoothstep(0.88, 0.03, ground_v)
            * (0.42 + glitter_gate * 0.34 + world_glint_gate * 0.24)
            * foreground_glint_suppression;
        let reflection = if night_view {
            Vec3::new(0.020, 0.050, 0.130).lerp(
                Vec3::new(0.060, 0.095, 0.190),
                smoothstep(0.15, 1.0, ground_v) * 0.28,
            )
        } else {
            sky_horizon
                .lerp(sky_mid, smoothstep(0.15, 1.0, ground_v) * 0.34)
                .lerp(horizon_air, shore * 0.18)
        };
        let reflection_mix = water
            * (if night_view { 0.10 } else { 0.16 }
                + fresnel * if night_view { 0.48 } else { 0.64 }
                + smoothstep(0.18, 0.0, ground_v) * if night_view { 0.080 } else { 0.13 })
            * if ocean_world { 0.70 } else { 1.0 };
        color = color.lerp(reflection, reflection_mix);
        let reflected_clouds = smoothstep(
            0.05,
            0.48,
            cast_cloud_mid * 0.54 + cast_cloud_far * 0.30 + cast_cloud * 0.16,
        ) * water
            * (fresnel * 0.38 + smoothstep(0.20, 0.0, ground_v) * 0.16)
            * (0.72 + cloud_altitude * 0.34)
            * (1.0 - smoothstep(0.14, 0.60, sample.cloud) * 0.34);
        color = color * (1.0 - reflected_clouds * if night_view { 0.030 } else { 0.055 });
        color = color.lerp(
            if night_view {
                Vec3::new(0.055, 0.090, 0.190)
            } else {
                Vec3::new(0.60, 0.72, 0.74)
            },
            reflected_clouds * if night_view { 0.14 } else { 0.22 },
        );
        let wave_shadow = water
            * wave_detail
            * smoothstep(0.36, 0.90, wave.chop)
            * (1.0 - wave.swell)
            * (0.022 + lod.foreground_detail * 0.018);
        color = color * (1.0 - wave_shadow);
        let foreground_water_detail = water
            * smoothstep(0.08, 0.92, ground_v)
            * (1.0 - smoothstep(0.58, 0.92, ground_v))
            * (0.32 + lod.foreground_detail * 0.68)
            * (1.0 - mixed_projection_safety * 0.38)
            * wave_detail;
        color = color
            * (1.0
                + ((glitter_breakup - 0.5) * 0.026
                    + (glint_islands - 0.5) * 0.018
                    + (wave.ripple - 0.5) * 0.018)
                    * foreground_water_detail);
        color = color.lerp(
            Vec3::new(0.05, 0.22, 0.34),
            water * smoothstep(0.16, 0.88, wave.ripple) * (0.038 + lod.foreground_detail * 0.018),
        );
        let horizon_sheet = water
            * smoothstep(0.16, 0.0, ground_v)
            * (0.055 + fresnel * 0.18)
            * (0.72 + ocean_air * 0.28)
            * if ocean_world { 0.64 } else { 1.0 };
        color = color.lerp(sky_horizon.lerp(horizon_air, 0.45), horizon_sheet);
        color += Vec3::new(0.54, 0.78, 0.82)
            * water
            * wave.foam
            * smoothstep(0.08, 0.70, ground_v)
            * (0.014 + lod.foreground_detail * 0.018);
        color += if night_view {
            Vec3::new(0.48, 0.62, 1.0)
        } else {
            Vec3::new(1.0, 0.86, 0.58)
        } * (spec_core * 1.05
            + spec_broad * 0.34
            + sun_path * glint_mask * smoothstep(0.58, 0.96, wave.chop) * 0.115);
    }

    if mixed_projection_safety > 0.001 {
        let screen_mass = fbm_periodic(
            fx * 1.35 + sx * 0.11,
            ground_v * 1.45 - fx * 0.09,
            17,
            5,
            profile.seed + 24_719,
            0.52,
        );
        let screen_detail = fbm_periodic(
            fx * 5.4 + screen_mass * 0.22,
            ground_v * 4.6 - sx * 0.28,
            73,
            3,
            profile.seed + 24_811,
            0.46,
        );
        let local_land = Vec3::new(0.42, 0.37, 0.27)
            .lerp(Vec3::new(0.70, 0.64, 0.49), screen_mass * 0.45)
            .lerp(Vec3::new(0.56, 0.52, 0.42), ridge(screen_detail) * 0.18);
        let local_water = Vec3::new(0.003, 0.030, 0.100)
            .lerp(Vec3::new(0.018, 0.098, 0.185), screen_mass * 0.36)
            .lerp(
                Vec3::new(0.060, 0.260, 0.315),
                shore * 0.34 + ridge(screen_detail) * 0.10,
            );
        let screen_water_mask = smoothstep(
            0.44,
            0.82,
            screen_mass * 0.58 + ridge(screen_detail) * 0.24 + (1.0 - screen_detail) * 0.10,
        );
        let projected_mask_weight =
            (1.0 - mixed_projection_safety * 0.98) * (1.0 - smoothstep(0.78, 0.99, ground_v));
        let local_water_mask = clamp01(
            water * projected_mask_weight
                + screen_water_mask * (1.0 - projected_mask_weight)
                + shore_water * 0.10,
        );
        let local_floor =
            local_land.lerp(local_water, local_water_mask) * (0.90 + (screen_detail - 0.5) * 0.08);
        let bottom_soften = smoothstep(0.78, 1.0, ground_v);
        let bottom_water_mask = smoothstep(0.42, 0.78, screen_mass * 0.82 + shore * 0.08);
        let bottom_floor = Vec3::new(0.48, 0.42, 0.30)
            .lerp(Vec3::new(0.66, 0.60, 0.46), screen_mass * 0.22)
            .lerp(
                Vec3::new(0.004, 0.040, 0.120)
                    .lerp(Vec3::new(0.026, 0.120, 0.195), screen_mass * 0.18),
                bottom_water_mask,
            );
        let local_floor = local_floor.lerp(bottom_floor, bottom_soften * 0.92);
        let mix = mixed_projection_safety
            * smoothstep(0.48, 0.96, ground_v)
            * (0.22 + water * 0.05 + land * 0.06);
        color = color.lerp(local_floor, mix);
    }

    if ocean_world && water > 0.18 {
        let screen_depth = fbm_periodic(
            fx * 1.35 + sx * 0.08,
            ground_v * 1.85 - fx * 0.18,
            641,
            5,
            profile.seed + 24_707,
            0.54,
        );
        let screen_swell = fbm_periodic(
            fx * 4.6 + screen_depth * 0.34,
            ground_v * 3.7 - sx * 0.28,
            683,
            3,
            profile.seed + 24_809,
            0.48,
        );
        let screen_cross = fbm_periodic(
            fx * 7.2 - screen_swell * 0.26,
            ground_v * 5.5 + sx * 0.42,
            727,
            2,
            profile.seed + 24_911,
            0.44,
        );
        let readable_water =
            water * smoothstep(0.04, 0.86, ground_v) * (1.0 - smoothstep(0.44, 0.68, ground_v));
        let trough = smoothstep(
            0.52,
            0.94,
            (1.0 - screen_depth) * 0.38 + (1.0 - screen_swell) * 0.30 + ridge(screen_cross) * 0.12,
        ) * readable_water;
        let crest = smoothstep(
            0.62,
            0.985,
            ridge(screen_swell) * 0.42 + ridge(screen_cross) * 0.36 + screen_depth * 0.22,
        ) * readable_water;
        color = color * (0.88 + (screen_depth - 0.5) * 0.16 + (screen_swell - 0.5) * 0.10);
        color = color.lerp(Vec3::new(0.002, 0.020, 0.080), trough * 0.16);
        color += Vec3::new(0.055, 0.155, 0.185) * crest * 0.30;
        color += Vec3::new(0.22, 0.38, 0.40)
            * smoothstep(0.84, 0.995, crest + ridge(screen_cross) * 0.18)
            * readable_water
            * 0.10;
    }

    if sample.cloud > 0.08 {
        let cloud_east = maps.sample(u + eps_u * 1.7, v).cloud;
        let cloud_west = maps.sample(u - eps_u * 1.7, v).cloud;
        let cloud_north = maps.sample(u, v - eps_v * 1.7).cloud;
        let cloud_south = maps.sample(u, v + eps_v * 1.7).cloud;
        let cloud_diag = maps.sample(u + eps_u * 1.2, v + eps_v * 1.2).cloud
            + maps.sample(u - eps_u * 1.2, v + eps_v * 1.2).cloud
            + maps.sample(u + eps_u * 1.2, v - eps_v * 1.2).cloud
            + maps.sample(u - eps_u * 1.2, v - eps_v * 1.2).cloud;
        let cloud_center = clamp01(
            sample.cloud * 0.42
                + (cloud_east + cloud_west + cloud_north + cloud_south) * 0.105
                + cloud_diag * 0.040,
        );
        let cloud_relief = clamp01(
            ((cloud_east - cloud_west).abs() + (cloud_south - cloud_north).abs()) * 3.2
                + cloud_edge * 1.6
                + ridge(cloud_center) * 0.18,
        );
        let cloud_n = Vec3::new(
            (cloud_west - cloud_east) * 2.1,
            1.0,
            (cloud_south - cloud_north) * 1.7,
        )
        .normalize();
        let cloud_ndotl = cloud_n.dot(light_dir).max(0.0);
        let cloud_alpha = cloud_center
            * smoothstep(0.03, 0.55, ground_v)
            * (0.065 + cloud_relief * 0.070 + cloud_altitude * 0.018)
            * if ocean_world { 1.32 } else { 1.0 };
        let cloud_self_shadow = smoothstep(0.14, 0.54, cloud_center)
            * (1.0 - cloud_ndotl)
            * (0.13 + cloud_altitude * 0.08);
        let cloud_light = clamp01(
            0.36 + cloud_ndotl * 0.72
                + ndotl * 0.22
                + (cloud_center - cast_cloud) * 0.24
                + cloud_relief * 0.10
                + cloud_altitude * 0.045
                - cloud_self_shadow,
        );
        let cloud_color = if night_view {
            Vec3::new(0.090, 0.115, 0.210).lerp(
                Vec3::new(0.240, 0.300, 0.500),
                smoothstep(0.06, 0.34, cloud_center),
            )
        } else {
            Vec3::new(0.66, 0.70, 0.68)
                .lerp(
                    Vec3::new(0.96, 0.97, 0.92),
                    smoothstep(0.06, 0.34, cloud_center),
                )
                .lerp(Vec3::new(0.78, 0.88, 0.90), water * ocean_air * 0.10)
        } * if night_view {
            cloud_light * 0.46
        } else {
            cloud_light
        };
        let cloud_air_gap = cloud_center
            * water
            * smoothstep(0.03, 0.32, ground_v)
            * (0.040 + cloud_altitude * 0.045);
        color = color.lerp(
            if night_view {
                Vec3::new(0.060, 0.095, 0.190)
            } else {
                Vec3::new(0.62, 0.76, 0.80)
            },
            cloud_air_gap * if night_view { 0.22 } else { 0.34 },
        );
        color = color.lerp(cloud_color, cloud_alpha);
        color += if night_view {
            Vec3::new(0.38, 0.50, 1.0)
        } else {
            Vec3::new(1.0, 0.84, 0.58)
        } * cloud_edge
            * cloud_ndotl
            * cloud_alpha
            * if night_view { 0.045 } else { 0.090 };
        if ocean_world {
            color = color.lerp(
                if night_view {
                    Vec3::new(0.070, 0.105, 0.220)
                } else {
                    Vec3::new(0.78, 0.86, 0.88)
                },
                cloud_center * smoothstep(0.02, 0.28, ground_v) * (0.080 + cloud_altitude * 0.055),
            );
        }
    }

    if night_view && sample.city > 0.015 {
        let city_visibility = sample.city
            * land
            * smoothstep(0.04, 0.84, ground_v)
            * (1.0 - smoothstep(0.70, 1.0, ground_v) * 0.45)
            * (1.0 - sample.cloud * 0.28);
        color += Vec3::new(2.8, 1.45, 0.48) * city_visibility * 0.92;
        color += Vec3::new(0.90, 0.50, 0.22)
            * city_visibility
            * sample.cloud
            * smoothstep(0.12, 0.70, ground_v)
            * 0.46;
    }

    let ground_depth = atmosphere_optical_depth(
        ground_v * 0.76 + 0.10 + (1.0 - hit.elevation) * 0.035,
        atmosphere,
    );
    let low_air = smoothstep(0.18, 0.0, ground_v);
    let ocean_mist = if ocean_world {
        water * low_air * (0.018 + ocean_air * 0.030) * (0.54 + smoothstep(1.8, 5.2, distance))
    } else {
        0.0
    };
    let air_refraction = fbm_periodic(
        fx * 1.18 + sx * 0.12 + distance * 0.025,
        ground_v * 0.92 - hit.elevation * 0.10,
        97,
        2,
        profile.seed + 24_337,
        0.45,
    );
    let horizon_occlusion = low_air
        * (0.070
            + atmosphere * 0.055
            + water * 0.035
            + cloud_shadow * (0.14 + cloud_altitude * 0.060)
            + ridge(air_refraction) * 0.014)
        * (1.0 - relief * land * 0.18);
    color = color * (1.0 - clamp01(horizon_occlusion));
    let detail_preserve = 1.0 - relief * 0.16 - land * micro_relief * 0.06;
    let distance_fog =
        smoothstep(1.35, 5.35, distance) * (0.075 + ground_depth * 0.120) * detail_preserve
            + ocean_mist * 0.34;
    let horizon_cloud_bank = low_air
        * smoothstep(0.08, 0.44, sample.cloud * 0.72 + cast_cloud_far * 0.28)
        * (0.30 + cloud_edge * 0.40 + cloud_altitude * 0.16);
    let horizon_haze = low_air
        * (0.075
            + ground_depth * 0.155
            + horizon_cloud_bank * 0.040
            + ridge(air_refraction) * atmosphere * 0.018)
        * (1.0 - relief * 0.20)
        + ocean_mist * 0.70;
    let haze_color = sky_horizon
        .lerp(horizon_air, smoothstep(0.12, 0.0, ground_v) * 0.45)
        .lerp(Vec3::new(0.620, 0.780, 0.820), ocean_air * water * 0.18);
    color = color.lerp(
        haze_color,
        clamp01(distance_fog + horizon_haze + horizon_cloud_bank * 0.055),
    );
    color += horizon_air * smoothstep(0.055, 0.0, ground_v) * (0.055 + ground_depth * 0.030);

    let foreground_vignette = smoothstep(0.98, 0.18, sx.abs()) * smoothstep(1.0, 0.45, ground_v);
    color = color * (0.88 + foreground_vignette * 0.15);

    if let Some(sky) = sky_color {
        let silhouette = hit.silhouette * smoothstep(-0.125, -0.004, pixel_ground_v);
        color = sky.lerp(color, silhouette);
    }

    if night_view {
        color = color * 0.66;
        color = color.lerp(Vec3::new(0.006, 0.018, 0.050), 0.16 + ground_v * 0.05);
        color += Vec3::new(0.032, 0.070, 0.175)
            * smoothstep(0.18, 0.0, ground_v)
            * (0.16 + atmosphere * 0.10 + water * 0.08);
    }
    color = apply_anti_band_grain(
        color,
        fx + air_refraction * 0.019,
        ground_v + hit.elevation * 0.011,
        profile.seed + 24_719,
        if night_view { 0.0046 } else { 0.0034 } + (distance_fog + horizon_haze) * 0.0018,
    );

    rgba(tone_map(color * if night_view { 1.08 } else { 0.90 }), 255)
}

#[derive(Debug, Clone, Copy)]
struct HeightfieldSurfacePoint {
    world_x: f32,
    world_z: f32,
    height: f32,
    u: f32,
    v: f32,
    detail: f32,
    water: f32,
    sample: SurfaceSample,
}

#[derive(Debug, Clone, Copy)]
struct HeightfieldRayHit {
    point: HeightfieldSurfacePoint,
    distance: f32,
}

fn heightfield_surface_point(
    frame: StableTerrainFrame,
    tile: &LocalTerrainTile,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    world_x: f32,
    world_z: f32,
) -> HeightfieldSurfacePoint {
    let phase = hash2(241, 419, profile.seed + 41_001) * PI * 2.0;
    let warp_x = ((world_x * 0.73 + world_z * 0.29 + phase).sin()
        + (world_x * -0.37 + world_z * 0.91 - phase * 0.7).sin() * 0.46)
        * 0.0017;
    let warp_z = ((world_x * -0.24 + world_z * 0.68 - phase).sin()
        + (world_x * 0.83 + world_z * 0.34 + phase * 1.3).sin() * 0.42)
        * 0.0016;
    let (u, v) = frame.map_uv(world_x, world_z, warp_x, warp_z);
    let sample = maps.sample(u, v);
    let (height, detail, water) = tile.sample_smooth(world_x, world_z);

    HeightfieldSurfacePoint {
        world_x,
        world_z,
        height,
        u,
        v,
        detail,
        water,
        sample,
    }
}

fn raymarch_heightfield(
    frame: StableTerrainFrame,
    tile: &LocalTerrainTile,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    origin: Vec3,
    direction: Vec3,
) -> Option<HeightfieldRayHit> {
    const MAX_DISTANCE: f32 = 8.4;
    const MARCH_STEPS: usize = 160;
    const REFINE_STEPS: usize = 8;

    let mut previous_t = 0.04_f32;
    let mut previous_clearance = f32::MAX;
    let mut t = previous_t;
    for _ in 0..MARCH_STEPS {
        let world_x = origin.x + direction.x * t;
        let world_z = origin.z + direction.z * t;
        let (terrain_height, _, _) = tile.sample(world_x, world_z);
        let ray_height = origin.y + direction.y * t;
        let clearance = ray_height - terrain_height;
        if clearance <= 0.0 && previous_clearance > 0.0 {
            let mut low = previous_t;
            let mut high = t;
            for _ in 0..REFINE_STEPS {
                let mid = (low + high) * 0.5;
                let mid_x = origin.x + direction.x * mid;
                let mid_z = origin.z + direction.z * mid;
                let mid_height = tile.sample_height_smooth(mid_x, mid_z);
                let mid_clearance = origin.y + direction.y * mid - mid_height;
                if mid_clearance > 0.0 {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            let hit_t = (low + high) * 0.5;
            return Some(HeightfieldRayHit {
                point: heightfield_surface_point(
                    frame,
                    tile,
                    maps,
                    profile,
                    origin.x + direction.x * hit_t,
                    origin.z + direction.z * hit_t,
                ),
                distance: hit_t,
            });
        }

        previous_t = t;
        previous_clearance = clearance;
        // Clearance-adaptive stepping prevents a ray from hopping across a
        // narrow ridge between two positive samples. It is deliberately
        // bounded below the smallest resolved terrain wavelength; the exact
        // crossing is then solved by the bracketed binary search above.
        t += (clearance.max(0.0) * 0.34 + 0.012).clamp(0.012, 0.082);
        if t > MAX_DISTANCE {
            break;
        }
    }
    None
}

fn heightfield_normal(
    tile: &LocalTerrainTile,
    point: HeightfieldSurfacePoint,
    distance: f32,
) -> Vec3 {
    let step = 0.018 + smoothstep(3.2, 7.6, distance) * 0.052;
    let left = tile.sample_height_smooth(point.world_x - step, point.world_z);
    let right = tile.sample_height_smooth(point.world_x + step, point.world_z);
    let far = tile.sample_height_smooth(point.world_x, point.world_z + step);
    let near = tile.sample_height_smooth(point.world_x, point.world_z - step);
    Vec3::new(
        (left - right) / (step * 2.0),
        1.0,
        (near - far) / (step * 2.0),
    )
    .normalize()
}

fn heightfield_contact_shadow(
    tile: &LocalTerrainTile,
    point: HeightfieldSurfacePoint,
    light_dir: Vec3,
) -> f32 {
    let ground_length = (light_dir.x * light_dir.x + light_dir.z * light_dir.z).sqrt();
    if light_dir.y <= 0.001 || ground_length <= 0.001 {
        return 1.0;
    }
    let ray_x = light_dir.x / ground_length;
    let ray_z = light_dir.z / ground_length;
    let rise = light_dir.y / ground_length;
    let mut shadow = 0.0_f32;
    for distance in [0.08_f32, 0.16, 0.30, 0.52, 0.84, 1.26, 1.86] {
        let (blocker_height, _, _) = tile.sample(
            point.world_x + ray_x * distance,
            point.world_z + ray_z * distance,
        );
        let ray_height = point.height + 0.008 + rise * distance;
        let penetration = blocker_height - ray_height;
        shadow = shadow.max(smoothstep(0.002, 0.065 + distance * 0.012, penetration));
    }
    shadow
}

#[allow(clippy::too_many_arguments)]
fn raymarched_terrain_overview_pixel(
    fx: f32,
    fy: f32,
    sx: f32,
    local_horizon: f32,
    aspect: f32,
    sun_screen: Vec3,
    sky_color: Option<Vec3>,
    frame: StableTerrainFrame,
    tile: &LocalTerrainTile,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    lighting_mode: LightingMode,
) -> [u8; 4] {
    let night_view = lighting_mode.is_night();
    let atmosphere = clamp(profile.atmosphere_density, 0.0, 1.6);
    let fallback_sky = if night_view {
        Vec3::new(0.006, 0.016, 0.050)
    } else {
        Vec3::new(0.54, 0.69, 0.80)
    };
    let vertical_ray = (local_horizon - fy) / (1.0 - local_horizon).max(0.1);
    let origin = Vec3::new(0.0, 0.48, -0.34);
    let direction = Vec3::new(
        sx * 0.46 / aspect.max(0.72).sqrt(),
        vertical_ray * 0.86,
        1.0,
    )
    .normalize();
    let Some(hit) = raymarch_heightfield(frame, tile, maps, profile, origin, direction) else {
        let miss_haze = if night_view {
            Vec3::new(0.062, 0.115, 0.235)
        } else {
            Vec3::new(0.34, 0.50, 0.60)
        };
        let horizon_proximity = smoothstep(0.16, 0.0, vertical_ray.abs());
        return rgba(
            tone_map(
                sky_color
                    .unwrap_or(fallback_sky)
                    .lerp(miss_haze, horizon_proximity * (0.62 + atmosphere * 0.12)),
            ),
            255,
        );
    };

    let point = hit.point;
    let sample = point.sample;
    let raw_water = clamp01(point.water);
    let water = smoothstep(0.24, 0.76, raw_water);
    let land = 1.0 - water;
    let shore = 1.0 - smoothstep(0.035, 0.20, (raw_water - 0.50).abs());
    let detail_visibility = 1.0 - smoothstep(3.2, 7.6, hit.distance) * 0.58;
    let mut normal = heightfield_normal(tile, point, hit.distance);
    let geometric_normal = normal;
    let detail_step = 0.026 + smoothstep(2.8, 7.2, hit.distance) * 0.058;
    let (_, detail_left, _) = tile.sample(point.world_x - detail_step, point.world_z);
    let (_, detail_right, _) = tile.sample(point.world_x + detail_step, point.world_z);
    let (_, detail_far, _) = tile.sample(point.world_x, point.world_z + detail_step);
    let (_, detail_near, _) = tile.sample(point.world_x, point.world_z - detail_step);
    let detail_dx = (detail_left - detail_right) / (detail_step * 2.0);
    let detail_dz = (detail_near - detail_far) / (detail_step * 2.0);
    let bump_strength = (land * 0.050 + water * 0.0025) * detail_visibility;
    normal = Vec3::new(
        normal.x + detail_dx * bump_strength,
        normal.y,
        normal.z + detail_dz * bump_strength,
    )
    .normalize();
    let grain_step = 0.008 + smoothstep(2.6, 7.0, hit.distance) * 0.020;
    let surface_grain = fbm_tiled(
        point.world_x * 0.48 + point.detail * 0.035,
        point.world_z * 0.44 - point.detail * 0.030,
        47,
        3,
        profile.seed + 41_223,
        0.44,
    );
    let grain_x = fbm_tiled(
        (point.world_x + grain_step) * 0.48 + point.detail * 0.035,
        point.world_z * 0.44 - point.detail * 0.030,
        47,
        3,
        profile.seed + 41_223,
        0.44,
    );
    let grain_z = fbm_tiled(
        point.world_x * 0.48 + point.detail * 0.035,
        (point.world_z + grain_step) * 0.44 - point.detail * 0.030,
        47,
        3,
        profile.seed + 41_223,
        0.44,
    );
    let grain_bump = land * detail_visibility * 0.018 / grain_step;
    normal = Vec3::new(
        normal.x + (surface_grain - grain_x) * grain_bump,
        normal.y,
        normal.z + (surface_grain - grain_z) * grain_bump,
    )
    .normalize();

    let light_dir = overview_light_dir(lighting_mode);
    let view_dir = (origin - Vec3::new(point.world_x, point.height, point.world_z)).normalize();
    let half = (light_dir + view_dir).normalize();
    let ndotl = normal.dot(light_dir).max(0.0);
    let ndotv = normal.dot(view_dir).max(0.0);
    let mut roughness = clamp(sample.roughness, 0.04, 0.96);
    let terrain_shadow = heightfield_contact_shadow(tile, point, light_dir) * land;
    let cast_cloud = maps
        .sample(point.u - light_dir.x * 0.018, point.v - light_dir.z * 0.014)
        .cloud;
    let cloud_shadow = smoothstep(0.10, 0.72, cast_cloud)
        * (0.10 + atmosphere * 0.035)
        * (1.0 - terrain_shadow * 0.30);
    let shadow = clamp01(terrain_shadow * 0.62 + cloud_shadow);
    let sky_visibility = 1.0 - sample.horizon_occlusion * land;
    let material_ao = sample.ambient_occlusion * land * (0.38 + roughness * 0.12);
    let local_detail = fbm_tiled(
        point.world_x * 0.30 + point.detail * 0.10,
        point.world_z * 0.27 - point.detail * 0.08,
        41,
        3,
        profile.seed + 41_211,
        0.47,
    );
    let material_fine_noise = fbm_tiled(
        point.world_x * 0.82 + local_detail * 0.045,
        point.world_z * 0.74 - point.detail * 0.038,
        53,
        2,
        profile.seed + 41_229,
        0.42,
    );
    let material_fine = clamp01(material_fine_noise * 0.64 + surface_grain * 0.36);
    let material_macro = fbm_tiled(
        point.world_x * 0.092 + point.detail * 0.075,
        point.world_z * 0.084 - local_detail * 0.060,
        19,
        4,
        profile.seed + 41_239,
        0.52,
    );
    let surface_slope = clamp01((1.0 - geometric_normal.y) / 0.24);
    let highland = smoothstep(0.055, 0.24, point.height);
    let rock_mask = smoothstep(
        0.16,
        0.68,
        surface_slope * 0.70
            + ridge(local_detail) * 0.24
            + ridge(point.detail) * 0.10
            + highland * 0.13,
    ) * land
        * detail_visibility;
    roughness = clamp(
        roughness
            + (material_fine - 0.5) * land * 0.34
            + (surface_grain - 0.5) * land * 0.28
            + rock_mask * 0.07
            - water * 0.34,
        0.045,
        0.96,
    );
    let mut base = sample.albedo
        * (0.58
            + (local_detail - 0.5) * land * 0.46 * detail_visibility
            + (point.detail - 0.5) * land * 0.24
            + (material_fine - 0.5) * land * 0.24 * detail_visibility);
    base = base * (0.74 + surface_grain * 0.52 * land * detail_visibility + water * 0.26);
    let soil = Vec3::new(0.12, 0.115, 0.085).lerp(Vec3::new(0.34, 0.27, 0.17), local_detail);
    base = base.lerp(soil, land * (0.28 + (1.0 - surface_slope) * 0.20));
    let mineral_patch =
        smoothstep(0.43, 0.70, material_macro) * smoothstep(0.30, 0.82, ridge(local_detail));
    base = base.lerp(
        Vec3::new(0.16, 0.145, 0.125).lerp(Vec3::new(0.43, 0.31, 0.20), material_fine),
        mineral_patch * land * (0.18 + surface_slope * 0.22),
    );
    base = base.lerp(
        Vec3::new(0.17, 0.18, 0.18).lerp(
            Vec3::new(0.56, 0.52, 0.43),
            local_detail * 0.68 + material_fine * 0.32,
        ),
        rock_mask * 0.84,
    );
    let scree = smoothstep(
        0.48,
        0.86,
        ridge(surface_grain) * 0.56 + ridge(material_fine) * 0.24 + surface_slope * 0.20,
    ) * land
        * (0.24 + surface_slope * 0.76)
        * detail_visibility;
    base = base.lerp(
        Vec3::new(0.065, 0.070, 0.068).lerp(Vec3::new(0.27, 0.25, 0.21), material_fine),
        scree * 0.62,
    );
    let fracture_noise = fbm_tiled(
        point.world_x * 0.22 + material_macro * 0.085,
        point.world_z * 0.20 - local_detail * 0.070,
        61,
        3,
        profile.seed + 41_251,
        0.46,
    );
    let fracture = smoothstep(0.88, 0.985, ridge(fracture_noise))
        * land
        * (0.24 + surface_slope * 0.76)
        * detail_visibility;
    base = base.lerp(Vec3::new(0.028, 0.032, 0.030), fracture * 0.62);
    base = base.lerp(
        Vec3::new(0.055, 0.070, 0.052),
        shore * land * (0.54 + sample.wetness * 0.18),
    );
    base = base.lerp(
        Vec3::new(0.10, 0.27, 0.10).lerp(Vec3::new(0.24, 0.38, 0.15), local_detail),
        sample.vegetation * land * (1.0 - surface_slope * 0.72) * 0.34,
    );
    let procedural_cover = smoothstep(
        0.38,
        0.62,
        local_detail * 0.44 + point.detail * 0.34 + material_fine * 0.22,
    ) * (1.0 - surface_slope)
        * (1.0 - highland * 0.64)
        * (1.0 - shore * 0.52)
        * land;
    base = base.lerp(
        Vec3::new(0.095, 0.225, 0.075).lerp(Vec3::new(0.20, 0.32, 0.11), point.detail),
        procedural_cover * (0.42 + sample.vegetation * 0.34),
    );
    let strata =
        ((point.height * 24.0 + point.world_x * 0.42 + (material_macro - 0.5) * 5.8).sin() * 0.5
            + 0.5)
            * surface_slope
            * rock_mask;
    base = base.lerp(
        Vec3::new(0.31, 0.28, 0.24).lerp(Vec3::new(0.62, 0.57, 0.47), material_fine),
        strata * 0.24 * detail_visibility,
    );

    let oren_nayar = 1.0 - roughness * 0.23 + roughness * 0.11 * (1.0 - ndotv);
    let direct_visibility = (1.0 - shadow) * (1.0 - material_ao * 0.42);
    let mut color = if night_view {
        let base_luma = base.x * 0.2126 + base.y * 0.7152 + base.z * 0.0722;
        let moon = Vec3::new(0.14, 0.27, 0.70) * base_luma;
        let sky_ambient = (Vec3::new(0.022, 0.046, 0.115) * base_luma + base * 0.009)
            * (0.52 + sky_visibility * 0.48);
        sky_ambient + moon * ndotl * oren_nayar * direct_visibility * 0.72
    } else {
        let sun = Vec3::new(base.x * 1.00, base.y * 0.94, base.z * 0.84);
        let ambient = base * (0.22 + atmosphere * 0.040) * (0.62 + sky_visibility * 0.38);
        ambient + sun * ndotl * oren_nayar * direct_visibility
    };

    if land > 0.01 {
        let land_specular = normal
            .dot(half)
            .max(0.0)
            .powf(12.0 + (1.0 - roughness) * 52.0)
            * ndotl
            * (1.0 - roughness)
            * (sample.wetness * 0.14 + rock_mask * 0.035)
            * (1.0 - shadow);
        color += if night_view {
            Vec3::new(0.16, 0.24, 0.58)
        } else {
            Vec3::new(0.92, 0.82, 0.64)
        } * land_specular;
    }

    if water > 0.01 {
        let depth = clamp01((0.62 - sample.height) * 1.70 + raw_water * 0.36);
        let water_base = Vec3::new(0.002, 0.020, 0.082)
            .lerp(Vec3::new(0.012, 0.082, 0.185), 1.0 - depth * 0.62)
            .lerp(Vec3::new(0.026, 0.072, 0.082), shore * (1.0 - depth) * 0.30);
        let reflected = (normal * (2.0 * normal.dot(view_dir)) - view_dir).normalize();
        let space_environment = sample_environment(
            reflected,
            profile.seed + 41_307,
            distant_light_for_mode(lighting_mode),
        );
        let grazing = (1.0 - ndotv).powf(1.5);
        let atmosphere_reflection = if night_view {
            Vec3::new(0.004, 0.012, 0.040).lerp(Vec3::new(0.020, 0.035, 0.082), grazing)
        } else {
            Vec3::new(0.16, 0.32, 0.52).lerp(Vec3::new(0.62, 0.73, 0.76), grazing)
        };
        // The atmospheric sky is the surface-water environment. Retain only
        // a very small deep-space contribution for genuinely tenuous air;
        // otherwise the distant-light lobe becomes a detached white blob
        // instead of a half-vector-aligned reflection streak.
        let environment =
            atmosphere_reflection.lerp(space_environment, (1.0 - clamp01(atmosphere)) * 0.08);
        let fresnel = schlick_fresnel(ndotv, 0.022);
        let wave_tone = (point.detail - 0.5) * 0.016 * detail_visibility;
        let water_direct = if night_view {
            Vec3::new(
                water_base.x * 0.08,
                water_base.y * 0.16,
                water_base.z * 0.42,
            ) * (0.045 + ndotl * 0.22 * (1.0 - shadow))
        } else {
            water_base * (0.16 + ndotl * 0.46 * (1.0 - shadow) + wave_tone)
        };
        let mut water_color = water_direct.lerp(
            environment,
            clamp01(fresnel * (0.56 + (1.0 - roughness) * 0.30)),
        );
        let specular = normal
            .dot(half)
            .max(0.0)
            .powf(62.0 + (1.0 - roughness) * 94.0)
            * ndotl
            * (1.0 - shadow)
            * (0.16 + fresnel * 1.30);
        water_color += if night_view {
            Vec3::new(0.34, 0.48, 1.0) * specular * 0.26
        } else {
            Vec3::new(1.0, 0.88, 0.64) * specular
        };
        let reflected_cloud =
            smoothstep(0.10, 0.68, cast_cloud) * fresnel * (0.20 + grazing * 0.22);
        water_color = water_color.lerp(
            if night_view {
                Vec3::new(0.018, 0.032, 0.075)
            } else {
                Vec3::new(0.58, 0.66, 0.68)
            },
            reflected_cloud * if night_view { 0.28 } else { 1.0 },
        );
        let foam = shore
            * smoothstep(
                0.43,
                0.91,
                ridge(local_detail) * 0.46 + ridge(point.detail) * 0.34,
            )
            * (0.30 + detail_visibility * 0.26);
        water_color += Vec3::new(0.62, 0.74, 0.72) * foam * if night_view { 0.035 } else { 0.18 };
        color = color.lerp(water_color, water);
    }

    if night_view && sample.city > 0.006 {
        let city_du = 1.25 / maps.width.max(1) as f32;
        let city_dv = 1.25 / maps.height.max(1) as f32;
        let city_field = sample.city * 0.56
            + maps.sample(point.u - city_du, point.v).city * 0.11
            + maps.sample(point.u + city_du, point.v).city * 0.11
            + maps.sample(point.u, point.v - city_dv).city * 0.11
            + maps.sample(point.u, point.v + city_dv).city * 0.11;
        let terrain_conformity = land
            * (1.0 - smoothstep(0.32, 0.92, surface_slope))
            * (1.0 - shore * 0.58)
            * (1.0 - sample.cloud * 0.46);
        let edge_fade = smoothstep(0.050, 0.125, fx) * smoothstep(0.050, 0.125, 1.0 - fx);
        let distance_fade = 1.0 - smoothstep(5.4, 8.0, hit.distance);
        let city = city_field.powf(1.22) * terrain_conformity * edge_fade * distance_fade;
        color += Vec3::new(2.15, 1.08, 0.34) * city;
    }
    let hot_albedo = clamp01(
        (sample.albedo.x - sample.albedo.z * 1.55) * 1.30
            + (sample.albedo.y - sample.albedo.z) * 0.52,
    );
    let lava_crack = smoothstep(
        0.72,
        0.96,
        ridge(material_fine) * 0.62 + ridge(local_detail) * 0.38,
    ) * smoothstep(0.82, 0.97, profile.volcanic_activity)
        * rock_mask;
    color += Vec3::new(2.8, 0.62, 0.085)
        * hot_albedo
        * lava_crack
        * if night_view { 0.30 } else { 0.045 };

    let aerial_breakup = fbm_tiled(
        point.world_x * 0.052 + point.detail * 0.025,
        point.world_z * 0.048 - local_detail * 0.020,
        17,
        2,
        profile.seed + 41_487,
        0.50,
    );
    let far_haze = smoothstep(3.0, 8.1, hit.distance)
        * (0.25 + atmosphere * 0.34 + sample.cloud * 0.045)
        * (0.88 + aerial_breakup * 0.12);
    let haze = if night_view {
        Vec3::new(0.022, 0.052, 0.120)
    } else {
        Vec3::new(0.34, 0.50, 0.60)
    };
    color = color.lerp(haze, clamp01(far_haze));
    let horizon_proximity = smoothstep(-0.34, 0.035, vertical_ray);
    let horizon_lod = horizon_proximity * if night_view { 0.42 } else { 0.26 };
    let horizon_air = if night_view {
        Vec3::new(0.062, 0.115, 0.235)
    } else {
        sky_color
            .unwrap_or(Vec3::new(0.34, 0.50, 0.60))
            .lerp(Vec3::new(0.34, 0.50, 0.60), 0.08)
    };
    color = color.lerp(horizon_air, horizon_lod);

    let reflection_x = if night_view { 0.74 } else { sun_screen.x };
    let screen_ground = clamp01((fy - local_horizon) / (1.0 - local_horizon).max(0.1));
    let water_glow = water
        * smoothstep(0.20, 0.0, ((fx - reflection_x) * aspect).abs())
        * smoothstep(0.76, 0.03, screen_ground);
    color += if night_view {
        Vec3::new(0.16, 0.24, 0.65)
    } else {
        Vec3::new(0.86, 0.68, 0.36)
    } * water_glow
        * if night_view { 0.024 } else { 0.050 };
    let vignette = smoothstep(1.08, 0.22, sx.abs()) * smoothstep(1.0, 0.38, screen_ground);
    color = color * (0.91 + vignette * 0.10);
    color = apply_anti_band_grain(
        color,
        fx + point.detail * 0.011,
        screen_ground + local_detail * 0.009,
        profile.seed + 41_503,
        if night_view { 0.0038 } else { 0.0028 } + far_haze * 0.0015,
    );
    rgba(tone_map(color * if night_view { 1.26 } else { 0.78 }), 255)
}

/// Stable local tangent-plane renderer for solid-surface temperate and
/// artificial worlds. Unlike the legacy reciprocal UV projector, this path
/// has bounded screen-to-world derivatives at both the horizon and the near
/// field. It therefore cannot magnify a few map texels into full-height
/// ribbons. Geometry remains a deterministic CPU heightfield product: global
/// maps provide macro materials and relief, while analytic microgeometry keeps
/// the near field resolved without allocating a second giant texture.
#[allow(clippy::too_many_arguments)]
fn stable_terrain_overview_pixel(
    fx: f32,
    fy: f32,
    sx: f32,
    local_horizon: f32,
    aspect: f32,
    sun_screen: Vec3,
    sky_color: Option<Vec3>,
    frame: StableTerrainFrame,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    lighting_mode: LightingMode,
) -> [u8; 4] {
    let night_view = lighting_mode.is_night();
    let atmosphere = clamp(profile.atmosphere_density, 0.0, 1.6);
    let skyline_mass = fbm_periodic(
        fx * 0.44 + frame.anchor_u * 0.17,
        0.37 + frame.anchor_v * 0.11,
        3,
        4,
        profile.seed + 40_101,
        0.53,
    );
    let skyline_detail = fbm_periodic(
        fx * 0.82 - frame.anchor_v * 0.13,
        0.61 + frame.anchor_u * 0.09,
        7,
        2,
        profile.seed + 40_127,
        0.46,
    );
    let skyline_lift =
        0.004 + smoothstep(0.48, 0.90, skyline_mass) * 0.017 + ridge(skyline_detail) * 0.0045;
    let skyline = clamp(
        local_horizon - skyline_lift,
        local_horizon - 0.028,
        local_horizon - 0.003,
    );
    let fallback_sky = if night_view {
        Vec3::new(0.008, 0.020, 0.060)
    } else {
        Vec3::new(0.54, 0.69, 0.80)
    };
    if fy < skyline {
        return rgba(tone_map(sky_color.unwrap_or(fallback_sky)), 255);
    }

    let ground_v = clamp01((fy - skyline) / (1.0 - skyline));
    let far = 1.0 - ground_v;
    let world_z = 0.22 + far * 5.35;
    let world_x = sx * (0.78 + world_z * 0.22) / aspect.max(0.72).sqrt();
    let domain_a = fbm_tiled(
        world_x * 0.095 + frame.anchor_u,
        world_z * 0.082 + frame.anchor_v,
        19,
        3,
        profile.seed + 40_211,
        0.51,
    );
    let domain_b = fbm_tiled(
        world_x * 0.071 - domain_a * 0.10,
        world_z * 0.104 + domain_a * 0.08,
        23,
        3,
        profile.seed + 40_237,
        0.49,
    );
    let warp_x = (domain_a - 0.5) * 0.0045 + (domain_b - 0.5) * 0.0020;
    let warp_z = (domain_b - 0.5) * 0.0040 - (domain_a - 0.5) * 0.0018;
    let (u, v) = frame.map_uv(world_x, world_z, warp_x, warp_z);

    let distance_filter = smoothstep(3.65, 5.55, world_z);
    let sample_step = 0.80 + distance_filter * 2.4;
    let eps_u = sample_step / maps.width.max(1) as f32;
    let eps_v = sample_step / maps.height.max(1) as f32;
    let sample = terrain_overview_sample_lod(maps, u, v, eps_u, eps_v, distance_filter * 0.82);
    let raw_water = clamp01(sample.water);
    let water = smoothstep(0.40, 0.64, raw_water);
    let land = 1.0 - water;
    let shore = 1.0 - smoothstep(0.028, 0.30, (raw_water - 0.50).abs());

    let map_gradient_step = 1.45 / maps.width.max(1) as f32;
    let side_u = frame.side_u * map_gradient_step;
    let side_v = frame.side_v * map_gradient_step;
    let forward_u = frame.forward_u * map_gradient_step;
    let forward_v = frame.forward_v * map_gradient_step;
    let height_left = terrain_overview_elevation(maps.sample(u - side_u, v - side_v));
    let height_right = terrain_overview_elevation(maps.sample(u + side_u, v + side_v));
    let height_far = terrain_overview_elevation(maps.sample(u + forward_u, v + forward_v));
    let height_near = terrain_overview_elevation(maps.sample(u - forward_u, v - forward_v));

    let seed_phase = hash2(211, 307, profile.seed + 40_307) * PI * 2.0;
    let phase_a = world_x * 7.4 + world_z * 2.7 + seed_phase;
    let phase_b = world_x * -3.8 + world_z * 8.1 - seed_phase * 0.63;
    let phase_c = world_x * 12.7 + world_z * -5.3 + seed_phase * 1.37;
    let micro_dx = (phase_a.cos() * 0.072 - phase_b.cos() * 0.030 + phase_c.cos() * 0.018)
        * land
        * (1.0 - distance_filter * 0.72);
    let micro_dz = (phase_a.cos() * 0.026 + phase_b.cos() * 0.064 - phase_c.cos() * 0.022)
        * land
        * (1.0 - distance_filter * 0.72);
    let relief_strength = 18.0 + land * 12.0 + smoothstep(0.60, 0.96, ground_v) * 7.0;
    let mut normal = Vec3::new(
        (height_left - height_right) * relief_strength + micro_dx,
        1.0,
        (height_near - height_far) * relief_strength + micro_dz,
    )
    .normalize();

    let wave_a = (world_x * 9.2 + world_z * 3.4 + seed_phase * 0.8).sin();
    let wave_b = (world_x * -5.7 + world_z * 11.3 - seed_phase).sin();
    let wave_c = (world_x * 17.1 + world_z * -7.6 + seed_phase * 1.6).sin();
    if water > 0.01 {
        let wave_normal = Vec3::new(
            wave_a * 0.055 - wave_b * 0.028 + wave_c * 0.012,
            1.0,
            wave_a * 0.021 + wave_b * 0.060 - wave_c * 0.018,
        )
        .normalize();
        normal = normal
            .lerp(wave_normal, water * (0.84 - distance_filter * 0.30))
            .normalize();
    }

    let local_detail = fbm_tiled(
        world_x * 0.18 + domain_a * 0.12,
        world_z * 0.16 - domain_b * 0.10,
        37,
        4,
        profile.seed + 40_401,
        0.48,
    );
    let local_fine = fbm_tiled(
        world_x * 0.46 - local_detail * 0.08,
        world_z * 0.41 + local_detail * 0.07,
        61,
        2,
        profile.seed + 40_433,
        0.44,
    );
    let light_dir = overview_light_dir(lighting_mode);
    let view_dir = Vec3::new(-sx * 0.08, 0.58 + ground_v * 0.10, 0.82).normalize();
    let half = (light_dir + view_dir).normalize();
    let ndotl = normal.dot(light_dir).max(0.0);
    let ndotv = normal.dot(view_dir).max(0.0);
    let roughness = clamp(sample.roughness, 0.04, 0.96);
    let sky_visibility = 1.0 - sample.horizon_occlusion * land;
    let ambient = (if night_view {
        0.045
    } else {
        0.205 + atmosphere * 0.040
    }) * (0.58 + sky_visibility * 0.42);
    let oren_nayar = 1.0 - roughness * 0.22 + roughness * 0.10 * (1.0 - ndotv);
    let cast_cloud = maps
        .sample(
            u - light_dir.x * (0.012 + sample.cloud * 0.008),
            v - light_dir.z * (0.010 + sample.cloud * 0.006),
        )
        .cloud;
    let cloud_shadow = smoothstep(0.10, 0.72, cast_cloud)
        * (0.12 + atmosphere * 0.035)
        * smoothstep(0.02, 0.78, ground_v);
    let material_ao = sample.ambient_occlusion * land * (0.34 + roughness * 0.12);

    let mut base = sample.albedo
        * (0.88 + (local_detail - 0.5) * land * 0.24 + (local_fine - 0.5) * land * 0.10);
    if land > 0.01 {
        let wet_soil = Vec3::new(0.22, 0.20, 0.15).lerp(Vec3::new(0.34, 0.32, 0.24), local_detail);
        base = base.lerp(wet_soil, shore * land * (0.24 + sample.wetness * 0.18));
        base = base.lerp(
            Vec3::new(0.19, 0.30, 0.16),
            sample.vegetation * land * (0.16 + local_detail * 0.10),
        );
    }
    let mut color =
        base * (ambient + ndotl * oren_nayar * (1.0 - cloud_shadow)) * (1.0 - material_ao);

    if water > 0.01 {
        let depth = clamp01((0.62 - sample.height) * 1.65 + raw_water * 0.34);
        let deep = Vec3::new(0.003, 0.024, 0.095);
        let mid = Vec3::new(0.012, 0.082, 0.190);
        let shelf = Vec3::new(0.045, 0.270, 0.350);
        let water_base = deep
            .lerp(mid, 1.0 - depth * 0.62)
            .lerp(shelf, shore * (1.0 - depth) * 0.72)
            * (0.84 + (local_detail - 0.5) * 0.08);
        let reflected = (normal * (2.0 * normal.dot(view_dir)) - view_dir).normalize();
        let environment = sample_environment(
            reflected,
            profile.seed + 40_509,
            distant_light_for_mode(lighting_mode),
        );
        let fresnel = schlick_fresnel(ndotv, 0.022);
        let water_diffuse = water_base
            * ((if night_view { 0.055 } else { 0.13 }) + ndotl * 0.42)
            * (1.0 - cloud_shadow * 0.54);
        let mut water_color = water_diffuse.lerp(
            environment,
            clamp01(fresnel * (0.52 + (1.0 - roughness) * 0.28)),
        );
        let specular = normal
            .dot(half)
            .max(0.0)
            .powf(48.0 + (1.0 - roughness) * 84.0)
            * ndotl
            * (0.18 + fresnel * 1.25);
        water_color += if night_view {
            Vec3::new(0.46, 0.60, 1.0)
        } else {
            Vec3::new(1.0, 0.88, 0.64)
        } * specular;
        let foam = shore
            * smoothstep(0.42, 0.94, ridge(local_fine) * 0.56 + wave_a.abs() * 0.24)
            * (0.30 + ground_v * 0.24);
        water_color += Vec3::new(0.58, 0.72, 0.70) * foam * if night_view { 0.08 } else { 0.19 };
        color = color.lerp(water_color, water);
    }

    if sample.cloud > 0.025 {
        let cloud_edge = clamp01(
            (maps.sample(u + eps_u, v).cloud - maps.sample(u - eps_u, v).cloud).abs()
                + (maps.sample(u, v + eps_v).cloud - maps.sample(u, v - eps_v).cloud).abs(),
        );
        let cloud_alpha = smoothstep(0.04, 0.72, sample.cloud)
            * (0.10 + cloud_edge * 0.16 + atmosphere * 0.025)
            * smoothstep(0.02, 0.64, ground_v);
        let cloud_color = if night_view {
            Vec3::new(0.12, 0.16, 0.30) * (0.34 + ndotl * 0.28)
        } else {
            Vec3::new(0.78, 0.82, 0.80).lerp(
                Vec3::new(0.98, 0.98, 0.94),
                ndotl * 0.58 + cloud_edge * 0.16,
            )
        };
        color = color.lerp(cloud_color, cloud_alpha);
    }

    if night_view && sample.city > 0.01 {
        let city = sample.city * land * (1.0 - sample.cloud * 0.38);
        color += Vec3::new(3.4, 1.72, 0.56) * city;
    }
    let hot_albedo = clamp01(
        (sample.albedo.x - sample.albedo.z * 1.55) * 1.30
            + (sample.albedo.y - sample.albedo.z) * 0.52,
    );
    color += Vec3::new(2.8, 0.64, 0.095)
        * hot_albedo
        * profile.volcanic_activity
        * land
        * if night_view { 0.34 } else { 0.075 };

    let far_haze =
        smoothstep(2.65, 5.56, world_z) * (0.12 + atmosphere * 0.18 + sample.cloud * 0.035);
    let haze = if night_view {
        Vec3::new(0.018, 0.040, 0.095)
    } else {
        Vec3::new(0.57, 0.70, 0.76)
    };
    color = color.lerp(haze, clamp01(far_haze));
    if let Some(sky) = sky_color {
        color = sky.lerp(color, smoothstep(0.0, 0.026, ground_v));
    }

    let reflection_x = if night_view { 0.74 } else { sun_screen.x };
    let water_glow = water
        * smoothstep(0.20, 0.0, ((fx - reflection_x) * aspect).abs())
        * smoothstep(0.74, 0.03, ground_v);
    color += if night_view {
        Vec3::new(0.24, 0.34, 0.78)
    } else {
        Vec3::new(0.86, 0.68, 0.36)
    } * water_glow
        * 0.055;
    let vignette = smoothstep(1.08, 0.22, sx.abs()) * smoothstep(1.0, 0.40, ground_v);
    color = color * (0.90 + vignette * 0.11);
    color = apply_anti_band_grain(
        color,
        fx + domain_a * 0.013,
        ground_v + domain_b * 0.011,
        profile.seed + 40_607,
        if night_view { 0.0042 } else { 0.0030 } + far_haze * 0.0016,
    );

    rgba(tone_map(color * if night_view { 1.04 } else { 0.98 }), 255)
}

#[allow(clippy::too_many_arguments)]
fn ocean_world_terrain_overview_pixel(
    fx: f32,
    fy: f32,
    sx: f32,
    local_horizon: f32,
    aspect: f32,
    sun_screen: Vec3,
    sky_color: Option<Vec3>,
    profile: &PlanetVisualProfile,
    lighting_mode: LightingMode,
    physics: PlanetPhysicsModel,
) -> [u8; 4] {
    let night_view = lighting_mode.is_night();
    if fy < local_horizon {
        return rgba(
            tone_map(sky_color.unwrap_or(if night_view {
                Vec3::new(0.010, 0.026, 0.070)
            } else {
                Vec3::new(0.08, 0.18, 0.32)
            })),
            255,
        );
    }

    let atmosphere = clamp(profile.atmosphere_density, 0.0, 1.6);
    let horizon_swell = fbm_tiled(
        fx * 0.24 + 0.13,
        local_horizon * 0.18 + 0.47,
        11,
        3,
        profile.seed + 26_101,
        0.52,
    );
    let water_horizon = clamp(
        local_horizon + 0.044 + (horizon_swell - 0.5) * 0.007,
        local_horizon + 0.034,
        local_horizon + 0.058,
    );
    let horizon_cloud_warp = fbm_tiled(
        fx * 0.32 + local_horizon * 0.08,
        local_horizon * 0.30 + 0.33,
        23,
        3,
        profile.seed + 27_011,
        0.50,
    );
    let horizon_cloud_mass = fbm_tiled(
        fx * 0.62 + horizon_cloud_warp * 0.14 + 0.19,
        local_horizon * 0.44 + horizon_cloud_warp * 0.10 + 0.33,
        41,
        5,
        profile.seed + 27_101,
        0.55,
    );
    let horizon_cloud_detail = fbm_tiled(
        fx * 1.70 + horizon_cloud_mass * 0.16,
        local_horizon * 0.72 + horizon_cloud_warp * 0.12 - 0.11,
        73,
        3,
        profile.seed + 27_203,
        0.48,
    );
    let horizon_cloud = smoothstep(
        0.44,
        0.86,
        horizon_cloud_mass * 0.62 + ridge(horizon_cloud_detail) * 0.24 + atmosphere * 0.06,
    ) * profile.cloud_density
        * (0.70 + atmosphere * 0.28);
    let horizon_water = if night_view {
        Vec3::new(0.018, 0.050, 0.115).lerp(Vec3::new(0.045, 0.085, 0.170), atmosphere * 0.28)
    } else {
        Vec3::new(0.48, 0.66, 0.76).lerp(Vec3::new(0.62, 0.76, 0.82), atmosphere * 0.22)
    };
    if fy < water_horizon {
        let t = smoothstep(local_horizon, water_horizon, fy);
        let sky = sky_color.unwrap_or(Vec3::new(0.54, 0.68, 0.76));
        let mist = if night_view {
            Vec3::new(0.040, 0.075, 0.150).lerp(horizon_water, 0.44)
        } else {
            Vec3::new(0.70, 0.80, 0.84).lerp(horizon_water, 0.34)
        };
        let mut color = sky.lerp(mist, t);
        let refract = fbm_tiled(
            fx * 0.70 + t * 0.18,
            local_horizon * 0.42 + t * 0.24,
            43,
            2,
            profile.seed + 27_257,
            0.47,
        );
        color = color.lerp(
            horizon_water,
            t * (0.10 + atmosphere * 0.040) * (0.74 + refract * 0.26),
        );
        let cloud_bank = horizon_cloud
            * smoothstep(local_horizon, water_horizon, fy)
            * smoothstep(water_horizon + 0.010, local_horizon + 0.014, fy);
        color = color.lerp(
            if night_view {
                Vec3::new(0.095, 0.125, 0.220)
            } else {
                Vec3::new(0.92, 0.95, 0.94)
            },
            cloud_bank * if night_view { 0.34 } else { 0.68 },
        );
        color = apply_anti_band_grain(
            color,
            fx,
            fy + refract * 0.013,
            profile.seed + 27_311,
            if night_view { 0.0038 } else { 0.0028 } + t * 0.0018,
        );
        return rgba(tone_map(color), 255);
    }

    let ground_v = clamp01((fy - water_horizon) / (1.0 - water_horizon));
    let perspective = 1.0 / (ground_v * 2.2 + 0.46);
    let world_x = sx * aspect * (1.7 + perspective * 0.55) + hash2(5, 11, profile.seed) * 8.0;
    let world_z = ground_v * 5.5 + perspective * 1.05 + hash2(13, 19, profile.seed) * 8.0;
    let warp = fbm_tiled(
        world_x * 0.050,
        world_z * 0.044,
        751,
        4,
        profile.seed + 26_211,
        0.52,
    );
    let ox = world_x + world_z * 0.20 + (warp - 0.5) * 2.4;
    let oz = world_z - world_x * 0.14 + (warp - 0.5) * 1.8;
    let regional = ocean_regional_cues(
        ox * 0.055 + warp * 0.035,
        oz * 0.045 - warp * 0.025,
        profile.seed + 26_909,
    );
    let physics_u = (ox * 0.035 + regional.gyre * 0.020).rem_euclid(1.0);
    let physics_v = (0.30 + oz * 0.030 + ground_v * 0.34).rem_euclid(1.0);
    let physics_sample = physics.sample(physics_u, physics_v);
    let physics_current_norm = clamp01(
        physics_sample.current_speed_mps / (physics.current_velocity_scale_mps * 1.45).max(0.001),
    );
    let density_front =
        clamp01((physics_sample.water_density_kg_m3 - physics.water_density_kg_m3).abs() / 48.0);
    let ox = ox
        + (regional.current - 0.5) * 1.25
        + (regional.gyre - 0.5) * 0.58
        + regional.current_edge * 0.24
        + physics_sample.ocean_current_mps.x * 0.18;
    let oz = oz + (regional.gyre - 0.5) * 1.05 - (regional.current - 0.5) * 0.42;
    let oz = oz + physics_sample.ocean_current_mps.y * 0.14;
    let basin = fbm_tiled(ox * 0.078, oz * 0.066, 797, 5, profile.seed + 26_307, 0.54);
    let swell = fbm_tiled(
        ox * 0.36 + basin * 0.10,
        oz * 0.28 - basin * 0.08,
        839,
        4,
        profile.seed + 26_401,
        0.50,
    );
    let cross = fbm_tiled(
        ox * 0.82 - swell * 0.12,
        oz * 0.68 + basin * 0.08,
        881,
        3,
        profile.seed + 26_503,
        0.46,
    );
    let ripple = fbm_tiled(
        ox * 1.16 + cross * 0.07 + regional.current * 0.12,
        oz * 0.94 - swell * 0.052 - regional.gyre * 0.10,
        233,
        2,
        profile.seed + 26_607,
        0.43,
    );
    let cloud_mass = fbm_tiled(
        ox * 0.090 + regional.gyre * 0.055,
        oz * 0.072 - regional.current * 0.050,
        937,
        5,
        profile.seed + 27_307,
        0.55,
    );
    let cloud_detail = fbm_tiled(
        ox * 0.34 + cloud_mass * 0.12,
        oz * 0.28 - cloud_mass * 0.09,
        977,
        3,
        profile.seed + 27_409,
        0.48,
    );
    let cloud_altitude = clamp01(
        0.36 + physics_sample.cloud_lift * 0.34
            + atmosphere * 0.12
            + ridge(cloud_detail) * 0.10
            + profile.cloud_density * 0.06,
    );
    let cloud_shadow = smoothstep(
        0.55,
        0.93,
        cloud_mass * 0.55
            + ridge(cloud_detail) * 0.25
            + regional.current_edge * 0.12
            + physics_sample.cloud_lift * 0.16
            + physics_sample.humidity * 0.06,
    ) * profile.cloud_density
        * smoothstep(0.04, 0.70, ground_v)
        * (1.0 - smoothstep(0.86, 1.0, ground_v))
        * (0.82 + cloud_altitude * 0.30);

    let depth = clamp01(
        0.52 + (1.0 - basin) * 0.20
            + (1.0 - swell) * 0.075
            + regional.depth_patch * 0.15
            + regional.current_edge * 0.055
            + physics_sample.current_shear * 0.046
            + density_front * 0.040
            + ground_v * 0.035
            + (ripple - 0.5) * 0.055,
    );
    let deep = Vec3::new(0.006, 0.034, 0.105);
    let trench = Vec3::new(0.0015, 0.014, 0.064);
    let mid = Vec3::new(0.016, 0.080, 0.175);
    let shelf = Vec3::new(0.040, 0.250, 0.345);
    let current_tint =
        Vec3::new(0.010, 0.072, 0.150).lerp(Vec3::new(0.030, 0.165, 0.220), regional.warm);
    let lit = Vec3::new(0.055, 0.180, 0.265);
    let shelf_signal =
        clamp01((1.0 - depth) * 0.42 + regional.reef * 0.22 + regional.sediment * 0.12);
    let mut color = trench
        .lerp(deep, 1.0 - depth * 0.28)
        .lerp(mid, 0.32 + basin * 0.26 + regional.gyre * 0.12)
        .lerp(shelf, shelf_signal)
        .lerp(
            current_tint,
            regional.current_edge * (0.24 + (1.0 - depth) * 0.18),
        )
        .lerp(
            lit,
            smoothstep(0.42, 0.90, regional.warm) * (1.0 - depth) * 0.12,
        )
        .lerp(Vec3::new(0.002, 0.018, 0.070), depth * 0.12);
    let wave_shadow = (swell - 0.5) * 0.075
        + (cross - 0.5) * 0.055
        + (ripple - 0.5) * 0.035
        + (regional.current - 0.5) * 0.050;
    let wave_phase_breakup = fbm_tiled(
        ox * 0.16 + regional.gyre * 0.090,
        oz * 0.13 - regional.current * 0.075,
        59,
        3,
        profile.seed + 26_671,
        0.48,
    );
    let wave_phase_fine = fbm_tiled(
        ox * 0.58 + wave_phase_breakup * 0.11,
        oz * 0.47 - wave_phase_breakup * 0.08,
        101,
        2,
        profile.seed + 26_719,
        0.44,
    );
    let long_wave_a = ((ox * 1.36
        + oz * 0.42
        + regional.gyre * 2.4
        + warp * 0.90
        + (wave_phase_breakup - 0.5) * 1.30
        + (wave_phase_fine - 0.5) * 0.46)
        .sin()
        * 0.5
        + 0.5)
        * (0.68 + swell * 0.24 + ridge(wave_phase_breakup) * 0.08);
    let long_wave_b = ((ox * -0.58
        + oz * 1.12
        + regional.current * 2.8
        + basin * 1.4
        + (wave_phase_breakup - 0.5) * 1.05
        - (wave_phase_fine - 0.5) * 0.38)
        .sin()
        * 0.5
        + 0.5)
        * (0.66 + cross * 0.24 + ridge(wave_phase_fine) * 0.10);
    let swell_lane = smoothstep(
        0.62,
        0.97,
        long_wave_a * 0.42 + long_wave_b * 0.28 + regional.current_edge * 0.18 + ripple * 0.12,
    ) * smoothstep(0.06, 0.78, ground_v)
        * (1.0 - smoothstep(0.88, 1.0, ground_v));
    let current_lane = smoothstep(
        0.44,
        0.90,
        regional.current_edge * 0.68
            + (regional.current - 0.5).abs() * 0.36
            + ridge(cross) * 0.13
            + ridge(ripple) * 0.08,
    ) * smoothstep(0.03, 0.74, ground_v);
    let current_ribbon = smoothstep(
        0.50,
        0.95,
        regional.current_edge * 0.55
            + ridge(long_wave_b) * 0.22
            + (regional.gyre - 0.5).abs() * 0.18
            + physics_sample.current_shear * 0.16
            + physics_current_norm * 0.10
            + (1.0 - depth) * 0.06,
    ) * smoothstep(0.02, 0.70, ground_v)
        * (1.0 - smoothstep(0.84, 1.0, ground_v));
    color = color * (0.91 + wave_shadow);
    color = color.lerp(
        current_tint.lerp(Vec3::new(0.060, 0.240, 0.300), regional.warm),
        current_lane * if night_view { 0.32 } else { 0.28 },
    );
    color += if night_view {
        Vec3::new(0.030, 0.110, 0.245)
    } else {
        Vec3::new(0.035, 0.115, 0.145)
    } * swell_lane
        * (0.62 + regional.current_edge * 0.36);
    color += if night_view {
        Vec3::new(0.045, 0.135, 0.300)
    } else {
        Vec3::new(0.020, 0.105, 0.135)
    } * current_ribbon
        * (0.30 + regional.warm * 0.18 + ridge(ripple) * 0.10);
    color = apply_optical_medium(
        color,
        OpticalMedium::ocean(depth),
        0.40 + perspective * 0.42 + ground_v * 0.18,
        0.34 + regional.current_edge * 0.20 + ridge(swell) * 0.16,
        0.18 + regional.current_edge * 0.12 + ridge(cross) * 0.12,
    );
    color = color * (1.0 - cloud_shadow * if night_view { 0.18 } else { 0.24 });
    color = color.lerp(
        if night_view {
            Vec3::new(0.035, 0.075, 0.165)
        } else {
            Vec3::new(0.46, 0.62, 0.68)
        },
        cloud_shadow * smoothstep(0.03, 0.32, ground_v) * if night_view { 0.10 } else { 0.14 },
    );
    let cloud_reflection = cloud_shadow
        * (0.24 + cloud_altitude * 0.28)
        * smoothstep(0.02, 0.42, ground_v)
        * (1.0 - smoothstep(0.74, 1.0, ground_v));
    color = color * (1.0 - cloud_reflection * if night_view { 0.040 } else { 0.065 });
    color = color.lerp(
        if night_view {
            Vec3::new(0.055, 0.095, 0.200)
        } else {
            Vec3::new(0.58, 0.70, 0.72)
        },
        cloud_reflection * if night_view { 0.13 } else { 0.20 },
    );
    color = color.lerp(
        Vec3::new(0.010, 0.072, 0.160).lerp(Vec3::new(0.040, 0.170, 0.220), regional.warm),
        (physics_sample.current_shear * 0.18 + density_front * 0.12)
            * smoothstep(0.05, 0.78, ground_v),
    );

    let reflection_x = if night_view { 0.74 } else { sun_screen.x };
    let sun_path = smoothstep(
        0.31,
        0.0,
        ((fx - reflection_x + (swell - 0.5) * 0.050 + (cross - 0.5) * 0.032) * aspect).abs(),
    ) * smoothstep(0.82, 0.04, ground_v);
    let crest = smoothstep(
        0.58,
        0.960,
        ridge(swell) * 0.34
            + ridge(cross) * 0.28
            + ridge(ripple) * 0.16
            + regional.current_edge * 0.18
            + swell_lane * 0.24,
    ) * smoothstep(0.06, 0.70, ground_v)
        * (1.0 - smoothstep(0.80, 1.0, ground_v));
    let foam = smoothstep(
        0.84,
        0.992,
        crest + ridge(ripple) * 0.18 + current_lane * 0.10 + current_ribbon * 0.05,
    ) * (0.060 + smoothstep(0.16, 0.70, ground_v) * 0.150 + current_lane * 0.045);
    color += if night_view {
        Vec3::new(0.050, 0.125, 0.250)
    } else {
        Vec3::new(0.08, 0.20, 0.22)
    } * crest
        * if night_view { 0.44 } else { 0.56 };
    color += if night_view {
        Vec3::new(0.15, 0.25, 0.40)
    } else {
        Vec3::new(0.30, 0.45, 0.44)
    } * foam
        * (1.0 + current_lane * 0.40);
    color += if night_view {
        Vec3::new(0.52, 0.64, 1.0)
    } else {
        Vec3::new(0.78, 0.86, 0.72)
    } * sun_path
        * if night_view {
            0.070 + crest * 0.080 + current_ribbon * 0.040
        } else {
            0.055 + crest * 0.070 + current_ribbon * 0.025
        };

    let refraction_ripple = fbm_tiled(
        fx * 1.10 + swell * 0.11 + wave_phase_breakup * 0.07,
        ground_v * 0.92 - cross * 0.08,
        89,
        2,
        profile.seed + 27_773,
        0.45,
    );
    let far_haze = smoothstep(0.26, 0.0, ground_v)
        * (0.26 + atmosphere * 0.12)
        * (0.90 + refraction_ripple * 0.18 + cloud_altitude * cloud_shadow * 0.10);
    let near_vignette = smoothstep(0.98, 0.18, sx.abs()) * smoothstep(1.0, 0.46, ground_v);
    color = apply_optical_medium(
        color,
        OpticalMedium::atmosphere(atmosphere),
        far_haze * (1.66 + refraction_ripple * 0.24),
        0.18 + sun_path * 0.40,
        0.42,
    );
    color = color.lerp(
        horizon_water.lerp(
            Vec3::new(0.68, 0.78, 0.80),
            cloud_shadow * cloud_altitude * 0.18,
        ),
        far_haze * 0.72,
    );
    let horizon_diffusion = smoothstep(0.0, 0.060 + atmosphere * 0.012, ground_v);
    color = horizon_water
        .lerp(color, horizon_diffusion)
        .lerp(color, smoothstep(0.012, 0.090, ground_v));
    let near_depth_grade = smoothstep(0.30, 0.92, ground_v);
    let near_current_detail = smoothstep(
        0.36,
        0.94,
        ridge(swell) * 0.26
            + ridge(cross) * 0.22
            + regional.current_edge * 0.22
            + physics_sample.current_shear * 0.18
            + ridge(refraction_ripple) * 0.12,
    ) * near_depth_grade;
    color = color.lerp(
        if night_view {
            Vec3::new(0.002, 0.016, 0.062)
        } else {
            Vec3::new(0.004, 0.042, 0.116)
        },
        near_depth_grade * if night_view { 0.20 } else { 0.34 },
    );
    color += if night_view {
        Vec3::new(0.018, 0.070, 0.155)
    } else {
        Vec3::new(0.018, 0.120, 0.155)
    } * near_current_detail
        * if night_view { 0.24 } else { 0.36 };
    color += Vec3::new(0.16, 0.30, 0.31)
        * foam
        * near_depth_grade
        * if night_view { 0.045 } else { 0.080 };
    let foreground_depth_grade = smoothstep(0.58, 0.96, ground_v);
    color = color.lerp(
        if night_view {
            Vec3::new(0.001, 0.010, 0.045)
        } else {
            Vec3::new(0.001, 0.028, 0.088)
        },
        foreground_depth_grade * if night_view { 0.28 } else { 0.46 },
    );
    color += Vec3::new(0.010, 0.082, 0.120)
        * (ridge(ripple) * 0.34 + regional.current_edge * 0.28 + crest * 0.22)
        * foreground_depth_grade
        * if night_view { 0.20 } else { 0.34 };
    color = color * (0.88 + near_vignette * 0.13);
    if night_view {
        color = color * (0.42 + current_lane * 0.10 + current_ribbon * 0.08 + crest * 0.06);
        color = color.lerp(Vec3::new(0.004, 0.018, 0.060), 0.18 + ground_v * 0.06);
        color += Vec3::new(0.035, 0.085, 0.210)
            * (current_lane * 0.22 + current_ribbon * 0.18 + swell_lane * 0.12);
        color += Vec3::new(0.035, 0.070, 0.170) * far_haze * 0.28;
    }
    color = apply_anti_band_grain(
        color,
        fx + wave_phase_breakup * 0.017,
        ground_v + refraction_ripple * 0.013,
        profile.seed + 27_929,
        if night_view { 0.0048 } else { 0.0036 } + far_haze * 0.0024,
    );

    rgba(tone_map(color * if night_view { 1.10 } else { 0.96 }), 255)
}

#[allow(clippy::too_many_arguments)]
fn dry_world_terrain_overview_pixel(
    fx: f32,
    fy: f32,
    sx: f32,
    local_horizon: f32,
    aspect: f32,
    sky_color: Option<Vec3>,
    profile: &PlanetVisualProfile,
    rocky_material: RockySurfaceMaterial,
    volcanic_world: bool,
    lighting_mode: LightingMode,
) -> [u8; 4] {
    let night_view = lighting_mode.is_night();
    let ridge_seed = profile.seed + 30_101;
    let rocky_palette = rocky_surface_palette(profile, rocky_material);
    let skyline_noise = fbm_periodic(fx * 0.92 + 0.17, 0.41, 23, 5, ridge_seed, 0.55);
    let skyline_detail = fbm_periodic(fx * 2.8 - 0.22, 0.67, 71, 3, ridge_seed + 41, 0.48);
    let skyline = clamp(
        local_horizon
            + 0.026
            + skyline_noise * 0.044
            + ridge(skyline_detail) * if volcanic_world { 0.032 } else { 0.020 },
        local_horizon + 0.018,
        local_horizon + 0.118,
    );

    let light_dir = overview_light_dir(lighting_mode);
    let volcanic_air = if volcanic_world {
        clamp01(
            rocky_material.ash * 0.42
                + rocky_material.sulfur * 0.32
                + profile.atmosphere_density * 0.24,
        )
    } else {
        0.0
    };
    let rocky_air = if volcanic_world {
        0.0
    } else {
        clamp01(profile.atmosphere_density * 0.30 + rocky_palette.atmosphere_boost)
    };
    let haze = Vec3::new(0.64, 0.76, 0.82)
        .lerp(rocky_palette.haze, rocky_air * 0.62)
        .lerp(Vec3::new(0.86, 0.68, 0.42), volcanic_air * 0.40);
    let regolith = if volcanic_world {
        Vec3::new(0.34, 0.31, 0.26)
    } else {
        rocky_palette.mid
    };
    let pale_regolith = if volcanic_world {
        Vec3::new(0.64, 0.58, 0.46)
    } else {
        rocky_palette.high
    };
    let basalt = if volcanic_world {
        Vec3::new(0.055, 0.054, 0.050)
    } else {
        rocky_palette.shadow
    };
    let basalt_lit = if volcanic_world {
        Vec3::new(0.24, 0.22, 0.18)
    } else {
        rocky_palette.low.lerp(rocky_palette.mid, 0.34)
    };
    let ash = Vec3::new(0.20, 0.19, 0.17);
    let ash_lit = Vec3::new(0.56, 0.53, 0.46);
    let oxide = if volcanic_world {
        Vec3::new(0.62, 0.31, 0.16)
    } else {
        rocky_palette.mineral
    };
    let sulfur = Vec3::new(0.88, 0.72, 0.24);
    let (monolith_mask, monolith_light) =
        dry_world_monolith_mask(fx, fy, skyline, profile.seed, volcanic_world);

    if fy < skyline {
        let ridge_v = smoothstep(local_horizon, skyline, fy);
        let ridge_light = if night_view {
            0.060 + skyline_detail * 0.050 + ridge_v * 0.040
        } else {
            0.28 + skyline_detail * 0.16 + ridge_v * 0.10
        };
        let ridge_color = if volcanic_world {
            basalt.lerp(ash_lit, 0.34 + rocky_material.ash * 0.26)
        } else {
            regolith.lerp(pale_regolith, 0.34)
        } * ridge_light;
        let sky = sky_color.unwrap_or(haze);
        if monolith_mask > 0.004 {
            let monolith_color = if volcanic_world {
                basalt
                    .lerp(ash, 0.34)
                    .lerp(sulfur, rocky_material.sulfur * 0.22)
            } else {
                basalt_lit.lerp(regolith, 0.28)
            } * if night_view {
                0.10 + monolith_light * 0.20
            } else {
                0.34 + monolith_light * 0.42
            };
            return rgba(tone_map(sky.lerp(monolith_color, monolith_mask)), 255);
        }
        return rgba(
            tone_map(sky.lerp(ridge_color.lerp(haze, 0.28), ridge_v)),
            255,
        );
    }

    let ground_v = clamp01((fy - skyline) / (1.0 - skyline));
    let perspective = 1.0 / (ground_v * 2.6 + 0.62);
    let world_x = sx * aspect * (2.0 + perspective * 0.42) + hash2(3, 7, profile.seed) * 12.0;
    let world_z = ground_v * 5.8 + perspective * 0.95 + hash2(11, 17, profile.seed) * 9.0;
    let warp_a = fbm_periodic(
        world_x * 0.045,
        world_z * 0.050,
        18,
        4,
        profile.seed + 30_199,
        0.53,
    );
    let warp_b = fbm_periodic(
        world_x * 0.052 + 0.31,
        world_z * 0.043 - 0.27,
        20,
        4,
        profile.seed + 30_233,
        0.51,
    );
    let terrain_x = world_x + world_z * 0.31 + (warp_a - 0.5) * 4.4;
    let terrain_z = world_z - world_x * 0.24 + (warp_b - 0.5) * 3.8;

    let broad = fbm_periodic(
        terrain_x * 0.072,
        terrain_z * 0.078,
        12,
        5,
        profile.seed + 30_311,
        0.55,
    );
    let mesas = fbm_periodic(
        terrain_x * 0.170 + broad * 0.18,
        terrain_z * 0.155 - broad * 0.12,
        26,
        4,
        profile.seed + 30_401,
        0.50,
    );
    let gravel = fbm_periodic(
        terrain_x * 0.34 + mesas * 0.10,
        terrain_z * 0.31 - broad * 0.08,
        44,
        3,
        profile.seed + 30_509,
        0.47,
    );
    let fine = fbm_periodic(
        terrain_x * 0.82 + gravel * 0.055,
        terrain_z * 0.74 - mesas * 0.040,
        70,
        2,
        profile.seed + 30_607,
        0.44,
    );
    let raw_fracture = smoothstep(
        if volcanic_world { 0.66 } else { 0.78 },
        0.992,
        ridge(fbm_periodic(
            terrain_x * 0.30 + broad * 0.10,
            terrain_z * 0.26 - gravel * 0.08,
            38,
            3,
            profile.seed + 30_709,
            0.46,
        )),
    );
    let fracture = raw_fracture * if volcanic_world { 1.0 } else { 0.42 };
    let raw_crater = smoothstep(
        0.55,
        0.96,
        ridge(mesas * 0.62 + gravel * 0.24 + fine * 0.14),
    );
    let crater = raw_crater * if volcanic_world { 0.48 } else { 0.115 };
    let highland = smoothstep(0.44, 0.88, broad * 0.58 + mesas * 0.42);
    let raw_valley = smoothstep(
        if volcanic_world { 0.52 } else { 0.68 },
        if volcanic_world { 0.96 } else { 0.985 },
        ridge(broad * 0.46 + mesas * 0.34 + fracture * 0.20),
    );
    let valley = raw_valley * if volcanic_world { 1.0 } else { 0.32 };
    let escarpment = smoothstep(
        if volcanic_world { 0.10 } else { 0.26 },
        if volcanic_world { 0.42 } else { 0.62 },
        (mesas - broad).abs(),
    ) * smoothstep(0.08, 0.86, ground_v)
        * if volcanic_world { 1.0 } else { 0.52 };
    let patch_a = fbm_periodic(
        terrain_x * 0.046 + warp_a * 0.12,
        terrain_z * 0.041 - warp_b * 0.10,
        9,
        5,
        profile.seed + 32_103,
        0.58,
    );
    let patch_b = fbm_periodic(
        terrain_x * 0.104 + patch_a * 0.14,
        terrain_z * 0.092 - patch_a * 0.11,
        17,
        4,
        profile.seed + 32_207,
        0.53,
    );
    let patch_c = fbm_periodic(
        terrain_x * 0.205 + patch_b * 0.08,
        terrain_z * 0.176 - patch_a * 0.06,
        31,
        3,
        profile.seed + 32_311,
        0.48,
    );
    let patch_altitude = clamp01(patch_a * 0.44 + patch_b * 0.26 + broad * 0.18 + highland * 0.12);
    let patch_basin = smoothstep(0.35, 0.07, patch_a) * (0.70 + patch_c * 0.30);
    let relief = clamp01(highland * 0.56 + fracture * 0.30 + crater * 0.32 + gravel * 0.16);

    let mut base = if volcanic_world {
        basalt
            .lerp(basalt_lit, gravel * 0.20 + highland * 0.30)
            .lerp(ash.lerp(ash_lit, broad), 0.34 + rocky_material.ash * 0.34)
            .lerp(
                sulfur,
                rocky_material.sulfur * smoothstep(0.44, 0.92, broad) * 0.38,
            )
            .lerp(basalt * 0.62, valley * 0.26 + escarpment * 0.18)
            .lerp(basalt * 0.44, crater * 0.24 + fracture * 0.18)
    } else {
        regolith
            .lerp(pale_regolith, broad * 0.46 + highland * 0.30)
            .lerp(basalt_lit, rocky_material.basalt * (0.20 + crater * 0.24))
            .lerp(oxide, rocky_material.oxide * (0.28 + gravel * 0.12))
            .lerp(regolith * 0.62, valley * 0.12 + escarpment * 0.08)
    };

    let lava = if volcanic_world {
        smoothstep(
            0.76,
            0.992,
            fracture * 0.52 + ridge(gravel) * 0.30 + ridge(fine) * 0.18,
        ) * rocky_material.lava
            * smoothstep(0.10, 0.86, ground_v)
    } else {
        0.0
    };
    let texture = 0.88
        + (broad - 0.5) * 0.24
        + (mesas - 0.5) * 0.18
        + (gravel - 0.5) * 0.035
        + (fine - 0.5) * 0.010
        - valley * if volcanic_world { 0.18 } else { 0.050 }
        - escarpment * if volcanic_world { 0.14 } else { 0.040 }
        - crater * if volcanic_world { 0.10 } else { 0.050 }
        - fracture * if volcanic_world { 0.08 } else { 0.030 };
    base = base * texture + Vec3::new(2.2, 0.50, 0.065) * lava * (0.26 + volcanic_air * 0.18);
    if !volcanic_world {
        let mineral_patch = smoothstep(0.28, 0.88, patch_a * 0.64 + patch_b * 0.36);
        let iron_palette = matches!(rocky_palette.kind, RockyPaletteKind::IronOxide);
        let basalt_shadow = clamp01(
            rocky_material.basalt * (0.14 + patch_basin * 0.16) + valley * 0.040 + crater * 0.028,
        );
        let oxide_mix = clamp01(
            rocky_material.oxide * (0.20 + mineral_patch * 0.34)
                + if iron_palette {
                    0.16 + mineral_patch * 0.24
                } else {
                    0.0
                },
        );
        let smooth_landform = rocky_palette
            .mid
            .lerp(
                rocky_palette.high,
                patch_altitude * if iron_palette { 0.40 } else { 0.62 },
            )
            .lerp(rocky_palette.low, patch_basin * 0.22 + basalt_shadow)
            .lerp(rocky_palette.mineral, oxide_mix)
            .lerp(rocky_palette.shadow, crater_like_patch(patch_b) * 0.045);
        let smooth_texture = 0.88
            + (patch_a - 0.5) * 0.17
            + (patch_b - 0.5) * 0.10
            + (patch_c - 0.5) * 0.035
            + (broad - 0.5) * 0.050
            - patch_basin * 0.034
            - valley * 0.018
            - crater * 0.012;
        base = smooth_landform * smooth_texture;
    }

    let eps = 0.030 + perspective * 0.004;
    let h = if volcanic_world {
        broad * 0.45 + mesas * 0.30 + gravel * 0.14 - crater * 0.14 + fracture * 0.08
            - valley * 0.10
            + escarpment * 0.05
    } else {
        patch_a * 0.50 + patch_b * 0.28 + patch_c * 0.10 + broad * 0.08 + highland * 0.06
            - patch_basin * 0.045
            - crater * 0.020
            - valley * 0.018
    };
    let (hx, hz) = if volcanic_world {
        (
            fbm_periodic(
                (terrain_x + eps) * 0.170,
                terrain_z * 0.155,
                26,
                4,
                profile.seed + 30_401,
                0.50,
            ) - fbm_periodic(
                (terrain_x - eps) * 0.170,
                terrain_z * 0.155,
                26,
                4,
                profile.seed + 30_401,
                0.50,
            ),
            fbm_periodic(
                terrain_x * 0.170,
                (terrain_z + eps) * 0.155,
                26,
                4,
                profile.seed + 30_401,
                0.50,
            ) - fbm_periodic(
                terrain_x * 0.170,
                (terrain_z - eps) * 0.155,
                26,
                4,
                profile.seed + 30_401,
                0.50,
            ),
        )
    } else {
        let patch_x_right = fbm_periodic(
            (terrain_x + eps) * 0.104 + patch_a * 0.14,
            terrain_z * 0.092 - patch_a * 0.11,
            17,
            4,
            profile.seed + 32_207,
            0.53,
        );
        let patch_x_left = fbm_periodic(
            (terrain_x - eps) * 0.104 + patch_a * 0.14,
            terrain_z * 0.092 - patch_a * 0.11,
            17,
            4,
            profile.seed + 32_207,
            0.53,
        );
        let patch_z_down = fbm_periodic(
            terrain_x * 0.104 + patch_a * 0.14,
            (terrain_z + eps) * 0.092 - patch_a * 0.11,
            17,
            4,
            profile.seed + 32_207,
            0.53,
        );
        let patch_z_up = fbm_periodic(
            terrain_x * 0.104 + patch_a * 0.14,
            (terrain_z - eps) * 0.092 - patch_a * 0.11,
            17,
            4,
            profile.seed + 32_207,
            0.53,
        );
        (
            (patch_x_right - patch_x_left) * 0.76 + (patch_b - patch_a) * 0.018,
            (patch_z_down - patch_z_up) * 0.76 + (patch_c - patch_b) * 0.014,
        )
    };
    let normal = if volcanic_world {
        Vec3::new(-hx * 6.8, 1.0, -hz * 5.4).normalize()
    } else {
        Vec3::new(-hx * 2.6, 1.0, -hz * 2.1).normalize()
    };
    let ndotl = normal.dot(light_dir).max(0.0);
    let ao = if volcanic_world {
        clamp01(
            crater * 0.16
                + fracture * 0.12
                + valley * 0.12
                + escarpment * 0.08
                + relief * 0.030
                + (1.0 - h) * 0.035,
        )
    } else {
        let patch_shadow =
            smoothstep(0.36, 0.08, patch_a) * 0.020 + crater_like_patch(patch_b) * 0.018;
        clamp01(
            crater * 0.014
                + fracture * 0.010
                + valley * 0.012
                + escarpment * 0.012
                + relief * 0.016
                + patch_shadow
                + (1.0 - h) * 0.014,
        )
    };
    let mut color =
        base * if night_view {
            0.050 + ndotl * 0.36
        } else {
            0.24 + ndotl * 0.92
        } * (1.0 - ao);
    color += if night_view {
        Vec3::new(0.34, 0.46, 1.0)
    } else {
        Vec3::new(1.0, 0.78, 0.50)
    } * ndotl
        * if night_view { 0.020 } else { 0.035 };
    color += Vec3::new(3.0, 0.76, 0.11)
        * lava
        * if night_view {
            0.42 + (1.0 - ndotl) * 0.50
        } else {
            0.18 + (1.0 - ndotl) * 0.24
        };

    if monolith_mask > 0.004 {
        let monolith_color = if volcanic_world {
            basalt
                .lerp(ash, 0.30 + rocky_material.ash * 0.24)
                .lerp(sulfur, rocky_material.sulfur * 0.18)
        } else {
            basalt_lit
                .lerp(regolith, 0.34)
                .lerp(oxide, rocky_material.oxide * 0.16)
        } * if night_view {
            0.12 + monolith_light * 0.22
        } else {
            0.36 + monolith_light * 0.46
        };
        color = color.lerp(monolith_color, monolith_mask * 0.92);
        color += Vec3::new(1.7, 0.40, 0.055)
            * lava
            * monolith_mask
            * if volcanic_world { 0.38 } else { 0.0 };
    }

    let far_haze = smoothstep(0.24, 0.0, ground_v) * (0.18 + profile.atmosphere_density * 0.12);
    let heat_haze = volcanic_air * smoothstep(0.34, 0.04, ground_v) * 0.14;
    let dust_field = fbm_periodic(
        fx * 1.35 + ground_v * 0.18,
        fy * 1.9 - sx * 0.10,
        617,
        4,
        profile.seed + 31_307,
        0.54,
    );
    let dust = smoothstep(0.44, 0.0, ground_v)
        * (0.12
            + dust_field * 0.16
            + if volcanic_world {
                volcanic_air * 0.10
            } else if matches!(rocky_palette.kind, RockyPaletteKind::DustyRegolith) {
                rocky_air * 0.12
            } else {
                rocky_air * 0.055
            });
    color = apply_optical_medium(
        color,
        OpticalMedium::dust(
            clamp01(dust + volcanic_air * 0.18 + rocky_air * 0.075),
            volcanic_world,
        ),
        0.42 + (1.0 - ground_v) * 1.15,
        0.28 + monolith_light * 0.18,
        0.70 + rocky_material.barren * 0.22,
    );
    let haze = if night_view {
        Vec3::new(0.014, 0.030, 0.080)
            .lerp(Vec3::new(0.060, 0.052, 0.060), volcanic_air * 0.42)
            .lerp(Vec3::new(0.040, 0.052, 0.090), rocky_air * 0.22)
    } else {
        haze
    };
    color = color.lerp(
        haze,
        clamp01(
            far_haze * if night_view { 0.46 } else { 0.70 }
                + heat_haze * if night_view { 0.46 } else { 1.0 }
                + dust * if night_view { 0.28 } else { 0.55 }
                + rocky_air * far_haze * if night_view { 0.14 } else { 0.24 },
        ),
    );
    let vignette = smoothstep(0.98, 0.18, sx.abs()) * smoothstep(1.0, 0.42, ground_v);
    color = color * (0.88 + vignette * 0.16);

    if night_view {
        color = color.lerp(Vec3::new(0.006, 0.014, 0.038), 0.18);
        color += Vec3::new(0.035, 0.070, 0.170) * smoothstep(0.22, 0.0, ground_v) * 0.18;
    }

    rgba(tone_map(color * if night_view { 1.03 } else { 0.94 }), 255)
}

fn dry_world_monolith_mask(
    fx: f32,
    fy: f32,
    skyline: f32,
    seed: u64,
    volcanic_world: bool,
) -> (f32, f32) {
    let mut mask = 0.0_f32;
    let mut light = 0.0_f32;
    let count = if volcanic_world { 5 } else { 7 };

    for i in 0..count {
        let ii = i as i32;
        let cx = 0.08 + hash2(ii * 31 + 3, 17, seed + 31_901) * 0.84;
        let width = (0.010 + hash2(ii * 37 + 5, 29, seed + 31_903) * 0.020)
            * if volcanic_world { 0.82 } else { 1.08 };
        let height = (0.052 + hash2(ii * 41 + 7, 43, seed + 31_907) * 0.126)
            * if volcanic_world { 0.72 } else { 1.0 };
        let base = skyline + 0.012 + hash2(ii * 47 + 11, 53, seed + 31_911) * 0.070;
        let top = base - height;
        let y_t = clamp01((fy - top) / (base - top).max(0.001));
        let ragged = fbm_periodic(
            fx * 12.0 + ii as f32 * 0.37,
            fy * 9.0 - ii as f32 * 0.19,
            701 + i as i32,
            2,
            seed + 31_927,
            0.45,
        );
        let taper = width * (0.30 + y_t * 0.88) * (0.88 + (ragged - 0.5) * 0.22);
        let x_mask = smoothstep(taper, taper * 0.56, (fx - cx).abs());
        let y_mask =
            smoothstep(top - 0.006, top + 0.020, fy) * smoothstep(base + 0.020, base - 0.004, fy);
        let m = clamp01(x_mask * y_mask);
        if m > mask {
            mask = m;
        }
        light += m * (0.36 + hash2(ii * 59 + 13, 61, seed + 31_929) * 0.54);
    }

    if mask > 0.000_1 {
        (
            clamp01(mask),
            clamp01(light / (mask * count as f32).max(0.000_1)),
        )
    } else {
        (0.0, 0.0)
    }
}

fn select_overview_anchor(maps: &PlanetMaps) -> (f32, f32) {
    let mut best = (0.42, 0.52, f32::MIN);
    for y in (maps.height / 5..maps.height * 4 / 5).step_by(9) {
        for x in (0..maps.width).step_by(9) {
            let u = x as f32 / maps.width as f32;
            let v = y as f32 / maps.height as f32;
            let s = maps.sample(u, v);
            let east = maps.sample(u + 10.0 / maps.width as f32, v);
            let north = maps.sample(u, v - 10.0 / maps.height as f32);
            let coast = 1.0 - smoothstep(0.08, 0.46, (s.water - 0.50).abs());
            let cloud_penalty = s.cloud * 0.35;
            let elevation = smoothstep(0.38, 0.72, s.height);
            let texture = ((s.height - east.height).abs() + (s.height - north.height).abs()) * 10.0;
            let water_mix = (s.water - east.water).abs() + (s.water - north.water).abs();
            let score =
                coast * 1.45 + elevation * 0.24 + texture * 0.30 + water_mix * 0.42 - cloud_penalty;
            if score > best.2 {
                best = (u, v, score);
            }
        }
    }
    (best.0, best.1)
}

fn terrain_sky_ring_alpha(fx: f32, fy: f32, camera: TerrainOverviewCamera) -> f32 {
    let x = camera.screen_x(fx);
    let curve = 0.305 + x * x * 0.112 - x * 0.040;
    let width = 0.0048 + (1.0 - fx).abs() * 0.0035;
    let inner = 1.0 - smoothstep(width, width * 3.5, (fy - curve).abs());
    let outer = 1.0 - smoothstep(width * 1.4, width * 5.0, (fy - curve - 0.022).abs());
    let gap = 1.0 - smoothstep(0.006, 0.0, (fy - curve - 0.010).abs());
    let fine = 0.52 + 0.28 * (x * 71.0).sin() + 0.20 * (x * 167.0 + 0.7).sin();
    clamp01(
        (inner * 0.82 + outer * 0.38)
            * gap
            * fine
            * smoothstep(0.02, 0.18, fy)
            * (1.0 - smoothstep(0.48, 0.56, fy)),
    )
}

fn terrain_sky_moon(
    fx: f32,
    fy: f32,
    seed: u64,
    light_dir: Vec3,
    aspect: f32,
) -> Option<(Vec3, f32)> {
    let cx = 0.785;
    let cy = 0.185;
    let radius = 0.042;
    let dx = (fx - cx) * aspect.max(0.1) / radius;
    let dy = (fy - cy) / radius;
    let d2 = dx * dx + dy * dy;
    if d2 > 1.0 {
        return None;
    }

    let z = (1.0 - d2).sqrt();
    let n = Vec3::new(dx, -dy, z).normalize();
    let moon_light = n.dot(light_dir).max(0.0);
    let rough = fbm_periodic(dx * 0.5 + 0.5, dy * 0.5 + 0.5, 10, 4, seed + 20_777, 0.54);
    let crater = smoothstep(0.72, 0.93, rough) * 0.18;
    let maria = smoothstep(0.30, 0.64, rough) * 0.10;
    let limb = 1.0 - smoothstep(0.84, 1.0, d2.sqrt());
    let phase = smoothstep(-0.18, 0.30, n.dot(light_dir));
    let color = Vec3::splat(0.18 + moon_light * 0.78 + crater - maria)
        .lerp(Vec3::new(0.86, 0.84, 0.76), 0.18);

    Some((color, limb * phase * 0.92))
}

fn gas_giant_overview_pixel(
    x: u32,
    y: u32,
    size: RenderSize,
    anchor_u: f32,
    anchor_v: f32,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
) -> [u8; 4] {
    let fx = x as f32 / size.width.saturating_sub(1).max(1) as f32;
    let fy = y as f32 / size.height.saturating_sub(1).max(1) as f32;
    let sx = (fx - 0.5) * 2.0;
    let sy = (fy - 0.5) * 2.0;
    let u = anchor_u + sx * 0.18 + sy * 0.030 + fy * 0.18;
    let v = clamp(anchor_v + sy * 0.42 + sx * sx * 0.035, 0.02, 0.98);
    let sample = maps.sample(u, v);
    let material = gas_giant_material(profile);

    let eps_u = 0.75 / maps.width as f32;
    let eps_v = 0.75 / maps.height as f32;
    let right = maps.sample(u + eps_u, v).height;
    let left = maps.sample(u - eps_u, v).height;
    let up = maps.sample(u, v - eps_v).height;
    let down = maps.sample(u, v + eps_v).height;
    let cloud_n = Vec3::new((left - right) * 7.0, 1.0, (down - up) * 5.0).normalize();
    let light_dir = Vec3::new(-0.44, 0.62, 0.65).normalize();
    let view_dir = Vec3::new(-sx * 0.12, 0.30, 0.95).normalize();
    let ndotl = cloud_n.dot(light_dir).max(0.0);
    let half = (light_dir + view_dir).normalize();
    let spec = cloud_n.dot(half).max(0.0).powf(42.0) * 0.10;
    let limb_haze = smoothstep(0.15, 1.0, sx.abs()) * 0.10 + smoothstep(0.72, 1.0, fy) * 0.06;
    let band_contrast = 0.90 + (sample.height - 0.5) * 0.42;
    let mut color = sample.albedo * band_contrast * (0.38 + ndotl * 0.92);
    color += material.lightning.lerp(Vec3::new(1.0, 0.86, 0.62), 0.55)
        * spec
        * material.specular_strength;
    color = color.lerp(
        material.limb_haze,
        clamp01(limb_haze * profile.atmosphere_density),
    );

    rgba(tone_map(color * 0.98), 255)
}

fn shade_surface(
    n: Vec3,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    style: PlanetRenderStyle,
    edge_alpha: f32,
    lighting_mode: LightingMode,
) -> [u8; 4] {
    let (u, v) = normal_to_uv(rotate_planet(n));
    let sample = maps.sample(u, v);

    if matches!(style, PlanetRenderStyle::GasGiant) {
        return shade_gas_giant_surface(n, maps, profile, u, v, sample, edge_alpha, lighting_mode);
    }

    let night_view = lighting_mode.is_night();
    let light_dir = planet_light_dir(lighting_mode);
    let ocean_world = matches!(style, PlanetRenderStyle::OceanWorld);
    let rocky_material = rocky_surface_material(profile);
    let rocky_palette = rocky_surface_palette(profile, rocky_material);
    let rocky_world = matches!(style, PlanetRenderStyle::RockyWorld);
    let volcanic_world = matches!(style, PlanetRenderStyle::VolcanicWorld);
    let atmosphere_density = clamp01(
        profile.atmosphere_density
            + if rocky_world {
                rocky_palette.atmosphere_boost * 0.42
            } else {
                0.0
            },
    );
    let eps_u = 1.35 / maps.width as f32;
    let eps_v = 1.35 / maps.height as f32;
    let h_l = maps.sample(u - eps_u, v).height;
    let h_r = maps.sample(u + eps_u, v).height;
    let h_u = maps.sample(u, v - eps_v).height;
    let h_d = maps.sample(u, v + eps_v).height;

    let tangent = Vec3::Y.cross(n).normalize_or(Vec3::X);
    let bitangent = n.cross(tangent).normalize_or(Vec3::Y);
    let land_for_bump = if ocean_world {
        0.0
    } else {
        1.0 - smoothstep(0.34, 0.68, sample.water)
    };
    let bump = if sample.water > 0.5 {
        0.045
    } else {
        let base_land_bump = if rocky_world { 0.24 } else { 0.34 };
        base_land_bump
            + land_for_bump
                * (rocky_material.relief * 0.18
                    + rocky_material.volcanic * 0.06
                    + if rocky_world {
                        0.055 + rocky_palette.relief_boost * 0.085
                    } else {
                        0.0
                    }
                    + if volcanic_world { 0.065 } else { 0.0 })
    };
    let mut surf_n =
        (n + tangent * (h_l - h_r) * bump + bitangent * (h_d - h_u) * bump).normalize();
    let rocky_land = land_for_bump * rocky_material.rocky * (1.0 - sample.vegetation * 0.22);
    if rocky_land > 0.01 {
        let micro_step_u = 0.85 / maps.width as f32;
        let micro_step_v = 0.85 / maps.height as f32;
        let micro_right = terrain_micro_detail(u + micro_step_u, v, profile.seed);
        let micro_left = terrain_micro_detail(u - micro_step_u, v, profile.seed);
        let micro_down = terrain_micro_detail(u, v + micro_step_v, profile.seed);
        let micro_up = terrain_micro_detail(u, v - micro_step_v, profile.seed);
        let micro_strength = rocky_land
            * (0.060
                + rocky_material.relief * 0.110
                + rocky_material.volcanic * 0.052
                + sample.roughness * 0.040
                + if rocky_world {
                    0.030 + rocky_palette.relief_boost * 0.040
                } else {
                    0.0
                }
                + if volcanic_world { 0.028 } else { 0.0 });
        surf_n = (surf_n
            + tangent * (micro_left - micro_right) * micro_strength
            + bitangent * (micro_down - micro_up) * micro_strength)
            .normalize();
    }
    if rocky_world && sample.water < 0.45 && land_for_bump > 0.05 {
        let patch_step_u = 1.90 / maps.width as f32;
        let patch_step_v = 1.90 / maps.height as f32;
        let patch_right = rocky_patch_height(u + patch_step_u, v, profile.seed);
        let patch_left = rocky_patch_height(u - patch_step_u, v, profile.seed);
        let patch_down = rocky_patch_height(u, v + patch_step_v, profile.seed);
        let patch_up = rocky_patch_height(u, v - patch_step_v, profile.seed);
        let patch_strength =
            land_for_bump * (0.075 + rocky_palette.relief_boost * 0.095 + sample.roughness * 0.018);
        surf_n = (surf_n
            + tangent * (patch_left - patch_right) * patch_strength
            + bitangent * (patch_down - patch_up) * patch_strength)
            .normalize();
    }
    if sample.water > 0.08 {
        let wave_breakup = fbm_periodic(
            u * 12.4 + sample.height * 0.10,
            v * 8.7 - sample.water * 0.07,
            101,
            2,
            profile.seed + 25_337,
            0.44,
        );
        let wave = ocean_wave_tangent_perturb(
            u + (wave_breakup - 0.5) * 0.0018,
            v + (ridge(wave_breakup) - 0.5) * 0.0012,
            profile.seed,
        ) * smoothstep(0.08, 0.72, sample.water)
            * (0.78 + ridge(wave_breakup) * 0.34)
            * (1.0 - sample.biome * 0.28);
        surf_n = (surf_n + tangent * wave.x + bitangent * wave.y).normalize();
    }

    let raw_light = n.dot(light_dir);
    let ndotl = surf_n.dot(light_dir).max(0.0);
    let view_dot = n.dot(VIEW).max(0.0);
    let half = (light_dir + VIEW).normalize();

    let roughness = if sample.water > 0.5 {
        clamp(0.045 + sample.roughness * 0.70, 0.055, 0.24)
    } else {
        clamp(
            0.45 + sample.roughness * (0.32 + rocky_material.ash * land_for_bump * 0.08)
                + rocky_material.barren * land_for_bump * 0.04
                + if rocky_world {
                    land_for_bump * 0.025
                } else {
                    0.0
                }
                + if volcanic_world {
                    land_for_bump * (0.030 + rocky_material.ash * 0.030)
                } else {
                    0.0
                }
                - rocky_material.lava * land_for_bump * 0.025,
            0.46,
            0.90,
        )
    };
    let oren = 1.0 - roughness * 0.24 + roughness * 0.12 * (1.0 - view_dot);
    let land_occlusion = land_for_bump * sample.ambient_occlusion;
    let sky_visibility = 1.0 - land_for_bump * sample.horizon_occlusion;
    let ambient = (if night_view {
        0.010 + atmosphere_density * 0.022
    } else {
        0.020 + atmosphere_density * 0.045
    }) * (0.58 + sky_visibility * 0.42);
    let terminator = smoothstep(-0.18, 0.10, raw_light);
    let cloud_altitude = clamp01(
        0.34 + atmosphere_density * 0.18
            + sample.cloud * 0.18
            + smoothstep(0.32, 0.82, sample.water) * 0.06
            + (1.0 - view_dot) * 0.08,
    );

    let shadow_near = maps
        .sample(
            u - light_dir.x * (0.022 + cloud_altitude * 0.018),
            v + light_dir.y * (0.017 + cloud_altitude * 0.014),
        )
        .cloud;
    let shadow_far = maps
        .sample(
            u - light_dir.x * (0.048 + cloud_altitude * 0.030),
            v + light_dir.y * (0.034 + cloud_altitude * 0.024),
        )
        .cloud;
    let shadow = (shadow_near * 0.68 + shadow_far * 0.32)
        * smoothstep(0.02, 0.55, sample.water)
        * terminator
        * (0.20 + (shadow_near - shadow_far).abs() * 0.24 + cloud_altitude * 0.055)
        * if ocean_world {
            1.0 + sample.water * 0.10
        } else {
            1.0
        };

    let mut color = sample.albedo
        * (ambient + ndotl * oren * (1.0 - shadow))
        * (1.0 - land_occlusion * (0.30 + roughness * 0.12));

    if ocean_world && sample.water > 0.35 {
        let depth_cues = ocean_depth_cues(u, v, profile.seed);
        let ocean_regions = ocean_regional_cues(u, v, profile.seed);
        let depth =
            clamp01(sample.water * ((0.66 - sample.height) * 1.95 + depth_cues.trench * 0.22));
        let color_depth = clamp01(
            depth * (0.88 + ocean_regions.depth_patch * 0.17)
                + (depth_cues.micro - 0.5) * 0.060
                + ocean_regions.current_edge * 0.040
                - smoothstep(0.58, 0.94, ocean_regions.warm) * (1.0 - depth) * 0.036,
        );
        let mid_layer = smoothstep(0.12, 0.62, color_depth)
            * (1.0 - smoothstep(0.76, 0.99, color_depth))
            * (0.38 + depth_cues.basin * 0.20 + ocean_regions.gyre * 0.12);
        let shallow_window = sample.water
            * (1.0 - color_depth)
            * smoothstep(0.28, 0.68, sample.height + ocean_regions.reef * 0.035)
            * (0.32 + depth_cues.shelf * 0.26 + ocean_regions.reef * 0.18);
        let shelf_light = smoothstep(0.34, 0.64, sample.height + ocean_regions.sediment * 0.025)
            * sample.water
            * (0.14 + depth_cues.ridge * 0.16 + ocean_regions.current_edge * 0.10);
        let current_front = smoothstep(
            0.44,
            0.92,
            ocean_regions.current_edge * 0.64
                + (ocean_regions.current - 0.5).abs() * 0.26
                + ocean_regions.depth_patch * 0.16
                + ridge(depth_cues.micro) * 0.08,
        ) * sample.water;
        let warm_front = current_front * smoothstep(0.46, 0.94, ocean_regions.warm);
        let cool_front = current_front * (1.0 - smoothstep(0.38, 0.84, ocean_regions.warm));
        let caustic = ridge(depth_cues.micro)
            * shallow_window
            * smoothstep(-0.04, 0.36, raw_light)
            * (0.18 + depth_cues.shelf * 0.18);
        color = color.lerp(
            Vec3::new(0.003, 0.022, 0.105),
            color_depth * (0.13 + depth_cues.trench * 0.10 + ocean_regions.depth_patch * 0.060),
        );
        color = color.lerp(
            Vec3::new(0.018, 0.110, 0.245).lerp(Vec3::new(0.040, 0.170, 0.230), ocean_regions.warm),
            mid_layer * 0.24,
        );
        color = color.lerp(
            Vec3::new(0.008, 0.060, 0.130).lerp(Vec3::new(0.020, 0.150, 0.205), ocean_regions.warm),
            ocean_regions.current_edge * sample.water * if night_view { 0.13 } else { 0.12 },
        );
        color = color.lerp(
            Vec3::new(0.010, 0.105, 0.215),
            cool_front * if night_view { 0.20 } else { 0.16 },
        );
        color = color.lerp(
            Vec3::new(0.030, 0.175, 0.230),
            warm_front * if night_view { 0.16 } else { 0.18 },
        );
        color += Vec3::new(0.012, 0.055, 0.085)
            * current_front
            * smoothstep(0.12, 0.86, view_dot)
            * if night_view { 0.045 } else { 0.075 };
        color += Vec3::new(0.018, 0.080, 0.095) * shelf_light * smoothstep(-0.04, 0.32, raw_light);
        color += Vec3::new(0.040, 0.175, 0.170) * caustic;
        color = apply_optical_medium(
            color,
            OpticalMedium::ocean(color_depth),
            0.22 + (1.0 - view_dot) * 0.62,
            raw_light.max(0.0),
            roughness,
        );
        let cloud_mirror = smoothstep(0.05, 0.46, shadow_near * 0.58 + shadow_far * 0.42)
            * sample.water
            * (schlick_fresnel(view_dot, 0.023) * 0.32 + (1.0 - view_dot) * 0.08)
            * (0.70 + cloud_altitude * 0.34);
        color = color * (1.0 - cloud_mirror * if night_view { 0.030 } else { 0.052 });
        color = color.lerp(
            if night_view {
                Vec3::new(0.032, 0.080, 0.190)
            } else {
                Vec3::new(0.34, 0.55, 0.62)
            },
            cloud_mirror * if night_view { 0.16 } else { 0.22 },
        );
    } else if !ocean_world && sample.water < 0.45 && land_for_bump > 0.05 {
        let dust_density = clamp01(
            rocky_material.barren * 0.18
                + rocky_material.ash * 0.24
                + rocky_material.volcanic * 0.10
                + if rocky_world {
                    rocky_palette.atmosphere_boost * 0.35
                } else {
                    0.0
                }
                + (1.0 - view_dot) * 0.16,
        );
        if dust_density > 0.025 {
            color = apply_optical_medium(
                color,
                OpticalMedium::dust(dust_density, volcanic_world),
                0.16 + (1.0 - view_dot) * 0.52,
                raw_light.max(0.0),
                roughness,
            );
        }
    }

    if sample.water > 0.35 && raw_light > -0.05 {
        let ndoth = surf_n.dot(half).max(0.0);
        let fresnel = schlick_fresnel(view_dot, 0.023);
        let wave_glint = ocean_wave_glint_mask(u, v, profile.seed);
        let glint_breakup = fbm_periodic(
            u * 18.0 + wave_glint * 0.12,
            v * 13.0 - sample.height * 0.08,
            149,
            2,
            profile.seed + 25_911,
            0.43,
        );
        let glint_visibility = wave_glint
            * smoothstep(0.16, 0.72, view_dot)
            * smoothstep(-0.02, 0.22, raw_light)
            * (0.46 + ridge(glint_breakup) * 0.54);
        let glitter = ndoth.powf(92.0) * ndotl * sample.water * (0.34 + glint_visibility * 0.58);
        let broad = ndoth.powf(28.0)
            * ndotl
            * sample.water
            * (0.052 + glint_visibility * 0.085 + fresnel * 0.05);
        let reflected = (surf_n * (2.0 * surf_n.dot(VIEW)) - VIEW).normalize();
        let scene_light = distant_light_for_mode(lighting_mode);
        let reflection = sample_environment(reflected, profile.seed, scene_light);
        let glint_color = if night_view {
            Vec3::new(0.52, 0.66, 1.00)
        } else {
            Vec3::new(0.72, 0.88, 1.00)
        };
        let reflection_strength = if night_view { 0.15 } else { 0.34 };
        color += glint_color * (glitter * (0.42 + fresnel * 1.35) + broad);
        color += reflection
            * fresnel
            * sample.water
            * smoothstep(-0.02, 0.30, raw_light)
            * reflection_strength;
        color = color.lerp(
            if night_view {
                Vec3::new(0.035, 0.095, 0.22)
            } else {
                Vec3::new(0.10, 0.30, 0.52)
            },
            fresnel * sample.water * if night_view { 0.42 } else { 0.32 },
        );
        color += Vec3::new(0.32, 0.62, 0.66)
            * sample.water
            * ridge(glint_breakup)
            * ocean_wave_spectrum(u, v, profile.seed + 17_909).foam
            * smoothstep(0.08, 0.56, view_dot)
            * if night_view { 0.012 } else { 0.020 };
    }

    let night = if night_view {
        0.92 + (1.0 - ndotl) * 0.08
    } else {
        1.0 - smoothstep(-0.12, 0.09, raw_light)
    };
    if !ocean_world && sample.water < 0.45 && rocky_material.lava > 0.02 {
        let hot_albedo = clamp01(
            (sample.albedo.x - sample.albedo.z * 1.65) * 1.45
                + (sample.albedo.y - sample.albedo.z) * 0.62,
        );
        let lava_emit = hot_albedo
            * rocky_material.lava
            * land_for_bump
            * smoothstep(0.18, 0.70, profile.volcanic_activity);
        color += Vec3::new(3.2, 0.82, 0.16) * lava_emit * (0.10 + night * 0.65 + ndotl * 0.05);
    }
    if sample.city > 0.02 {
        let city_scatter = if night_view { 1.25 } else { 1.0 };
        color += Vec3::new(4.2, 2.2, 0.85) * sample.city * night * city_scatter;
        color += Vec3::new(0.55, 0.38, 0.18)
            * sample.city
            * sample.cloud
            * night
            * if night_view { 0.80 } else { 0.26 };
    }

    if sample.cloud > 0.015 {
        let cloud_east = maps.sample(u + eps_u * 1.2, v).cloud;
        let cloud_west = maps.sample(u - eps_u * 1.2, v).cloud;
        let cloud_north = maps.sample(u, v - eps_v * 1.2).cloud;
        let cloud_south = maps.sample(u, v + eps_v * 1.2).cloud;
        let cloud_center = (sample.cloud * 0.32
            + cloud_east * 0.11
            + cloud_west * 0.11
            + cloud_south * 0.11
            + cloud_north * 0.11
            + maps.sample(u + eps_u * 1.2, v + eps_v * 1.2).cloud * 0.060
            + maps.sample(u - eps_u * 1.2, v + eps_v * 1.2).cloud * 0.060
            + maps.sample(u + eps_u * 1.2, v - eps_v * 1.2).cloud * 0.060
            + maps.sample(u - eps_u * 1.2, v - eps_v * 1.2).cloud * 0.060)
            .min(1.0);
        let cloud_ndotl = smoothstep(-0.08, 0.25, raw_light);
        let cloud_edge = ((cloud_east - cloud_west).abs()
            + (cloud_south - cloud_north).abs()
            + ridge(cloud_center) * 0.08)
            .min(0.75);
        let cloud_n = (n
            + tangent * (cloud_west - cloud_east) * (0.15 + cloud_altitude * 0.050)
            + bitangent * (cloud_south - cloud_north) * (0.11 + cloud_altitude * 0.040))
            .normalize();
        let relief_light = cloud_n.dot(light_dir).max(0.0);
        let cloud_alpha = cloud_center
            * cloud_ndotl
            * (0.42 + cloud_edge * 0.16 + cloud_altitude * 0.055)
            * if ocean_world { 1.10 } else { 1.0 };
        let cloud_self_shadow = smoothstep(0.08, 0.36, cloud_center)
            * (1.0 - relief_light)
            * (0.13 + cloud_altitude * 0.08);
        let cloud_base = if night_view {
            Vec3::new(0.16, 0.20, 0.34).lerp(
                Vec3::new(0.35, 0.42, 0.62),
                smoothstep(0.06, 0.38, cloud_center),
            )
        } else {
            Vec3::new(0.80, 0.83, 0.80)
                .lerp(
                    Vec3::new(1.0, 1.0, 0.96),
                    smoothstep(0.06, 0.38, cloud_center),
                )
                .lerp(
                    Vec3::new(0.70, 0.84, 0.90),
                    if ocean_world {
                        sample.water * 0.08
                    } else {
                        0.0
                    },
                )
        };
        let cloud_color = cloud_base
            * (if night_view { 0.18 } else { 0.28 }
                + relief_light * if night_view { 0.48 } else { 0.86 }
                + ndotl * if night_view { 0.12 } else { 0.22 }
                + cloud_edge * 0.09
                + cloud_altitude * if night_view { 0.018 } else { 0.038 }
                - shadow * 0.14
                - cloud_self_shadow);
        if ocean_world && sample.water > 0.20 {
            let marine_gap = cloud_center
                * sample.water
                * (1.0 - view_dot).powf(1.4)
                * (0.030 + cloud_altitude * 0.055);
            color = color.lerp(
                if night_view {
                    Vec3::new(0.040, 0.080, 0.180)
                } else {
                    Vec3::new(0.40, 0.62, 0.72)
                },
                marine_gap,
            );
        }
        color = color.lerp(cloud_color, cloud_alpha);
        color += if night_view {
            Vec3::new(0.42, 0.54, 1.0)
        } else {
            Vec3::new(1.0, 0.86, 0.62)
        } * cloud_edge
            * relief_light
            * cloud_alpha
            * smoothstep(-0.02, 0.34, raw_light)
            * if night_view { 0.026 } else { 0.040 };
        if ocean_world {
            color += Vec3::new(0.76, 0.88, 1.0)
                * cloud_edge
                * cloud_alpha
                * smoothstep(0.14, 0.84, view_dot)
                * if night_view { 0.018 } else { 0.030 };
        }
    }

    let limb = (1.0 - view_dot).powf(1.90);
    let optical_depth = atmosphere_optical_depth(view_dot, atmosphere_density);
    let refractive_limb = smoothstep(0.42, 1.20, optical_depth)
        * limb
        * (0.46 + atmosphere_density * 0.38)
        * (0.84 + sample.water * if ocean_world { 0.22 } else { 0.0 });
    color = apply_optical_medium(
        color,
        OpticalMedium::atmosphere(atmosphere_density),
        optical_depth * (0.48 + refractive_limb * 0.18),
        raw_light.max(0.0),
        roughness,
    );
    let day_air = smoothstep(-0.28, 0.24, raw_light) * if night_view { 0.34 } else { 1.0 };
    let sunset = if night_view {
        0.0
    } else {
        smoothstep(-0.10, 0.12, raw_light) * (1.0 - smoothstep(0.14, 0.42, raw_light))
    };
    let horizon_air = smoothstep(0.38, 1.26, optical_depth) * limb;
    let rayleigh = Vec3::new(0.10, 0.34, 1.00) * limb * day_air * optical_depth * 0.30;
    let mie = Vec3::new(1.00, 0.48, 0.18) * limb * sunset * optical_depth * 0.48;
    let lower_haze = refractive_limb * day_air * (0.035 + cloud_altitude * sample.cloud * 0.018);
    color = color.lerp(
        Vec3::new(0.11, 0.29, 0.62).lerp(Vec3::new(0.18, 0.42, 0.68), sample.water * 0.16),
        horizon_air * day_air * 0.070 + lower_haze,
    );
    color += rayleigh
        + mie
        + Vec3::new(0.70, 0.84, 1.0) * horizon_air * 0.018
        + Vec3::new(0.46, 0.74, 0.92) * refractive_limb * day_air * 0.020;
    if ocean_world {
        let marine_limb = sample.water
            * day_air
            * horizon_air
            * (0.050
                + smoothstep(0.70, 1.0, profile.ocean_fraction) * 0.060
                + refractive_limb * 0.030);
        color = color.lerp(Vec3::new(0.24, 0.47, 0.68), marine_limb);
        color += Vec3::new(0.46, 0.76, 0.92) * marine_limb * 0.060;
    }

    if night_view {
        let moon_lift = smoothstep(0.18, 0.92, ndotl) * (0.08 + atmosphere_density * 0.035);
        color = color * (0.64 + moon_lift);
        color = color.lerp(
            Vec3::new(0.006, 0.018, 0.052).lerp(Vec3::new(0.030, 0.060, 0.150), view_dot),
            0.20 + (1.0 - view_dot) * 0.16,
        );
        color += Vec3::new(0.050, 0.120, 0.310)
            * limb
            * optical_depth
            * (0.055 + atmosphere_density * 0.060);
    }

    let exposure = if night_view { 1.08 } else { 0.94 };
    color = apply_anti_band_grain(
        color,
        u + n.x * 0.037,
        v + n.y * 0.029,
        profile.seed + 25_019,
        if night_view { 0.0038 } else { 0.0028 } + refractive_limb * 0.0018,
    );
    rgba(tone_map(color * exposure), (255.0 * edge_alpha) as u8)
}

fn shade_gas_giant_surface(
    n: Vec3,
    maps: &PlanetMaps,
    profile: &PlanetVisualProfile,
    u: f32,
    v: f32,
    sample: SurfaceSample,
    edge_alpha: f32,
    lighting_mode: LightingMode,
) -> [u8; 4] {
    let night_view = lighting_mode.is_night();
    let material = gas_giant_material(profile);
    let light_dir = planet_light_dir(lighting_mode);
    let eps_u = 1.10 / maps.width as f32;
    let eps_v = 1.10 / maps.height as f32;
    let h_l = maps.sample(u - eps_u, v).height;
    let h_r = maps.sample(u + eps_u, v).height;
    let h_u = maps.sample(u, v - eps_v).height;
    let h_d = maps.sample(u, v + eps_v).height;

    let tangent = Vec3::Y.cross(n).normalize_or(Vec3::X);
    let bitangent = n.cross(tangent).normalize_or(Vec3::Y);
    let cloud_n = (n + tangent * (h_l - h_r) * 0.13 + bitangent * (h_d - h_u) * 0.08).normalize();
    let raw_light = n.dot(light_dir);
    let ndotl = cloud_n.dot(light_dir).max(0.0);
    let view_dot = n.dot(VIEW).max(0.0);
    let half = (light_dir + VIEW).normalize();
    let terminator = smoothstep(-0.24, 0.10, raw_light);
    let ambient = if night_view {
        0.018 + profile.atmosphere_density * 0.040
    } else {
        0.052 + profile.atmosphere_density * 0.090
    };
    let band_shadow = smoothstep(
        0.018,
        0.080,
        maps.sample(u, v + eps_v * 1.8).height - sample.height,
    ) * 0.16;
    let mut color = sample.albedo * (ambient + ndotl * 1.18 * terminator) * (1.0 - band_shadow);

    let cloud_spec =
        cloud_n.dot(half).max(0.0).powf(58.0) * ndotl * 0.13 * material.specular_strength;
    color += if night_view {
        Vec3::new(0.44, 0.58, 1.0)
    } else {
        material.lightning.lerp(Vec3::new(1.0, 0.88, 0.62), 0.58)
    } * cloud_spec
        * if night_view { 0.72 } else { 1.0 };

    if sample.cloud > 0.08 {
        let plume = material.plume * (0.30 + ndotl * 0.84);
        color = color.lerp(plume, sample.cloud * 0.36);
    }

    let limb = (1.0 - view_dot).powf(1.48);
    let optical_depth = atmosphere_optical_depth(view_dot, profile.atmosphere_density);
    let day_air = smoothstep(-0.22, 0.26, raw_light) * if night_view { 0.34 } else { 1.0 };
    let sunset = if night_view {
        0.0
    } else {
        smoothstep(-0.08, 0.10, raw_light) * (1.0 - smoothstep(0.12, 0.38, raw_light))
    };
    color += material.limb_haze * limb * day_air * optical_depth * 0.18;
    color += material.limb_haze.lerp(Vec3::new(1.0, 0.38, 0.14), 0.52)
        * limb
        * sunset
        * optical_depth
        * 0.25;
    color += Vec3::new(0.62, 0.72, 1.0) * limb * day_air * optical_depth * 0.030;

    if material.thermal_glow > 0.0 && !night_view {
        let equator = 1.0 - smoothstep(0.20, 0.90, (v - 0.5).abs() * 2.0);
        color += Vec3::new(1.30, 0.22, 0.055)
            * equator
            * material.thermal_glow
            * smoothstep(-0.08, 0.46, raw_light)
            * 0.16;
    }

    if night_view {
        color = color.lerp(
            Vec3::new(0.035, 0.050, 0.120).lerp(Vec3::new(0.11, 0.13, 0.24), view_dot),
            0.28 + limb * 0.12,
        );
    }

    rgba(
        tone_map(color * if night_view { 1.05 } else { 0.96 }),
        (255.0 * edge_alpha) as u8,
    )
}

#[derive(Debug, Clone, Copy)]
struct OceanWaveSpectrum {
    swell: f32,
    chop: f32,
    ripple: f32,
    foam: f32,
    glint: f32,
    slope: Vec3,
}

#[derive(Debug, Clone, Copy)]
struct OceanDepthCues {
    basin: f32,
    ridge: f32,
    trench: f32,
    shelf: f32,
    turbidity: f32,
    micro: f32,
}

#[derive(Debug, Clone, Copy)]
struct OceanRegionalCues {
    gyre: f32,
    current: f32,
    current_edge: f32,
    warm: f32,
    sediment: f32,
    depth_patch: f32,
    reef: f32,
}

fn ocean_depth_cues(u: f32, v: f32, seed: u64) -> OceanDepthCues {
    let basin = fbm_tiled(u * 1.15 + 0.09, v * 0.92 - 0.04, 6, 5, seed + 24_601, 0.57);
    let folded = fbm_tiled(
        u * 2.7 + basin * 0.11,
        v * 1.8 - basin * 0.07,
        13,
        4,
        seed + 24_697,
        0.52,
    );
    let fracture = fbm_tiled(
        u * 3.8 + folded * 0.06,
        v * 2.6 - basin * 0.04,
        31,
        3,
        seed + 24_763,
        0.48,
    );
    let shelf = fbm_tiled(
        u * 5.2 + folded * 0.05,
        v * 4.3 - basin * 0.03,
        43,
        3,
        seed + 24_827,
        0.49,
    );
    let micro = fbm_tiled(
        u * 11.4 + fracture * 0.045,
        v * 8.6 - shelf * 0.035,
        97,
        2,
        seed + 24_911,
        0.45,
    );
    let ridge_angle = hash2(701, 881, seed) * PI * 2.0;
    let directed_ridge = curved_directional_band(
        u,
        v,
        seed + 25_037,
        ridge_angle,
        5.8 + hash2(727, 907, seed) * 2.4,
        0.24,
    );
    let directed_trench = curved_directional_band(
        u + basin * 0.020,
        v - folded * 0.016,
        seed + 25_139,
        ridge_angle + 0.76,
        7.4 + hash2(743, 929, seed) * 3.0,
        0.18,
    );
    let ridge_field = ridge(folded);
    let ridge_cue = clamp01(
        ridge_field * 0.52 + directed_ridge * 0.24 + ridge(fracture) * 0.16 + ridge(micro) * 0.08,
    );
    let trench = clamp01(
        smoothstep(
            0.62,
            0.96,
            ridge_field * 0.42
                + directed_trench * 0.22
                + (1.0 - fracture) * 0.24
                + (basin - 0.5) * 0.12,
        ) * (0.78 + ridge(micro) * 0.22),
    );
    let turbidity =
        clamp01(shelf * 0.46 + ridge(micro) * 0.16 + (1.0 - basin) * 0.12 + directed_trench * 0.06);

    OceanDepthCues {
        basin,
        ridge: ridge_cue,
        trench,
        shelf,
        turbidity,
        micro,
    }
}

fn ocean_regional_cues(u: f32, v: f32, seed: u64) -> OceanRegionalCues {
    let gyre = fbm_tiled(u * 0.62 + 0.17, v * 0.48 - 0.09, 5, 5, seed + 27_101, 0.59);
    let basin = fbm_tiled(
        u * 1.12 + gyre * 0.16,
        v * 0.86 - gyre * 0.11,
        9,
        5,
        seed + 27_211,
        0.55,
    );
    let current = fbm_tiled(
        u * 2.10 + basin * 0.18,
        v * 1.48 - gyre * 0.16,
        19,
        4,
        seed + 27_331,
        0.52,
    );
    let stream = fbm_tiled(
        u * 4.8 + current * 0.11,
        v * 3.2 - basin * 0.08,
        37,
        3,
        seed + 27_457,
        0.48,
    );
    let warm = fbm_tiled(
        u * 0.84 - gyre * 0.10,
        v * 0.70 + current * 0.10,
        13,
        4,
        seed + 27_563,
        0.54,
    );
    let sediment = fbm_tiled(
        u * 3.4 + stream * 0.08,
        v * 2.4 - gyre * 0.06,
        43,
        3,
        seed + 27_677,
        0.49,
    );
    let reef = fbm_tiled(
        u * 7.6 + sediment * 0.07,
        v * 5.4 - stream * 0.05,
        79,
        2,
        seed + 27_809,
        0.45,
    );

    OceanRegionalCues {
        gyre,
        current,
        current_edge: clamp01((current - stream).abs() * 1.9 + ridge(stream) * 0.16),
        warm,
        sediment,
        depth_patch: clamp01(basin * 0.55 + (1.0 - gyre) * 0.24 + stream * 0.21),
        reef,
    }
}

fn ocean_wave_spectrum(u: f32, v: f32, seed: u64) -> OceanWaveSpectrum {
    let lat = ((v - 0.5) * 2.0).abs();
    let polar_soften = 1.0 - smoothstep(0.72, 0.98, lat) * 0.45;
    let basin = fbm_tiled(u * 2.0 + 0.11, v + 0.03, 7, 4, seed + 12_901, 0.56);
    let eddy = fbm_tiled(
        u * 4.0 + basin * 0.10,
        v * 2.0 - basin * 0.08,
        17,
        3,
        seed + 12_977,
        0.52,
    );
    let shear = fbm_tiled(
        u * 8.0 - eddy * 0.08,
        v * 4.0 + basin * 0.12,
        29,
        2,
        seed + 13_019,
        0.48,
    );
    let regional_turn = (eddy - 0.5) * 0.44 + (basin - 0.5) * 0.24;
    let regional_fan = (shear - 0.5) * 0.34 + ridge(eddy) * 0.10;
    let local_energy = clamp(0.82 + shear * 0.28 + ridge(eddy) * 0.16, 0.72, 1.22);
    let uu = (u
        + ((basin - 0.5) * 0.044 + (eddy - 0.5) * 0.018 + (shear - 0.5) * 0.006) * polar_soften)
        .rem_euclid(1.0);
    let vv = clamp01(v + (eddy - 0.5) * 0.030 + (basin - 0.5) * 0.012 + (shear - 0.5) * 0.004);

    let turn = hash2(13, 29, seed) * PI * 2.0 + regional_turn;
    let fan = (hash2(7, 61, seed) - 0.5) * 0.42 + regional_fan;
    let (s0, c0, dx0, dy0) = directional_wave(
        uu,
        vv,
        turn + 0.18 + fan,
        6.4 + hash2(1, 7, seed) * 1.8 + basin * 0.70,
        hash2(1, 7, seed),
        0.68,
    );
    let (s1, c1, dx1, dy1) = directional_wave(
        uu,
        vv,
        turn + 1.54 - fan * 0.4,
        10.4 + hash2(11, 5, seed) * 2.7 + eddy * 1.20,
        hash2(11, 5, seed),
        0.54,
    );
    let (s2, c2, dx2, dy2) = directional_wave(
        uu,
        vv,
        turn - 1.34 + fan * 0.3,
        17.0 + hash2(17, 31, seed) * 3.2 + shear * 2.0,
        hash2(17, 31, seed),
        0.48,
    );
    let (s3, c3, dx3, dy3) = directional_wave(
        uu,
        vv,
        turn + 2.46 + fan * 0.2,
        29.0 + hash2(23, 3, seed) * 5.5 + ridge(eddy) * 3.0,
        hash2(23, 3, seed),
        0.38,
    );
    let (s4, c4, dx4, dy4) = directional_wave(
        uu,
        vv,
        turn - 2.18 - fan * 0.2,
        45.0 + hash2(37, 19, seed) * 8.0 + shear * 5.0,
        hash2(37, 19, seed),
        0.28,
    );
    let (s5, c5, dx5, dy5) = directional_wave(
        uu,
        vv,
        turn + 0.88 + fan * 0.5,
        67.0 + hash2(43, 53, seed) * 12.0 + ridge(shear) * 7.0,
        hash2(43, 53, seed),
        0.22,
    );

    let swell = clamp01(
        0.5 + (s0 * 0.42 + s1 * 0.30 + s2 * 0.16 + (basin - 0.5) * 0.18)
            * 0.5
            * (0.86 + basin * 0.22),
    );
    let chop = clamp01(
        0.5 + (s1 * 0.18 + s2 * 0.34 + s3 * 0.34 + s4 * 0.14) * 0.5 * (0.82 + shear * 0.26),
    );
    let ripple = clamp01(
        0.5 + (s3 * 0.20 + s4 * 0.31 + s5 * 0.26 + (shear - 0.5) * 0.28) * 0.5 * local_energy,
    );
    let patch = fbm_tiled(
        uu * 4.0 + basin * 0.08,
        vv * 3.0 + eddy * 0.06,
        37,
        3,
        seed + 13_111,
        0.50,
    );
    let freckle = fbm_tiled(
        uu * 12.0 + patch * 0.08,
        vv * 6.0 - patch * 0.05,
        83,
        2,
        seed + 13_307,
        0.46,
    );
    let crest_breakup = clamp01(ridge(patch) * 0.58 + ridge(freckle) * 0.30 + (shear - 0.5) * 0.12);
    let foam = clamp01(
        smoothstep(
            0.58,
            0.96,
            chop * 0.62 + ripple * 0.30 + crest_breakup * 0.08,
        ) * (0.34 + crest_breakup * 0.54)
            * (0.72 + polar_soften * 0.28),
    );
    let glint = clamp01(
        (smoothstep(0.56, 0.94, chop) * 0.34
            + smoothstep(0.58, 0.96, ripple) * 0.24
            + smoothstep(0.52, 0.88, swell) * 0.12)
            * (0.36 + crest_breakup * 0.58)
            * (0.78 + polar_soften * 0.22),
    );
    let slope = Vec3::new(
        (dx0 * c0 * 0.021
            + dx1 * c1 * 0.016
            + dx2 * c2 * 0.012
            + dx3 * c3 * 0.0075
            + dx4 * c4 * 0.0046
            + dx5 * c5 * 0.0028
            + (eddy - basin) * 0.010)
            * polar_soften,
        (dy0 * c0 * 0.021
            + dy1 * c1 * 0.016
            + dy2 * c2 * 0.012
            + dy3 * c3 * 0.0075
            + dy4 * c4 * 0.0046
            + dy5 * c5 * 0.0028
            + (shear - eddy) * 0.008)
            * polar_soften,
        0.0,
    );

    OceanWaveSpectrum {
        swell,
        chop,
        ripple,
        foam,
        glint,
        slope,
    }
}

fn directional_wave(
    u: f32,
    v: f32,
    angle: f32,
    frequency: f32,
    phase_seed: f32,
    bend: f32,
) -> (f32, f32, f32, f32) {
    let dx = angle.cos();
    let dy = angle.sin();
    let along = u * dx + v * dy;
    let across = u * -dy + v * dx;
    let lateral_bend =
        (across * PI * 2.0 * (frequency * 0.17 + 1.0) + phase_seed * PI * 3.0).sin() * bend;
    let slow_bend = ((u + v) * PI * 2.0 * 2.0 + phase_seed * PI).sin() * bend * 0.28;
    let phase = along * PI * 2.0 * frequency + lateral_bend + slow_bend + phase_seed * PI * 2.0;
    (phase.sin(), phase.cos(), dx, dy)
}

fn ocean_wave_tangent_perturb(u: f32, v: f32, seed: u64) -> Vec3 {
    ocean_wave_spectrum(u, v, seed).slope
}

fn ocean_wave_glint_mask(u: f32, v: f32, seed: u64) -> f32 {
    ocean_wave_spectrum(u, v, seed).glint
}

fn terrain_micro_detail(u: f32, v: f32, seed: u64) -> f32 {
    let grain = fbm_periodic(
        u * 1.9 + v * 0.17,
        v * 2.4 - u * 0.09,
        128,
        3,
        seed + 18_211,
        0.49,
    );
    let pebble = fbm_periodic(
        u * 4.8 + grain * 0.12,
        v * 4.0 - grain * 0.10,
        149,
        3,
        seed + 18_337,
        0.47,
    );
    let dust = fbm_periodic(
        u * 10.5 + pebble * 0.045,
        v * 8.2 - grain * 0.035,
        181,
        2,
        seed + 18_593,
        0.44,
    );
    clamp01(grain * 0.60 + pebble * 0.28 + dust * 0.12)
}

fn render_space_background_with_progress<P>(
    canvas: &mut Canvas,
    seed: u64,
    execution_mode: RenderExecutionMode,
    progress: &mut P,
) where
    P: FnMut(RenderProgressEvent),
{
    let size = RenderSize {
        width: canvas.width,
        height: canvas.height,
    };
    let plan = TilePlan::for_size(size, execution_mode);
    render_tiled_into_canvas(canvas, &plan, RenderPhase::Background, progress, |x, y| {
        let u = x as f32 / size.width.saturating_sub(1).max(1) as f32;
        let v = y as f32 / size.height.saturating_sub(1).max(1) as f32;
        rgba(tone_map(deep_space_color(u, v, seed)), 255)
    });
}

fn draw_rings(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, front: bool) {
    let x0 = ((cx - radius * 2.55) as i32).max(0);
    let x1 = ((cx + radius * 2.55) as i32).min(canvas.width as i32 - 1);
    let y0 = ((cy - radius * 0.86) as i32).max(0);
    let y1 = ((cy + radius * 0.86) as i32).min(canvas.height as i32 - 1);
    let angle = -0.155_f32;
    let ca = angle.cos();
    let sa = angle.sin();
    let sun = distant_light_vec(DistantLight::solar_default());

    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let xr = dx * ca - dy * sa;
            let yr = dx * sa + dy * ca;
            let radial = ((xr / (radius * 1.86)).powi(2) + (yr / (radius * 0.34)).powi(2)).sqrt();
            if !(0.73..=1.31).contains(&radial) {
                continue;
            }

            let disk = (dx * dx + dy * dy).sqrt() / radius;
            if front {
                if !(disk < 1.04 && yr > 0.0) {
                    continue;
                }
            } else if disk < 1.035 {
                continue;
            }

            let gap_a = 1.0 - smoothstep(0.012, 0.0, (radial - 0.91).abs());
            let gap_b = 1.0 - smoothstep(0.014, 0.0, (radial - 1.13).abs());
            let bands = 0.62 + 0.24 * (radial * 116.0).sin() + 0.10 * (radial * 271.0 + 0.7).sin();
            let edge = smoothstep(0.73, 0.79, radial) * (1.0 - smoothstep(1.24, 1.31, radial));
            let mut alpha = clamp01(bands) * gap_a * gap_b * edge;
            if alpha <= 0.018 {
                continue;
            }
            if front {
                alpha *= 0.58;
            }
            let lit = smoothstep(-0.25, 0.75, sun.x * 0.35 + sun.z * 0.65);
            let color = Vec3::new(0.42, 0.39, 0.33).lerp(Vec3::new(0.88, 0.80, 0.64), bands)
                * (0.34 + lit * 0.78);
            canvas.blend(x, y, rgba(tone_map(color), (alpha * 126.0) as u8));
        }
    }
}

fn render_moon(canvas: &mut Canvas, cx: f32, cy: f32, radius: f32, seed: u64) {
    let x0 = (cx - radius - 2.0) as i32;
    let x1 = (cx + radius + 2.0) as i32;
    let y0 = (cy - radius - 2.0) as i32;
    let y1 = (cy + radius + 2.0) as i32;
    let sun = distant_light_vec(DistantLight::solar_default());

    for y in y0..=y1 {
        let dy = (y as f32 + 0.5 - cy) / radius;
        for x in x0..=x1 {
            let dx = (x as f32 + 0.5 - cx) / radius;
            let d2 = dx * dx + dy * dy;
            if d2 > 1.0 {
                continue;
            }
            let z = (1.0 - d2).sqrt();
            let n = Vec3::new(dx, dy, z).normalize();
            let light = n.dot(sun).max(0.0);
            let rough = fbm_periodic(dx * 0.5 + 0.5, dy * 0.5 + 0.5, 9, 4, seed + 3033, 0.52);
            let crater = smoothstep(0.76, 0.94, rough) * 0.19;
            let color = Vec3::splat(0.20 + light * 0.72 + crater);
            let edge = 1.0 - smoothstep(0.965, 1.0, d2.sqrt());
            canvas.blend(x, y, rgba(tone_map(color), (255.0 * edge) as u8));
        }
    }
}

fn normal_to_uv(n: Vec3) -> (f32, f32) {
    let lon = n.z.atan2(n.x);
    let lat = n.y.clamp(-1.0, 1.0).asin();
    ((lon / (PI * 2.0) + 0.5).rem_euclid(1.0), 0.5 - lat / PI)
}

fn rotate_planet(n: Vec3) -> Vec3 {
    let yaw = 0.92_f32;
    let pitch = -0.12_f32;
    let roll = 0.06_f32;

    let (cy, sy) = (yaw.cos(), yaw.sin());
    let (cp, sp) = (pitch.cos(), pitch.sin());
    let (cr, sr) = (roll.cos(), roll.sin());

    let x1 = n.x * cy + n.z * sy;
    let z1 = -n.x * sy + n.z * cy;
    let y2 = n.y * cp - z1 * sp;
    let z2 = n.y * sp + z1 * cp;
    Vec3::new(x1 * cr - y2 * sr, x1 * sr + y2 * cr, z2)
}

fn fbm_periodic(u: f32, v: f32, base_cells: i32, octaves: i32, seed: u64, persistence: f32) -> f32 {
    let mut amp = 1.0;
    let mut total = 0.0;
    let mut norm = 0.0;
    for octave in 0..octaves {
        let cells = base_cells * (1 << octave);
        total += value_noise_periodic(
            u,
            v,
            cells,
            (cells / 2).max(2),
            seed + octave as u64 * 7_919,
        ) * amp;
        norm += amp;
        amp *= persistence;
    }
    total / norm
}

fn fbm_tiled(u: f32, v: f32, base_cells: i32, octaves: i32, seed: u64, persistence: f32) -> f32 {
    fbm_periodic(
        u.rem_euclid(1.0),
        v.rem_euclid(1.0),
        base_cells,
        octaves,
        seed,
        persistence,
    )
}

fn value_noise_periodic(u: f32, v: f32, cells_x: i32, cells_y: i32, seed: u64) -> f32 {
    let x = u.rem_euclid(1.0) * cells_x as f32;
    let y = clamp01(v) * cells_y as f32;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let tx = fade(x - ix as f32);
    let ty = fade(y - iy as f32);
    let ix0 = ix.rem_euclid(cells_x);
    let ix1 = (ix + 1).rem_euclid(cells_x);
    let iy0 = iy.clamp(0, cells_y - 1);
    let iy1 = (iy + 1).clamp(0, cells_y - 1);

    let v00 = hash2(ix0, iy0, seed);
    let v10 = hash2(ix1, iy0, seed);
    let v01 = hash2(ix0, iy1, seed);
    let v11 = hash2(ix1, iy1, seed);
    bilerp(v00, v10, v01, v11, tx, ty)
}

fn hash2(x: i32, y: i32, seed: u64) -> f32 {
    let mut n = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    n ^= n >> 30;
    n = n.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    n ^= n >> 27;
    n = n.wrapping_mul(0x94D0_49BB_1331_11EB);
    n ^= n >> 31;
    ((n >> 40) as f32) / ((1_u64 << 24) as f32)
}

fn bilerp(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> f32 {
    let ab = a + (b - a) * tx;
    let cd = c + (d - c) * tx;
    ab + (cd - ab) * ty
}

fn bilerp_vec3(a: Vec3, b: Vec3, c: Vec3, d: Vec3, tx: f32, ty: f32) -> Vec3 {
    a.lerp(b, tx).lerp(c.lerp(d, tx), ty)
}

fn ridge(value: f32) -> f32 {
    1.0 - (value * 2.0 - 1.0).abs()
}

fn schlick_fresnel(cos_theta: f32, f0: f32) -> f32 {
    f0 + (1.0 - f0) * (1.0 - cos_theta).powi(5)
}

fn anti_band_grain(u: f32, v: f32, seed: u64) -> f32 {
    let x = (u.rem_euclid(1.0) * 4096.0).floor() as i32;
    let y = (clamp01(v) * 4096.0).floor() as i32;
    let white = hash2(x, y, seed) - 0.5;
    let fine = fbm_tiled(
        u * 1.73 + white * 0.015,
        v * 1.41 - white * 0.011,
        113,
        2,
        seed + 37_711,
        0.46,
    ) - 0.5;
    white * 0.58 + fine * 0.42
}

fn apply_anti_band_grain(color: Vec3, u: f32, v: f32, seed: u64, amount: f32) -> Vec3 {
    let grain = anti_band_grain(u, v, seed) * amount;
    color + Vec3::new(grain, grain * 0.94, grain * 0.86)
}

fn atmosphere_optical_depth(view_dot: f32, density: f32) -> f32 {
    let density = clamp(density, 0.0, 1.8);
    if density <= 0.0 {
        return 0.0;
    }
    let mu = view_dot.max(0.0);
    let grazing_path = 1.0 / (0.075 + mu * 0.925);
    let density_scale = density * (0.095 + density * 0.145);
    clamp(grazing_path * density_scale, 0.0, 1.45)
}

#[derive(Debug, Clone, Copy)]
struct OpticalMedium {
    absorption: Vec3,
    scatter_color: Vec3,
    density: f32,
    anisotropy: f32,
}

impl OpticalMedium {
    fn atmosphere(density: f32) -> Self {
        Self {
            absorption: Vec3::new(0.055, 0.030, 0.012),
            scatter_color: Vec3::new(0.56, 0.72, 0.92),
            density: clamp(density, 0.0, 1.8),
            anisotropy: 0.18,
        }
    }

    fn ocean(depth: f32) -> Self {
        Self {
            absorption: Vec3::new(1.70, 0.48, 0.145),
            scatter_color: Vec3::new(0.035, 0.180, 0.280),
            density: clamp(depth, 0.0, 2.2),
            anisotropy: 0.42,
        }
    }

    fn dust(density: f32, volcanic: bool) -> Self {
        Self {
            absorption: if volcanic {
                Vec3::new(0.34, 0.28, 0.22)
            } else {
                Vec3::new(0.24, 0.20, 0.16)
            },
            scatter_color: if volcanic {
                Vec3::new(0.90, 0.62, 0.36)
            } else {
                Vec3::new(0.78, 0.68, 0.52)
            },
            density: clamp01(density),
            anisotropy: 0.36,
        }
    }
}

fn beer_lambert(absorption: Vec3, density: f32, distance: f32) -> Vec3 {
    let path = density.max(0.0) * distance.max(0.0);
    Vec3::new(
        (-absorption.x.max(0.0) * path).exp(),
        (-absorption.y.max(0.0) * path).exp(),
        (-absorption.z.max(0.0) * path).exp(),
    )
}

fn henyey_greenstein_phase(cos_theta: f32, anisotropy: f32) -> f32 {
    let g = clamp(anisotropy, -0.82, 0.82);
    let denom = (1.0 + g * g - 2.0 * g * clamp(cos_theta, -1.0, 1.0)).powf(1.5);
    ((1.0 - g * g) / denom.max(0.025)).min(8.0)
}

fn distance_diffusion(distance: f32, density: f32, roughness: f32) -> f32 {
    let d = distance.max(0.0);
    let spread = 0.34 + density.max(0.0) * 0.42 + roughness.max(0.0) * 0.30;
    1.0 / (1.0 + d * d * spread)
}

fn apply_optical_medium(
    surface: Vec3,
    medium: OpticalMedium,
    distance: f32,
    light_cos: f32,
    roughness: f32,
) -> Vec3 {
    let transmittance = beer_lambert(medium.absorption, medium.density, distance);
    let phase = henyey_greenstein_phase(light_cos, medium.anisotropy);
    let diffusion = distance_diffusion(distance, medium.density, roughness);
    let scatter_strength = clamp01((1.0 - diffusion) * (0.22 + medium.density * 0.52) * phase);
    Vec3::new(
        surface.x * transmittance.x,
        surface.y * transmittance.y,
        surface.z * transmittance.z,
    )
    .lerp(medium.scatter_color, scatter_strength)
}

fn sample_environment(dir: Vec3, seed: u64, light: DistantLight) -> Vec3 {
    let d = dir.normalize_or(Vec3::new(0.0, 0.0, 1.0));
    let light_dir = distant_light_vec(light);
    let sun_disk = smoothstep(
        light.sun_disk_cosine_threshold() - 0.0025,
        1.0,
        d.dot(light_dir),
    );
    let u = (d.z.atan2(d.x) / (PI * 2.0) + 0.5).rem_euclid(1.0);
    let v = (0.5 - d.y.asin() / PI).clamp(0.0, 1.0);
    deep_space_color(u, v, seed) + distant_light_color(light) * (7.4 * sun_disk)
}

fn deep_space_color(u: f32, v: f32, seed: u64) -> Vec3 {
    let u = u.rem_euclid(1.0);
    let v = clamp01(v);
    let dust = fbm_periodic(u * 0.80 + 0.11, v * 0.92 + 0.07, 10, 4, seed + 9_001, 0.54);
    let filament = fbm_periodic(
        u * 1.8 + dust * 0.05,
        v * 1.4 - dust * 0.04,
        24,
        3,
        seed + 9_113,
        0.50,
    );
    let core = (-(((u - 0.13).powi(2) / 0.020) + ((v - 0.18).powi(2) / 0.050))).exp();
    let band = (-(((u - 0.53).powi(2) / 0.22) + ((v - 1.18).powi(2) / 0.18))).exp();
    let veil = smoothstep(0.44, 0.88, dust * 0.62 + filament * 0.38);
    let mut color = Vec3::new(0.006, 0.010, 0.030)
        + Vec3::new(0.065, 0.075, 0.180) * core
        + Vec3::new(0.050, 0.038, 0.090) * band
        + Vec3::new(0.025, 0.036, 0.085) * veil * 0.46
        + Vec3::new(0.080, 0.030, 0.070) * ridge(filament) * veil * 0.24;
    color += starfield_layer(u, v, 520.0, 260.0, seed + 9_211, 1.00);
    color += starfield_layer(u + 0.371, v + 0.113, 960.0, 480.0, seed + 9_307, 0.46);
    color
}

fn starfield_layer(u: f32, v: f32, cells_x: f32, cells_y: f32, seed: u64, intensity: f32) -> Vec3 {
    let x = u.rem_euclid(1.0) * cells_x;
    let y = clamp01(v) * cells_y;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let h = hash2(ix, iy, seed);
    if h < 0.988 {
        return Vec3::ZERO;
    }
    let cx = hash2(ix + 17, iy - 31, seed) * 0.80 + 0.10;
    let cy = hash2(ix - 11, iy + 23, seed) * 0.80 + 0.10;
    let dx = x - ix as f32 - cx;
    let dy = y - iy as f32 - cy;
    let point = smoothstep(0.040, 0.0, (dx * dx + dy * dy).sqrt());
    let brightness = ((h - 0.988) / 0.012).powf(1.8) * point * intensity;
    let warmth = hash2(ix + 101, iy + 7, seed);
    Vec3::new(0.58, 0.72, 1.0).lerp(Vec3::new(1.0, 0.84, 0.55), warmth) * brightness * 1.55
}

fn tone_map(color: Vec3) -> Vec3 {
    let mapped = Vec3::new(
        aces(color.x.max(0.0)),
        aces(color.y.max(0.0)),
        aces(color.z.max(0.0)),
    );
    Vec3::new(
        mapped.x.powf(1.0 / 2.2),
        mapped.y.powf(1.0 / 2.2),
        mapped.z.powf(1.0 / 2.2),
    )
}

fn color_luma(color: Vec3) -> f32 {
    color.x * 0.2126 + color.y * 0.7152 + color.z * 0.0722
}

fn aces(x: f32) -> f32 {
    clamp01((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14))
}

fn rgba(color: Vec3, alpha: u8) -> [u8; 4] {
    [
        (clamp01(color.x) * 255.0) as u8,
        (clamp01(color.y) * 255.0) as u8,
        (clamp01(color.z) * 255.0) as u8,
        alpha,
    ]
}

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge0 - edge1).abs() < f32::EPSILON {
        return 0.0;
    }
    let t = clamp01((value - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

fn clamp01(value: f32) -> f32 {
    clamp(value, 0.0, 1.0)
}

fn clamp(value: f32, low: f32, high: f32) -> f32 {
    value.max(low).min(high)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    const X: Self = Self::new(1.0, 0.0, 0.0);
    const Y: Self = Self::new(0.0, 1.0, 0.0);

    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    const fn splat(value: f32) -> Self {
        Self::new(value, value, value)
    }

    fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    fn cross(self, rhs: Self) -> Self {
        Self::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn normalize(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            Self::ZERO
        } else {
            self / len
        }
    }

    fn normalize_or(self, fallback: Self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            fallback
        } else {
            self / len
        }
    }

    fn lerp(self, rhs: Self, t: f32) -> Self {
        self + (rhs - self) * clamp01(t)
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

impl std::ops::Div<f32> for Vec3 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

const VIEW: Vec3 = Vec3::new(0.0, 0.0, 1.0);

fn distant_light_for_mode(mode: LightingMode) -> DistantLight {
    match mode {
        LightingMode::Day => DistantLight::solar_default(),
        LightingMode::Night => DistantLight::night_fill(),
    }
}

fn distant_light_vec(light: DistantLight) -> Vec3 {
    Vec3::new(light.direction[0], light.direction[1], light.direction[2]).normalize()
}

fn distant_light_color(light: DistantLight) -> Vec3 {
    Vec3::new(
        light.color_linear[0],
        light.color_linear[1],
        light.color_linear[2],
    ) * light.intensity
}

fn planet_light_dir(mode: LightingMode) -> Vec3 {
    distant_light_vec(distant_light_for_mode(mode))
}

fn overview_light_dir(mode: LightingMode) -> Vec3 {
    distant_light_vec(distant_light_for_mode(mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optical_helpers_absorb_diffuse_and_scatter_predictably() {
        let shallow = beer_lambert(Vec3::new(1.7, 0.48, 0.145), 0.35, 0.25);
        let deep = beer_lambert(Vec3::new(1.7, 0.48, 0.145), 1.40, 1.20);
        assert!(deep.x < shallow.x);
        assert!(deep.y < shallow.y);
        assert!(deep.z < shallow.z);
        assert!(
            deep.x < deep.z,
            "water absorption should suppress red more strongly than blue"
        );

        let near = distance_diffusion(0.25, 0.2, 0.1);
        let far = distance_diffusion(3.0, 0.8, 0.6);
        assert!(near > far);
        assert!(near <= 1.0 && far >= 0.0);

        let forward = henyey_greenstein_phase(0.85, 0.45);
        let backward = henyey_greenstein_phase(-0.85, 0.45);
        assert!(forward > backward);
    }

    #[test]
    fn optical_medium_application_is_finite_and_density_sensitive() {
        let surface = Vec3::new(0.64, 0.48, 0.34);
        let thin = apply_optical_medium(surface, OpticalMedium::dust(0.12, false), 0.35, 0.2, 0.4);
        let thick = apply_optical_medium(surface, OpticalMedium::dust(0.90, false), 1.40, 0.2, 0.4);
        for channel in [thin.x, thin.y, thin.z, thick.x, thick.y, thick.z] {
            assert!(channel.is_finite());
            assert!(channel >= 0.0);
        }
        let rgb_delta =
            (thick.x - thin.x).abs() + (thick.y - thin.y).abs() + (thick.z - thin.z).abs();
        assert!(rgb_delta > 0.001);
    }

    #[test]
    fn rocky_palette_selects_distinct_deterministic_variants() {
        let mercury = PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(11).with_archetype_key("catalog.archetype.mercury-like"),
        );
        let basalt = PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(11).with_archetype_key("catalog.archetype.barren-basalt"),
        );
        let mercury_palette = rocky_surface_palette(&mercury, rocky_surface_material(&mercury));
        let mercury_again = rocky_surface_palette(&mercury, rocky_surface_material(&mercury));
        let basalt_palette = rocky_surface_palette(&basalt, rocky_surface_material(&basalt));

        assert_eq!(mercury_palette.kind, RockyPaletteKind::PaleSilicate);
        assert_eq!(mercury_palette.kind, mercury_again.kind);
        assert_eq!(mercury_palette.mid, mercury_again.mid);
        assert_eq!(basalt_palette.kind, RockyPaletteKind::Basaltic);
        assert_ne!(mercury_palette.mid, basalt_palette.mid);

        let iron_variant = (0_u64..256)
            .map(|seed| {
                PlanetVisualProfile::from_seed_input(
                    ProfileSeedInput::new(seed).with_archetype_key("catalog.archetype.iron"),
                )
            })
            .map(|profile| rocky_surface_palette(&profile, rocky_surface_material(&profile)))
            .find(|palette| palette.kind == RockyPaletteKind::IronOxide)
            .expect("iron archetype should have deterministic iron-oxide seed variants");
        assert!(iron_variant.mineral.x > iron_variant.mid.x);
        assert!(iron_variant.mineral.x > iron_variant.mineral.z * 2.0);
    }

    #[test]
    fn profile_is_deterministic() {
        let a = PlanetVisualProfile::from_seed(42);
        let b = PlanetVisualProfile::from_seed(42);
        let c = PlanetVisualProfile::from_seed(43);
        assert_eq!(a, b);
        assert_ne!(a.radius_km, c.radius_km);
    }

    #[test]
    fn icon_render_is_nonblank() {
        let profile = PlanetVisualProfile::from_seed(0x5EED_1208_0001);
        let renderer = PlanetRenderer {
            profile: profile.clone(),
            maps: PlanetMaps::generate(&profile, 128, 64),
            stable_terrain: None,
        };
        let icon = renderer.render_icon(96);
        let nontransparent = icon.pixels().filter(|p| p[3] > 0).count();
        let bright = icon
            .pixels()
            .filter(|p| p[0] as u16 + p[1] as u16 + p[2] as u16 > 80)
            .count();
        assert!(nontransparent > 2_000);
        assert!(bright > 1_000);
    }

    #[test]
    fn night_icon_and_overview_are_distinct_outputs() {
        let profile = PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(0x5EED_1208_00A1)
                .with_archetype_key("catalog.archetype.temperate-continents"),
        );
        let renderer = PlanetRenderer {
            profile: profile.clone(),
            maps: PlanetMaps::generate(&profile, 128, 64),
            stable_terrain: None,
        };

        let day_icon = renderer.render_icon_with_options(96, RenderOptions::preview());
        let night_icon = renderer.render_night_icon_with_options(96, RenderOptions::preview());
        let overview_size = RenderSize {
            width: 160,
            height: 90,
        };
        let day_overview =
            renderer.render_terrain_overview_with_options(overview_size, RenderOptions::preview());
        let night_overview = renderer
            .render_night_terrain_overview_with_options(overview_size, RenderOptions::preview());

        assert_eq!((night_icon.width(), night_icon.height()), (96, 96));
        assert_eq!(
            (night_overview.width(), night_overview.height()),
            (overview_size.width, overview_size.height)
        );
        assert!(night_icon.pixels().filter(|p| p[3] > 0).count() > 2_000);
        assert!(night_overview.pixels().all(|p| p[3] == 255));
        assert!(average_rgb_delta(&day_icon, &night_icon) > 10.0);
        assert!(average_rgb_delta(&day_overview, &night_overview) > 10.0);
    }

    #[test]
    fn native_render_presets_validate() {
        let presets = [
            RenderSizePreset::P480,
            RenderSizePreset::P720,
            RenderSizePreset::P1080,
            RenderSizePreset::Uhd4K,
            RenderSizePreset::Uhd8K,
            RenderSizePreset::Square512,
            RenderSizePreset::Square1024,
            RenderSizePreset::Square2048,
            RenderSizePreset::Square4096,
            RenderSizePreset::Portrait720P,
            RenderSizePreset::Portrait1080P,
            RenderSizePreset::Portrait4K,
        ];

        for preset in presets {
            assert!(
                preset.size().validate_native().is_ok(),
                "{preset:?} should be a supported native output size"
            );
        }
    }

    #[test]
    fn tile_plan_covers_requested_size() {
        let size = RenderSize {
            width: 300,
            height: 260,
        };
        let plan = TilePlan::with_tile_size(
            size,
            128,
            96,
            RenderExecutionMode::MultiThreaded { threads: 32 },
        );

        assert_eq!(plan.total_pixels(), size.pixel_count());
        assert_eq!(plan.worker_threads, plan.tiles.len());
        for tile in &plan.tiles {
            assert!(tile.x_end() <= size.width);
            assert!(tile.y_end() <= size.height);
            assert!(tile.width > 0);
            assert!(tile.height > 0);
        }
    }

    #[test]
    fn surface_map_parallel_matches_serial() {
        let profile = PlanetVisualProfile::from_seed(0x5EED_1208_0002);
        let renderer = PlanetRenderer {
            profile: profile.clone(),
            maps: PlanetMaps::generate(&profile, 96, 48),
            stable_terrain: None,
        };
        let size = RenderSize {
            width: 160,
            height: 80,
        };

        let serial =
            renderer.render_surface_map_with_progress(size, RenderExecutionMode::Serial, |_| {});
        let parallel = renderer.render_surface_map_with_progress(
            size,
            RenderExecutionMode::MultiThreaded { threads: 3 },
            |_| {},
        );

        assert_eq!(serial.as_raw(), parallel.as_raw());
    }

    #[test]
    fn terrain_overview_progress_completes() {
        let profile = PlanetVisualProfile::from_seed(0x5EED_1208_0003);
        let renderer = PlanetRenderer {
            profile: profile.clone(),
            maps: PlanetMaps::generate(&profile, 128, 64),
            stable_terrain: None,
        };
        let size = RenderSize {
            width: 192,
            height: 160,
        };
        let mut events = Vec::new();

        let image = renderer.render_terrain_overview_with_progress(
            size,
            RenderOptions::preview(),
            RenderExecutionMode::MultiThreaded { threads: 2 },
            |event| events.push(event),
        );

        assert_eq!((image.width(), image.height()), (size.width, size.height));
        assert!(events.iter().any(|event| {
            event.progress.phase == RenderPhase::TerrainOverview && event.worker_threads == 2
        }));
        let last = events.last().expect("progress should emit completion");
        assert_eq!(last.progress.phase, RenderPhase::Complete);
        assert_eq!(last.progress.completed_pixels, size.pixel_count());
    }

    #[test]
    fn terrain_camera_is_aspect_aware() {
        let portrait = TerrainOverviewCamera::for_size(RENDER_SIZE_PORTRAIT_1080P);
        let square = TerrainOverviewCamera::for_size(RENDER_SIZE_SQUARE_1024);
        let landscape = TerrainOverviewCamera::for_size(RENDER_SIZE_1080P);

        assert!(portrait.x_scale < square.x_scale);
        assert!(landscape.x_scale > square.x_scale);
        assert!(portrait.base_horizon > square.base_horizon);
        assert!(square.base_horizon > landscape.base_horizon);
        assert!(portrait.forward_scale > square.forward_scale);
        assert!(landscape.spread_scale > portrait.spread_scale);
    }

    #[test]
    fn material_maps_are_sized_and_data_bearing() {
        let profile = PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(0x5EED_1208_0004)
                .with_archetype_key("catalog.archetype.temperate-continents"),
        );
        let renderer = PlanetRenderer {
            profile: profile.clone(),
            maps: PlanetMaps::generate(&profile, 128, 64),
            stable_terrain: None,
        };
        let size = RenderSize {
            width: 128,
            height: 64,
        };

        let normal = renderer.render_normal_map(size);
        let height = renderer.render_height_map(size);
        let vegetation = renderer.render_vegetation_map(size);
        let roughness = renderer.render_roughness_map(size);

        for image in [&normal, &height, &vegetation, &roughness] {
            assert_eq!((image.width(), image.height()), (size.width, size.height));
            assert!(image.pixels().all(|pixel| pixel[3] == 255));
        }

        assert!(channel_range(&normal, 0) > 8);
        assert!(channel_range(&height, 0) > 32);
        assert!(channel_range(&vegetation, 1) > 16);
        assert!(channel_range(&roughness, 0) > 16);
        assert!(channel_range(&roughness, 1) > 16);
    }

    #[test]
    fn physics_model_samples_are_bounded_and_deterministic() {
        let profile = PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(0x5EED_1208_00D1)
                .with_archetype_key("catalog.archetype.global-ocean"),
        );
        let model = PlanetPhysicsModel::from_profile(&profile);
        let a = model.sample(0.23, 0.57);
        let b = model.sample(0.23, 0.57);
        let polar = model.sample(0.23, 0.08);

        assert_eq!(a, b);
        assert!(model.rotation_period_hours.is_finite());
        assert!(model.rotation_period_hours >= 7.0);
        assert!(a.current_speed_mps >= 0.0);
        assert!(a.current_speed_mps <= model.current_velocity_scale_mps * 1.45 + f32::EPSILON);
        assert!((920.0..=1_220.0).contains(&a.water_density_kg_m3));
        assert!((0.001..=20.0).contains(&a.surface_pressure_bar));
        assert!((0.0..=1.0).contains(&a.current_shear));
        assert!((0.0..=1.0).contains(&a.cloud_lift));
        assert!(a.ocean_current_mps.length().is_finite());
        assert!(a.cloud_flow_mps.length().is_finite());
        assert!(
            polar.magnetic_field_microtesla >= a.magnetic_field_microtesla * 0.65,
            "dipole field should stay physically plausible toward polar regions"
        );
    }

    #[test]
    fn physics_and_density_maps_are_sized_opaque_and_data_bearing() {
        let profile = PlanetVisualProfile::from_seed_input(
            ProfileSeedInput::new(0x5EED_1208_00D2)
                .with_archetype_key("catalog.archetype.global-ocean"),
        );
        let renderer = PlanetRenderer {
            profile: profile.clone(),
            maps: PlanetMaps::generate(&profile, 128, 64),
            stable_terrain: None,
        };
        let size = RenderSize {
            width: 128,
            height: 64,
        };

        let physics = renderer.render_physics_map(size);
        let density = renderer.render_density_map(size);

        for image in [&physics, &density] {
            assert_eq!((image.width(), image.height()), (size.width, size.height));
            assert!(image.pixels().all(|pixel| pixel[3] == 255));
        }
        assert!(
            channel_range(&physics, 0) > 12,
            "current channel should vary"
        );
        assert!(
            channel_range(&physics, 1) > 12,
            "cloud/lift channel should vary"
        );
        assert!(
            channel_range(&physics, 2) > 8,
            "magnetic channel should vary"
        );
        assert!(
            channel_range(&density, 0) > 8,
            "water-density channel should vary"
        );
        assert!(
            channel_range(&density, 1) > 4,
            "atmosphere-density channel should vary"
        );
        assert!(
            channel_range(&density, 2) > 4,
            "pressure channel should vary"
        );
    }

    #[test]
    fn ocean_wave_spectrum_is_bounded_and_directional() {
        let seed = 0x5EED_1208_00F0;
        let first = ocean_wave_spectrum(0.23, 0.57, seed);
        let second = ocean_wave_spectrum(0.23, 0.57, seed);

        assert_eq!(first.swell.to_bits(), second.swell.to_bits());
        assert_eq!(first.chop.to_bits(), second.chop.to_bits());
        assert_eq!(first.ripple.to_bits(), second.ripple.to_bits());
        assert_eq!(first.foam.to_bits(), second.foam.to_bits());
        assert_eq!(first.glint.to_bits(), second.glint.to_bits());
        assert_eq!(first.slope.x.to_bits(), second.slope.x.to_bits());
        assert_eq!(first.slope.y.to_bits(), second.slope.y.to_bits());

        let mut slope_x = 0.0;
        let mut slope_y = 0.0;
        for y in 0..6 {
            for x in 0..9 {
                let wave =
                    ocean_wave_spectrum(x as f32 / 9.0 + 0.017, y as f32 / 7.0 + 0.029, seed);
                for value in [wave.swell, wave.chop, wave.ripple, wave.foam, wave.glint] {
                    assert!(value.is_finite());
                    assert!((0.0..=1.0).contains(&value));
                }
                assert!(wave.slope.x.is_finite());
                assert!(wave.slope.y.is_finite());
                assert!(wave.slope.length() < 0.12);
                slope_x += wave.slope.x.abs();
                slope_y += wave.slope.y.abs();
            }
        }

        let balance = slope_x.min(slope_y) / slope_x.max(slope_y).max(f32::EPSILON);
        assert!(
            balance > 0.25,
            "wave slopes should not collapse into one axis"
        );
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

    fn average_rgb_delta(a: &RgbaImage, b: &RgbaImage) -> f32 {
        assert_eq!((a.width(), a.height()), (b.width(), b.height()));
        let total: u64 = a
            .pixels()
            .zip(b.pixels())
            .map(|(left, right)| {
                (left[0] as i32 - right[0] as i32).unsigned_abs() as u64
                    + (left[1] as i32 - right[1] as i32).unsigned_abs() as u64
                    + (left[2] as i32 - right[2] as i32).unsigned_abs() as u64
            })
            .sum();
        total as f32 / (a.width() * a.height()) as f32 / 3.0
    }

    #[test]
    fn native_policy_guards_8k_supersampling() {
        let ultra = RenderOptions::ultra();

        assert_eq!(ultra.native_supersample_for_size(RENDER_SIZE_1080P), 2);
        assert_eq!(ultra.native_supersample_for_size(RENDER_SIZE_4K), 2);
        assert_eq!(ultra.native_supersample_for_size(RENDER_SIZE_8K), 1);
        assert_eq!(
            ultra.native_supersample_for_size(RENDER_SIZE_SQUARE_4096),
            1
        );
    }

    #[test]
    fn native_policy_rejects_oversized_outputs() {
        assert_eq!(
            validate_render_size(RenderSize {
                width: 0,
                height: 1
            }),
            Err(RenderSizeValidationError::Empty {
                width: 0,
                height: 1
            })
        );
        assert_eq!(
            validate_render_size(RenderSize {
                width: 7_681,
                height: 4_320
            }),
            Err(RenderSizeValidationError::LongEdgeTooLarge {
                value: 7_681,
                maximum: MAX_NATIVE_RENDER_LONG_EDGE
            })
        );
    }
}
