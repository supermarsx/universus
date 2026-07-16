//! Deterministic planet profile generation.
//!
//! This module is intentionally self-contained. It does not import catalog or
//! modifier modules because those can be authored in parallel; it keeps stable
//! string keys and local lightweight enums so the generated data can be mapped
//! into richer catalog records later.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSeedInput {
    pub seed: u64,
    pub catalog_version: String,
    pub forced_archetype_key: Option<String>,
    pub modifier_budget: usize,
    pub allow_rare_modifiers: bool,
}

impl ProfileSeedInput {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            catalog_version: DEFAULT_CATALOG_VERSION.to_string(),
            forced_archetype_key: None,
            modifier_budget: DEFAULT_MODIFIER_BUDGET,
            allow_rare_modifiers: true,
        }
    }

    pub fn with_archetype_key(mut self, key: impl Into<String>) -> Self {
        self.forced_archetype_key = Some(key.into());
        self
    }

    pub fn with_modifier_budget(mut self, budget: usize) -> Self {
        self.modifier_budget = budget;
        self
    }

    pub fn without_rare_modifiers(mut self) -> Self {
        self.allow_rare_modifiers = false;
        self
    }
}

impl From<u64> for ProfileSeedInput {
    fn from(seed: u64) -> Self {
        Self::new(seed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedPlanetClass {
    BarrenRock,
    MercuryLike,
    MarsLike,
    VenusLike,
    DesertWorld,
    TemperateTerrestrial,
    OceanWorld,
    LowWaterWorld,
    MegaContinentWorld,
    ArchipelagoWorld,
    IceAgeWorld,
    GreenhouseWorld,
    PostApocalypticWorld,
    ActiveVolcanicWorld,
    DenseAtmosphereWorld,
    SwampJungleWorld,
    DesertDuneWorld,
    IceWorld,
    SnowballWorld,
    IceShellWorld,
    HydrocarbonWorld,
    CarbonWorld,
    IronWorld,
    ChthonianWorld,
    LavaMagmaWorld,
    SulfurIoLike,
    AcidCloudWorld,
    VolcanicWorld,
    SuperEarth,
    MiniNeptune,
    SubNeptune,
    WaterSteamHycean,
    GasDwarf,
    GasGiant,
    HotJupiter,
    ColdJupiter,
    SaturnLike,
    IceGiant,
    HeliumGiant,
    PuffyGiant,
    RoguePlanet,
    TidallyLockedEyeball,
    TwilightBeltWorld,
    CircumbinaryWorld,
    EccentricSeasonWorld,
    ProtoPlanet,
    DwarfAsteroidLike,
    Exomoon,
    CapturedMoon,
    ArtificialWorld,
    Ecumenopolis,
}

impl GeneratedPlanetClass {
    pub const fn key(self) -> &'static str {
        match self {
            Self::BarrenRock => "barren-rock",
            Self::MercuryLike => "mercury-like",
            Self::MarsLike => "mars-like",
            Self::VenusLike => "venus-like",
            Self::DesertWorld => "desert-world",
            Self::TemperateTerrestrial => "temperate-terrestrial",
            Self::OceanWorld => "ocean-world",
            Self::LowWaterWorld => "low-water",
            Self::MegaContinentWorld => "megacontinent",
            Self::ArchipelagoWorld => "archipelago",
            Self::IceAgeWorld => "ice-age",
            Self::GreenhouseWorld => "greenhouse",
            Self::PostApocalypticWorld => "post-apocalyptic",
            Self::ActiveVolcanicWorld => "active-volcanic",
            Self::DenseAtmosphereWorld => "dense-atmosphere",
            Self::SwampJungleWorld => "swamp-jungle",
            Self::DesertDuneWorld => "desert-dune",
            Self::IceWorld => "ice-world",
            Self::SnowballWorld => "snowball",
            Self::IceShellWorld => "ice-shell",
            Self::HydrocarbonWorld => "hydrocarbon-titan-like",
            Self::CarbonWorld => "carbon-world",
            Self::IronWorld => "iron-world",
            Self::ChthonianWorld => "chthonian",
            Self::LavaMagmaWorld => "lava-magma",
            Self::SulfurIoLike => "sulfur-io-like",
            Self::AcidCloudWorld => "acid-cloud",
            Self::VolcanicWorld => "volcanic-world",
            Self::SuperEarth => "super-earth",
            Self::MiniNeptune => "mini-neptune",
            Self::SubNeptune => "sub-neptune",
            Self::WaterSteamHycean => "water-steam-hycean",
            Self::GasDwarf => "gas-dwarf",
            Self::GasGiant => "gas-giant",
            Self::HotJupiter => "hot-jupiter",
            Self::ColdJupiter => "cold-jupiter",
            Self::SaturnLike => "saturn-like",
            Self::IceGiant => "ice-giant",
            Self::HeliumGiant => "helium-giant",
            Self::PuffyGiant => "puffy-giant",
            Self::RoguePlanet => "rogue-planet",
            Self::TidallyLockedEyeball => "tidally-locked-eyeball",
            Self::TwilightBeltWorld => "twilight-belt",
            Self::CircumbinaryWorld => "circumbinary",
            Self::EccentricSeasonWorld => "eccentric-season",
            Self::ProtoPlanet => "proto-planet",
            Self::DwarfAsteroidLike => "dwarf-asteroid-like",
            Self::Exomoon => "exomoon",
            Self::CapturedMoon => "captured-moon",
            Self::ArtificialWorld => "artificial",
            Self::Ecumenopolis => "ecumenopolis",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedPlanetSizeClass {
    Small,
    Medium,
    Large,
}

impl GeneratedPlanetSizeClass {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedPlanetScaleBand {
    Tiny,
    Compact,
    Standard,
    Broad,
    Massive,
    Colossal,
}

impl GeneratedPlanetScaleBand {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Tiny => "tiny",
            Self::Compact => "compact",
            Self::Standard => "standard",
            Self::Broad => "broad",
            Self::Massive => "massive",
            Self::Colossal => "colossal",
        }
    }

    pub const fn size_class(self) -> GeneratedPlanetSizeClass {
        match self {
            Self::Tiny | Self::Compact => GeneratedPlanetSizeClass::Small,
            Self::Standard | Self::Broad => GeneratedPlanetSizeClass::Medium,
            Self::Massive | Self::Colossal => GeneratedPlanetSizeClass::Large,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedPlanetProfile {
    pub seed: u64,
    pub algorithm: String,
    pub catalog_version: String,
    pub archetype_key: String,
    pub planet_class: GeneratedPlanetClass,
    pub class_key: String,
    pub size_class: GeneratedPlanetSizeClass,
    pub size_key: String,
    pub scale_band: GeneratedPlanetScaleBand,
    pub scale_key: String,
    pub radius_scale: f32,
    pub radius_km: i32,
    pub radius_earth: f32,
    pub mass_earth: f32,
    pub density_earth: f32,
    pub gravity_g: f32,
    pub temperature_c: i32,
    pub atmosphere: GeneratedAtmosphere,
    pub surface: GeneratedSurface,
    pub hydrosphere: GeneratedHydrosphere,
    pub rings: GeneratedRings,
    pub modifiers: Vec<GeneratedModifier>,
    pub palette_key: String,
    pub render_model_key: String,
}

impl GeneratedPlanetProfile {
    pub fn from_seed(seed: u64) -> Self {
        generate_planet_profile(ProfileSeedInput::new(seed))
    }

    pub fn legacy_planet_class_label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();

        if self.rings.present {
            parts.push("ringed");
        }

        if self.temperature_c <= -90 {
            parts.push("frozen");
        } else if self.temperature_c < 0 {
            parts.push("cold");
        } else if self.temperature_c <= 35 {
            parts.push("temperate");
        } else if self.temperature_c <= 120 {
            parts.push("hot");
        } else {
            parts.push("scorched");
        }

        if self.hydrosphere.ocean_fraction >= 0.55 {
            parts.push("ocean");
        } else if self.hydrosphere.ice_fraction >= 0.45 {
            parts.push("ice");
        } else if self.surface.volcanic_activity >= 0.55 {
            parts.push("volcanic");
        }

        parts.push(self.planet_class.key());
        parts.join(" ")
    }

    pub fn modifier_keys(&self) -> Vec<&str> {
        self.modifiers
            .iter()
            .map(|modifier| modifier.key.as_str())
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedAtmosphere {
    pub key: String,
    pub density: f32,
    pub pressure_bar: f32,
    pub greenhouse_factor: f32,
    pub cloud_density: f32,
    pub haze: f32,
    pub scattering_key: String,
    pub dominant_gases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedSurface {
    pub key: String,
    pub terrain_key: String,
    pub palette_key: String,
    pub albedo: f32,
    pub roughness: f32,
    pub crater_density: f32,
    pub tectonic_activity: f32,
    pub volcanic_activity: f32,
    pub vegetation_fraction: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedHydrosphere {
    pub key: String,
    pub fluid_key: String,
    pub ocean_fraction: f32,
    pub ice_fraction: f32,
    pub snow_fraction: f32,
    pub salinity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedRings {
    pub key: String,
    pub present: bool,
    pub density: f32,
    pub inner_radius_planet: f32,
    pub outer_radius_planet: f32,
    pub inclination_deg: f32,
    pub color_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedModifierFamily {
    Atmosphere,
    Climate,
    Surface,
    Hydrosphere,
    Orbital,
    Ring,
    Biosphere,
    Anomaly,
}

impl GeneratedModifierFamily {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Atmosphere => "atmosphere",
            Self::Climate => "climate",
            Self::Surface => "surface",
            Self::Hydrosphere => "hydrosphere",
            Self::Orbital => "orbital",
            Self::Ring => "ring",
            Self::Biosphere => "biosphere",
            Self::Anomaly => "anomaly",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedModifier {
    pub key: String,
    pub family: GeneratedModifierFamily,
    pub intensity: f32,
    pub tags: Vec<String>,
}

pub fn generate_planet_profile(input: impl Into<ProfileSeedInput>) -> GeneratedPlanetProfile {
    let input = input.into();
    let archetype = select_archetype(&input);
    let archetype_rng_seed = mix_seed(input.seed, stable_key_hash(archetype.key));
    let mut rng = ProfileRng::new(archetype_rng_seed);

    let base_radius_km = archetype.radius_km.sample(&mut rng);
    let scale = select_planet_scale(&input, &archetype, base_radius_km);
    let radius_km = scale.radius_km;
    let temperature_c = archetype.temperature_c.sample(&mut rng);
    let ocean_fraction = archetype.ocean_fraction.sample(&mut rng);
    let ice_fraction = archetype.ice_fraction.sample(&mut rng);
    let atmosphere_density = archetype.atmosphere_density.sample(&mut rng);
    let cloud_density = archetype.cloud_density.sample(&mut rng);
    let volcanic_activity = archetype.volcanic_activity.sample(&mut rng);
    let ring_roll = rng.next_f32();
    let rings_present = ring_roll < archetype.ring_chance;
    let modifiers = select_modifiers(&input, &archetype, input.modifier_budget);
    let modifier_effects = ModifierEffects::from_modifiers(&modifiers);

    let adjusted_temperature_c =
        (temperature_c as f32 + modifier_effects.temperature_delta_c).round() as i32;
    let adjusted_ocean_fraction =
        clamp01(ocean_fraction + modifier_effects.ocean_delta + modifier_effects.ice_to_ocean);
    let adjusted_ice_fraction =
        clamp01(ice_fraction + modifier_effects.ice_delta - modifier_effects.ice_to_ocean);
    let adjusted_atmosphere_density =
        clamp01(atmosphere_density + modifier_effects.atmosphere_delta + scale.atmosphere_delta);
    let adjusted_cloud_density = clamp01(cloud_density + modifier_effects.cloud_delta);
    let adjusted_volcanic_activity = clamp01(volcanic_activity + modifier_effects.volcanic_delta);

    let radius_earth = radius_km as f32 / EARTH_RADIUS_KM;
    let class_mass_factor = archetype.mass_factor.sample(&mut rng);
    let density_earth = round_to(class_mass_factor * scale.density_multiplier, 3);
    let mass_earth = round_to(radius_earth.powf(3.0) * density_earth, 3);
    let gravity_g = round_to(mass_earth / radius_earth.max(0.2).powi(2), 3);
    let size_class = scale.band.size_class();

    GeneratedPlanetProfile {
        seed: input.seed,
        algorithm: ALGORITHM_KEY.to_string(),
        catalog_version: input.catalog_version,
        archetype_key: archetype.key.to_string(),
        planet_class: archetype.class,
        class_key: archetype.class.key().to_string(),
        size_class,
        size_key: size_class.key().to_string(),
        scale_band: scale.band,
        scale_key: scale.band.key().to_string(),
        radius_scale: scale.radius_scale,
        radius_km,
        radius_earth: round_to(radius_earth, 3),
        mass_earth,
        density_earth,
        gravity_g,
        temperature_c: adjusted_temperature_c,
        atmosphere: GeneratedAtmosphere {
            key: archetype.atmosphere_key.to_string(),
            density: round_to(adjusted_atmosphere_density, 3),
            pressure_bar: round_to(
                archetype.pressure_bar.sample(&mut rng)
                    * adjusted_atmosphere_density.max(0.05)
                    * scale.pressure_multiplier,
                3,
            ),
            greenhouse_factor: round_to(
                archetype.greenhouse_factor.sample(&mut rng) + modifier_effects.greenhouse_delta,
                3,
            ),
            cloud_density: round_to(adjusted_cloud_density, 3),
            haze: round_to(
                clamp01(archetype.haze.sample(&mut rng) + modifier_effects.haze_delta),
                3,
            ),
            scattering_key: archetype.scattering_key.to_string(),
            dominant_gases: archetype
                .dominant_gases
                .iter()
                .map(|gas| (*gas).to_string())
                .collect(),
        },
        surface: GeneratedSurface {
            key: archetype.surface_key.to_string(),
            terrain_key: archetype.terrain_key.to_string(),
            palette_key: archetype.palette_key.to_string(),
            albedo: round_to(clamp01(archetype.albedo.sample(&mut rng)), 3),
            roughness: round_to(clamp01(archetype.roughness.sample(&mut rng)), 3),
            crater_density: round_to(clamp01(archetype.crater_density.sample(&mut rng)), 3),
            tectonic_activity: round_to(
                clamp01(archetype.tectonic_activity.sample(&mut rng) + scale.tectonic_delta),
                3,
            ),
            volcanic_activity: round_to(adjusted_volcanic_activity, 3),
            vegetation_fraction: round_to(
                clamp01(
                    archetype.vegetation_fraction.sample(&mut rng)
                        + modifier_effects.vegetation_delta,
                ),
                3,
            ),
        },
        hydrosphere: GeneratedHydrosphere {
            key: archetype.hydrosphere_key.to_string(),
            fluid_key: archetype.fluid_key.to_string(),
            ocean_fraction: round_to(adjusted_ocean_fraction, 3),
            ice_fraction: round_to(adjusted_ice_fraction, 3),
            snow_fraction: round_to(
                clamp01(archetype.snow_fraction.sample(&mut rng) + modifier_effects.snow_delta),
                3,
            ),
            salinity: round_to(clamp01(archetype.salinity.sample(&mut rng)), 3),
        },
        rings: GeneratedRings {
            key: if rings_present {
                archetype.ring_key.to_string()
            } else {
                "rings.none".to_string()
            },
            present: rings_present,
            density: if rings_present {
                round_to(archetype.ring_density.sample(&mut rng), 3)
            } else {
                0.0
            },
            inner_radius_planet: if rings_present {
                round_to(archetype.ring_inner_radius.sample(&mut rng), 3)
            } else {
                0.0
            },
            outer_radius_planet: if rings_present {
                round_to(archetype.ring_outer_radius.sample(&mut rng), 3)
            } else {
                0.0
            },
            inclination_deg: if rings_present {
                round_to(archetype.ring_inclination_deg.sample(&mut rng), 2)
            } else {
                0.0
            },
            color_key: if rings_present {
                archetype.ring_color_key.to_string()
            } else {
                "transparent".to_string()
            },
        },
        modifiers,
        palette_key: archetype.palette_key.to_string(),
        render_model_key: archetype.render_model_key.to_string(),
    }
}

pub fn select_archetype_key(seed: u64) -> &'static str {
    let input = ProfileSeedInput::new(seed);
    select_archetype(&input).key
}

pub fn known_planet_archetype_keys() -> impl Iterator<Item = &'static str> {
    PLANET_TYPE_MATRIX
        .iter()
        .map(|entry| entry.key)
        .chain(ARCHETYPES.iter().map(|archetype| archetype.key))
}

pub fn select_modifier_keys(seed: u64, budget: usize) -> Vec<String> {
    let input = ProfileSeedInput::new(seed).with_modifier_budget(budget);
    let archetype = select_archetype(&input);
    select_modifiers(&input, &archetype, budget)
        .into_iter()
        .map(|modifier| modifier.key)
        .collect()
}

pub fn stable_key_hash(key: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

pub fn mix_seed(seed: u64, salt: u64) -> u64 {
    let mut mixed = seed ^ salt.rotate_left(17);
    mixed = mixed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

pub fn weighted_index(seed: u64, salt: u64, weights: &[u32]) -> Option<usize> {
    if weights.is_empty() {
        return None;
    }

    let total = weights
        .iter()
        .fold(0_u64, |sum, weight| sum + u64::from(*weight));
    if total == 0 {
        return None;
    }

    let mut rng = ProfileRng::new(mix_seed(seed, salt));
    let mut roll = rng.range_u64(0, total);
    for (index, weight) in weights.iter().enumerate() {
        let weight = u64::from(*weight);
        if roll < weight {
            return Some(index);
        }
        roll -= weight;
    }

    Some(weights.len() - 1)
}

#[derive(Debug, Clone)]
pub struct ProfileRng {
    state: u64,
}

impl ProfileRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1_u64 << 24) as f32)
    }

    pub fn range_u64(&mut self, min: u64, max: u64) -> u64 {
        debug_assert!(min < max);
        min + (self.next_u64() % (max - min))
    }

    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        debug_assert!(min <= max);
        if min == max {
            return min;
        }
        min + (self.next_f32() * (max - min + 1) as f32).floor() as i32
    }

    pub fn range_f32(&mut self, min: f32, max: f32) -> f32 {
        debug_assert!(min <= max);
        if (max - min).abs() <= f32::EPSILON {
            return min;
        }
        min + self.next_f32() * (max - min)
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.next_f32() < probability.clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy)]
struct I32Range {
    min: i32,
    max: i32,
}

impl I32Range {
    const fn new(min: i32, max: i32) -> Self {
        Self { min, max }
    }

    fn offset(self, delta: i32) -> Self {
        Self {
            min: self.min.saturating_add(delta),
            max: self.max.saturating_add(delta),
        }
    }

    fn clamp(self, min: i32, max: i32) -> Self {
        Self {
            min: self.min.clamp(min, max),
            max: self.max.clamp(min, max),
        }
    }

    fn sample(self, rng: &mut ProfileRng) -> i32 {
        rng.range_i32(self.min, self.max)
    }
}

#[derive(Debug, Clone, Copy)]
struct F32Range {
    min: f32,
    max: f32,
}

impl F32Range {
    const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    fn offset(self, delta: f32) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    fn clamp01(self) -> Self {
        Self {
            min: clamp01(self.min),
            max: clamp01(self.max),
        }
    }

    fn sample(self, rng: &mut ProfileRng) -> f32 {
        rng.range_f32(self.min, self.max)
    }
}

#[derive(Debug, Clone, Copy)]
struct ArchetypeSpec {
    key: &'static str,
    class: GeneratedPlanetClass,
    weight: u32,
    radius_km: I32Range,
    mass_factor: F32Range,
    temperature_c: I32Range,
    ocean_fraction: F32Range,
    ice_fraction: F32Range,
    snow_fraction: F32Range,
    salinity: F32Range,
    atmosphere_density: F32Range,
    pressure_bar: F32Range,
    greenhouse_factor: F32Range,
    cloud_density: F32Range,
    haze: F32Range,
    volcanic_activity: F32Range,
    albedo: F32Range,
    roughness: F32Range,
    crater_density: F32Range,
    tectonic_activity: F32Range,
    vegetation_fraction: F32Range,
    ring_chance: f32,
    ring_density: F32Range,
    ring_inner_radius: F32Range,
    ring_outer_radius: F32Range,
    ring_inclination_deg: F32Range,
    atmosphere_key: &'static str,
    surface_key: &'static str,
    terrain_key: &'static str,
    hydrosphere_key: &'static str,
    fluid_key: &'static str,
    palette_key: &'static str,
    scattering_key: &'static str,
    dominant_gases: &'static [&'static str],
    ring_key: &'static str,
    ring_color_key: &'static str,
    render_model_key: &'static str,
    modifier_tags: &'static [&'static str],
}

#[derive(Debug, Clone, Copy)]
struct PlanetTypeMatrixEntry {
    key: &'static str,
    class: GeneratedPlanetClass,
    weight: u32,
    base_key: &'static str,
    modifier_tags: &'static [&'static str],
    temperature_delta_c: i32,
    ocean_delta: f32,
    ice_delta: f32,
    atmosphere_delta: f32,
    greenhouse_delta: f32,
    cloud_delta: f32,
    haze_delta: f32,
    volcanic_delta: f32,
    vegetation_delta: f32,
    ring_chance_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct ModifierSpec {
    key: &'static str,
    family: GeneratedModifierFamily,
    weight: u32,
    rarity: ModifierRarity,
    tags: &'static [&'static str],
    effects: ModifierEffects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierRarity {
    Common,
    Uncommon,
    Rare,
}

#[derive(Debug, Clone, Copy, Default)]
struct ModifierEffects {
    temperature_delta_c: f32,
    ocean_delta: f32,
    ice_delta: f32,
    ice_to_ocean: f32,
    atmosphere_delta: f32,
    cloud_delta: f32,
    greenhouse_delta: f32,
    haze_delta: f32,
    volcanic_delta: f32,
    vegetation_delta: f32,
    snow_delta: f32,
}

impl ModifierEffects {
    const fn none() -> Self {
        Self {
            temperature_delta_c: 0.0,
            ocean_delta: 0.0,
            ice_delta: 0.0,
            ice_to_ocean: 0.0,
            atmosphere_delta: 0.0,
            cloud_delta: 0.0,
            greenhouse_delta: 0.0,
            haze_delta: 0.0,
            volcanic_delta: 0.0,
            vegetation_delta: 0.0,
            snow_delta: 0.0,
        }
    }

    fn from_modifiers(modifiers: &[GeneratedModifier]) -> Self {
        let mut effects = Self::none();
        for modifier in modifiers {
            let Some(spec) = MODIFIERS.iter().find(|spec| spec.key == modifier.key) else {
                continue;
            };
            effects.temperature_delta_c += spec.effects.temperature_delta_c * modifier.intensity;
            effects.ocean_delta += spec.effects.ocean_delta * modifier.intensity;
            effects.ice_delta += spec.effects.ice_delta * modifier.intensity;
            effects.ice_to_ocean += spec.effects.ice_to_ocean * modifier.intensity;
            effects.atmosphere_delta += spec.effects.atmosphere_delta * modifier.intensity;
            effects.cloud_delta += spec.effects.cloud_delta * modifier.intensity;
            effects.greenhouse_delta += spec.effects.greenhouse_delta * modifier.intensity;
            effects.haze_delta += spec.effects.haze_delta * modifier.intensity;
            effects.volcanic_delta += spec.effects.volcanic_delta * modifier.intensity;
            effects.vegetation_delta += spec.effects.vegetation_delta * modifier.intensity;
            effects.snow_delta += spec.effects.snow_delta * modifier.intensity;
        }
        effects
    }
}

#[derive(Debug, Clone, Copy)]
struct ScaleBandSpec {
    band: GeneratedPlanetScaleBand,
    center_fraction: f32,
    spread_fraction: f32,
    density_multiplier: F32Range,
    atmosphere_delta: f32,
    pressure_multiplier: f32,
    tectonic_delta: f32,
}

#[derive(Debug, Clone, Copy)]
struct ScaleSelection {
    band: GeneratedPlanetScaleBand,
    radius_km: i32,
    radius_scale: f32,
    density_multiplier: f32,
    atmosphere_delta: f32,
    pressure_multiplier: f32,
    tectonic_delta: f32,
}

const DEFAULT_CATALOG_VERSION: &str = "planet-profile-catalog.v0.local";
const ALGORITHM_KEY: &str = "procedural-planet-profile-v0";
const DEFAULT_MODIFIER_BUDGET: usize = 4;
const EARTH_RADIUS_KM: f32 = 6_371.0;
const ARCHETYPE_SELECTION_SALT: u64 = 0xA11C_A7A1_0000_0001;
const MODIFIER_SELECTION_SALT: u64 = 0xB10C_7E5E_0000_0002;
const SCALE_SELECTION_SALT: u64 = 0x5CA1_E5E1_0000_0003;
const SCALE_BAND_COUNT: usize = 6;

const SCALE_BAND_SPECS: [ScaleBandSpec; SCALE_BAND_COUNT] = [
    ScaleBandSpec {
        band: GeneratedPlanetScaleBand::Tiny,
        center_fraction: 0.12,
        spread_fraction: 0.16,
        density_multiplier: F32Range::new(0.88, 1.02),
        atmosphere_delta: -0.08,
        pressure_multiplier: 0.76,
        tectonic_delta: -0.05,
    },
    ScaleBandSpec {
        band: GeneratedPlanetScaleBand::Compact,
        center_fraction: 0.28,
        spread_fraction: 0.22,
        density_multiplier: F32Range::new(0.94, 1.05),
        atmosphere_delta: -0.04,
        pressure_multiplier: 0.9,
        tectonic_delta: -0.02,
    },
    ScaleBandSpec {
        band: GeneratedPlanetScaleBand::Standard,
        center_fraction: 0.5,
        spread_fraction: 0.26,
        density_multiplier: F32Range::new(0.98, 1.06),
        atmosphere_delta: 0.0,
        pressure_multiplier: 1.0,
        tectonic_delta: 0.0,
    },
    ScaleBandSpec {
        band: GeneratedPlanetScaleBand::Broad,
        center_fraction: 0.66,
        spread_fraction: 0.24,
        density_multiplier: F32Range::new(1.0, 1.1),
        atmosphere_delta: 0.03,
        pressure_multiplier: 1.12,
        tectonic_delta: 0.02,
    },
    ScaleBandSpec {
        band: GeneratedPlanetScaleBand::Massive,
        center_fraction: 0.82,
        spread_fraction: 0.2,
        density_multiplier: F32Range::new(1.04, 1.16),
        atmosphere_delta: 0.06,
        pressure_multiplier: 1.28,
        tectonic_delta: 0.04,
    },
    ScaleBandSpec {
        band: GeneratedPlanetScaleBand::Colossal,
        center_fraction: 0.94,
        spread_fraction: 0.12,
        density_multiplier: F32Range::new(1.08, 1.24),
        atmosphere_delta: 0.08,
        pressure_multiplier: 1.42,
        tectonic_delta: 0.05,
    },
];

const GAS_N2_O2_AR: &[&str] = &["nitrogen", "oxygen", "argon"];
const GAS_CO2_N2_SO2: &[&str] = &["carbon-dioxide", "nitrogen", "sulfur-dioxide"];
const GAS_N2_CO2_AR: &[&str] = &["nitrogen", "carbon-dioxide", "argon"];
const GAS_H2_HE_CH4: &[&str] = &["hydrogen", "helium", "methane"];
const GAS_H2_HE_NH3: &[&str] = &["hydrogen", "helium", "ammonia"];
const GAS_H2_HE_NH3_H2S: &[&str] = &["hydrogen", "helium", "ammonia", "hydrogen-sulfide"];
const GAS_H2_HE_NA_K_SIO: &[&str] = &[
    "hydrogen",
    "helium",
    "sodium",
    "potassium",
    "silicate-vapor",
];
const GAS_HE_H2_CO: &[&str] = &["helium", "hydrogen", "carbon-monoxide"];
const GAS_CO2_SO2_H2SO4: &[&str] = &["carbon-dioxide", "sulfur-dioxide", "sulfuric-acid-aerosol"];
const GAS_CO_CH4_N2: &[&str] = &["carbon-monoxide", "methane", "nitrogen"];
const GAS_CH4_N2: &[&str] = &["methane", "nitrogen"];
const GAS_TRACE_EXOSPHERE: &[&str] = &["helium", "sodium", "potassium"];

const TAG_ROCKY: &[&str] = &["rocky", "dry"];
const TAG_DRY_HOT: &[&str] = &["rocky", "dry", "hot"];
const TAG_TEMPERATE: &[&str] = &["rocky", "wet", "temperate"];
const TAG_OCEAN: &[&str] = &["rocky", "wet", "ocean"];
const TAG_ICE: &[&str] = &["rocky", "ice", "cold"];
const TAG_VOLCANIC: &[&str] = &["rocky", "volcanic", "hot"];
const TAG_SUPER_EARTH: &[&str] = &["rocky", "massive", "temperate"];
const TAG_GAS_DWARF: &[&str] = &["gas", "small", "volatile"];
const TAG_GAS_GIANT: &[&str] = &["gas", "massive", "storm", "ammonia", "ring-prone"];
const TAG_ICE_GIANT: &[&str] = &["gas", "ice", "methane", "cold", "storm", "ring-prone"];
const TAG_ROGUE: &[&str] = &["dark", "cold", "isolated"];
const TAG_MARS_LIKE: &[&str] = &["rocky", "dry", "cold"];
const TAG_VENUS_LIKE: &[&str] = &["rocky", "dry", "hot", "massive"];
const TAG_LOW_WATER: &[&str] = &["rocky", "dry", "temperate"];
const TAG_ICE_AGE: &[&str] = &["rocky", "wet", "cold", "ice"];
const TAG_DENSE_ATMOSPHERE: &[&str] = &["rocky", "massive", "temperate"];
const TAG_SWAMP_JUNGLE: &[&str] = &["rocky", "wet", "temperate"];
const TAG_HYDROCARBON: &[&str] = &[
    "ice",
    "cold",
    "volatile",
    "methane",
    "hydrocarbon",
    "tholin",
];
const TAG_CHTHONIAN: &[&str] = &["rocky", "hot", "massive"];
const TAG_HYCEAN: &[&str] = &["wet", "ocean", "volatile", "massive"];
const TAG_HOT_GAS_GIANT: &[&str] = &[
    "gas",
    "massive",
    "hot",
    "metal-vapor",
    "silicate",
    "storm",
    "ring-prone",
];
const TAG_COLD_GAS_GIANT: &[&str] = &[
    "gas",
    "massive",
    "cold",
    "ammonia",
    "storm",
    "ice",
    "ring-prone",
];
const TAG_SULFUR_VOLCANIC: &[&str] = &[
    "rocky", "volcanic", "hot", "sulfur", "acid", "toxic", "storm",
];
const TAG_ACID_CLOUD: &[&str] = &[
    "rocky", "dry", "hot", "massive", "sulfur", "acid", "toxic", "storm",
];
const TAG_CARBON_WORLD: &[&str] = &["rocky", "dry", "carbon", "diamond", "graphite", "exotic"];
const TAG_DIAMOND_RAIN_GIANT: &[&str] = &[
    "gas",
    "massive",
    "methane",
    "diamond",
    "storm",
    "high-pressure",
];
const TAG_SULFUR_GAS_GIANT: &[&str] = &[
    "gas",
    "massive",
    "sulfur",
    "acid",
    "storm",
    "toxic",
    "ring-prone",
];
const TAG_HELIUM_GIANT: &[&str] = &[
    "gas",
    "massive",
    "helium",
    "hot",
    "metal-vapor",
    "ring-prone",
];
const TAG_PUFFY_GIANT: &[&str] = &["gas", "massive", "hot", "puffy", "haze", "low-density"];
const TAG_AMMONIA_ICE_WORLD: &[&str] = &[
    "rocky",
    "ice",
    "cold",
    "ammonia",
    "volatile",
    "cryovolcanic",
];
const TAG_TIDAL_EYEBALL: &[&str] = &["rocky", "wet", "cold", "temperate"];
const TAG_PROTO_PLANET: &[&str] = &["rocky", "volcanic", "dry"];
const TAG_DWARF_ASTEROID: &[&str] = &["rocky", "dry", "ice"];
const TAG_EXOMOON: &[&str] = &["rocky", "wet", "ice"];
const TAG_ARTIFICIAL: &[&str] = &["rocky", "massive", "temperate"];

#[allow(clippy::too_many_arguments)]
const fn planet_type(
    key: &'static str,
    class: GeneratedPlanetClass,
    weight: u32,
    base_key: &'static str,
    modifier_tags: &'static [&'static str],
    temperature_delta_c: i32,
    ocean_delta: f32,
    ice_delta: f32,
    atmosphere_delta: f32,
    greenhouse_delta: f32,
    cloud_delta: f32,
    haze_delta: f32,
    volcanic_delta: f32,
    vegetation_delta: f32,
    ring_chance_delta: f32,
) -> PlanetTypeMatrixEntry {
    PlanetTypeMatrixEntry {
        key,
        class,
        weight,
        base_key,
        modifier_tags,
        temperature_delta_c,
        ocean_delta,
        ice_delta,
        atmosphere_delta,
        greenhouse_delta,
        cloud_delta,
        haze_delta,
        volcanic_delta,
        vegetation_delta,
        ring_chance_delta,
    }
}

const PLANET_TYPE_MATRIX: &[PlanetTypeMatrixEntry] = &[
    planet_type(
        "catalog.archetype.mercury-like",
        GeneratedPlanetClass::MercuryLike,
        5,
        "catalog.archetype.barren-basalt",
        TAG_DRY_HOT,
        80,
        -0.02,
        -0.06,
        -0.08,
        -0.03,
        -0.02,
        -0.04,
        -0.02,
        0.0,
        -0.03,
    ),
    planet_type(
        "catalog.archetype.mars-like",
        GeneratedPlanetClass::MarsLike,
        6,
        "catalog.archetype.red-dune-desert",
        TAG_MARS_LIKE,
        -42,
        -0.03,
        0.08,
        -0.22,
        -0.04,
        -0.08,
        -0.08,
        -0.04,
        0.0,
        -0.04,
    ),
    planet_type(
        "catalog.archetype.venus-like",
        GeneratedPlanetClass::VenusLike,
        4,
        "catalog.archetype.red-dune-desert",
        TAG_VENUS_LIKE,
        310,
        -0.08,
        -0.05,
        0.34,
        0.32,
        0.42,
        0.28,
        0.04,
        0.0,
        -0.05,
    ),
    planet_type(
        "catalog.archetype.ocean",
        GeneratedPlanetClass::OceanWorld,
        8,
        "catalog.archetype.global-ocean",
        TAG_OCEAN,
        0,
        0.04,
        0.0,
        0.02,
        0.0,
        0.06,
        0.0,
        0.0,
        0.02,
        0.0,
    ),
    planet_type(
        "catalog.archetype.global-ocean",
        GeneratedPlanetClass::OceanWorld,
        8,
        "catalog.archetype.global-ocean",
        TAG_OCEAN,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.earth-like",
        GeneratedPlanetClass::TemperateTerrestrial,
        0,
        "catalog.archetype.temperate-continents",
        TAG_TEMPERATE,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.low-water",
        GeneratedPlanetClass::LowWaterWorld,
        6,
        "catalog.archetype.temperate-continents",
        TAG_LOW_WATER,
        8,
        -0.28,
        -0.04,
        -0.02,
        0.03,
        -0.08,
        0.03,
        0.0,
        -0.08,
        0.0,
    ),
    planet_type(
        "catalog.archetype.megacontinent",
        GeneratedPlanetClass::MegaContinentWorld,
        5,
        "catalog.archetype.temperate-continents",
        TAG_LOW_WATER,
        5,
        -0.2,
        -0.02,
        0.0,
        0.02,
        -0.04,
        0.02,
        0.0,
        -0.03,
        0.01,
    ),
    planet_type(
        "catalog.archetype.archipelago",
        GeneratedPlanetClass::ArchipelagoWorld,
        6,
        "catalog.archetype.global-ocean",
        TAG_OCEAN,
        1,
        -0.08,
        -0.02,
        0.0,
        0.0,
        0.04,
        0.0,
        0.02,
        0.04,
        0.0,
    ),
    planet_type(
        "catalog.archetype.ice-age",
        GeneratedPlanetClass::IceAgeWorld,
        6,
        "catalog.archetype.temperate-continents",
        TAG_ICE_AGE,
        -34,
        -0.12,
        0.3,
        -0.03,
        -0.02,
        -0.02,
        0.0,
        -0.03,
        -0.06,
        0.02,
    ),
    planet_type(
        "catalog.archetype.greenhouse",
        GeneratedPlanetClass::GreenhouseWorld,
        5,
        "catalog.archetype.red-dune-desert",
        TAG_VENUS_LIKE,
        90,
        -0.06,
        -0.05,
        0.18,
        0.22,
        0.18,
        0.18,
        0.0,
        -0.02,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.post-apocalyptic",
        GeneratedPlanetClass::PostApocalypticWorld,
        3,
        "catalog.archetype.temperate-continents",
        TAG_LOW_WATER,
        18,
        -0.16,
        -0.04,
        -0.12,
        0.05,
        -0.12,
        0.16,
        0.02,
        -0.28,
        0.0,
    ),
    planet_type(
        "catalog.archetype.active-volcanic",
        GeneratedPlanetClass::ActiveVolcanicWorld,
        6,
        "catalog.archetype.sulfur-volcanic",
        TAG_SULFUR_VOLCANIC,
        24,
        -0.02,
        0.0,
        0.04,
        0.06,
        0.06,
        0.1,
        0.18,
        0.0,
        -0.01,
    ),
    planet_type(
        "catalog.archetype.dense-atmosphere",
        GeneratedPlanetClass::DenseAtmosphereWorld,
        5,
        "catalog.archetype.temperate-super-earth",
        TAG_DENSE_ATMOSPHERE,
        12,
        -0.02,
        -0.03,
        0.24,
        0.12,
        0.16,
        0.08,
        0.0,
        -0.02,
        0.04,
    ),
    planet_type(
        "catalog.archetype.swamp-jungle",
        GeneratedPlanetClass::SwampJungleWorld,
        4,
        "catalog.archetype.temperate-continents",
        TAG_SWAMP_JUNGLE,
        10,
        0.08,
        -0.08,
        0.06,
        0.04,
        0.22,
        0.06,
        -0.02,
        0.3,
        -0.01,
    ),
    planet_type(
        "catalog.archetype.desert-dune",
        GeneratedPlanetClass::DesertDuneWorld,
        8,
        "catalog.archetype.red-dune-desert",
        TAG_DRY_HOT,
        16,
        -0.02,
        -0.04,
        0.0,
        0.02,
        -0.08,
        0.12,
        -0.02,
        -0.02,
        0.0,
    ),
    planet_type(
        "catalog.archetype.snowball",
        GeneratedPlanetClass::SnowballWorld,
        6,
        "catalog.archetype.cryogenic-ice",
        TAG_ICE,
        -22,
        -0.04,
        0.16,
        0.02,
        -0.02,
        -0.02,
        0.0,
        -0.02,
        0.0,
        0.03,
    ),
    planet_type(
        "catalog.archetype.ice-shell",
        GeneratedPlanetClass::IceShellWorld,
        5,
        "catalog.archetype.cryogenic-ice",
        TAG_ICE,
        -55,
        -0.08,
        0.2,
        -0.04,
        -0.04,
        -0.08,
        -0.04,
        0.03,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.frozen-super-earth",
        GeneratedPlanetClass::IceWorld,
        3,
        "catalog.archetype.cryogenic-ice",
        TAG_ICE,
        -12,
        0.0,
        0.10,
        0.08,
        0.02,
        0.04,
        0.02,
        0.02,
        0.0,
        0.04,
    ),
    planet_type(
        "catalog.archetype.europa-like-ice-ocean",
        GeneratedPlanetClass::IceShellWorld,
        2,
        "catalog.archetype.cryogenic-ice",
        TAG_ICE,
        -70,
        -0.05,
        0.20,
        -0.05,
        -0.04,
        -0.06,
        -0.02,
        0.04,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.enceladus-like-geyser-world",
        GeneratedPlanetClass::IceShellWorld,
        2,
        "catalog.archetype.cryogenic-ice",
        TAG_ICE,
        -88,
        -0.06,
        0.22,
        -0.06,
        -0.04,
        -0.04,
        -0.03,
        0.08,
        0.0,
        0.05,
    ),
    planet_type(
        "catalog.archetype.hydrocarbon-titan-like",
        GeneratedPlanetClass::HydrocarbonWorld,
        4,
        "catalog.archetype.cryogenic-ice",
        TAG_HYDROCARBON,
        -18,
        0.04,
        -0.08,
        0.18,
        0.05,
        0.12,
        0.24,
        -0.02,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.ammonia-world",
        GeneratedPlanetClass::IceWorld,
        3,
        "catalog.archetype.cryogenic-ice",
        TAG_AMMONIA_ICE_WORLD,
        -35,
        0.04,
        0.04,
        0.06,
        0.02,
        0.1,
        0.08,
        0.04,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.carbon",
        GeneratedPlanetClass::CarbonWorld,
        3,
        "catalog.archetype.carbon-diamond",
        TAG_CARBON_WORLD,
        35,
        -0.02,
        -0.08,
        -0.02,
        0.02,
        -0.02,
        0.02,
        0.02,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.diamond-carbon-world",
        GeneratedPlanetClass::CarbonWorld,
        2,
        "catalog.archetype.carbon-diamond",
        TAG_CARBON_WORLD,
        95,
        -0.02,
        -0.08,
        0.02,
        0.05,
        0.02,
        0.04,
        0.04,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.iron",
        GeneratedPlanetClass::IronWorld,
        3,
        "catalog.archetype.barren-basalt",
        TAG_ROCKY,
        20,
        -0.02,
        -0.08,
        -0.08,
        -0.04,
        -0.04,
        -0.04,
        -0.03,
        0.0,
        -0.01,
    ),
    planet_type(
        "catalog.archetype.chthonian",
        GeneratedPlanetClass::ChthonianWorld,
        2,
        "catalog.archetype.barren-basalt",
        TAG_CHTHONIAN,
        420,
        -0.04,
        -0.1,
        -0.14,
        0.02,
        -0.04,
        0.06,
        0.12,
        0.0,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.lava-magma",
        GeneratedPlanetClass::LavaMagmaWorld,
        4,
        "catalog.archetype.sulfur-volcanic",
        TAG_SULFUR_VOLCANIC,
        110,
        -0.02,
        0.0,
        0.02,
        0.08,
        0.04,
        0.08,
        0.22,
        0.0,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.sulfur-io-like",
        GeneratedPlanetClass::SulfurIoLike,
        4,
        "catalog.archetype.sulfur-volcanic",
        TAG_SULFUR_VOLCANIC,
        45,
        -0.02,
        0.0,
        -0.04,
        -0.06,
        0.0,
        -0.06,
        0.36,
        0.0,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.acid-cloud",
        GeneratedPlanetClass::AcidCloudWorld,
        3,
        "catalog.archetype.acid-sulfur-clouds",
        TAG_ACID_CLOUD,
        120,
        -0.08,
        -0.05,
        0.28,
        0.24,
        0.32,
        0.3,
        0.02,
        0.0,
        -0.03,
    ),
    planet_type(
        "catalog.archetype.super-earth",
        GeneratedPlanetClass::SuperEarth,
        7,
        "catalog.archetype.temperate-super-earth",
        TAG_SUPER_EARTH,
        0,
        0.0,
        0.0,
        0.02,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.mini-neptune",
        GeneratedPlanetClass::MiniNeptune,
        5,
        "catalog.archetype.sub-neptune-haze",
        TAG_GAS_DWARF,
        -20,
        0.0,
        0.02,
        0.04,
        0.02,
        0.04,
        0.06,
        0.0,
        0.0,
        0.04,
    ),
    planet_type(
        "catalog.archetype.sub-neptune",
        GeneratedPlanetClass::SubNeptune,
        6,
        "catalog.archetype.sub-neptune-haze",
        TAG_GAS_DWARF,
        0,
        0.0,
        0.0,
        0.02,
        0.02,
        0.02,
        0.02,
        0.0,
        0.0,
        0.03,
    ),
    planet_type(
        "catalog.archetype.water-steam-hycean",
        GeneratedPlanetClass::WaterSteamHycean,
        4,
        "catalog.archetype.global-ocean",
        TAG_HYCEAN,
        38,
        0.08,
        -0.06,
        0.18,
        0.18,
        0.24,
        0.1,
        0.0,
        0.04,
        0.02,
    ),
    planet_type(
        "catalog.archetype.gas-dwarf",
        GeneratedPlanetClass::GasDwarf,
        5,
        "catalog.archetype.sub-neptune-haze",
        TAG_GAS_DWARF,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.gas-giant",
        GeneratedPlanetClass::GasGiant,
        5,
        "catalog.archetype.banded-gas-giant",
        TAG_GAS_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.storm-gas-giant",
        GeneratedPlanetClass::GasGiant,
        3,
        "catalog.archetype.cold-ammonia-jupiter",
        TAG_GAS_GIANT,
        24,
        0.0,
        0.0,
        0.04,
        0.02,
        0.12,
        0.08,
        0.0,
        0.0,
        0.04,
    ),
    planet_type(
        "catalog.archetype.hot-jupiter",
        GeneratedPlanetClass::HotJupiter,
        3,
        "catalog.archetype.hot-jupiter-clouds",
        TAG_HOT_GAS_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.hot-neptune",
        GeneratedPlanetClass::MiniNeptune,
        3,
        "catalog.archetype.methane-ice-giant",
        TAG_GAS_DWARF,
        320,
        0.0,
        -0.08,
        0.08,
        0.10,
        0.10,
        0.14,
        0.0,
        0.0,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.cold-jupiter",
        GeneratedPlanetClass::ColdJupiter,
        4,
        "catalog.archetype.cold-ammonia-jupiter",
        TAG_COLD_GAS_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.saturn-like",
        GeneratedPlanetClass::SaturnLike,
        4,
        "catalog.archetype.saturn-ring-giant",
        TAG_GAS_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.sulfur-gas-world",
        GeneratedPlanetClass::GasGiant,
        2,
        "catalog.archetype.sulfur-rain-gas-giant",
        TAG_SULFUR_GAS_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.diamond-rain-gas-giant",
        GeneratedPlanetClass::GasGiant,
        1,
        "catalog.archetype.diamond-rain-gas-giant",
        TAG_DIAMOND_RAIN_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.ammonia-storm-giant",
        GeneratedPlanetClass::GasGiant,
        3,
        "catalog.archetype.cold-ammonia-jupiter",
        TAG_COLD_GAS_GIANT,
        -18,
        0.0,
        0.0,
        0.0,
        -0.02,
        0.04,
        0.02,
        0.0,
        0.0,
        0.08,
    ),
    planet_type(
        "catalog.archetype.ice-giant",
        GeneratedPlanetClass::IceGiant,
        5,
        "catalog.archetype.methane-ice-giant",
        TAG_ICE_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.methane-ice-giant",
        GeneratedPlanetClass::IceGiant,
        4,
        "catalog.archetype.methane-ice-giant",
        TAG_ICE_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.uranus-like-ice-giant",
        GeneratedPlanetClass::IceGiant,
        2,
        "catalog.archetype.methane-ice-giant",
        TAG_ICE_GIANT,
        -45,
        0.0,
        0.02,
        0.0,
        -0.03,
        -0.08,
        0.02,
        0.0,
        0.0,
        0.04,
    ),
    planet_type(
        "catalog.archetype.neptune-like-storm-giant",
        GeneratedPlanetClass::IceGiant,
        2,
        "catalog.archetype.methane-ice-giant",
        TAG_ICE_GIANT,
        -25,
        0.0,
        0.0,
        0.02,
        -0.02,
        0.12,
        0.04,
        0.0,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.helium-giant",
        GeneratedPlanetClass::HeliumGiant,
        2,
        "catalog.archetype.helium-cloud-giant",
        TAG_HELIUM_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.puffy-giant",
        GeneratedPlanetClass::PuffyGiant,
        2,
        "catalog.archetype.puffy-haze-giant",
        TAG_PUFFY_GIANT,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.rogue-gas-giant",
        GeneratedPlanetClass::GasGiant,
        2,
        "catalog.archetype.cold-ammonia-jupiter",
        TAG_COLD_GAS_GIANT,
        -120,
        0.0,
        0.0,
        -0.08,
        -0.08,
        0.02,
        -0.02,
        0.0,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.rogue",
        GeneratedPlanetClass::RoguePlanet,
        3,
        "catalog.archetype.rogue-frozen",
        TAG_ROGUE,
        0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.tidally-locked-eyeball",
        GeneratedPlanetClass::TidallyLockedEyeball,
        4,
        "catalog.archetype.temperate-continents",
        TAG_TIDAL_EYEBALL,
        -12,
        0.02,
        0.16,
        0.0,
        0.0,
        0.06,
        0.02,
        -0.02,
        -0.02,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.twilight-belt",
        GeneratedPlanetClass::TwilightBeltWorld,
        4,
        "catalog.archetype.temperate-continents",
        TAG_TIDAL_EYEBALL,
        4,
        -0.06,
        0.08,
        0.0,
        0.02,
        0.04,
        0.04,
        0.0,
        0.0,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.circumbinary",
        GeneratedPlanetClass::CircumbinaryWorld,
        3,
        "catalog.archetype.temperate-continents",
        TAG_TEMPERATE,
        6,
        0.0,
        -0.02,
        0.0,
        0.02,
        0.04,
        0.02,
        0.0,
        0.0,
        0.02,
    ),
    planet_type(
        "catalog.archetype.eccentric-season",
        GeneratedPlanetClass::EccentricSeasonWorld,
        4,
        "catalog.archetype.temperate-continents",
        TAG_ICE_AGE,
        -6,
        -0.04,
        0.12,
        0.0,
        0.01,
        0.02,
        0.04,
        0.0,
        -0.02,
        0.0,
    ),
    planet_type(
        "catalog.archetype.proto-planet",
        GeneratedPlanetClass::ProtoPlanet,
        3,
        "catalog.archetype.barren-basalt",
        TAG_PROTO_PLANET,
        160,
        -0.02,
        -0.05,
        0.04,
        0.04,
        0.04,
        0.14,
        0.2,
        0.0,
        0.08,
    ),
    planet_type(
        "catalog.archetype.dwarf-asteroid-like",
        GeneratedPlanetClass::DwarfAsteroidLike,
        5,
        "catalog.archetype.barren-basalt",
        TAG_DWARF_ASTEROID,
        -35,
        -0.02,
        0.05,
        -0.08,
        -0.03,
        -0.04,
        -0.04,
        -0.04,
        0.0,
        -0.02,
    ),
    planet_type(
        "catalog.archetype.exomoon",
        GeneratedPlanetClass::Exomoon,
        4,
        "catalog.archetype.cryogenic-ice",
        TAG_EXOMOON,
        -18,
        0.06,
        0.04,
        0.02,
        0.0,
        0.04,
        0.02,
        0.06,
        0.0,
        0.03,
    ),
    planet_type(
        "catalog.archetype.captured-moon",
        GeneratedPlanetClass::CapturedMoon,
        4,
        "catalog.archetype.barren-basalt",
        TAG_DWARF_ASTEROID,
        -24,
        -0.02,
        0.08,
        -0.06,
        -0.02,
        -0.02,
        -0.02,
        0.0,
        0.0,
        0.0,
    ),
    planet_type(
        "catalog.archetype.rare-artificial",
        GeneratedPlanetClass::ArtificialWorld,
        1,
        "catalog.archetype.temperate-super-earth",
        TAG_ARTIFICIAL,
        8,
        -0.18,
        -0.04,
        0.06,
        0.04,
        -0.02,
        0.06,
        -0.02,
        -0.12,
        0.12,
    ),
    planet_type(
        "catalog.archetype.ecumenopolis",
        GeneratedPlanetClass::Ecumenopolis,
        1,
        "catalog.archetype.temperate-super-earth",
        TAG_ARTIFICIAL,
        18,
        -0.24,
        -0.08,
        0.1,
        0.08,
        0.02,
        0.12,
        -0.04,
        -0.26,
        0.08,
    ),
];

const ARCHETYPES: &[ArchetypeSpec] = &[
    ArchetypeSpec {
        key: "catalog.archetype.barren-basalt",
        class: GeneratedPlanetClass::BarrenRock,
        weight: 10,
        radius_km: I32Range::new(2_900, 7_200),
        mass_factor: F32Range::new(0.65, 1.05),
        temperature_c: I32Range::new(-70, 210),
        ocean_fraction: F32Range::new(0.0, 0.04),
        ice_fraction: F32Range::new(0.0, 0.18),
        snow_fraction: F32Range::new(0.0, 0.12),
        salinity: F32Range::new(0.0, 0.05),
        atmosphere_density: F32Range::new(0.0, 0.2),
        pressure_bar: F32Range::new(0.0, 0.12),
        greenhouse_factor: F32Range::new(0.0, 0.08),
        cloud_density: F32Range::new(0.0, 0.08),
        haze: F32Range::new(0.0, 0.15),
        volcanic_activity: F32Range::new(0.0, 0.24),
        albedo: F32Range::new(0.08, 0.26),
        roughness: F32Range::new(0.62, 0.92),
        crater_density: F32Range::new(0.42, 0.92),
        tectonic_activity: F32Range::new(0.0, 0.16),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.04,
        ring_density: F32Range::new(0.1, 0.34),
        ring_inner_radius: F32Range::new(1.65, 2.0),
        ring_outer_radius: F32Range::new(2.15, 3.0),
        ring_inclination_deg: F32Range::new(-9.0, 9.0),
        atmosphere_key: "atmosphere.exosphere.trace",
        surface_key: "surface.basalt.regolith",
        terrain_key: "terrain.cratered-highlands",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.none",
        palette_key: "palette.basalt-ash-rust",
        scattering_key: "scattering.none",
        dominant_gases: GAS_TRACE_EXOSPHERE,
        ring_key: "rings.sparse-dust",
        ring_color_key: "ring-color.ash",
        render_model_key: "render.pbr-relief-airless",
        modifier_tags: TAG_ROCKY,
    },
    ArchetypeSpec {
        key: "catalog.archetype.red-dune-desert",
        class: GeneratedPlanetClass::DesertWorld,
        weight: 9,
        radius_km: I32Range::new(4_800, 8_900),
        mass_factor: F32Range::new(0.75, 1.18),
        temperature_c: I32Range::new(20, 145),
        ocean_fraction: F32Range::new(0.0, 0.08),
        ice_fraction: F32Range::new(0.0, 0.05),
        snow_fraction: F32Range::new(0.0, 0.02),
        salinity: F32Range::new(0.2, 0.75),
        atmosphere_density: F32Range::new(0.28, 0.78),
        pressure_bar: F32Range::new(0.25, 1.4),
        greenhouse_factor: F32Range::new(0.12, 0.38),
        cloud_density: F32Range::new(0.02, 0.24),
        haze: F32Range::new(0.15, 0.62),
        volcanic_activity: F32Range::new(0.02, 0.22),
        albedo: F32Range::new(0.18, 0.42),
        roughness: F32Range::new(0.34, 0.7),
        crater_density: F32Range::new(0.08, 0.42),
        tectonic_activity: F32Range::new(0.03, 0.28),
        vegetation_fraction: F32Range::new(0.0, 0.02),
        ring_chance: 0.06,
        ring_density: F32Range::new(0.12, 0.38),
        ring_inner_radius: F32Range::new(1.7, 2.15),
        ring_outer_radius: F32Range::new(2.35, 3.35),
        ring_inclination_deg: F32Range::new(-12.0, 12.0),
        atmosphere_key: "atmosphere.thin-dusty",
        surface_key: "surface.oxide-dunes",
        terrain_key: "terrain.dune-seas-canyons",
        hydrosphere_key: "hydrosphere.brine-trace",
        fluid_key: "fluid.water-brine",
        palette_key: "palette.oxide-sand-hematite",
        scattering_key: "scattering.dust-forward",
        dominant_gases: GAS_N2_CO2_AR,
        ring_key: "rings.sparse-dust",
        ring_color_key: "ring-color.ochre",
        render_model_key: "render.pbr-relief-haze",
        modifier_tags: TAG_DRY_HOT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.temperate-continents",
        class: GeneratedPlanetClass::TemperateTerrestrial,
        weight: 12,
        radius_km: I32Range::new(5_900, 9_200),
        mass_factor: F32Range::new(0.85, 1.18),
        temperature_c: I32Range::new(-5, 34),
        ocean_fraction: F32Range::new(0.32, 0.72),
        ice_fraction: F32Range::new(0.02, 0.2),
        snow_fraction: F32Range::new(0.02, 0.18),
        salinity: F32Range::new(0.18, 0.52),
        atmosphere_density: F32Range::new(0.46, 0.94),
        pressure_bar: F32Range::new(0.65, 1.7),
        greenhouse_factor: F32Range::new(0.12, 0.36),
        cloud_density: F32Range::new(0.22, 0.64),
        haze: F32Range::new(0.02, 0.24),
        volcanic_activity: F32Range::new(0.02, 0.28),
        albedo: F32Range::new(0.22, 0.42),
        roughness: F32Range::new(0.28, 0.62),
        crater_density: F32Range::new(0.0, 0.16),
        tectonic_activity: F32Range::new(0.16, 0.64),
        vegetation_fraction: F32Range::new(0.04, 0.42),
        ring_chance: 0.05,
        ring_density: F32Range::new(0.12, 0.36),
        ring_inner_radius: F32Range::new(1.75, 2.25),
        ring_outer_radius: F32Range::new(2.4, 3.5),
        ring_inclination_deg: F32Range::new(-10.0, 10.0),
        atmosphere_key: "atmosphere.temperate-nitrogen-oxygen",
        surface_key: "surface.continents-ocean",
        terrain_key: "terrain.plates-mountains-basins",
        hydrosphere_key: "hydrosphere.liquid-water",
        fluid_key: "fluid.water",
        palette_key: "palette.deep-ocean-forest-oxide-cloud",
        scattering_key: "scattering.rayleigh-mie-balanced",
        dominant_gases: GAS_N2_O2_AR,
        ring_key: "rings.icy-shepherded",
        ring_color_key: "ring-color.ice-silicate",
        render_model_key: "render.pbr-relief-cloud-shadow-rayleigh-mie-tonemap",
        modifier_tags: TAG_TEMPERATE,
    },
    ArchetypeSpec {
        key: "catalog.archetype.global-ocean",
        class: GeneratedPlanetClass::OceanWorld,
        weight: 8,
        radius_km: I32Range::new(6_200, 10_800),
        mass_factor: F32Range::new(0.82, 1.12),
        temperature_c: I32Range::new(-12, 42),
        ocean_fraction: F32Range::new(0.72, 0.98),
        ice_fraction: F32Range::new(0.0, 0.24),
        snow_fraction: F32Range::new(0.0, 0.22),
        salinity: F32Range::new(0.1, 0.42),
        atmosphere_density: F32Range::new(0.52, 0.98),
        pressure_bar: F32Range::new(0.75, 2.4),
        greenhouse_factor: F32Range::new(0.1, 0.32),
        cloud_density: F32Range::new(0.36, 0.82),
        haze: F32Range::new(0.02, 0.24),
        volcanic_activity: F32Range::new(0.0, 0.2),
        albedo: F32Range::new(0.18, 0.36),
        roughness: F32Range::new(0.08, 0.34),
        crater_density: F32Range::new(0.0, 0.06),
        tectonic_activity: F32Range::new(0.05, 0.36),
        vegetation_fraction: F32Range::new(0.0, 0.18),
        ring_chance: 0.04,
        ring_density: F32Range::new(0.1, 0.3),
        ring_inner_radius: F32Range::new(1.8, 2.25),
        ring_outer_radius: F32Range::new(2.35, 3.2),
        ring_inclination_deg: F32Range::new(-7.0, 7.0),
        atmosphere_key: "atmosphere.humid-marine",
        surface_key: "surface.global-ocean",
        terrain_key: "terrain.archipelagos-ridges",
        hydrosphere_key: "hydrosphere.global-ocean",
        fluid_key: "fluid.water",
        palette_key: "palette.cobalt-ocean-storm-white",
        scattering_key: "scattering.humid-mie",
        dominant_gases: GAS_N2_O2_AR,
        ring_key: "rings.icy-shepherded",
        ring_color_key: "ring-color.blue-ice",
        render_model_key: "render.ocean-cloud-glint-atmosphere",
        modifier_tags: TAG_OCEAN,
    },
    ArchetypeSpec {
        key: "catalog.archetype.cryogenic-ice",
        class: GeneratedPlanetClass::IceWorld,
        weight: 9,
        radius_km: I32Range::new(3_300, 8_700),
        mass_factor: F32Range::new(0.58, 0.96),
        temperature_c: I32Range::new(-210, -45),
        ocean_fraction: F32Range::new(0.0, 0.1),
        ice_fraction: F32Range::new(0.48, 0.94),
        snow_fraction: F32Range::new(0.28, 0.84),
        salinity: F32Range::new(0.0, 0.32),
        atmosphere_density: F32Range::new(0.02, 0.46),
        pressure_bar: F32Range::new(0.02, 0.8),
        greenhouse_factor: F32Range::new(0.0, 0.18),
        cloud_density: F32Range::new(0.0, 0.34),
        haze: F32Range::new(0.0, 0.28),
        volcanic_activity: F32Range::new(0.0, 0.12),
        albedo: F32Range::new(0.38, 0.82),
        roughness: F32Range::new(0.22, 0.64),
        crater_density: F32Range::new(0.08, 0.48),
        tectonic_activity: F32Range::new(0.0, 0.22),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.08,
        ring_density: F32Range::new(0.18, 0.54),
        ring_inner_radius: F32Range::new(1.65, 2.1),
        ring_outer_radius: F32Range::new(2.4, 3.65),
        ring_inclination_deg: F32Range::new(-14.0, 14.0),
        atmosphere_key: "atmosphere.cold-thin",
        surface_key: "surface.nitrogen-water-ice",
        terrain_key: "terrain.ice-cracks-cryovolcanic-plains",
        hydrosphere_key: "hydrosphere.frozen",
        fluid_key: "fluid.water-ice",
        palette_key: "palette.ice-cyan-slate",
        scattering_key: "scattering.thin-blue",
        dominant_gases: GAS_CH4_N2,
        ring_key: "rings.bright-ice",
        ring_color_key: "ring-color.ice",
        render_model_key: "render.ice-relief-forward-scatter",
        modifier_tags: TAG_ICE,
    },
    ArchetypeSpec {
        key: "catalog.archetype.sulfur-volcanic",
        class: GeneratedPlanetClass::VolcanicWorld,
        weight: 6,
        radius_km: I32Range::new(3_800, 8_400),
        mass_factor: F32Range::new(0.78, 1.28),
        temperature_c: I32Range::new(90, 520),
        ocean_fraction: F32Range::new(0.0, 0.02),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.16, 0.86),
        pressure_bar: F32Range::new(0.18, 3.4),
        greenhouse_factor: F32Range::new(0.18, 0.62),
        cloud_density: F32Range::new(0.06, 0.46),
        haze: F32Range::new(0.18, 0.78),
        volcanic_activity: F32Range::new(0.58, 1.0),
        albedo: F32Range::new(0.1, 0.34),
        roughness: F32Range::new(0.38, 0.86),
        crater_density: F32Range::new(0.0, 0.18),
        tectonic_activity: F32Range::new(0.12, 0.76),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.03,
        ring_density: F32Range::new(0.1, 0.24),
        ring_inner_radius: F32Range::new(1.85, 2.2),
        ring_outer_radius: F32Range::new(2.25, 3.0),
        ring_inclination_deg: F32Range::new(-6.0, 6.0),
        atmosphere_key: "atmosphere.sulfurous-ash",
        surface_key: "surface.lava-sulfur-basalt",
        terrain_key: "terrain.calderas-lava-floods",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.silicate-melt",
        palette_key: "palette.sulfur-oxide-basalt-lava",
        scattering_key: "scattering.ash-aerosol",
        dominant_gases: GAS_CO2_N2_SO2,
        ring_key: "rings.dark-ejecta",
        ring_color_key: "ring-color.basalt",
        render_model_key: "render.emissive-lava-ash-atmosphere",
        modifier_tags: TAG_VOLCANIC,
    },
    ArchetypeSpec {
        key: "catalog.archetype.temperate-super-earth",
        class: GeneratedPlanetClass::SuperEarth,
        weight: 7,
        radius_km: I32Range::new(9_200, 14_800),
        mass_factor: F32Range::new(1.12, 1.84),
        temperature_c: I32Range::new(-18, 52),
        ocean_fraction: F32Range::new(0.2, 0.7),
        ice_fraction: F32Range::new(0.0, 0.22),
        snow_fraction: F32Range::new(0.0, 0.18),
        salinity: F32Range::new(0.16, 0.56),
        atmosphere_density: F32Range::new(0.62, 1.0),
        pressure_bar: F32Range::new(1.2, 5.2),
        greenhouse_factor: F32Range::new(0.16, 0.48),
        cloud_density: F32Range::new(0.28, 0.72),
        haze: F32Range::new(0.04, 0.34),
        volcanic_activity: F32Range::new(0.06, 0.42),
        albedo: F32Range::new(0.2, 0.44),
        roughness: F32Range::new(0.24, 0.68),
        crater_density: F32Range::new(0.0, 0.12),
        tectonic_activity: F32Range::new(0.22, 0.86),
        vegetation_fraction: F32Range::new(0.0, 0.36),
        ring_chance: 0.11,
        ring_density: F32Range::new(0.18, 0.62),
        ring_inner_radius: F32Range::new(1.55, 2.05),
        ring_outer_radius: F32Range::new(2.4, 4.2),
        ring_inclination_deg: F32Range::new(-12.0, 12.0),
        atmosphere_key: "atmosphere.dense-temperate",
        surface_key: "surface.super-earth-continents",
        terrain_key: "terrain.high-gravity-plates",
        hydrosphere_key: "hydrosphere.liquid-water",
        fluid_key: "fluid.water",
        palette_key: "palette.deep-ocean-forest-oxide-cloud",
        scattering_key: "scattering.dense-rayleigh-mie",
        dominant_gases: GAS_N2_O2_AR,
        ring_key: "rings.massive-ice-rock",
        ring_color_key: "ring-color.ice-silicate",
        render_model_key: "render.pbr-dense-atmosphere-cloud-shadow",
        modifier_tags: TAG_SUPER_EARTH,
    },
    ArchetypeSpec {
        key: "catalog.archetype.sub-neptune-haze",
        class: GeneratedPlanetClass::GasDwarf,
        weight: 6,
        radius_km: I32Range::new(13_000, 25_000),
        mass_factor: F32Range::new(0.16, 0.5),
        temperature_c: I32Range::new(-80, 180),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.28),
        snow_fraction: F32Range::new(0.0, 0.08),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.78, 1.0),
        pressure_bar: F32Range::new(8.0, 80.0),
        greenhouse_factor: F32Range::new(0.2, 0.72),
        cloud_density: F32Range::new(0.42, 0.92),
        haze: F32Range::new(0.34, 0.9),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.18, 0.58),
        roughness: F32Range::new(0.0, 0.08),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.18,
        ring_density: F32Range::new(0.12, 0.5),
        ring_inner_radius: F32Range::new(1.45, 1.85),
        ring_outer_radius: F32Range::new(2.0, 3.4),
        ring_inclination_deg: F32Range::new(-16.0, 16.0),
        atmosphere_key: "atmosphere.hydrogen-helium-haze",
        surface_key: "surface.none-cloud-deck",
        terrain_key: "terrain.bandless-clouds",
        hydrosphere_key: "hydrosphere.deep-volatile",
        fluid_key: "fluid.supercritical-water-ammonia",
        palette_key: "palette.peach-haze-slate",
        scattering_key: "scattering.tholin-haze",
        dominant_gases: GAS_H2_HE_CH4,
        ring_key: "rings.faint-volatile-dust",
        ring_color_key: "ring-color.taupe-ice",
        render_model_key: "render.gas-cloud-haze-limb",
        modifier_tags: TAG_GAS_DWARF,
    },
    ArchetypeSpec {
        key: "catalog.archetype.banded-gas-giant",
        class: GeneratedPlanetClass::GasGiant,
        weight: 6,
        radius_km: I32Range::new(48_000, 82_000),
        mass_factor: F32Range::new(0.02, 0.08),
        temperature_c: I32Range::new(-160, 70),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.92, 1.0),
        pressure_bar: F32Range::new(100.0, 600.0),
        greenhouse_factor: F32Range::new(0.12, 0.42),
        cloud_density: F32Range::new(0.72, 1.0),
        haze: F32Range::new(0.1, 0.5),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.28, 0.62),
        roughness: F32Range::new(0.0, 0.03),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.42,
        ring_density: F32Range::new(0.18, 0.72),
        ring_inner_radius: F32Range::new(1.35, 1.8),
        ring_outer_radius: F32Range::new(2.2, 4.8),
        ring_inclination_deg: F32Range::new(-18.0, 18.0),
        atmosphere_key: "atmosphere.hydrogen-helium-banded",
        surface_key: "surface.none-cloud-belts",
        terrain_key: "terrain.zonal-belts-storms",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.metallic-hydrogen-depth",
        palette_key: "palette.amber-cream-russet-storm",
        scattering_key: "scattering.gas-giant-limb",
        dominant_gases: GAS_H2_HE_NH3,
        ring_key: "rings.ice-rock-broad",
        ring_color_key: "ring-color.warm-ice-dust",
        render_model_key: "render.gas-bands-storms-rings",
        modifier_tags: TAG_GAS_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.hot-jupiter-clouds",
        class: GeneratedPlanetClass::HotJupiter,
        weight: 4,
        radius_km: I32Range::new(72_000, 154_000),
        mass_factor: F32Range::new(0.008, 0.045),
        temperature_c: I32Range::new(720, 1_850),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.94, 1.0),
        pressure_bar: F32Range::new(180.0, 1_400.0),
        greenhouse_factor: F32Range::new(0.38, 0.88),
        cloud_density: F32Range::new(0.62, 1.0),
        haze: F32Range::new(0.36, 0.98),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.06, 0.46),
        roughness: F32Range::new(0.0, 0.03),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.1,
        ring_density: F32Range::new(0.08, 0.34),
        ring_inner_radius: F32Range::new(1.35, 1.7),
        ring_outer_radius: F32Range::new(1.9, 3.0),
        ring_inclination_deg: F32Range::new(-8.0, 8.0),
        atmosphere_key: "atmosphere.hot-hydrogen-metal-vapor",
        surface_key: "surface.none-irradiated-cloud-deck",
        terrain_key: "terrain.sheared-equatorial-metal-clouds",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.metallic-hydrogen-silicate-vapor",
        palette_key: "palette.hot-jupiter-charcoal-crimson-metal-clouds",
        scattering_key: "scattering.irradiated-alkali-absorption",
        dominant_gases: GAS_H2_HE_NA_K_SIO,
        ring_key: "rings.evaporating-dust",
        ring_color_key: "ring-color.dark-silicate",
        render_model_key: "render.gas-bands-storms-rings-hot-metal-vapor",
        modifier_tags: TAG_HOT_GAS_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.cold-ammonia-jupiter",
        class: GeneratedPlanetClass::ColdJupiter,
        weight: 5,
        radius_km: I32Range::new(52_000, 96_000),
        mass_factor: F32Range::new(0.018, 0.074),
        temperature_c: I32Range::new(-225, -85),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.94, 1.0),
        pressure_bar: F32Range::new(140.0, 820.0),
        greenhouse_factor: F32Range::new(0.04, 0.28),
        cloud_density: F32Range::new(0.78, 1.0),
        haze: F32Range::new(0.08, 0.42),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.44, 0.76),
        roughness: F32Range::new(0.0, 0.03),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.5,
        ring_density: F32Range::new(0.18, 0.68),
        ring_inner_radius: F32Range::new(1.36, 1.82),
        ring_outer_radius: F32Range::new(2.15, 4.65),
        ring_inclination_deg: F32Range::new(-18.0, 18.0),
        atmosphere_key: "atmosphere.cold-ammonia-hydrogen-sulfide",
        surface_key: "surface.none-ammonia-storm-belts",
        terrain_key: "terrain.ammonia-bands-oval-storms",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.metallic-hydrogen-ammonia-depth",
        palette_key: "palette.cold-jupiter-methane-ammonia-cream-blue-gray",
        scattering_key: "scattering.ammonia-cloud-forward",
        dominant_gases: GAS_H2_HE_NH3_H2S,
        ring_key: "rings.ice-rock-broad",
        ring_color_key: "ring-color.cold-ice-dust",
        render_model_key: "render.gas-bands-storms-rings-methane-ammonia",
        modifier_tags: TAG_COLD_GAS_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.saturn-ring-giant",
        class: GeneratedPlanetClass::SaturnLike,
        weight: 4,
        radius_km: I32Range::new(56_000, 118_000),
        mass_factor: F32Range::new(0.01, 0.044),
        temperature_c: I32Range::new(-190, -65),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.9, 1.0),
        pressure_bar: F32Range::new(95.0, 620.0),
        greenhouse_factor: F32Range::new(0.04, 0.24),
        cloud_density: F32Range::new(0.64, 0.94),
        haze: F32Range::new(0.18, 0.58),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.48, 0.82),
        roughness: F32Range::new(0.0, 0.025),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.86,
        ring_density: F32Range::new(0.62, 1.0),
        ring_inner_radius: F32Range::new(1.18, 1.42),
        ring_outer_radius: F32Range::new(3.3, 6.8),
        ring_inclination_deg: F32Range::new(-24.0, 24.0),
        atmosphere_key: "atmosphere.pale-hydrogen-helium-haze",
        surface_key: "surface.none-pale-ring-shadow-bands",
        terrain_key: "terrain.subtle-zonal-belts-polar-hexagon",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.metallic-hydrogen-helium-depth",
        palette_key: "palette.saturn-sulfur-cream-gold-ring-shadow",
        scattering_key: "scattering.pale-haze-ring-shadow",
        dominant_gases: GAS_H2_HE_NH3,
        ring_key: "rings.bright-wide-ice",
        ring_color_key: "ring-color.bright-ice-silicate",
        render_model_key: "render.gas-bands-storms-rings-sulfur-saturn",
        modifier_tags: TAG_GAS_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.sulfur-rain-gas-giant",
        class: GeneratedPlanetClass::GasGiant,
        weight: 2,
        radius_km: I32Range::new(45_000, 98_000),
        mass_factor: F32Range::new(0.018, 0.08),
        temperature_c: I32Range::new(-35, 340),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.96, 1.0),
        pressure_bar: F32Range::new(180.0, 1_100.0),
        greenhouse_factor: F32Range::new(0.22, 0.64),
        cloud_density: F32Range::new(0.82, 1.0),
        haze: F32Range::new(0.42, 1.0),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.22, 0.68),
        roughness: F32Range::new(0.0, 0.04),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.34,
        ring_density: F32Range::new(0.16, 0.62),
        ring_inner_radius: F32Range::new(1.36, 1.84),
        ring_outer_radius: F32Range::new(2.0, 4.2),
        ring_inclination_deg: F32Range::new(-16.0, 16.0),
        atmosphere_key: "atmosphere.hydrogen-sulfide-acid-storms",
        surface_key: "surface.none-sulfur-rain-cloud-deck",
        terrain_key: "terrain.yellow-acid-belts-brutal-storms",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.metallic-hydrogen-sulfur-rain-depth",
        palette_key: "palette.sulfur-rain-yellow-orange-brown-storm",
        scattering_key: "scattering.sulfur-aerosol-absorption",
        dominant_gases: GAS_H2_HE_NH3_H2S,
        ring_key: "rings.sulfur-ice-dust",
        ring_color_key: "ring-color.sulfur-ice",
        render_model_key: "render.gas-bands-storms-rings-sulfur-rain",
        modifier_tags: TAG_SULFUR_GAS_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.diamond-rain-gas-giant",
        class: GeneratedPlanetClass::GasGiant,
        weight: 2,
        radius_km: I32Range::new(38_000, 94_000),
        mass_factor: F32Range::new(0.024, 0.1),
        temperature_c: I32Range::new(-180, 120),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.94, 1.0),
        pressure_bar: F32Range::new(220.0, 1_500.0),
        greenhouse_factor: F32Range::new(0.1, 0.42),
        cloud_density: F32Range::new(0.72, 1.0),
        haze: F32Range::new(0.18, 0.68),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.16, 0.56),
        roughness: F32Range::new(0.0, 0.035),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.28,
        ring_density: F32Range::new(0.12, 0.5),
        ring_inner_radius: F32Range::new(1.38, 1.86),
        ring_outer_radius: F32Range::new(2.0, 4.1),
        ring_inclination_deg: F32Range::new(-14.0, 14.0),
        atmosphere_key: "atmosphere.methane-carbon-diamond-rain",
        surface_key: "surface.none-dark-methane-cloud-deck",
        terrain_key: "terrain.deep-carbon-storm-belts",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.metallic-hydrogen-diamond-rain-depth",
        palette_key: "palette.diamond-rain-indigo-charcoal-bright-plume",
        scattering_key: "scattering.methane-carbon-absorption",
        dominant_gases: GAS_H2_HE_CH4,
        ring_key: "rings.dark-ice-carbon-dust",
        ring_color_key: "ring-color.carbon-ice",
        render_model_key: "render.gas-bands-storms-rings-diamond-rain",
        modifier_tags: TAG_DIAMOND_RAIN_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.helium-cloud-giant",
        class: GeneratedPlanetClass::HeliumGiant,
        weight: 2,
        radius_km: I32Range::new(38_000, 108_000),
        mass_factor: F32Range::new(0.014, 0.07),
        temperature_c: I32Range::new(120, 1_300),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.82, 1.0),
        pressure_bar: F32Range::new(80.0, 900.0),
        greenhouse_factor: F32Range::new(0.12, 0.46),
        cloud_density: F32Range::new(0.38, 0.86),
        haze: F32Range::new(0.04, 0.38),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.22, 0.58),
        roughness: F32Range::new(0.0, 0.025),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.24,
        ring_density: F32Range::new(0.12, 0.44),
        ring_inner_radius: F32Range::new(1.4, 1.86),
        ring_outer_radius: F32Range::new(2.0, 3.9),
        ring_inclination_deg: F32Range::new(-14.0, 14.0),
        atmosphere_key: "atmosphere.helium-dominated-stripped",
        surface_key: "surface.none-helium-cloud-deck",
        terrain_key: "terrain.pearl-helium-bands",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.helium-metallic-hydrogen-depth",
        palette_key: "palette.helium-pearl-silver-gold",
        scattering_key: "scattering.helium-thin-limb",
        dominant_gases: GAS_HE_H2_CO,
        ring_key: "rings.faint-ice-dust",
        ring_color_key: "ring-color.pale-ice",
        render_model_key: "render.gas-bands-storms-rings-helium",
        modifier_tags: TAG_HELIUM_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.puffy-haze-giant",
        class: GeneratedPlanetClass::PuffyGiant,
        weight: 2,
        radius_km: I32Range::new(86_000, 170_000),
        mass_factor: F32Range::new(0.004, 0.022),
        temperature_c: I32Range::new(360, 1_600),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.92, 1.0),
        pressure_bar: F32Range::new(30.0, 720.0),
        greenhouse_factor: F32Range::new(0.28, 0.72),
        cloud_density: F32Range::new(0.76, 1.0),
        haze: F32Range::new(0.52, 1.0),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.18, 0.62),
        roughness: F32Range::new(0.0, 0.02),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.16,
        ring_density: F32Range::new(0.08, 0.34),
        ring_inner_radius: F32Range::new(1.4, 1.8),
        ring_outer_radius: F32Range::new(1.9, 3.3),
        ring_inclination_deg: F32Range::new(-10.0, 10.0),
        atmosphere_key: "atmosphere.inflated-hydrogen-tholin-haze",
        surface_key: "surface.none-puffy-haze-cloud-deck",
        terrain_key: "terrain.soft-broad-haze-bands",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.low-density-hydrogen-helium",
        palette_key: "palette.puffy-peach-rose-cream-haze",
        scattering_key: "scattering.extended-haze-limb",
        dominant_gases: GAS_H2_HE_CH4,
        ring_key: "rings.faint-volatile-dust",
        ring_color_key: "ring-color.rose-ice",
        render_model_key: "render.gas-bands-storms-rings-puffy-haze",
        modifier_tags: TAG_PUFFY_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.methane-ice-giant",
        class: GeneratedPlanetClass::IceGiant,
        weight: 5,
        radius_km: I32Range::new(20_000, 34_000),
        mass_factor: F32Range::new(0.08, 0.2),
        temperature_c: I32Range::new(-235, -115),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.2),
        snow_fraction: F32Range::new(0.0, 0.03),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.88, 1.0),
        pressure_bar: F32Range::new(55.0, 360.0),
        greenhouse_factor: F32Range::new(0.04, 0.24),
        cloud_density: F32Range::new(0.48, 0.92),
        haze: F32Range::new(0.08, 0.42),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.28, 0.62),
        roughness: F32Range::new(0.0, 0.035),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.36,
        ring_density: F32Range::new(0.12, 0.56),
        ring_inner_radius: F32Range::new(1.42, 1.92),
        ring_outer_radius: F32Range::new(2.0, 4.0),
        ring_inclination_deg: F32Range::new(-24.0, 24.0),
        atmosphere_key: "atmosphere.methane-blue-high-pressure",
        surface_key: "surface.none-methane-ice-giant-clouds",
        terrain_key: "terrain.deep-blue-bands-dark-spots",
        hydrosphere_key: "hydrosphere.deep-ice-mantle",
        fluid_key: "fluid.water-ammonia-methane-diamond-depth",
        palette_key: "palette.methane-deep-blue-cyan-dark-spot",
        scattering_key: "scattering.methane-absorption-strong",
        dominant_gases: GAS_H2_HE_CH4,
        ring_key: "rings.dark-ice-narrow",
        ring_color_key: "ring-color.blue-gray",
        render_model_key: "render.ice-giant-cloud-gradient-methane-storms",
        modifier_tags: TAG_ICE_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.carbon-diamond",
        class: GeneratedPlanetClass::CarbonWorld,
        weight: 3,
        radius_km: I32Range::new(4_600, 11_400),
        mass_factor: F32Range::new(1.05, 1.9),
        temperature_c: I32Range::new(-80, 180),
        ocean_fraction: F32Range::new(0.0, 0.02),
        ice_fraction: F32Range::new(0.0, 0.04),
        snow_fraction: F32Range::new(0.0, 0.02),
        salinity: F32Range::new(0.0, 0.04),
        atmosphere_density: F32Range::new(0.0, 0.18),
        pressure_bar: F32Range::new(0.0, 0.7),
        greenhouse_factor: F32Range::new(0.0, 0.12),
        cloud_density: F32Range::new(0.0, 0.1),
        haze: F32Range::new(0.0, 0.12),
        volcanic_activity: F32Range::new(0.0, 0.28),
        albedo: F32Range::new(0.02, 0.16),
        roughness: F32Range::new(0.42, 0.88),
        crater_density: F32Range::new(0.16, 0.64),
        tectonic_activity: F32Range::new(0.02, 0.32),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.08,
        ring_density: F32Range::new(0.08, 0.36),
        ring_inner_radius: F32Range::new(1.6, 2.05),
        ring_outer_radius: F32Range::new(2.2, 3.6),
        ring_inclination_deg: F32Range::new(-12.0, 12.0),
        atmosphere_key: "atmosphere.carbon-monoxide-trace",
        surface_key: "surface.graphite-carbide-diamond",
        terrain_key: "terrain.graphite-plains-diamond-ridges",
        hydrosphere_key: "hydrosphere.none",
        fluid_key: "fluid.none",
        palette_key: "palette.graphite-carbide-diamond-glint",
        scattering_key: "scattering.carbon-haze-thin",
        dominant_gases: GAS_CO_CH4_N2,
        ring_key: "rings.carbon-dust",
        ring_color_key: "ring-color.graphite",
        render_model_key: "render.pbr-relief-carbon-diamond-low-albedo",
        modifier_tags: TAG_CARBON_WORLD,
    },
    ArchetypeSpec {
        key: "catalog.archetype.acid-sulfur-clouds",
        class: GeneratedPlanetClass::AcidCloudWorld,
        weight: 3,
        radius_km: I32Range::new(5_400, 11_200),
        mass_factor: F32Range::new(0.9, 1.45),
        temperature_c: I32Range::new(210, 640),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.0),
        snow_fraction: F32Range::new(0.0, 0.0),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.76, 1.0),
        pressure_bar: F32Range::new(18.0, 160.0),
        greenhouse_factor: F32Range::new(0.46, 0.9),
        cloud_density: F32Range::new(0.82, 1.0),
        haze: F32Range::new(0.56, 1.0),
        volcanic_activity: F32Range::new(0.08, 0.58),
        albedo: F32Range::new(0.46, 0.88),
        roughness: F32Range::new(0.24, 0.62),
        crater_density: F32Range::new(0.0, 0.12),
        tectonic_activity: F32Range::new(0.08, 0.54),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.04,
        ring_density: F32Range::new(0.08, 0.24),
        ring_inner_radius: F32Range::new(1.7, 2.1),
        ring_outer_radius: F32Range::new(2.25, 3.0),
        ring_inclination_deg: F32Range::new(-6.0, 6.0),
        atmosphere_key: "atmosphere.sulfuric-acid-opaque",
        surface_key: "surface.acid-obscured-volcanic",
        terrain_key: "terrain.radar-volcanic-highlands-acid-rain",
        hydrosphere_key: "hydrosphere.acid-clouds",
        fluid_key: "fluid.sulfuric-acid-aerosol",
        palette_key: "palette.acid-cream-sulfur-yellow-green-haze",
        scattering_key: "scattering.sulfuric-acid-mie",
        dominant_gases: GAS_CO2_SO2_H2SO4,
        ring_key: "rings.dark-ejecta",
        ring_color_key: "ring-color.sulfur-dust",
        render_model_key: "render.dense-acid-clouds-volcanic-haze",
        modifier_tags: TAG_ACID_CLOUD,
    },
    ArchetypeSpec {
        key: "catalog.archetype.blue-ice-giant",
        class: GeneratedPlanetClass::IceGiant,
        weight: 5,
        radius_km: I32Range::new(21_000, 32_000),
        mass_factor: F32Range::new(0.08, 0.18),
        temperature_c: I32Range::new(-230, -90),
        ocean_fraction: F32Range::new(0.0, 0.0),
        ice_fraction: F32Range::new(0.0, 0.16),
        snow_fraction: F32Range::new(0.0, 0.02),
        salinity: F32Range::new(0.0, 0.0),
        atmosphere_density: F32Range::new(0.86, 1.0),
        pressure_bar: F32Range::new(40.0, 240.0),
        greenhouse_factor: F32Range::new(0.04, 0.22),
        cloud_density: F32Range::new(0.32, 0.84),
        haze: F32Range::new(0.04, 0.34),
        volcanic_activity: F32Range::new(0.0, 0.0),
        albedo: F32Range::new(0.34, 0.68),
        roughness: F32Range::new(0.0, 0.04),
        crater_density: F32Range::new(0.0, 0.0),
        tectonic_activity: F32Range::new(0.0, 0.0),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.32,
        ring_density: F32Range::new(0.1, 0.52),
        ring_inner_radius: F32Range::new(1.45, 1.9),
        ring_outer_radius: F32Range::new(2.0, 3.8),
        ring_inclination_deg: F32Range::new(-22.0, 22.0),
        atmosphere_key: "atmosphere.methane-blue",
        surface_key: "surface.none-ice-giant-clouds",
        terrain_key: "terrain.soft-bands-polar-hood",
        hydrosphere_key: "hydrosphere.deep-ice-mantle",
        fluid_key: "fluid.water-ammonia-methane",
        palette_key: "palette.azure-cyan-deep-blue",
        scattering_key: "scattering.methane-absorption",
        dominant_gases: GAS_H2_HE_CH4,
        ring_key: "rings.dark-ice-narrow",
        ring_color_key: "ring-color.blue-gray",
        render_model_key: "render.ice-giant-cloud-gradient",
        modifier_tags: TAG_ICE_GIANT,
    },
    ArchetypeSpec {
        key: "catalog.archetype.rogue-frozen",
        class: GeneratedPlanetClass::RoguePlanet,
        weight: 3,
        radius_km: I32Range::new(3_400, 16_000),
        mass_factor: F32Range::new(0.7, 1.7),
        temperature_c: I32Range::new(-260, -120),
        ocean_fraction: F32Range::new(0.0, 0.02),
        ice_fraction: F32Range::new(0.32, 0.98),
        snow_fraction: F32Range::new(0.18, 0.84),
        salinity: F32Range::new(0.0, 0.25),
        atmosphere_density: F32Range::new(0.0, 0.42),
        pressure_bar: F32Range::new(0.0, 2.2),
        greenhouse_factor: F32Range::new(0.0, 0.16),
        cloud_density: F32Range::new(0.0, 0.18),
        haze: F32Range::new(0.0, 0.18),
        volcanic_activity: F32Range::new(0.0, 0.18),
        albedo: F32Range::new(0.18, 0.68),
        roughness: F32Range::new(0.22, 0.78),
        crater_density: F32Range::new(0.12, 0.74),
        tectonic_activity: F32Range::new(0.0, 0.24),
        vegetation_fraction: F32Range::new(0.0, 0.0),
        ring_chance: 0.04,
        ring_density: F32Range::new(0.08, 0.24),
        ring_inner_radius: F32Range::new(1.7, 2.2),
        ring_outer_radius: F32Range::new(2.2, 3.3),
        ring_inclination_deg: F32Range::new(-10.0, 10.0),
        atmosphere_key: "atmosphere.collapsed-frost",
        surface_key: "surface.dark-ice-regolith",
        terrain_key: "terrain.frozen-cratered-rifts",
        hydrosphere_key: "hydrosphere.frozen",
        fluid_key: "fluid.water-ice",
        palette_key: "palette.black-ice-violet",
        scattering_key: "scattering.none",
        dominant_gases: GAS_TRACE_EXOSPHERE,
        ring_key: "rings.faint-ice-dust",
        ring_color_key: "ring-color.dark-ice",
        render_model_key: "render.low-light-ice-relief",
        modifier_tags: TAG_ROGUE,
    },
];

