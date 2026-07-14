//! Deterministic time-evolved climate and field sampling for planet visuals.
//!
//! The model is intentionally analytic rather than simulated. A renderer can
//! request any timestamp directly from the seed and get stable winds, ocean
//! currents, cloud density, cyclone influence, density, and magnetism fields.

use crate::{PhysicsVector2, PlanetVisualProfile};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub const DEFAULT_TICKS_PER_DAY: u32 = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvolutionTime {
    pub tick: i64,
    pub ticks_per_day: u32,
}

impl EvolutionTime {
    pub const ZERO: Self = Self::new(0, DEFAULT_TICKS_PER_DAY);

    pub const fn new(tick: i64, ticks_per_day: u32) -> Self {
        Self {
            tick,
            ticks_per_day,
        }
    }

    pub fn from_days(days: f64) -> Self {
        Self::from_days_with_resolution(days, DEFAULT_TICKS_PER_DAY)
    }

    pub fn from_days_with_resolution(days: f64, ticks_per_day: u32) -> Self {
        let ticks_per_day = ticks_per_day.max(1);
        let tick = (days * ticks_per_day as f64).round() as i64;
        Self {
            tick,
            ticks_per_day,
        }
    }

    pub fn days(self) -> f64 {
        self.tick as f64 / self.ticks_per_day.max(1) as f64
    }

    pub fn advance_ticks(self, ticks: i64) -> Self {
        Self {
            tick: self.tick.saturating_add(ticks),
            ticks_per_day: self.ticks_per_day,
        }
    }

    pub fn advance_days(self, days: f64) -> Self {
        self.advance_ticks((days * self.ticks_per_day.max(1) as f64).round() as i64)
    }
}

