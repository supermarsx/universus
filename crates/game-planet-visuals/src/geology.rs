use crate::{
    profile::{mix_seed, stable_key_hash, ProfileRng},
    PlanetVisualProfile,
};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

const EARTH_RADIUS_KM: f32 = 6_371.0;
const EARTH_SURFACE_HEAT_FLOW_MW_M2: f32 = 87.0;
const RIFT_CHAIN_COUNT: usize = 4;
const RIFT_CHAIN_POINTS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CrustDomain {
    Continental,
    Oceanic,
    Transitional,
    IceShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DensityLayerKind {
    SurfaceSediment,
    ContinentalCrust,
    OceanicCrust,
    LithosphereMantle,
    AsthenosphereMantle,
    LowerMantle,
    MetallicCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RadioactiveIsotope {
    Uranium238,
    Uranium235,
    Thorium232,
    Potassium40,
}

impl RadioactiveIsotope {
    pub const ALL: [Self; 4] = [
        Self::Uranium238,
        Self::Uranium235,
        Self::Thorium232,
        Self::Potassium40,
    ];

    pub const fn half_life_ga(self) -> f32 {
        match self {
            Self::Uranium238 => 4.468,
            Self::Uranium235 => 0.704,
            Self::Thorium232 => 14.05,
            Self::Potassium40 => 1.248,
        }
    }

    pub const fn present_heat_weight(self) -> f32 {
        match self {
            Self::Uranium238 => 0.40,
            Self::Uranium235 => 0.015,
            Self::Thorium232 => 0.42,
            Self::Potassium40 => 0.165,
        }
    }

    pub fn remaining_parent_fraction(self, age_ga: f32) -> f32 {
        2.0_f32.powf(-age_ga.max(0.0) / self.half_life_ga())
    }

    pub fn heat_fraction_since_formation(self, age_ga: f32) -> f32 {
        let age_ga = age_ga.max(0.0);
        let present = self.remaining_parent_fraction(age_ga);
        let initial = 1.0;
        if initial <= f32::EPSILON {
            0.0
        } else {
            present / initial
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DensityLayer {
    pub kind: DensityLayerKind,
    pub top_depth_km: f32,
    pub bottom_depth_km: f32,
    pub density_kg_m3: f32,
    pub thermal_conductivity_w_m_k: f32,
}

impl DensityLayer {
    pub fn contains_depth(self, depth_km: f32) -> bool {
        self.top_depth_km <= depth_km && depth_km < self.bottom_depth_km
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiftNode {
    pub u: f32,
    pub v: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiftChain {
    pub id: u8,
    pub oceanic: bool,
    pub width: f32,
    pub intensity: f32,
    pub nodes: [RiftNode; RIFT_CHAIN_POINTS],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioactiveHeatBudget {
    pub age_ga: f32,
    pub present_fraction: f32,
    pub heat_flow_mw_m2: f32,
    pub mantle_heat_fraction: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeologySample {
    pub crust_domain: CrustDomain,
    pub terrain_elevation_m: f32,
    pub ocean_depth_m: f32,
    pub crust_thickness_km: f32,
    pub rift: f32,
    pub oceanic_rift: f32,
    pub continental_rift: f32,
    pub basin: f32,
    pub trench: f32,
    pub uplift: f32,
    pub volcanic_heat: f32,
    pub radioactive_heat_fraction: f32,
    pub surface_heat_flow_mw_m2: f32,
    pub crust_density_kg_m3: f32,
    pub mantle_density_kg_m3: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeologyModel {
    pub seed: u64,
    pub radius_earth: f32,
    pub density_earth: f32,
    pub ocean_fraction: f32,
    pub tectonic_activity: f32,
    pub volcanic_activity: f32,
    pub mean_crust_thickness_km: f32,
    pub mean_oceanic_crust_thickness_km: f32,
    pub radiogenic_budget: RadioactiveHeatBudget,
    pub density_layers: Vec<DensityLayer>,
    pub rift_chains: Vec<RiftChain>,
}

impl GeologyModel {
    pub fn from_profile(profile: &PlanetVisualProfile) -> Self {
        let radius_earth = if profile.radius_earth > 0.0 {
            profile.radius_earth
        } else {
            profile.radius_km.max(1) as f32 / EARTH_RADIUS_KM
        };
        let density_earth = profile.density_earth.max(0.25);
        let ocean_fraction = profile.ocean_fraction.clamp(0.0, 1.0);
        let volcanic_activity = profile.volcanic_activity.clamp(0.0, 1.0);
        let heat_seed = mix_seed(profile.seed, stable_key_hash("geology.age"));
        let age_ga = 0.65 + seeded_unit(heat_seed) * 4.25;
        let radiogenic_budget = radioactive_heat_budget(age_ga, profile);
        let tectonic_activity = clamp01(
            0.18 + volcanic_activity * 0.44
                + ocean_fraction * 0.16
                + radiogenic_budget.mantle_heat_fraction * 0.28
                + (density_earth - 0.85) * 0.10
                - profile.ice_fraction.clamp(0.0, 1.0) * 0.12,
        );
        let mean_crust_thickness_km = clamp(
            39.0 / radius_earth.max(0.35).sqrt()
                + (1.0 - tectonic_activity) * 12.0
                + (1.0 - ocean_fraction) * 8.0,
            14.0,
            82.0,
        );
        let mean_oceanic_crust_thickness_km = clamp(
            mean_crust_thickness_km * (0.18 + ocean_fraction * 0.08 + tectonic_activity * 0.08),
            5.0,
            18.0,
        );
        let density_layers = density_layers_for(
            radius_earth,
            density_earth,
            ocean_fraction,
            mean_crust_thickness_km,
            mean_oceanic_crust_thickness_km,
        );
        let rift_chains = build_rift_chains(profile.seed, ocean_fraction, tectonic_activity);

        Self {
            seed: profile.seed,
            radius_earth,
            density_earth,
            ocean_fraction,
            tectonic_activity,
            volcanic_activity,
            mean_crust_thickness_km,
            mean_oceanic_crust_thickness_km,
            radiogenic_budget,
            density_layers,
            rift_chains,
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> GeologySample {
        let u = u.rem_euclid(1.0);
        let v = v.clamp(0.0, 1.0);
        let lat = ((v - 0.5) * 2.0).abs();
        let continentality = self.continentality_field(u, v);
        let ocean_threshold = (1.0 - self.ocean_fraction).clamp(0.02, 0.92);
        let ocean_bias = smoothstep(
            ocean_threshold,
            (ocean_threshold + 0.28).min(0.98),
            1.0 - continentality,
        ) * smoothstep(0.02, 0.18, self.ocean_fraction);
        let oceanic_domain = ocean_bias > 0.52;
        let rift = self.rift_field(u, v);
        let oceanic_rift = rift * ocean_bias;
        let continental_rift = rift * (1.0 - ocean_bias);
        let basin = self.basin_field(u, v, ocean_bias, oceanic_rift);
        let trench = self.trench_field(u, v, ocean_bias);
        let uplift = clamp01(
            self.uplift_field(u, v) * (1.0 - ocean_bias * 0.55)
                + continental_rift * 0.34
                + trench * 0.46,
        );
        let volcanic_heat = clamp01(
            self.volcanic_activity * 0.55
                + rift * (0.18 + self.tectonic_activity * 0.35)
                + trench * 0.24
                + self.hotspot_field(u, v) * self.volcanic_activity * 0.30,
        );
        let crust_thickness_km = if oceanic_domain {
            clamp(
                self.mean_oceanic_crust_thickness_km
                    * (1.15 - oceanic_rift * 0.42 + basin * 0.24 - trench * 0.10),
                3.5,
                22.0,
            )
        } else {
            clamp(
                self.mean_crust_thickness_km
                    * (0.78 + continentality * 0.34 + uplift * 0.16 - continental_rift * 0.20),
                12.0,
                92.0,
            )
        };
        let crust_density_kg_m3 = crust_density(oceanic_domain, self.density_earth, volcanic_heat);
        let mantle_density_kg_m3 = mantle_density(self.density_earth, crust_thickness_km);
        let radioactive_heat_fraction = self.radiogenic_budget.present_fraction;
        let surface_heat_flow_mw_m2 = clamp(
            self.radiogenic_budget.heat_flow_mw_m2
                * (0.62 + volcanic_heat * 0.55 + rift * 0.30 + trench * 0.20)
                * (1.0 - lat * 0.08),
            8.0,
            480.0,
        );
        let terrain_elevation_m = terrain_elevation_m(
            continentality,
            ocean_bias,
            basin,
            trench,
            uplift,
            rift,
            self.seed,
            u,
            v,
        );
        let ocean_depth_m = if oceanic_domain {
            clamp(
                2_700.0 + basin * 4_700.0 + trench * 3_200.0 - oceanic_rift * 1_250.0
                    + (1.0 - continentality) * 950.0,
                120.0,
                11_500.0,
            )
        } else {
            0.0
        };
        let crust_domain = if oceanic_domain {
            CrustDomain::Oceanic
        } else if self.ocean_fraction > 0.35 && (0.42..=0.58).contains(&ocean_bias) {
            CrustDomain::Transitional
        } else if self.ocean_fraction < 0.05 && self.mean_crust_thickness_km < 22.0 {
            CrustDomain::IceShell
        } else {
            CrustDomain::Continental
        };

        GeologySample {
            crust_domain,
            terrain_elevation_m,
            ocean_depth_m,
            crust_thickness_km,
            rift,
            oceanic_rift,
            continental_rift,
            basin,
            trench,
            uplift,
            volcanic_heat,
            radioactive_heat_fraction,
            surface_heat_flow_mw_m2,
            crust_density_kg_m3,
            mantle_density_kg_m3,
        }
    }

    pub fn density_at_depth_km(&self, depth_km: f32) -> DensityLayer {
        let depth_km = depth_km.max(0.0);
        self.density_layers
            .iter()
            .copied()
            .find(|layer| layer.contains_depth(depth_km))
            .unwrap_or_else(|| {
                *self
                    .density_layers
                    .last()
                    .expect("density layers are non-empty")
            })
    }

    fn continentality_field(&self, u: f32, v: f32) -> f32 {
        let broad = fbm_periodic(
            u * 0.92 + self.ocean_fraction * 0.11,
            v * 0.82 - self.tectonic_activity * 0.05,
            self.seed + 13_003,
            5,
            0.56,
        );
        let plates = fbm_periodic(
            u * 2.1 + broad * 0.11,
            v * 1.7 - broad * 0.07,
            self.seed + 13_101,
            4,
            0.52,
        );
        clamp01(broad * 0.68 + plates * 0.32)
    }

    fn rift_field(&self, u: f32, v: f32) -> f32 {
        let mut field: f32 = 0.0;
        for chain in &self.rift_chains {
            let distance = distance_to_chain(u, v, &chain.nodes);
            let core = smoothstep(chain.width, chain.width * 0.18, distance);
            let flank = smoothstep(chain.width * 2.4, chain.width * 0.65, distance) * 0.34;
            field = field.max((core + flank) * chain.intensity);
        }
        clamp01(field * self.tectonic_activity.max(0.18))
    }

    fn basin_field(&self, u: f32, v: f32, ocean_bias: f32, oceanic_rift: f32) -> f32 {
        let basins = fbm_periodic(
            u * 1.32 + oceanic_rift * 0.08,
            v * 1.10 - self.ocean_fraction * 0.09,
            self.seed + 13_211,
            5,
            0.54,
        );
        clamp01(smoothstep(0.34, 0.82, basins) * (0.45 + ocean_bias * 0.70) + oceanic_rift * 0.26)
    }

    fn trench_field(&self, u: f32, v: f32, ocean_bias: f32) -> f32 {
        let arcs = fbm_periodic(
            u * 2.8 + self.tectonic_activity * 0.13,
            v * 2.2 - self.ocean_fraction * 0.10,
            self.seed + 13_307,
            4,
            0.50,
        );
        let plate_edges = ridge(arcs);
        clamp01(plate_edges * ocean_bias * self.tectonic_activity * 1.25)
    }

    fn uplift_field(&self, u: f32, v: f32) -> f32 {
        let ridges = fbm_periodic(
            u * 2.4 + self.density_earth * 0.08,
            v * 1.9 - self.tectonic_activity * 0.07,
            self.seed + 13_401,
            5,
            0.51,
        );
        smoothstep(0.48, 0.92, ridge(ridges) * 0.55 + ridges * 0.45)
    }

    fn hotspot_field(&self, u: f32, v: f32) -> f32 {
        let mut field: f32 = 0.0;
        for index in 0..5 {
            let salt = self.seed + 13_503 + index * 97;
            let center_u = seeded_unit(mix_seed(salt, 11));
            let center_v = 0.18 + seeded_unit(mix_seed(salt, 23)) * 0.64;
            let radius = 0.035 + seeded_unit(mix_seed(salt, 37)) * 0.070;
            let d = torus_distance(u, v, center_u, center_v);
            field = field.max(smoothstep(radius, radius * 0.18, d));
        }
        field
    }
}

pub fn radioactive_heat_budget(
    age_ga: f32,
    profile: &PlanetVisualProfile,
) -> RadioactiveHeatBudget {
    let age_ga = age_ga.clamp(0.0, 8.0);
    let mut present_fraction = 0.0;
    for isotope in RadioactiveIsotope::ALL {
        present_fraction +=
            isotope.present_heat_weight() * isotope.heat_fraction_since_formation(age_ga);
    }
    let density_factor = profile.density_earth.max(0.35).sqrt();
    let volcanic_factor = 0.82 + profile.volcanic_activity.clamp(0.0, 1.0) * 0.48;
    let heat_flow_mw_m2 =
        EARTH_SURFACE_HEAT_FLOW_MW_M2 * present_fraction * density_factor * volcanic_factor;
    let mantle_heat_fraction = clamp01(present_fraction * 0.72 + volcanic_factor * 0.18);

    RadioactiveHeatBudget {
        age_ga,
        present_fraction,
        heat_flow_mw_m2,
        mantle_heat_fraction,
    }
}

pub fn density_layers_for_profile(profile: &PlanetVisualProfile) -> Vec<DensityLayer> {
    GeologyModel::from_profile(profile).density_layers
}

fn build_rift_chains(seed: u64, ocean_fraction: f32, tectonic_activity: f32) -> Vec<RiftChain> {
    let mut chains = Vec::with_capacity(RIFT_CHAIN_COUNT);
    let count = if tectonic_activity < 0.22 {
        2
    } else {
        RIFT_CHAIN_COUNT
    };

    for index in 0..count {
        let salt = stable_key_hash("geology.rift-chain") ^ index as u64;
        let mut rng = ProfileRng::new(mix_seed(seed, salt));
        let oceanic = rng.next_f32() < (0.28 + ocean_fraction * 0.58);
        let width = rng.range_f32(0.018, 0.052) * if oceanic { 1.15 } else { 0.92 };
        let intensity = clamp01(
            rng.range_f32(0.46, 1.0)
                * (0.55 + tectonic_activity * 0.70)
                * if oceanic {
                    1.0 + ocean_fraction * 0.16
                } else {
                    1.0
                },
        );
        let start_u = rng.next_f32();
        let start_v = rng.range_f32(0.14, 0.86);
        let drift_u = rng.range_f32(-0.18, 0.18);
        let drift_v = rng.range_f32(-0.22, 0.22);
        let phase = rng.range_f32(0.0, PI * 2.0);
        let wave = rng.range_f32(0.035, 0.115);
        let nodes = std::array::from_fn(|node_index| {
            let t = node_index as f32 / (RIFT_CHAIN_POINTS - 1) as f32;
            let arc = (phase + t * PI * 1.65).sin();
            RiftNode {
                u: (start_u + drift_u * (t - 0.5) + arc * wave).rem_euclid(1.0),
                v: clamp(
                    start_v + drift_v * (t - 0.5) + arc * wave * 0.72,
                    0.04,
                    0.96,
                ),
            }
        });
        chains.push(RiftChain {
            id: index as u8,
            oceanic,
            width,
            intensity,
            nodes,
        });
    }

    chains
}

fn density_layers_for(
    radius_earth: f32,
    density_earth: f32,
    ocean_fraction: f32,
    continental_crust_km: f32,
    oceanic_crust_km: f32,
) -> Vec<DensityLayer> {
    let radius_km = radius_earth.max(0.1) * EARTH_RADIUS_KM;
    let representative_crust =
        continental_crust_km * (1.0 - ocean_fraction) + oceanic_crust_km * ocean_fraction;
    let lithosphere_bottom = clamp(
        representative_crust + 82.0 * radius_earth.sqrt(),
        38.0,
        180.0,
    );
    let asthenosphere_bottom = clamp(
        lithosphere_bottom + 210.0 * radius_earth.sqrt(),
        140.0,
        520.0,
    );
    let lower_mantle_bottom =
        (radius_km * 0.53).clamp(asthenosphere_bottom + 80.0, radius_km * 0.76);
    let scale = density_earth.clamp(0.45, 2.2);
    let crust_density = 2_720.0 + (scale - 1.0) * 260.0 + ocean_fraction * 120.0;
    let ocean_crust_density = 2_940.0 + (scale - 1.0) * 230.0;
    let mantle_density_base = 3_340.0 + (scale - 1.0) * 420.0;

    vec![
        DensityLayer {
            kind: DensityLayerKind::SurfaceSediment,
            top_depth_km: 0.0,
            bottom_depth_km: 1.8 + ocean_fraction * 1.7,
            density_kg_m3: 1_850.0 + ocean_fraction * 210.0,
            thermal_conductivity_w_m_k: 1.7,
        },
        DensityLayer {
            kind: DensityLayerKind::ContinentalCrust,
            top_depth_km: 1.8 + ocean_fraction * 1.7,
            bottom_depth_km: representative_crust.max(5.0),
            density_kg_m3: crust_density,
            thermal_conductivity_w_m_k: 2.6,
        },
        DensityLayer {
            kind: DensityLayerKind::OceanicCrust,
            top_depth_km: representative_crust.max(5.0),
            bottom_depth_km: lithosphere_bottom,
            density_kg_m3: ocean_crust_density.max(crust_density + 80.0),
            thermal_conductivity_w_m_k: 2.2,
        },
        DensityLayer {
            kind: DensityLayerKind::LithosphereMantle,
            top_depth_km: lithosphere_bottom,
            bottom_depth_km: asthenosphere_bottom,
            density_kg_m3: mantle_density_base,
            thermal_conductivity_w_m_k: 3.3,
        },
        DensityLayer {
            kind: DensityLayerKind::AsthenosphereMantle,
            top_depth_km: asthenosphere_bottom,
            bottom_depth_km: lower_mantle_bottom,
            density_kg_m3: mantle_density_base + 320.0,
            thermal_conductivity_w_m_k: 4.1,
        },
        DensityLayer {
            kind: DensityLayerKind::LowerMantle,
            top_depth_km: lower_mantle_bottom,
            bottom_depth_km: radius_km * 0.84,
            density_kg_m3: mantle_density_base + 1_120.0,
            thermal_conductivity_w_m_k: 5.3,
        },
        DensityLayer {
            kind: DensityLayerKind::MetallicCore,
            top_depth_km: radius_km * 0.84,
            bottom_depth_km: radius_km,
            density_kg_m3: 7_600.0 + (scale - 1.0) * 1_650.0,
            thermal_conductivity_w_m_k: 42.0,
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn terrain_elevation_m(
    continentality: f32,
    ocean_bias: f32,
    basin: f32,
    trench: f32,
    uplift: f32,
    rift: f32,
    seed: u64,
    u: f32,
    v: f32,
) -> f32 {
    let rough = fbm_periodic(
        u * 4.0 + uplift * 0.08,
        v * 3.2 - rift * 0.06,
        seed + 13_607,
        4,
        0.48,
    );
    let continental_relief = 180.0 + continentality * 2_700.0 + uplift * 4_200.0 + rough * 950.0;
    let ocean_floor = -2_200.0 - basin * 4_100.0 - trench * 4_300.0 + rift * 1_350.0;
    continental_relief * (1.0 - ocean_bias) + ocean_floor * ocean_bias
}

fn crust_density(oceanic: bool, density_earth: f32, heat: f32) -> f32 {
    let base = if oceanic { 2_970.0 } else { 2_720.0 };
    clamp(
        base + (density_earth - 1.0) * 260.0 - heat * 45.0,
        2_350.0,
        3_280.0,
    )
}

fn mantle_density(density_earth: f32, crust_thickness_km: f32) -> f32 {
    clamp(
        3_340.0 + (density_earth - 1.0) * 420.0 + crust_thickness_km * 1.4,
        3_050.0,
        4_350.0,
    )
}

fn distance_to_chain(u: f32, v: f32, nodes: &[RiftNode; RIFT_CHAIN_POINTS]) -> f32 {
    nodes
        .windows(2)
        .map(|pair| distance_to_segment_wrapped(u, v, pair[0], pair[1]))
        .fold(f32::MAX, f32::min)
}

fn distance_to_segment_wrapped(u: f32, v: f32, a: RiftNode, b: RiftNode) -> f32 {
    let ax = a.u;
    let ay = a.v;
    let mut bx = b.u;
    if bx - ax > 0.5 {
        bx -= 1.0;
    } else if ax - bx > 0.5 {
        bx += 1.0;
    }
    let mut px = u;
    if px - ax > 0.5 {
        px -= 1.0;
    } else if ax - px > 0.5 {
        px += 1.0;
    }
    let abx = bx - ax;
    let aby = b.v - ay;
    let apx = px - ax;
    let apy = v - ay;
    let ab2 = abx * abx + aby * aby;
    let t = if ab2 <= f32::EPSILON {
        0.0
    } else {
        ((apx * abx + apy * aby) / ab2).clamp(0.0, 1.0)
    };
    let closest_x = ax + abx * t;
    let closest_y = ay + aby * t;
    ((px - closest_x).powi(2) + (v - closest_y).powi(2)).sqrt()
}

fn torus_distance(u: f32, v: f32, center_u: f32, center_v: f32) -> f32 {
    let du = (u - center_u + 0.5).rem_euclid(1.0) - 0.5;
    let dv = v - center_v;
    (du * du + dv * dv).sqrt()
}

fn fbm_periodic(u: f32, v: f32, seed: u64, octaves: u8, persistence: f32) -> f32 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut norm = 0.0;
    let mut frequency = 1.0;
    for octave in 0..octaves {
        total += value_noise_periodic(u * frequency, v * frequency, seed + u64::from(octave) * 101)
            * amplitude;
        norm += amplitude;
        amplitude *= persistence;
        frequency *= 2.0;
    }
    total / norm.max(f32::EPSILON)
}

fn value_noise_periodic(u: f32, v: f32, seed: u64) -> f32 {
    let x = u.floor() as i32;
    let y = v.floor() as i32;
    let fx = u - x as f32;
    let fy = v - y as f32;
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = lattice_hash(x, y, seed);
    let b = lattice_hash(x + 1, y, seed);
    let c = lattice_hash(x, y + 1, seed);
    let d = lattice_hash(x + 1, y + 1, seed);
    lerp(lerp(a, b, sx), lerp(c, d, sx), sy)
}

fn lattice_hash(x: i32, y: i32, seed: u64) -> f32 {
    let mixed = mix_seed(
        seed ^ (x as u64).wrapping_mul(0x9E37_79B1),
        (y as u64).wrapping_mul(0x85EB_CA77),
    );
    ((mixed >> 40) as f32) / ((1_u64 << 24) as f32)
}

fn seeded_unit(seed: u64) -> f32 {
    ((mix_seed(seed, 0xA076_1D64_78BD_642F) >> 40) as f32) / ((1_u64 << 24) as f32)
}

fn ridge(value: f32) -> f32 {
    1.0 - (value * 2.0 - 1.0).abs()
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn clamp(value: f32, low: f32, high: f32) -> f32 {
    value.clamp(low, high)
}