const MODIFIERS: &[ModifierSpec] = &[
    modifier(
        "modifier.atmosphere.high-pressure",
        GeneratedModifierFamily::Atmosphere,
        9,
        ModifierRarity::Common,
        &["gas", "rocky", "massive"],
        ModifierEffects {
            atmosphere_delta: 0.12,
            cloud_delta: 0.04,
            greenhouse_delta: 0.05,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.thin-air",
        GeneratedModifierFamily::Atmosphere,
        8,
        ModifierRarity::Common,
        &["rocky", "dry", "cold"],
        ModifierEffects {
            atmosphere_delta: -0.14,
            cloud_delta: -0.05,
            greenhouse_delta: -0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.photochemical-haze",
        GeneratedModifierFamily::Atmosphere,
        7,
        ModifierRarity::Common,
        &["gas", "volatile", "hot", "cold"],
        ModifierEffects {
            haze_delta: 0.18,
            cloud_delta: 0.03,
            atmosphere_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.sulfuric-acid-rain",
        GeneratedModifierFamily::Atmosphere,
        10,
        ModifierRarity::Common,
        &["sulfur", "acid", "toxic", "storm"],
        ModifierEffects {
            atmosphere_delta: 0.08,
            cloud_delta: 0.18,
            haze_delta: 0.18,
            greenhouse_delta: 0.06,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.hydrogen-sulfide-belts",
        GeneratedModifierFamily::Atmosphere,
        9,
        ModifierRarity::Common,
        &["gas", "sulfur", "storm", "toxic"],
        ModifierEffects {
            cloud_delta: 0.14,
            haze_delta: 0.12,
            greenhouse_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.ammonia-megastorms",
        GeneratedModifierFamily::Atmosphere,
        10,
        ModifierRarity::Common,
        &["gas", "ammonia", "storm", "cold"],
        ModifierEffects {
            cloud_delta: 0.16,
            haze_delta: 0.05,
            temperature_delta_c: -6.0,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.silicate-metal-clouds",
        GeneratedModifierFamily::Atmosphere,
        9,
        ModifierRarity::Common,
        &["gas", "hot", "metal-vapor", "silicate"],
        ModifierEffects {
            temperature_delta_c: 38.0,
            cloud_delta: 0.12,
            haze_delta: 0.16,
            greenhouse_delta: 0.08,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.methane-ice-weather",
        GeneratedModifierFamily::Atmosphere,
        8,
        ModifierRarity::Common,
        &["gas", "methane", "ice", "cold"],
        ModifierEffects {
            cloud_delta: 0.1,
            haze_delta: 0.08,
            temperature_delta_c: -10.0,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.extended-puffy-haze",
        GeneratedModifierFamily::Atmosphere,
        9,
        ModifierRarity::Common,
        &["gas", "puffy", "haze", "low-density", "hot"],
        ModifierEffects {
            atmosphere_delta: 0.06,
            cloud_delta: 0.12,
            haze_delta: 0.22,
            greenhouse_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.atmosphere.auroral-oxygen",
        GeneratedModifierFamily::Atmosphere,
        3,
        ModifierRarity::Rare,
        &["rocky", "temperate", "ice"],
        ModifierEffects {
            atmosphere_delta: 0.02,
            haze_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.climate.greenhouse-runaway",
        GeneratedModifierFamily::Climate,
        4,
        ModifierRarity::Uncommon,
        &["hot", "dry", "massive"],
        ModifierEffects {
            temperature_delta_c: 42.0,
            greenhouse_delta: 0.16,
            ocean_delta: -0.08,
            ice_delta: -0.08,
            cloud_delta: 0.08,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.climate.long-winter",
        GeneratedModifierFamily::Climate,
        6,
        ModifierRarity::Common,
        &["cold", "rocky", "ice"],
        ModifierEffects {
            temperature_delta_c: -18.0,
            ice_delta: 0.12,
            snow_delta: 0.14,
            cloud_delta: -0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.climate.monsoon-belts",
        GeneratedModifierFamily::Climate,
        6,
        ModifierRarity::Common,
        &["wet", "temperate", "ocean"],
        ModifierEffects {
            cloud_delta: 0.16,
            vegetation_delta: 0.07,
            ocean_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.climate.day-night-terminator",
        GeneratedModifierFamily::Climate,
        4,
        ModifierRarity::Uncommon,
        &["rocky", "dry", "cold", "hot"],
        ModifierEffects {
            temperature_delta_c: -8.0,
            ice_delta: 0.05,
            haze_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.surface.young-crust",
        GeneratedModifierFamily::Surface,
        7,
        ModifierRarity::Common,
        &["rocky", "volcanic", "temperate"],
        ModifierEffects {
            volcanic_delta: 0.08,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.surface.heavy-cratering",
        GeneratedModifierFamily::Surface,
        8,
        ModifierRarity::Common,
        &["rocky", "dry", "ice", "isolated"],
        ModifierEffects::none(),
    ),
    modifier(
        "modifier.surface.active-plate-network",
        GeneratedModifierFamily::Surface,
        5,
        ModifierRarity::Uncommon,
        &["rocky", "temperate", "massive"],
        ModifierEffects {
            volcanic_delta: 0.05,
            vegetation_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.surface.lava-flood-basins",
        GeneratedModifierFamily::Surface,
        5,
        ModifierRarity::Uncommon,
        &["volcanic", "hot", "rocky"],
        ModifierEffects {
            volcanic_delta: 0.18,
            temperature_delta_c: 8.0,
            haze_delta: 0.06,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.surface.metal-rich-regolith",
        GeneratedModifierFamily::Surface,
        4,
        ModifierRarity::Uncommon,
        &["rocky", "dry"],
        ModifierEffects::none(),
    ),
    modifier(
        "modifier.surface.graphite-diamond-crust",
        GeneratedModifierFamily::Surface,
        10,
        ModifierRarity::Common,
        &["carbon", "diamond", "graphite", "exotic"],
        ModifierEffects {
            atmosphere_delta: -0.02,
            haze_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.surface.carbon-soot-basins",
        GeneratedModifierFamily::Surface,
        8,
        ModifierRarity::Common,
        &["carbon", "diamond", "graphite", "exotic"],
        ModifierEffects {
            haze_delta: 0.05,
            volcanic_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.surface.irradiated-blue-ice",
        GeneratedModifierFamily::Surface,
        8,
        ModifierRarity::Common,
        &["ice", "cold", "methane", "volatile"],
        ModifierEffects {
            ice_delta: 0.1,
            snow_delta: 0.08,
            haze_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.hydrosphere.deep-ocean-basins",
        GeneratedModifierFamily::Hydrosphere,
        6,
        ModifierRarity::Common,
        &["wet", "ocean", "temperate"],
        ModifierEffects {
            ocean_delta: 0.12,
            cloud_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.hydrosphere.salty-inland-seas",
        GeneratedModifierFamily::Hydrosphere,
        5,
        ModifierRarity::Common,
        &["dry", "temperate", "rocky"],
        ModifierEffects {
            ocean_delta: 0.04,
            cloud_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.hydrosphere.subsurface-ocean",
        GeneratedModifierFamily::Hydrosphere,
        6,
        ModifierRarity::Common,
        &["ice", "cold", "isolated"],
        ModifierEffects {
            ice_delta: 0.05,
            ocean_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.hydrosphere.tidal-meltwater",
        GeneratedModifierFamily::Hydrosphere,
        4,
        ModifierRarity::Uncommon,
        &["ice", "volcanic", "cold"],
        ModifierEffects {
            ice_to_ocean: 0.1,
            volcanic_delta: 0.04,
            cloud_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.hydrosphere.ammonia-slush-basins",
        GeneratedModifierFamily::Hydrosphere,
        7,
        ModifierRarity::Uncommon,
        &["ammonia", "ice", "cold", "volatile"],
        ModifierEffects {
            ice_to_ocean: 0.06,
            ice_delta: 0.04,
            cloud_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.orbital.high-obliquity",
        GeneratedModifierFamily::Orbital,
        6,
        ModifierRarity::Common,
        &["rocky", "ice", "wet", "dry"],
        ModifierEffects {
            ice_delta: 0.05,
            snow_delta: 0.08,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.orbital.eccentric-orbit",
        GeneratedModifierFamily::Orbital,
        5,
        ModifierRarity::Common,
        &["rocky", "gas", "ice"],
        ModifierEffects {
            temperature_delta_c: 6.0,
            cloud_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.orbital.tidal-lock",
        GeneratedModifierFamily::Orbital,
        4,
        ModifierRarity::Uncommon,
        &["rocky", "dry", "cold", "hot"],
        ModifierEffects {
            cloud_delta: 0.05,
            ice_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.orbital.recent-impact",
        GeneratedModifierFamily::Orbital,
        3,
        ModifierRarity::Rare,
        &["rocky", "ice", "dry"],
        ModifierEffects {
            haze_delta: 0.12,
            temperature_delta_c: 5.0,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.ring.bright-shepherded-arc",
        GeneratedModifierFamily::Ring,
        5,
        ModifierRarity::Uncommon,
        &["ring-prone", "massive", "gas"],
        ModifierEffects::none(),
    ),
    modifier(
        "modifier.ring.dark-rubble-belt",
        GeneratedModifierFamily::Ring,
        4,
        ModifierRarity::Uncommon,
        &["ring-prone", "rocky", "isolated"],
        ModifierEffects::none(),
    ),
    modifier(
        "modifier.ring.transient-ejecta-ring",
        GeneratedModifierFamily::Ring,
        2,
        ModifierRarity::Rare,
        &["rocky", "ice"],
        ModifierEffects {
            haze_delta: 0.03,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.biosphere.microbial-bloom",
        GeneratedModifierFamily::Biosphere,
        4,
        ModifierRarity::Uncommon,
        &["wet", "temperate", "ocean"],
        ModifierEffects {
            vegetation_delta: 0.14,
            cloud_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.biosphere.dark-vegetation",
        GeneratedModifierFamily::Biosphere,
        2,
        ModifierRarity::Rare,
        &["wet", "temperate"],
        ModifierEffects {
            vegetation_delta: 0.22,
            atmosphere_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.biosphere.reef-continents",
        GeneratedModifierFamily::Biosphere,
        3,
        ModifierRarity::Rare,
        &["wet", "ocean", "temperate"],
        ModifierEffects {
            vegetation_delta: 0.12,
            ocean_delta: 0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.anomaly.crystal-fields",
        GeneratedModifierFamily::Anomaly,
        2,
        ModifierRarity::Rare,
        &["rocky", "ice", "dry"],
        ModifierEffects::none(),
    ),
    modifier(
        "modifier.anomaly.electrical-megastorm",
        GeneratedModifierFamily::Anomaly,
        2,
        ModifierRarity::Rare,
        &["gas", "massive", "volatile"],
        ModifierEffects {
            cloud_delta: 0.12,
            haze_delta: 0.05,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.anomaly.diamond-rain-deep-layer",
        GeneratedModifierFamily::Anomaly,
        5,
        ModifierRarity::Uncommon,
        &["gas", "diamond", "methane", "high-pressure", "storm"],
        ModifierEffects {
            cloud_delta: 0.08,
            haze_delta: 0.06,
            atmosphere_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.anomaly.helium-rain-layering",
        GeneratedModifierFamily::Anomaly,
        4,
        ModifierRarity::Uncommon,
        &["gas", "helium", "massive"],
        ModifierEffects {
            cloud_delta: 0.06,
            haze_delta: -0.02,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.anomaly.hyperstorm-vortex-chain",
        GeneratedModifierFamily::Anomaly,
        5,
        ModifierRarity::Uncommon,
        &["gas", "storm", "massive"],
        ModifierEffects {
            cloud_delta: 0.14,
            haze_delta: 0.04,
            ..ModifierEffects::none()
        },
    ),
    modifier(
        "modifier.anomaly.nightside-glow",
        GeneratedModifierFamily::Anomaly,
        2,
        ModifierRarity::Rare,
        &["volcanic", "isolated", "hot"],
        ModifierEffects {
            volcanic_delta: 0.1,
            ..ModifierEffects::none()
        },
    ),
];

const fn modifier(
    key: &'static str,
    family: GeneratedModifierFamily,
    weight: u32,
    rarity: ModifierRarity,
    tags: &'static [&'static str],
    effects: ModifierEffects,
) -> ModifierSpec {
    ModifierSpec {
        key,
        family,
        weight,
        rarity,
        tags,
        effects,
    }
}

fn select_archetype(input: &ProfileSeedInput) -> ArchetypeSpec {
    if let Some(forced_key) = input.forced_archetype_key.as_deref() {
        if let Some(entry) = PLANET_TYPE_MATRIX
            .iter()
            .find(|entry| entry.key == forced_key)
        {
            return archetype_from_matrix_entry(entry);
        }

        if let Some(archetype) = ARCHETYPES
            .iter()
            .find(|archetype| archetype.key == forced_key)
        {
            return *archetype;
        }
    }

    let weights: Vec<u32> = PLANET_TYPE_MATRIX
        .iter()
        .map(|entry| entry.weight)
        .collect();
    let index = weighted_index(input.seed, ARCHETYPE_SELECTION_SALT, &weights).unwrap_or(0);
    archetype_from_matrix_entry(&PLANET_TYPE_MATRIX[index])
}

fn archetype_from_matrix_entry(entry: &PlanetTypeMatrixEntry) -> ArchetypeSpec {
    let base = find_base_archetype(entry.base_key).unwrap_or(&ARCHETYPES[0]);
    debug_assert!(base.weight > 0);

    ArchetypeSpec {
        key: entry.key,
        class: entry.class,
        weight: entry.weight,
        temperature_c: base
            .temperature_c
            .offset(entry.temperature_delta_c)
            .clamp(-300, 900),
        ocean_fraction: base.ocean_fraction.offset(entry.ocean_delta).clamp01(),
        ice_fraction: base.ice_fraction.offset(entry.ice_delta).clamp01(),
        snow_fraction: base
            .snow_fraction
            .offset((entry.ice_delta * 0.6).max(-0.5))
            .clamp01(),
        atmosphere_density: base
            .atmosphere_density
            .offset(entry.atmosphere_delta)
            .clamp01(),
        greenhouse_factor: base
            .greenhouse_factor
            .offset(entry.greenhouse_delta)
            .clamp01(),
        cloud_density: base.cloud_density.offset(entry.cloud_delta).clamp01(),
        haze: base.haze.offset(entry.haze_delta).clamp01(),
        volcanic_activity: base
            .volcanic_activity
            .offset(entry.volcanic_delta)
            .clamp01(),
        vegetation_fraction: base
            .vegetation_fraction
            .offset(entry.vegetation_delta)
            .clamp01(),
        ring_chance: clamp01(base.ring_chance + entry.ring_chance_delta),
        modifier_tags: entry.modifier_tags,
        ..*base
    }
}

fn find_base_archetype(key: &str) -> Option<&'static ArchetypeSpec> {
    ARCHETYPES.iter().find(|archetype| archetype.key == key)
}

fn select_planet_scale(
    input: &ProfileSeedInput,
    archetype: &ArchetypeSpec,
    base_radius_km: i32,
) -> ScaleSelection {
    let weights = scale_band_weights(archetype.class);
    let band_index = weighted_index(
        input.seed,
        stable_key_hash(archetype.key) ^ SCALE_SELECTION_SALT,
        &weights,
    )
    .unwrap_or(2);
    let spec = SCALE_BAND_SPECS[band_index];
    let mut rng = ProfileRng::new(mix_seed(
        input.seed,
        stable_key_hash(archetype.key) ^ stable_key_hash(spec.band.key()) ^ SCALE_SELECTION_SALT,
    ));
    let radius_km = scaled_radius_km(archetype.radius_km, base_radius_km, spec);
    let density_multiplier = spec.density_multiplier.sample(&mut rng);

    ScaleSelection {
        band: spec.band,
        radius_km,
        radius_scale: round_to(radius_km as f32 / base_radius_km.max(1) as f32, 3),
        density_multiplier,
        atmosphere_delta: spec.atmosphere_delta,
        pressure_multiplier: spec.pressure_multiplier,
        tectonic_delta: spec.tectonic_delta,
    }
}

fn scale_band_weights(class: GeneratedPlanetClass) -> [u32; SCALE_BAND_COUNT] {
    match class {
        GeneratedPlanetClass::DwarfAsteroidLike
        | GeneratedPlanetClass::Exomoon
        | GeneratedPlanetClass::CapturedMoon
        | GeneratedPlanetClass::MercuryLike
        | GeneratedPlanetClass::MarsLike
        | GeneratedPlanetClass::ProtoPlanet => [8, 9, 4, 1, 0, 0],
        GeneratedPlanetClass::BarrenRock
        | GeneratedPlanetClass::IceWorld
        | GeneratedPlanetClass::SnowballWorld
        | GeneratedPlanetClass::IceShellWorld
        | GeneratedPlanetClass::HydrocarbonWorld
        | GeneratedPlanetClass::SulfurIoLike
        | GeneratedPlanetClass::VolcanicWorld
        | GeneratedPlanetClass::RoguePlanet => [4, 7, 7, 3, 1, 0],
        GeneratedPlanetClass::SuperEarth
        | GeneratedPlanetClass::DenseAtmosphereWorld
        | GeneratedPlanetClass::ChthonianWorld
        | GeneratedPlanetClass::ArtificialWorld
        | GeneratedPlanetClass::Ecumenopolis => [0, 2, 5, 7, 7, 2],
        GeneratedPlanetClass::MiniNeptune
        | GeneratedPlanetClass::SubNeptune
        | GeneratedPlanetClass::WaterSteamHycean
        | GeneratedPlanetClass::GasDwarf
        | GeneratedPlanetClass::IceGiant => [0, 2, 5, 7, 5, 1],
        GeneratedPlanetClass::GasGiant
        | GeneratedPlanetClass::HotJupiter
        | GeneratedPlanetClass::ColdJupiter
        | GeneratedPlanetClass::SaturnLike
        | GeneratedPlanetClass::HeliumGiant
        | GeneratedPlanetClass::PuffyGiant => [0, 0, 2, 5, 8, 5],
        _ => [2, 5, 8, 5, 2, 0],
    }
}

fn scaled_radius_km(range: I32Range, base_radius_km: i32, spec: ScaleBandSpec) -> i32 {
    if range.min >= range.max {
        return range.min;
    }

    let width = (range.max - range.min) as f32;
    let base_fraction = ((base_radius_km - range.min) as f32 / width).clamp(0.0, 1.0);
    let scaled_fraction =
        (spec.center_fraction + (base_fraction - 0.5) * spec.spread_fraction).clamp(0.0, 1.0);

    (range.min as f32 + width * scaled_fraction).round() as i32
}

fn select_modifiers(
    input: &ProfileSeedInput,
    archetype: &ArchetypeSpec,
    budget: usize,
) -> Vec<GeneratedModifier> {
    if budget == 0 {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut excluded_keys = Vec::new();
    let mut rng = ProfileRng::new(mix_seed(
        input.seed,
        stable_key_hash(archetype.key) ^ MODIFIER_SELECTION_SALT,
    ));

    let rolls = budget.min(MODIFIERS.len());
    for _ in 0..rolls {
        let Some(spec) = select_modifier_spec(
            &mut rng,
            archetype.modifier_tags,
            input.allow_rare_modifiers,
            &excluded_keys,
        ) else {
            break;
        };

        excluded_keys.push(spec.key);
        selected.push(GeneratedModifier {
            key: spec.key.to_string(),
            family: spec.family,
            intensity: round_to(rng.range_f32(0.35, 1.0), 3),
            tags: spec.tags.iter().map(|tag| (*tag).to_string()).collect(),
        });
    }

    selected
}

fn select_modifier_spec<'a>(
    rng: &mut ProfileRng,
    archetype_tags: &[&str],
    allow_rare_modifiers: bool,
    excluded_keys: &[&str],
) -> Option<&'a ModifierSpec> {
    let mut weighted_indices = Vec::new();
    let mut total_weight = 0_u64;

    for (index, spec) in MODIFIERS.iter().enumerate() {
        if excluded_keys.contains(&spec.key) {
            continue;
        }
        if spec.rarity == ModifierRarity::Rare && !allow_rare_modifiers {
            continue;
        }

        let affinity = tag_affinity(archetype_tags, spec.tags);
        if affinity == 0 {
            continue;
        }

        let rarity_weight = match spec.rarity {
            ModifierRarity::Common => 5,
            ModifierRarity::Uncommon => 3,
            ModifierRarity::Rare => 1,
        };
        let weight = u64::from(spec.weight) * u64::from(affinity) * rarity_weight;
        weighted_indices.push((index, weight));
        total_weight += weight;
    }

    if total_weight == 0 {
        return None;
    }

    let mut roll = rng.range_u64(0, total_weight);
    for (index, weight) in weighted_indices {
        if roll < weight {
            return Some(&MODIFIERS[index]);
        }
        roll -= weight;
    }

    None
}

fn tag_affinity(archetype_tags: &[&str], modifier_tags: &[&str]) -> u32 {
    let mut score = 0_u32;
    for archetype_tag in archetype_tags {
        if modifier_tags.contains(archetype_tag) {
            score += 1;
        }
    }
    score
}

fn clamp01(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn round_to(value: f32, decimals: i32) -> f32 {
    let factor = 10_f32.powi(decimals);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_generate_profile_from_seed() {
        let profile = GeneratedPlanetProfile::from_seed(0x5EED_1208_0001);

        assert_eq!(profile.seed, 0x5EED_1208_0001);
        assert_eq!(profile.algorithm, ALGORITHM_KEY);
        assert!(!profile.archetype_key.is_empty());
        assert!(!profile.palette_key.is_empty());
        assert_eq!(profile.size_key, profile.size_class.key());
        assert_eq!(profile.scale_key, profile.scale_band.key());
        assert!(profile.radius_scale > 0.0);
        assert!(!profile.modifiers.is_empty());
        assert_eq!(profile.modifiers.len(), DEFAULT_MODIFIER_BUDGET);
    }

    #[test]
    fn example_force_catalog_archetype_by_string_key() {
        let profile = generate_planet_profile(
            ProfileSeedInput::new(42).with_archetype_key("catalog.archetype.global-ocean"),
        );

        assert_eq!(profile.archetype_key, "catalog.archetype.global-ocean");
        assert_eq!(profile.planet_class, GeneratedPlanetClass::OceanWorld);
        assert!(profile.hydrosphere.ocean_fraction >= 0.65);
    }

    #[test]
    fn generation_is_stable_for_same_seed() {
        let a = GeneratedPlanetProfile::from_seed(12_345);
        let b = GeneratedPlanetProfile::from_seed(12_345);

        assert_eq!(a, b);
    }

    #[test]
    fn generated_profiles_expose_size_scale_and_physical_hints() {
        let profile = generate_planet_profile(
            ProfileSeedInput::new(123).with_archetype_key("catalog.archetype.temperate-continents"),
        );

        assert_eq!(profile.size_key, profile.size_class.key());
        assert_eq!(profile.scale_key, profile.scale_band.key());
        assert_eq!(
            profile.radius_earth,
            round_to(profile.radius_km as f32 / EARTH_RADIUS_KM, 3)
        );
        assert!(profile.density_earth > 0.0);
        assert_eq!(
            profile.gravity_g,
            round_to(
                profile.mass_earth
                    / (profile.radius_km as f32 / EARTH_RADIUS_KM)
                        .max(0.2)
                        .powi(2),
                3
            )
        );
    }

    #[test]
    fn generated_scale_selection_reaches_user_size_classes() {
        let mut seen_small = false;
        let mut seen_medium = false;
        let mut seen_large = false;

        for seed in 0..1024 {
            let profile = generate_planet_profile(
                ProfileSeedInput::new(seed)
                    .with_archetype_key("catalog.archetype.temperate-continents"),
            );

            match profile.size_class {
                GeneratedPlanetSizeClass::Small => seen_small = true,
                GeneratedPlanetSizeClass::Medium => seen_medium = true,
                GeneratedPlanetSizeClass::Large => seen_large = true,
            }

            if seen_small && seen_medium && seen_large {
                break;
            }
        }

        assert!(seen_small);
        assert!(seen_medium);
        assert!(seen_large);
    }

    #[test]
    fn scale_bands_bias_radius_in_order() {
        let range = I32Range::new(1_000, 2_000);
        let base_radius = 1_500;

        let tiny = scaled_radius_km(range, base_radius, SCALE_BAND_SPECS[0]);
        let standard = scaled_radius_km(range, base_radius, SCALE_BAND_SPECS[2]);
        let massive = scaled_radius_km(range, base_radius, SCALE_BAND_SPECS[4]);
        let colossal = scaled_radius_km(range, base_radius, SCALE_BAND_SPECS[5]);

        assert!(tiny < standard);
        assert!(standard < massive);
        assert!(massive < colossal);
    }

    #[test]
    fn generation_varies_for_different_seeds() {
        let a = GeneratedPlanetProfile::from_seed(12_345);
        let b = GeneratedPlanetProfile::from_seed(12_346);

        assert_ne!(
            (
                a.archetype_key.as_str(),
                a.radius_km,
                a.temperature_c,
                a.modifier_keys()
            ),
            (
                b.archetype_key.as_str(),
                b.radius_km,
                b.temperature_c,
                b.modifier_keys()
            )
        );
    }

    #[test]
    fn deterministic_helpers_select_stable_keys() {
        let archetype_a = select_archetype_key(77);
        let archetype_b = select_archetype_key(77);
        let modifiers_a = select_modifier_keys(77, 5);
        let modifiers_b = select_modifier_keys(77, 5);

        assert_eq!(archetype_a, archetype_b);
        assert_eq!(modifiers_a, modifiers_b);
        assert_eq!(modifiers_a.len(), 5);
    }

    #[test]
    fn rare_modifiers_can_be_disabled() {
        let profile = generate_planet_profile(
            ProfileSeedInput::new(99)
                .with_modifier_budget(20)
                .without_rare_modifiers(),
        );

        for modifier in profile.modifiers {
            let spec = MODIFIERS
                .iter()
                .find(|spec| spec.key == modifier.key)
                .expect("generated modifier should exist in local table");
            assert_ne!(spec.rarity, ModifierRarity::Rare);
        }
    }

    #[test]
    fn expanded_matrix_contains_worker_l_planet_types() {
        let required = [
            (
                "catalog.archetype.mercury-like",
                GeneratedPlanetClass::MercuryLike,
            ),
            (
                "catalog.archetype.mars-like",
                GeneratedPlanetClass::MarsLike,
            ),
            (
                "catalog.archetype.venus-like",
                GeneratedPlanetClass::VenusLike,
            ),
            ("catalog.archetype.ocean", GeneratedPlanetClass::OceanWorld),
            (
                "catalog.archetype.low-water",
                GeneratedPlanetClass::LowWaterWorld,
            ),
            (
                "catalog.archetype.megacontinent",
                GeneratedPlanetClass::MegaContinentWorld,
            ),
            (
                "catalog.archetype.archipelago",
                GeneratedPlanetClass::ArchipelagoWorld,
            ),
            (
                "catalog.archetype.ice-age",
                GeneratedPlanetClass::IceAgeWorld,
            ),
            (
                "catalog.archetype.greenhouse",
                GeneratedPlanetClass::GreenhouseWorld,
            ),
            (
                "catalog.archetype.post-apocalyptic",
                GeneratedPlanetClass::PostApocalypticWorld,
            ),
            (
                "catalog.archetype.active-volcanic",
                GeneratedPlanetClass::ActiveVolcanicWorld,
            ),
            (
                "catalog.archetype.dense-atmosphere",
                GeneratedPlanetClass::DenseAtmosphereWorld,
            ),
            (
                "catalog.archetype.swamp-jungle",
                GeneratedPlanetClass::SwampJungleWorld,
            ),
            (
                "catalog.archetype.desert-dune",
                GeneratedPlanetClass::DesertDuneWorld,
            ),
            (
                "catalog.archetype.snowball",
                GeneratedPlanetClass::SnowballWorld,
            ),
            (
                "catalog.archetype.ice-shell",
                GeneratedPlanetClass::IceShellWorld,
            ),
            (
                "catalog.archetype.frozen-super-earth",
                GeneratedPlanetClass::IceWorld,
            ),
            (
                "catalog.archetype.europa-like-ice-ocean",
                GeneratedPlanetClass::IceShellWorld,
            ),
            (
                "catalog.archetype.enceladus-like-geyser-world",
                GeneratedPlanetClass::IceShellWorld,
            ),
            (
                "catalog.archetype.hydrocarbon-titan-like",
                GeneratedPlanetClass::HydrocarbonWorld,
            ),
            (
                "catalog.archetype.ammonia-world",
                GeneratedPlanetClass::IceWorld,
            ),
            (
                "catalog.archetype.carbon",
                GeneratedPlanetClass::CarbonWorld,
            ),
            (
                "catalog.archetype.diamond-carbon-world",
                GeneratedPlanetClass::CarbonWorld,
            ),
            ("catalog.archetype.iron", GeneratedPlanetClass::IronWorld),
            (
                "catalog.archetype.chthonian",
                GeneratedPlanetClass::ChthonianWorld,
            ),
            (
                "catalog.archetype.lava-magma",
                GeneratedPlanetClass::LavaMagmaWorld,
            ),
            (
                "catalog.archetype.sulfur-io-like",
                GeneratedPlanetClass::SulfurIoLike,
            ),
            (
                "catalog.archetype.acid-cloud",
                GeneratedPlanetClass::AcidCloudWorld,
            ),
            (
                "catalog.archetype.super-earth",
                GeneratedPlanetClass::SuperEarth,
            ),
            (
                "catalog.archetype.mini-neptune",
                GeneratedPlanetClass::MiniNeptune,
            ),
            (
                "catalog.archetype.sub-neptune",
                GeneratedPlanetClass::SubNeptune,
            ),
            (
                "catalog.archetype.water-steam-hycean",
                GeneratedPlanetClass::WaterSteamHycean,
            ),
            (
                "catalog.archetype.gas-dwarf",
                GeneratedPlanetClass::GasDwarf,
            ),
            (
                "catalog.archetype.gas-giant",
                GeneratedPlanetClass::GasGiant,
            ),
            (
                "catalog.archetype.storm-gas-giant",
                GeneratedPlanetClass::GasGiant,
            ),
            (
                "catalog.archetype.sulfur-gas-world",
                GeneratedPlanetClass::GasGiant,
            ),
            (
                "catalog.archetype.diamond-rain-gas-giant",
                GeneratedPlanetClass::GasGiant,
            ),
            (
                "catalog.archetype.ammonia-storm-giant",
                GeneratedPlanetClass::GasGiant,
            ),
            (
                "catalog.archetype.hot-jupiter",
                GeneratedPlanetClass::HotJupiter,
            ),
            (
                "catalog.archetype.hot-neptune",
                GeneratedPlanetClass::MiniNeptune,
            ),
            (
                "catalog.archetype.cold-jupiter",
                GeneratedPlanetClass::ColdJupiter,
            ),
            (
                "catalog.archetype.saturn-like",
                GeneratedPlanetClass::SaturnLike,
            ),
            (
                "catalog.archetype.ice-giant",
                GeneratedPlanetClass::IceGiant,
            ),
            (
                "catalog.archetype.methane-ice-giant",
                GeneratedPlanetClass::IceGiant,
            ),
            (
                "catalog.archetype.uranus-like-ice-giant",
                GeneratedPlanetClass::IceGiant,
            ),
            (
                "catalog.archetype.neptune-like-storm-giant",
                GeneratedPlanetClass::IceGiant,
            ),
            (
                "catalog.archetype.helium-giant",
                GeneratedPlanetClass::HeliumGiant,
            ),
            (
                "catalog.archetype.puffy-giant",
                GeneratedPlanetClass::PuffyGiant,
            ),
            (
                "catalog.archetype.rogue-gas-giant",
                GeneratedPlanetClass::GasGiant,
            ),
            ("catalog.archetype.rogue", GeneratedPlanetClass::RoguePlanet),
            (
                "catalog.archetype.tidally-locked-eyeball",
                GeneratedPlanetClass::TidallyLockedEyeball,
            ),
            (
                "catalog.archetype.twilight-belt",
                GeneratedPlanetClass::TwilightBeltWorld,
            ),
            (
                "catalog.archetype.circumbinary",
                GeneratedPlanetClass::CircumbinaryWorld,
            ),
            (
                "catalog.archetype.eccentric-season",
                GeneratedPlanetClass::EccentricSeasonWorld,
            ),
            (
                "catalog.archetype.proto-planet",
                GeneratedPlanetClass::ProtoPlanet,
            ),
            (
                "catalog.archetype.dwarf-asteroid-like",
                GeneratedPlanetClass::DwarfAsteroidLike,
            ),
            ("catalog.archetype.exomoon", GeneratedPlanetClass::Exomoon),
            (
                "catalog.archetype.captured-moon",
                GeneratedPlanetClass::CapturedMoon,
            ),
            (
                "catalog.archetype.rare-artificial",
                GeneratedPlanetClass::ArtificialWorld,
            ),
            (
                "catalog.archetype.ecumenopolis",
                GeneratedPlanetClass::Ecumenopolis,
            ),
        ];

        assert!(PLANET_TYPE_MATRIX.len() >= required.len());
        for (key, expected_class) in required {
            let profile = generate_planet_profile(ProfileSeedInput::new(1).with_archetype_key(key));
            assert_eq!(profile.archetype_key, key);
            assert_eq!(profile.planet_class, expected_class);
            assert_eq!(profile.class_key, expected_class.key());
        }
    }

    #[test]
    fn extreme_archetypes_expose_stronger_material_and_atmosphere_metadata() {
        let sulfur = generate_planet_profile(
            ProfileSeedInput::new(0x5EED_1208_5101)
                .with_archetype_key("catalog.archetype.sulfur-gas-world")
                .with_modifier_budget(12),
        );
        assert_eq!(sulfur.archetype_key, "catalog.archetype.sulfur-gas-world");
        assert!(sulfur.palette_key.contains("sulfur-rain"));
        assert!(sulfur.hydrosphere.fluid_key.contains("sulfur-rain"));
        assert!(sulfur
            .atmosphere
            .dominant_gases
            .iter()
            .any(|gas| gas == "hydrogen-sulfide"));
        assert!(sulfur
            .modifier_keys()
            .iter()
            .any(|key| key.contains("sulfur") || key.contains("storm")));

        let diamond_giant = generate_planet_profile(
            ProfileSeedInput::new(0x5EED_1208_5102)
                .with_archetype_key("catalog.archetype.diamond-rain-gas-giant")
                .with_modifier_budget(12),
        );
        assert!(diamond_giant.palette_key.contains("diamond-rain"));
        assert!(diamond_giant.hydrosphere.fluid_key.contains("diamond-rain"));
        assert!(diamond_giant
            .modifier_keys()
            .iter()
            .any(|key| key.contains("diamond")));

        let carbon = generate_planet_profile(
            ProfileSeedInput::new(0x5EED_1208_5103)
                .with_archetype_key("catalog.archetype.diamond-carbon-world")
                .with_modifier_budget(12),
        );
        assert_eq!(carbon.planet_class, GeneratedPlanetClass::CarbonWorld);
        assert!(carbon.surface.key.contains("graphite"));
        assert!(carbon.palette_key.contains("diamond"));
        assert!(carbon
            .modifier_keys()
            .iter()
            .any(|key| key.contains("carbon") || key.contains("diamond")));

        let hot_jupiter = generate_planet_profile(
            ProfileSeedInput::new(0x5EED_1208_5104)
                .with_archetype_key("catalog.archetype.hot-jupiter")
                .with_modifier_budget(12),
        );
        assert_eq!(hot_jupiter.planet_class, GeneratedPlanetClass::HotJupiter);
        assert!(hot_jupiter.palette_key.contains("hot-jupiter"));
        assert!(hot_jupiter
            .atmosphere
            .dominant_gases
            .iter()
            .any(|gas| { matches!(gas.as_str(), "sodium" | "potassium" | "silicate-vapor") }));
        assert!(hot_jupiter.temperature_c >= 700);

        let methane_ice = generate_planet_profile(
            ProfileSeedInput::new(0x5EED_1208_5105)
                .with_archetype_key("catalog.archetype.methane-ice-giant")
                .with_modifier_budget(12),
        );
        assert_eq!(methane_ice.planet_class, GeneratedPlanetClass::IceGiant);
        assert!(methane_ice.palette_key.contains("methane"));
        assert!(methane_ice.hydrosphere.fluid_key.contains("diamond"));
        assert!(methane_ice.temperature_c <= -100);
    }

    #[test]
    fn seeded_selection_reaches_a_broad_planet_type_set() {
        let mut selected_keys = Vec::new();

        for seed in 0..512 {
            let key = select_archetype_key(seed);
            if !selected_keys.contains(&key) {
                selected_keys.push(key);
            }
        }

        assert!(selected_keys.len() >= 24, "{:?}", selected_keys);
    }

    #[test]
    fn weighted_index_handles_empty_and_zero_weight_tables() {
        assert_eq!(weighted_index(1, 2, &[]), None);
        assert_eq!(weighted_index(1, 2, &[0, 0, 0]), None);
        assert_eq!(weighted_index(1, 2, &[0, 9, 0]), Some(1));
    }

    #[test]
    fn labels_are_legacy_profile_friendly() {
        let profile = generate_planet_profile(
            ProfileSeedInput::new(7).with_archetype_key("catalog.archetype.temperate-super-earth"),
        );
        let label = profile.legacy_planet_class_label();

        assert!(label.contains("super-earth"));
        assert!(!label.contains("catalog."));
    }
}