pub fn evolved_seed(seed: u64, time: EvolutionTime) -> u64 {
    let tick = time.tick as u64;
    splitmix64(
        seed ^ tick.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (time.ticks_per_day as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9),
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetEvolutionModel {
    pub seed: u64,
    pub profile: PlanetVisualProfile,
    pub rotation_period_hours: f32,
    pub surface_gravity_m_s2: f32,
    pub wind_velocity_scale_mps: f32,
    pub ocean_velocity_scale_mps: f32,
    pub cloud_velocity_scale_mps: f32,
    pub water_density_kg_m3: f32,
    pub atmosphere_density_kg_m3: f32,
    pub surface_pressure_bar: f32,
    pub magnetic_field_microtesla: f32,
    pub magnetosphere_strength: f32,
    pub cyclone_potential: f32,
}

impl PlanetEvolutionModel {
    pub fn from_seed(seed: u64) -> Self {
        Self::from_profile(PlanetVisualProfile::from_seed(seed))
    }

    pub fn from_profile(profile: PlanetVisualProfile) -> Self {
        let seed = profile.seed;
        let radius_earth = if profile.radius_earth > 0.0 {
            profile.radius_earth
        } else {
            profile.radius_km.max(1) as f32 / 6_371.0
        };
        let density_earth = if profile.density_earth > 0.0 {
            profile.density_earth
        } else {
            clamp(
                0.70 + profile.gravity_g * 0.30 / radius_earth.max(0.20),
                0.35,
                2.3,
            )
        };
        let gravity_g = if profile.gravity_g > 0.0 {
            profile.gravity_g
        } else {
            clamp(density_earth * radius_earth, 0.03, 4.8)
        };
        let warm = smoothstep(-10.0, 45.0, profile.temperature_c as f32);
        let hot = smoothstep(35.0, 190.0, profile.temperature_c as f32);
        let ice = clamp01(
            profile.ice_fraction + smoothstep(10.0, -80.0, profile.temperature_c as f32) * 0.35,
        );
        let gas_or_storm = profile_text_contains(
            &profile,
            &["gas", "jupiter", "saturn", "neptune", "giant", "storm"],
        );
        let rotation_jitter = hash2(17, 83, seed);
        let rotation_period_hours = if gas_or_storm {
            clamp(
                7.5 + rotation_jitter * 13.0 + radius_earth.sqrt() * 0.7,
                6.5,
                24.0,
            )
        } else {
            clamp(
                10.0 + rotation_jitter * 42.0 + radius_earth * 2.0 - gravity_g * 1.5,
                7.0,
                120.0,
            )
        };
        let atmosphere_density_kg_m3 = clamp(
            1.225
                * profile.atmosphere_density.max(0.01)
                * (0.65 + gravity_g * 0.35)
                * (1.0 - hot * 0.18 + ice * 0.10),
            0.002,
            12.0,
        );
        let surface_pressure_bar = clamp(
            profile.atmosphere_density * (0.56 + gravity_g * 0.44) + hot * 0.22,
            0.001,
            24.0,
        );
        let salinity = clamp01(
            0.38 + profile.ocean_fraction * 0.28 + profile.volcanic_activity * 0.16 + hot * 0.12
                - profile.ice_fraction * 0.10,
        );
        let water_density_kg_m3 = clamp(
            992.0 + salinity * 56.0 + ice * 20.0 - hot * 24.0 + gravity_g * 7.0,
            920.0,
            1_220.0,
        );
        let spin = 24.0 / rotation_period_hours;
        let wind_velocity_scale_mps = clamp(
            2.0 + profile.atmosphere_density * 26.0
                + profile.cloud_density * 18.0
                + warm * 8.0
                + hot * 12.0
                + spin * 9.0,
            0.4,
            if gas_or_storm { 180.0 } else { 85.0 },
        );
        let ocean_velocity_scale_mps = clamp(
            0.05 + profile.ocean_fraction * 1.7
                + profile.atmosphere_density * 0.35
                + spin.sqrt() * 0.45
                + profile.volcanic_activity * 0.16
                - profile.ice_fraction * 0.45,
            0.0,
            5.2,
        );
        let cloud_velocity_scale_mps = clamp(
            wind_velocity_scale_mps * (0.78 + profile.cloud_density * 0.36),
            0.2,
            220.0,
        );
        let core_bonus = if profile_text_contains(&profile, &["iron", "metal", "mercury"]) {
            0.24
        } else {
            0.0
        };
        let dynamo = clamp01(
            density_earth * 0.32
                + gravity_g * 0.22
                + spin * 0.24
                + profile.volcanic_activity * 0.12
                + core_bonus
                - hot * 0.10,
        );
        let magnetic_field_microtesla =
            clamp(3.0 + dynamo * 78.0 + hash2(29, 71, seed) * 9.0, 0.5, 140.0);
        let magnetosphere_strength = clamp01(magnetic_field_microtesla / 80.0 + dynamo * 0.16);
        let cyclone_potential = clamp01(
            profile.cloud_density * 0.34
                + profile.atmosphere_density * 0.26
                + profile.ocean_fraction * 0.22
                + warm * 0.16
                + gas_or_storm as u8 as f32 * 0.28
                - profile.ice_fraction * 0.18,
        );

        Self {
            seed,
            profile,
            rotation_period_hours,
            surface_gravity_m_s2: gravity_g * 9.80665,
            wind_velocity_scale_mps,
            ocean_velocity_scale_mps,
            cloud_velocity_scale_mps,
            water_density_kg_m3,
            atmosphere_density_kg_m3,
            surface_pressure_bar,
            magnetic_field_microtesla,
            magnetosphere_strength,
            cyclone_potential,
        }
    }

    pub fn advance_seed(&self, time: EvolutionTime) -> u64 {
        evolved_seed(self.seed, time)
    }

    pub fn snapshot_at(&self, time: EvolutionTime) -> PlanetClimateSnapshot {
        PlanetClimateSnapshot::new(self, time)
    }

    pub fn sample(&self, u: f32, v: f32, time: EvolutionTime) -> PlanetClimateSample {
        self.snapshot_at(time).sample(u, v)
    }

    pub fn flow_at(&self, u: f32, v: f32, time: EvolutionTime) -> FlowSample {
        self.sample(u, v, time).flow
    }

    pub fn density_at(&self, u: f32, v: f32, time: EvolutionTime) -> DensitySample {
        self.sample(u, v, time).density
    }

    pub fn magnetism_at(&self, u: f32, v: f32, time: EvolutionTime) -> MagnetismSample {
        self.sample(u, v, time).magnetism
    }

    pub fn cyclone_at(&self, u: f32, v: f32, time: EvolutionTime) -> CycloneSample {
        self.sample(u, v, time).cyclone
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetClimateSnapshot {
    pub seed: u64,
    pub evolved_seed: u64,
    pub time: EvolutionTime,
    pub day_phase: f32,
    pub seasonal_phase: f32,
    pub rotation_phase: f32,
    pub cyclone_systems: Vec<CycloneSystem>,
    model: PlanetEvolutionModel,
}

impl PlanetClimateSnapshot {
    fn new(model: &PlanetEvolutionModel, time: EvolutionTime) -> Self {
        let days = time.days() as f32;
        let evolved_seed = model.advance_seed(time);
        let rotation_days = model.rotation_period_hours.max(0.1) / 24.0;
        let day_phase = fract(days / rotation_days);
        let seasonal_phase = fract(days / seasonal_period_days(model.seed));
        let rotation_phase = fract(days * 24.0 / model.rotation_period_hours.max(0.1));
        let cyclone_systems = cyclone_systems_for(model, time, day_phase, seasonal_phase);

        Self {
            seed: model.seed,
            evolved_seed,
            time,
            day_phase,
            seasonal_phase,
            rotation_phase,
            cyclone_systems,
            model: model.clone(),
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> PlanetClimateSample {
        let u = u.rem_euclid(1.0);
        let v = clamp01(v);
        let lat_signed = (0.5 - v) * 2.0;
        let lat_abs = lat_signed.abs();
        let lat_rad = lat_signed * PI * 0.5;
        let phase_u = (u + self.rotation_phase).rem_euclid(1.0);
        let thermal = self.thermal_field(phase_u, v);
        let pressure = self.pressure_field(phase_u, v);
        let pressure_e = self.pressure_field(phase_u + 0.004, v);
        let pressure_w = self.pressure_field(phase_u - 0.004, v);
        let pressure_n = self.pressure_field(phase_u, v - 0.004);
        let pressure_s = self.pressure_field(phase_u, v + 0.004);
        let dpdu = pressure_e - pressure_w;
        let dpdv = pressure_s - pressure_n;
        let stream = self.stream_field(phase_u, v);
        let cloud_base = self.cloud_field(phase_u, v);
        let cyclone = self.cyclone_sample(u, v);
        let coriolis = lat_rad.sin() * (24.0 / self.model.rotation_period_hours.max(0.1));
        let jet = (lat_rad * 5.0 + stream * 2.0 + self.seasonal_phase * PI * 2.0).sin();
        let trade = PhysicsVector2::new(
            jet * 0.52 + (thermal - 0.5) * 0.20,
            -lat_signed * 0.22 - dpdu * 0.75,
        );
        let pressure_flow =
            PhysicsVector2::new(-dpdv * coriolis.signum(), dpdu * coriolis.signum());
        let cyclone_wind = vector_scale(
            cyclone.tangent,
            cyclone.influence * cyclone.intensity * 1.25,
        );
        let wind_dir =
            normalize_or_zero(vector_add(vector_add(trade, pressure_flow), cyclone_wind));
        let wind_speed_mps = clamp(
            self.model.wind_velocity_scale_mps
                * (0.22
                    + pressure_gradient(dpdu, dpdv) * 0.48
                    + stream * 0.16
                    + cyclone.influence * 0.62)
                * (1.0 - smoothstep(0.86, 1.0, lat_abs) * 0.28),
            0.0,
            self.model.wind_velocity_scale_mps * 1.55,
        );
        let cloud_flow_dir = normalize_or_zero(vector_add(
            vector_scale(wind_dir, 0.82),
            vector_scale(cyclone.tangent, cyclone.influence * 0.42),
        ));
        let cloud_speed_mps = clamp(
            self.model.cloud_velocity_scale_mps
                * (0.28 + cloud_base * 0.34 + cyclone.influence * 0.60),
            0.0,
            self.model.cloud_velocity_scale_mps * 1.65,
        );
        let gyre = PhysicsVector2::new(
            (lat_rad * 2.0).cos() * 0.50 + (stream - 0.5) * 0.30,
            -lat_signed * 0.22 + (thermal - 0.5) * 0.18,
        );
        let ocean_dir = normalize_or_zero(vector_add(
            gyre,
            vector_scale(pressure_flow, 0.8 + self.model.profile.ocean_fraction),
        ));
        let ocean_speed_mps = clamp(
            self.model.ocean_velocity_scale_mps
                * (0.20
                    + stream * 0.34
                    + pressure_gradient(dpdu, dpdv) * 0.35
                    + cyclone.influence * 0.16)
                * (1.0 - self.model.profile.ice_fraction.clamp(0.0, 0.9) * 0.55),
            0.0,
            self.model.ocean_velocity_scale_mps * 1.45,
        );
        let cloud_density = clamp01(
            self.model.profile.cloud_density * (0.42 + cloud_base * 0.78)
                + cyclone.cloud_wall * 0.45
                + smoothstep(0.12, 0.72, thermal) * 0.08,
        );
        let humidity = clamp01(
            self.model.profile.ocean_fraction * 0.48
                + cloud_density * 0.35
                + (1.0 - lat_abs) * 0.10
                + cyclone.inflow * 0.12,
        );
        let density = self.density_sample(v, thermal, pressure, humidity);
        let magnetism = self.magnetism_sample(u, v, lat_abs);

        PlanetClimateSample {
            u,
            v,
            flow: FlowSample {
                wind_mps: vector_scale(wind_dir, wind_speed_mps),
                ocean_current_mps: vector_scale(ocean_dir, ocean_speed_mps),
                cloud_flow_mps: vector_scale(cloud_flow_dir, cloud_speed_mps),
                wind_speed_mps,
                ocean_current_speed_mps: ocean_speed_mps,
                cloud_speed_mps,
                coriolis,
                shear: clamp01(pressure_gradient(dpdu, dpdv) * 2.2 + cyclone.influence * 0.28),
            },
            density,
            magnetism,
            cyclone,
            cloud_density,
            humidity,
            thermal,
            pressure,
        }
    }

    pub fn flow_at(&self, u: f32, v: f32) -> FlowSample {
        self.sample(u, v).flow
    }

    pub fn density_at(&self, u: f32, v: f32) -> DensitySample {
        self.sample(u, v).density
    }

    pub fn magnetism_at(&self, u: f32, v: f32) -> MagnetismSample {
        self.sample(u, v).magnetism
    }

    pub fn cyclone_at(&self, u: f32, v: f32) -> CycloneSample {
        self.sample(u, v).cyclone
    }

    fn pressure_field(&self, u: f32, v: f32) -> f32 {
        let days = self.time.days() as f32;
        let planetary = fbm_periodic(
            u * 1.25 + self.day_phase * 0.22,
            v * 1.05 + self.seasonal_phase * 0.08,
            9,
            4,
            self.seed + 10_101,
            0.54,
        );
        let waves = ((u + days * 0.018) * PI * 2.0 + v * PI * 5.0).sin() * 0.5 + 0.5;
        clamp01(planetary * 0.72 + waves * 0.18 + self.model.profile.atmosphere_density * 0.10)
    }

    fn stream_field(&self, u: f32, v: f32) -> f32 {
        let days = self.time.days() as f32;
        fbm_periodic(
            u * 2.0 + days * 0.010,
            v * 1.6 - days * 0.004,
            16,
            4,
            self.seed + 20_303,
            0.52,
        )
    }

    fn thermal_field(&self, u: f32, v: f32) -> f32 {
        let lat = ((v - 0.5) * 2.0).abs();
        let daylight = ((u + self.day_phase).rem_euclid(1.0) * PI * 2.0).cos() * 0.5 + 0.5;
        let season = (self.seasonal_phase * PI * 2.0).sin()
            * (0.08 + self.model.profile.ice_fraction * 0.10);
        let weather = fbm_periodic(
            u * 1.7 + self.seasonal_phase * 0.28,
            v * 1.3 - self.day_phase * 0.10,
            12,
            3,
            self.seed + 30_707,
            0.48,
        );
        clamp01((1.0 - lat).powf(0.72) * 0.48 + daylight * 0.18 + weather * 0.24 + season + 0.05)
    }

    fn cloud_field(&self, u: f32, v: f32) -> f32 {
        let days = self.time.days() as f32;
        let streaks = fbm_periodic(
            u * 3.1 + days * 0.026,
            v * 1.8 + days * 0.008,
            24,
            4,
            self.seed + 40_909,
            0.58,
        );
        let cells = fbm_periodic(
            u * 8.0 - days * 0.034,
            v * 4.0 + days * 0.011,
            48,
            3,
            self.seed + 41_003,
            0.50,
        );
        clamp01(streaks * 0.68 + cells * 0.32)
    }

    fn density_sample(&self, v: f32, thermal: f32, pressure: f32, humidity: f32) -> DensitySample {
        let lat = ((v - 0.5) * 2.0).abs();
        let cold_pool = smoothstep(0.45, 0.95, lat) * (1.0 - thermal * 0.45);
        let water_density_kg_m3 = clamp(
            self.model.water_density_kg_m3 + cold_pool * 18.0 - thermal * 16.0 + humidity * 5.0,
            900.0,
            1_240.0,
        );
        let atmosphere_density_kg_m3 = clamp(
            self.model.atmosphere_density_kg_m3
                * (0.84 + pressure * 0.30 - thermal * 0.08 + cold_pool * 0.08),
            0.001,
            16.0,
        );
        let surface_pressure_bar = clamp(
            self.model.surface_pressure_bar * (0.82 + pressure * 0.34 + cold_pool * 0.07),
            0.0005,
            32.0,
        );

        DensitySample {
            water_density_kg_m3,
            atmosphere_density_kg_m3,
            surface_pressure_bar,
            humidity,
        }
    }

    fn magnetism_sample(&self, u: f32, v: f32, lat_abs: f32) -> MagnetismSample {
        let anomaly = fbm_periodic(
            u * 2.4 + self.seasonal_phase * 0.07,
            v * 2.1 - self.day_phase * 0.05,
            18,
            3,
            self.seed + 50_207,
            0.50,
        ) - 0.5;
        let dipole = 0.74 + lat_abs.powf(1.8) * 0.58;
        let solar_buffet = 0.92 + (self.day_phase * PI * 2.0 + u * PI).sin() * 0.05;
        let magnetic_field_microtesla = clamp(
            self.model.magnetic_field_microtesla * dipole * solar_buffet + anomaly * 9.0,
            0.1,
            180.0,
        );
        let aurora_power = clamp01(
            self.model.magnetosphere_strength
                * smoothstep(0.54, 0.95, lat_abs)
                * (0.72 + anomaly.abs() * 0.56)
                * (0.70 + self.model.profile.atmosphere_density * 0.18),
        );

        MagnetismSample {
            magnetic_field_microtesla,
            magnetosphere_strength: self.model.magnetosphere_strength,
            aurora_power,
            anomaly,
        }
    }

    fn cyclone_sample(&self, u: f32, v: f32) -> CycloneSample {
        let mut nearest = None;
        let mut influence = 0.0;
        let mut cloud_wall = 0.0;
        let mut inflow = 0.0;
        let mut tangent = PhysicsVector2::ZERO;
        let mut pressure_drop = 0.0;
        let mut intensity: f32 = 0.0;

        for system in &self.cyclone_systems {
            let dx = wrap_delta(u - system.center_u);
            let dy = v - system.center_v;
            let lat_scale = (((v + system.center_v) * 0.5 - 0.5) * PI)
                .cos()
                .abs()
                .max(0.18);
            let distance = ((dx * lat_scale).powi(2) + dy.powi(2)).sqrt();
            let local = smoothstep(system.radius_uv, 0.0, distance);
            if local <= 0.0 {
                continue;
            }
            let eye = smoothstep(system.radius_uv * 0.12, system.radius_uv * 0.34, distance);
            let wall = local * eye;
            influence += local * system.intensity;
            cloud_wall += wall * system.intensity;
            inflow +=
                smoothstep(system.radius_uv, system.radius_uv * 0.18, distance) * system.intensity;
            pressure_drop += system.pressure_drop_bar * local;
            intensity = intensity.max(system.intensity * local);
            nearest = Some(system.id);
            let radial = normalize_or_zero(PhysicsVector2::new(dx * lat_scale, dy));
            let spin = system.rotation_sign;
            let swirl = PhysicsVector2::new(-radial.y * spin, radial.x * spin);
            tangent = vector_add(tangent, vector_scale(swirl, local * system.intensity));
        }

        CycloneSample {
            nearest_system_id: nearest,
            influence: clamp01(influence),
            cloud_wall: clamp01(cloud_wall),
            inflow: clamp01(inflow),
            tangent: normalize_or_zero(tangent),
            pressure_drop_bar: pressure_drop.max(0.0),
            intensity: clamp01(intensity),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanetClimateSample {
    pub u: f32,
    pub v: f32,
    pub flow: FlowSample,
    pub density: DensitySample,
    pub magnetism: MagnetismSample,
    pub cyclone: CycloneSample,
    pub cloud_density: f32,
    pub humidity: f32,
    pub thermal: f32,
    pub pressure: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSample {
    pub wind_mps: PhysicsVector2,
    pub ocean_current_mps: PhysicsVector2,
    pub cloud_flow_mps: PhysicsVector2,
    pub wind_speed_mps: f32,
    pub ocean_current_speed_mps: f32,
    pub cloud_speed_mps: f32,
    pub coriolis: f32,
    pub shear: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DensitySample {
    pub water_density_kg_m3: f32,
    pub atmosphere_density_kg_m3: f32,
    pub surface_pressure_bar: f32,
    pub humidity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MagnetismSample {
    pub magnetic_field_microtesla: f32,
    pub magnetosphere_strength: f32,
    pub aurora_power: f32,
    pub anomaly: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CycloneSample {
    pub nearest_system_id: Option<u8>,
    pub influence: f32,
    pub cloud_wall: f32,
    pub inflow: f32,
    pub tangent: PhysicsVector2,
    pub pressure_drop_bar: f32,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CycloneSystem {
    pub id: u8,
    pub center_u: f32,
    pub center_v: f32,
    pub radius_uv: f32,
    pub intensity: f32,
    pub rotation_sign: f32,
    pub drift_u_per_day: f32,
    pub pressure_drop_bar: f32,
}

fn cyclone_systems_for(
    model: &PlanetEvolutionModel,
    time: EvolutionTime,
    day_phase: f32,
    seasonal_phase: f32,
) -> Vec<CycloneSystem> {
    let base_count = (model.cyclone_potential * 5.0).round() as usize;
    let gas_bonus = if profile_text_contains(
        &model.profile,
        &["gas", "jupiter", "saturn", "neptune", "giant"],
    ) {
        2
    } else {
        0
    };
    let count = (base_count + gas_bonus).clamp(0, 8);
    let days = time.days() as f32;
    let mut systems = Vec::with_capacity(count);

    for i in 0..count {
        let idx = i as i32;
        let seed = model.seed + 60_000 + i as u64 * 997;
        let hemisphere = if hash2(idx, 11, seed) < 0.5 {
            -1.0
        } else {
            1.0
        };
        let latitude = hemisphere * (0.16 + hash2(idx, 19, seed) * 0.46);
        let center_v_base = clamp01(0.5 - latitude * 0.5);
        let drift_u_per_day = (0.002 + hash2(idx, 23, seed) * 0.018) * hemisphere.signum();
        let center_u =
            (hash2(idx, 29, seed) + days * drift_u_per_day + seasonal_phase * 0.12).rem_euclid(1.0);
        let wobble = ((days * (0.03 + hash2(idx, 31, seed) * 0.04) + day_phase) * PI * 2.0).sin();
        let center_v = clamp(center_v_base + wobble * 0.035, 0.06, 0.94);
        let radius_uv = clamp(
            0.045 + model.cyclone_potential * 0.075 + hash2(idx, 37, seed) * 0.055,
            0.035,
            0.20,
        );
        let intensity =
            clamp01(0.36 + model.cyclone_potential * 0.48 + hash2(idx, 41, seed) * 0.28);
        let pressure_drop_bar = model.surface_pressure_bar * (0.012 + intensity * 0.055);

        systems.push(CycloneSystem {
            id: i as u8,
            center_u,
            center_v,
            radius_uv,
            intensity,
            rotation_sign: hemisphere,
            drift_u_per_day,
            pressure_drop_bar,
        });
    }

    systems
}

fn seasonal_period_days(seed: u64) -> f32 {
    160.0 + hash2(3, 5, seed) * 540.0
}

fn profile_text_contains(profile: &PlanetVisualProfile, needles: &[&str]) -> bool {
    needles.iter().any(|needle| {
        let needle = needle.to_ascii_lowercase();
        profile.planet_class.to_ascii_lowercase().contains(&needle)
            || profile.archetype_key.to_ascii_lowercase().contains(&needle)
            || profile.class_key.to_ascii_lowercase().contains(&needle)
            || profile
                .modifier_keys
                .iter()
                .any(|modifier| modifier.to_ascii_lowercase().contains(&needle))
    })
}

fn fbm_periodic(
    u: f32,
    v: f32,
    base_cells: i32,
    octaves: usize,
    seed: u64,
    persistence: f32,
) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut max_amplitude = 0.0;
    let mut cells = base_cells.max(1);

    for octave in 0..octaves.max(1) {
        total += value_noise_periodic(u, v, cells, (cells / 2).max(2), seed + octave as u64 * 101)
            * amplitude;
        max_amplitude += amplitude;
        amplitude *= persistence;
        cells *= 2;
    }

    total / max_amplitude.max(f32::EPSILON)
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
    let n = splitmix64(
        seed ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9),
    );
    ((n >> 40) as f32) / ((1_u64 << 24) as f32)
}

fn splitmix64(mut n: u64) -> u64 {
    n ^= n >> 30;
    n = n.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    n ^= n >> 27;
    n = n.wrapping_mul(0x94D0_49BB_1331_11EB);
    n ^ (n >> 31)
}

fn bilerp(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> f32 {
    let ab = a + (b - a) * tx;
    let cd = c + (d - c) * tx;
    ab + (cd - ab) * ty
}

fn pressure_gradient(dpdu: f32, dpdv: f32) -> f32 {
    (dpdu * dpdu + dpdv * dpdv).sqrt()
}

fn vector_add(a: PhysicsVector2, b: PhysicsVector2) -> PhysicsVector2 {
    PhysicsVector2::new(a.x + b.x, a.y + b.y)
}

fn vector_scale(v: PhysicsVector2, scale: f32) -> PhysicsVector2 {
    PhysicsVector2::new(v.x * scale, v.y * scale)
}

fn normalize_or_zero(v: PhysicsVector2) -> PhysicsVector2 {
    let len = (v.x * v.x + v.y * v.y).sqrt();
    if len <= f32::EPSILON {
        PhysicsVector2::ZERO
    } else {
        PhysicsVector2::new(v.x / len, v.y / len)
    }
}

fn wrap_delta(delta: f32) -> f32 {
    let delta = delta.rem_euclid(1.0);
    if delta > 0.5 {
        delta - 1.0
    } else {
        delta
    }
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

fn fract(value: f32) -> f32 {
    value - value.floor()
}

fn clamp01(value: f32) -> f32 {
    clamp(value, 0.0, 1.0)
}

fn clamp(value: f32, low: f32, high: f32) -> f32 {
    value.max(low).min(high)
}
