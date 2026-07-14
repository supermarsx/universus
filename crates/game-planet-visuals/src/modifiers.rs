//! Deterministic planet modifier catalog.
//!
//! This module is intentionally self-contained so it can be wired into profile
//! or map generation later without changing the existing renderer surface.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanetModifierKind {
    ThinAir,
    DenseAir,
    MethaneHaze,
    SulfurousSky,
    HighAlbedoCloudDeck,
    DustLoadedAtmosphere,
    AuroraRichMagnetosphere,
    GlobalOcean,
    ArchipelagoSeas,
    BrineFlats,
    FreshwaterBasins,
    CryoOceanCrust,
    TidalMarshes,
    DryRiverbeds,
    YoungGraniteContinents,
    IronRichDeserts,
    CarbonateShelves,
    BasaltShieldPlains,
    FoldedHighlands,
    KarstSinkfields,
    SilicateDunes,
    SuperEarthScale,
    DeepFreeze,
    TemperateWindow,
    RunawayGreenhouse,
    TwilightTerminatorHabitability,
    GeothermalHotspots,
    SeasonalExtremes,
    CryovolcanicColdTraps,
    HighObliquity,
    LowObliquity,
    EccentricOrbit,
    TidallyLocked,
    RapidRotation,
    SlowRotation,
    ResonantDayNightCycle,
    AncientBombardment,
    FreshRayCrater,
    MultiRingBasin,
    CraterChain,
    EjectaBlankets,
    ShatteredHemisphere,
    MicrometeoriteGardening,
    ActivePlateMosaic,
    StagnantLid,
    SupercontinentCycle,
    RiftValleys,
    SubductionArcs,
    TransformFaultWeb,
    UpliftedPlateaus,
    FloodBasaltProvince,
    StratovolcanoChains,
    LavaLakes,
    AshfallFields,
    FumaroleBelts,
    CryovolcanoFields,
    DormantCalderaScars,
    CyanobacteriaBloom,
    ForestContinents,
    CoralReefBelts,
    DesertLichenCrust,
    BioluminescentOceans,
    SeasonalVegetationPulse,
    MicrobialMats,
    MegacityNightside,
    OrbitalElevatorGlints,
    AgriculturalPatchwork,
    TerraformingMirrors,
    RuinedInfrastructure,
    IndustrialAerosols,
    NavigationBeaconGrid,
    NuclearWinter,
    SterilizingFlareScars,
    OceanBoiloff,
    GreyGooPatchwork,
    ImpactWinter,
    CollapsedEcumenopolis,
    BiosphereDieback,
    AmmoniaClouds,
    HydrocarbonLakes,
    SulfurDioxideFrost,
    SulfuricAcidRain,
    HydrogenSulfideStormBands,
    ChlorineAtmosphere,
    MetallicSnow,
    SilicateMetalClouds,
    DiamondRainDeepLayer,
    HeliumRainLayering,
    PeroxideIce,
    CarbonSootDeposits,
    GraphiteDiamondCrust,
    IrradiatedBlueIce,
    EquatorialJetBands,
    GreatOvalStorm,
    PolarHexagon,
    BandShearFilaments,
    LightningStormTowers,
    AmmoniaStormCells,
    BrutalJetStreamStorms,
    MethaneIceWeather,
    DeepBlueNeptuneBands,
    BroadIceRings,
    ThinDustRings,
    ShepherdMoons,
    CapturedAsteroidMoon,
    TidalHeatingMoon,
    RingShadowStripes,
    BrokenMoonDebris,
    RedOxidePalette,
    JadeMineralStreaks,
    SaltPanMosaics,
    BlackGlassFields,
    PaleAeolianRipples,
    BlueIceFractures,
    GoldenSavannaBands,
    StrongAtmosphericRim,
    SubtleAtmosphericRim,
    HighReliefShading,
    SmoothOceanSpecular,
    CityLightBloom,
    CloudShadowEmphasis,
    LowAlbedoExposureCompensation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModifierCategory {
    Atmosphere,
    Hydrosphere,
    Geology,
    Thermal,
    Orbital,
    ImpactCrater,
    Tectonic,
    Volcanic,
    Biological,
    Civilization,
    CatastrophicPostApocalyptic,
    ExoticChemistry,
    GasGiantBandsStorms,
    RingsMoons,
    SurfaceTextureColor,
    RenderingHints,
}

impl ModifierCategory {
    pub const ALL: [Self; 16] = [
        Self::Atmosphere,
        Self::Hydrosphere,
        Self::Geology,
        Self::Thermal,
        Self::Orbital,
        Self::ImpactCrater,
        Self::Tectonic,
        Self::Volcanic,
        Self::Biological,
        Self::Civilization,
        Self::CatastrophicPostApocalyptic,
        Self::ExoticChemistry,
        Self::GasGiantBandsStorms,
        Self::RingsMoons,
        Self::SurfaceTextureColor,
        Self::RenderingHints,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Atmosphere => "atmosphere",
            Self::Hydrosphere => "hydrosphere",
            Self::Geology => "geology",
            Self::Thermal => "thermal",
            Self::Orbital => "orbital",
            Self::ImpactCrater => "impact/crater",
            Self::Tectonic => "tectonic",
            Self::Volcanic => "volcanic",
            Self::Biological => "biological",
            Self::Civilization => "civilization",
            Self::CatastrophicPostApocalyptic => "catastrophic/post-apocalyptic",
            Self::ExoticChemistry => "exotic chemistry",
            Self::GasGiantBandsStorms => "gas giant bands/storms",
            Self::RingsMoons => "rings/moons",
            Self::SurfaceTextureColor => "surface texture/color",
            Self::RenderingHints => "rendering hints",
        }
    }

    #[cfg(test)]
    const fn index(self) -> usize {
        match self {
            Self::Atmosphere => 0,
            Self::Hydrosphere => 1,
            Self::Geology => 2,
            Self::Thermal => 3,
            Self::Orbital => 4,
            Self::ImpactCrater => 5,
            Self::Tectonic => 6,
            Self::Volcanic => 7,
            Self::Biological => 8,
            Self::Civilization => 9,
            Self::CatastrophicPostApocalyptic => 10,
            Self::ExoticChemistry => 11,
            Self::GasGiantBandsStorms => 12,
            Self::RingsMoons => 13,
            Self::SurfaceTextureColor => 14,
            Self::RenderingHints => 15,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModifierEffect {
    Delta {
        target: &'static str,
        amount: f32,
    },
    Multiplier {
        target: &'static str,
        factor: f32,
    },
    Clamp {
        target: &'static str,
        min: f32,
        max: f32,
    },
    Tag(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModifierDefinition {
    pub kind: PlanetModifierKind,
    pub id: &'static str,
    pub name: &'static str,
    pub category: ModifierCategory,
    pub weight: u16,
    pub effects: &'static [ModifierEffect],
}

pub const MODIFIER_CATALOG_COUNT: usize = CATALOG.len();

pub fn modifier_catalog() -> &'static [ModifierDefinition] {
    CATALOG
}

macro_rules! modifier {
    ($kind:ident, $id:literal, $name:literal, $category:ident, $weight:literal, [$($effect:expr),+ $(,)?]) => {
        ModifierDefinition {
            kind: PlanetModifierKind::$kind,
            id: $id,
            name: $name,
            category: ModifierCategory::$category,
            weight: $weight,
            effects: &[$($effect),+],
        }
    };
}

const CATALOG: &[ModifierDefinition] = &[
    modifier!(
        ThinAir,
        "thin_air",
        "Thin Air",
        Atmosphere,
        90,
        [
            ModifierEffect::Multiplier {
                target: "atmosphere_density",
                factor: 0.42
            },
            ModifierEffect::Multiplier {
                target: "cloud_density",
                factor: 0.58
            },
            ModifierEffect::Tag("thin_atmosphere"),
        ]
    ),
    modifier!(
        DenseAir,
        "dense_air",
        "Dense Air",
        Atmosphere,
        82,
        [
            ModifierEffect::Multiplier {
                target: "atmosphere_density",
                factor: 1.58
            },
            ModifierEffect::Delta {
                target: "greenhouse_c",
                amount: 18.0
            },
            ModifierEffect::Clamp {
                target: "atmosphere_density",
                min: 0.55,
                max: 2.25
            },
        ]
    ),
    modifier!(
        MethaneHaze,
        "methane_haze",
        "Methane Haze",
        Atmosphere,
        58,
        [
            ModifierEffect::Delta {
                target: "haze_density",
                amount: 0.32
            },
            ModifierEffect::Multiplier {
                target: "rayleigh_blue",
                factor: 0.72
            },
            ModifierEffect::Tag("amber_haze"),
        ]
    ),
    modifier!(
        SulfurousSky,
        "sulfurous_sky",
        "Sulfurous Sky",
        Atmosphere,
        52,
        [
            ModifierEffect::Delta {
                target: "sulfur_aerosol",
                amount: 0.46
            },
            ModifierEffect::Multiplier {
                target: "cloud_yellow_tint",
                factor: 1.34
            },
            ModifierEffect::Tag("acid_clouds"),
        ]
    ),
    modifier!(
        HighAlbedoCloudDeck,
        "high_albedo_cloud_deck",
        "High-Albedo Cloud Deck",
        Atmosphere,
        76,
        [
            ModifierEffect::Delta {
                target: "cloud_density",
                amount: 0.28
            },
            ModifierEffect::Multiplier {
                target: "planet_albedo",
                factor: 1.18
            },
            ModifierEffect::Clamp {
                target: "cloud_density",
                min: 0.35,
                max: 0.95
            },
        ]
    ),
    modifier!(
        DustLoadedAtmosphere,
        "dust_loaded_atmosphere",
        "Dust-Loaded Atmosphere",
        Atmosphere,
        64,
        [
            ModifierEffect::Delta {
                target: "dust_opacity",
                amount: 0.38
            },
            ModifierEffect::Multiplier {
                target: "surface_contrast",
                factor: 0.82
            },
            ModifierEffect::Tag("dust_haze"),
        ]
    ),
    modifier!(
        AuroraRichMagnetosphere,
        "aurora_rich_magnetosphere",
        "Aurora-Rich Magnetosphere",
        Atmosphere,
        45,
        [
            ModifierEffect::Multiplier {
                target: "magnetosphere_strength",
                factor: 1.75
            },
            ModifierEffect::Delta {
                target: "aurora_intensity",
                amount: 0.62
            },
            ModifierEffect::Tag("polar_aurora"),
        ]
    ),
    modifier!(
        GlobalOcean,
        "global_ocean",
        "Global Ocean",
        Hydrosphere,
        48,
        [
            ModifierEffect::Delta {
                target: "ocean_fraction",
                amount: 0.36
            },
            ModifierEffect::Multiplier {
                target: "land_fraction",
                factor: 0.28
            },
            ModifierEffect::Clamp {
                target: "ocean_fraction",
                min: 0.72,
                max: 0.98
            },
        ]
    ),
    modifier!(
        ArchipelagoSeas,
        "archipelago_seas",
        "Archipelago Seas",
        Hydrosphere,
        78,
        [
            ModifierEffect::Delta {
                target: "ocean_fraction",
                amount: 0.18
            },
            ModifierEffect::Multiplier {
                target: "coastline_complexity",
                factor: 1.62
            },
            ModifierEffect::Tag("island_chains"),
        ]
    ),
    modifier!(
        BrineFlats,
        "brine_flats",
        "Brine Flats",
        Hydrosphere,
        60,
        [
            ModifierEffect::Delta {
                target: "salinity",
                amount: 0.42
            },
            ModifierEffect::Multiplier {
                target: "shallow_water_albedo",
                factor: 1.28
            },
            ModifierEffect::Tag("salt_crusts"),
        ]
    ),
    modifier!(
        FreshwaterBasins,
        "freshwater_basins",
        "Freshwater Basins",
        Hydrosphere,
        70,
        [
            ModifierEffect::Delta {
                target: "lake_density",
                amount: 0.24
            },
            ModifierEffect::Multiplier {
                target: "river_network_density",
                factor: 1.35
            },
            ModifierEffect::Tag("inland_lakes"),
        ]
    ),
    modifier!(
        CryoOceanCrust,
        "cryo_ocean_crust",
        "Cryo-Ocean Crust",
        Hydrosphere,
        42,
        [
            ModifierEffect::Delta {
                target: "ice_fraction",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "ocean_surface_visibility",
                factor: 0.35
            },
            ModifierEffect::Tag("subsurface_ocean"),
        ]
    ),
    modifier!(
        TidalMarshes,
        "tidal_marshes",
        "Tidal Marshes",
        Hydrosphere,
        66,
        [
            ModifierEffect::Multiplier {
                target: "tidal_range",
                factor: 1.85
            },
            ModifierEffect::Delta {
                target: "wetland_fraction",
                amount: 0.20
            },
            ModifierEffect::Tag("mudflat_coasts"),
        ]
    ),
    modifier!(
        DryRiverbeds,
        "dry_riverbeds",
        "Dry Riverbeds",
        Hydrosphere,
        62,
        [
            ModifierEffect::Multiplier {
                target: "ocean_fraction",
                factor: 0.62
            },
            ModifierEffect::Delta {
                target: "erosion_channels",
                amount: 0.31
            },
            ModifierEffect::Tag("seasonal_drainage"),
        ]
    ),
    modifier!(
        YoungGraniteContinents,
        "young_granite_continents",
        "Young Granite Continents",
        Geology,
        58,
        [
            ModifierEffect::Delta {
                target: "continent_elevation",
                amount: 0.18
            },
            ModifierEffect::Multiplier {
                target: "crust_buoyancy",
                factor: 1.22
            },
            ModifierEffect::Tag("light_continental_crust"),
        ]
    ),
    modifier!(
        IronRichDeserts,
        "iron_rich_deserts",
        "Iron-Rich Deserts",
        Geology,
        76,
        [
            ModifierEffect::Delta {
                target: "red_oxide_tint",
                amount: 0.38
            },
            ModifierEffect::Multiplier {
                target: "soil_brightness",
                factor: 0.92
            },
            ModifierEffect::Tag("oxidized_regolith"),
            ModifierEffect::Tag("metal_rich_regolith"),
        ]
    ),
    modifier!(
        CarbonateShelves,
        "carbonate_shelves",
        "Carbonate Shelves",
        Geology,
        54,
        [
            ModifierEffect::Delta {
                target: "continental_shelf_width",
                amount: 0.27
            },
            ModifierEffect::Multiplier {
                target: "shallow_water_brightness",
                factor: 1.18
            },
            ModifierEffect::Tag("limestone_coasts"),
        ]
    ),
    modifier!(
        BasaltShieldPlains,
        "basalt_shield_plains",
        "Basalt Shield Plains",
        Geology,
        68,
        [
            ModifierEffect::Delta {
                target: "basalt_fraction",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "surface_albedo",
                factor: 0.78
            },
            ModifierEffect::Tag("dark_lava_plains"),
        ]
    ),
    modifier!(
        FoldedHighlands,
        "folded_highlands",
        "Folded Highlands",
        Geology,
        60,
        [
            ModifierEffect::Multiplier {
                target: "ridge_frequency",
                factor: 1.42
            },
            ModifierEffect::Delta {
                target: "height_scale",
                amount: 0.16
            },
            ModifierEffect::Tag("fold_belts"),
        ]
    ),
    modifier!(
        KarstSinkfields,
        "karst_sinkfields",
        "Karst Sinkfields",
        Geology,
        44,
        [
            ModifierEffect::Delta {
                target: "sinkhole_density",
                amount: 0.28
            },
            ModifierEffect::Multiplier {
                target: "surface_roughness",
                factor: 1.12
            },
            ModifierEffect::Tag("dissolved_limestone"),
        ]
    ),
    modifier!(
        SilicateDunes,
        "silicate_dunes",
        "Silicate Dunes",
        Geology,
        72,
        [
            ModifierEffect::Delta {
                target: "dune_coverage",
                amount: 0.30
            },
            ModifierEffect::Multiplier {
                target: "fine_noise_scale",
                factor: 1.25
            },
            ModifierEffect::Tag("wind_sculpted_sand"),
        ]
    ),
    modifier!(
        SuperEarthScale,
        "super_earth_scale",
        "Super-Earth Scale",
        Geology,
        36,
        [
            ModifierEffect::Multiplier {
                target: "planet_radius",
                factor: 1.45
            },
            ModifierEffect::Multiplier {
                target: "surface_gravity",
                factor: 1.62
            },
            ModifierEffect::Tag("super_earth"),
        ]
    ),
    modifier!(
        DeepFreeze,
        "deep_freeze",
        "Deep Freeze",
        Thermal,
        56,
        [
            ModifierEffect::Delta {
                target: "temperature_c",
                amount: -72.0
            },
            ModifierEffect::Delta {
                target: "ice_fraction",
                amount: 0.44
            },
            ModifierEffect::Clamp {
                target: "temperature_c",
                min: -240.0,
                max: -15.0
            },
        ]
    ),
    modifier!(
        TemperateWindow,
        "temperate_window",
        "Temperate Window",
        Thermal,
        84,
        [
            ModifierEffect::Clamp {
                target: "temperature_c",
                min: -5.0,
                max: 35.0
            },
            ModifierEffect::Multiplier {
                target: "habitability_score",
                factor: 1.24
            },
            ModifierEffect::Tag("liquid_water_stable"),
            ModifierEffect::Tag("habitable_zone"),
        ]
    ),
    modifier!(
        RunawayGreenhouse,
        "runaway_greenhouse",
        "Runaway Greenhouse",
        Thermal,
        34,
        [
            ModifierEffect::Delta {
                target: "temperature_c",
                amount: 155.0
            },
            ModifierEffect::Multiplier {
                target: "cloud_density",
                factor: 1.38
            },
            ModifierEffect::Tag("runaway_greenhouse"),
        ]
    ),
    modifier!(
        TwilightTerminatorHabitability,
        "twilight_terminator_habitability",
        "Twilight Terminator Habitability",
        Thermal,
        38,
        [
            ModifierEffect::Delta {
                target: "terminator_habitability",
                amount: 0.52
            },
            ModifierEffect::Multiplier {
                target: "day_night_temperature_gradient",
                factor: 1.65
            },
            ModifierEffect::Tag("twilight_belt"),
        ]
    ),
    modifier!(
        GeothermalHotspots,
        "geothermal_hotspots",
        "Geothermal Hotspots",
        Thermal,
        62,
        [
            ModifierEffect::Delta {
                target: "geothermal_flux",
                amount: 0.31
            },
            ModifierEffect::Multiplier {
                target: "polar_ice_persistence",
                factor: 0.86
            },
            ModifierEffect::Tag("thermal_oases"),
        ]
    ),
    modifier!(
        SeasonalExtremes,
        "seasonal_extremes",
        "Seasonal Extremes",
        Thermal,
        58,
        [
            ModifierEffect::Multiplier {
                target: "seasonal_temperature_amplitude",
                factor: 1.85
            },
            ModifierEffect::Delta {
                target: "ice_line_variability",
                amount: 0.33
            },
            ModifierEffect::Tag("strong_seasons"),
        ]
    ),
    modifier!(
        CryovolcanicColdTraps,
        "cryovolcanic_cold_traps",
        "Cryovolcanic Cold Traps",
        Thermal,
        40,
        [
            ModifierEffect::Delta {
                target: "volatile_frost_fraction",
                amount: 0.29
            },
            ModifierEffect::Multiplier {
                target: "night_side_albedo",
                factor: 1.16
            },
            ModifierEffect::Tag("cold_trap_deposits"),
        ]
    ),
    modifier!(
        HighObliquity,
        "high_obliquity",
        "High Obliquity",
        Orbital,
        48,
        [
            ModifierEffect::Delta {
                target: "axial_tilt_degrees",
                amount: 56.0
            },
            ModifierEffect::Multiplier {
                target: "seasonal_temperature_amplitude",
                factor: 1.72
            },
            ModifierEffect::Tag("wandering_poles"),
        ]
    ),
    modifier!(
        LowObliquity,
        "low_obliquity",
        "Low Obliquity",
        Orbital,
        64,
        [
            ModifierEffect::Clamp {
                target: "axial_tilt_degrees",
                min: 0.0,
                max: 8.0
            },
            ModifierEffect::Multiplier {
                target: "seasonal_temperature_amplitude",
                factor: 0.35
            },
            ModifierEffect::Tag("stable_poles"),
        ]
    ),
    modifier!(
        EccentricOrbit,
        "eccentric_orbit",
        "Eccentric Orbit",
        Orbital,
        42,
        [
            ModifierEffect::Delta {
                target: "orbital_eccentricity",
                amount: 0.22
            },
            ModifierEffect::Multiplier {
                target: "annual_insolation_variance",
                factor: 1.95
            },
            ModifierEffect::Tag("elliptical_year"),
        ]
    ),
    modifier!(
        TidallyLocked,
        "tidally_locked",
        "Tidally Locked",
        Orbital,
        36,
        [
            ModifierEffect::Multiplier {
                target: "rotation_rate",
                factor: 0.0
            },
            ModifierEffect::Multiplier {
                target: "day_night_temperature_gradient",
                factor: 2.25
            },
            ModifierEffect::Tag("synchronous_rotation"),
        ]
    ),
    modifier!(
        RapidRotation,
        "rapid_rotation",
        "Rapid Rotation",
        Orbital,
        70,
        [
            ModifierEffect::Multiplier {
                target: "rotation_rate",
                factor: 2.65
            },
            ModifierEffect::Multiplier {
                target: "jet_stream_strength",
                factor: 1.55
            },
            ModifierEffect::Tag("flattened_oblate_profile"),
        ]
    ),
    modifier!(
        SlowRotation,
        "slow_rotation",
        "Slow Rotation",
        Orbital,
        58,
        [
            ModifierEffect::Multiplier {
                target: "rotation_rate",
                factor: 0.32
            },
            ModifierEffect::Multiplier {
                target: "thermal_inertia_visibility",
                factor: 1.42
            },
            ModifierEffect::Tag("long_days"),
        ]
    ),
    modifier!(
        ResonantDayNightCycle,
        "resonant_day_night_cycle",
        "Resonant Day-Night Cycle",
        Orbital,
        44,
        [
            ModifierEffect::Delta {
                target: "spin_orbit_resonance",
                amount: 1.5
            },
            ModifierEffect::Multiplier {
                target: "terminator_cloud_banding",
                factor: 1.38
            },
            ModifierEffect::Tag("orbital_resonance"),
        ]
    ),
    modifier!(
        AncientBombardment,
        "ancient_bombardment",
        "Ancient Bombardment",
        ImpactCrater,
        72,
        [
            ModifierEffect::Delta {
                target: "crater_density",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "erosion_softening",
                factor: 1.18
            },
            ModifierEffect::Tag("old_cratered_terrain"),
        ]
    ),
    modifier!(
        FreshRayCrater,
        "fresh_ray_crater",
        "Fresh Ray Crater",
        ImpactCrater,
        38,
        [
            ModifierEffect::Delta {
                target: "bright_ejecta_rays",
                amount: 0.47
            },
            ModifierEffect::Multiplier {
                target: "local_albedo_contrast",
                factor: 1.42
            },
            ModifierEffect::Tag("fresh_impact"),
        ]
    ),
    modifier!(
        MultiRingBasin,
        "multi_ring_basin",
        "Multi-Ring Basin",
        ImpactCrater,
        32,
        [
            ModifierEffect::Delta {
                target: "large_basin_frequency",
                amount: 0.20
            },
            ModifierEffect::Multiplier {
                target: "radial_fault_visibility",
                factor: 1.55
            },
            ModifierEffect::Tag("multi_ring_impact"),
        ]
    ),
    modifier!(
        CraterChain,
        "crater_chain",
        "Crater Chain",
        ImpactCrater,
        44,
        [
            ModifierEffect::Delta {
                target: "linear_crater_frequency",
                amount: 0.26
            },
            ModifierEffect::Multiplier {
                target: "impact_alignment_strength",
                factor: 1.74
            },
            ModifierEffect::Tag("fragmented_impactor"),
        ]
    ),
    modifier!(
        EjectaBlankets,
        "ejecta_blankets",
        "Ejecta Blankets",
        ImpactCrater,
        58,
        [
            ModifierEffect::Delta {
                target: "ejecta_coverage",
                amount: 0.31
            },
            ModifierEffect::Multiplier {
                target: "fine_surface_noise",
                factor: 1.19
            },
            ModifierEffect::Tag("impact_ejecta"),
        ]
    ),
    modifier!(
        ShatteredHemisphere,
        "shattered_hemisphere",
        "Shattered Hemisphere",
        ImpactCrater,
        18,
        [
            ModifierEffect::Delta {
                target: "hemisphere_asymmetry",
                amount: 0.62
            },
            ModifierEffect::Multiplier {
                target: "fault_scarring",
                factor: 1.88
            },
            ModifierEffect::Tag("giant_impact_damage"),
        ]
    ),
    modifier!(
        MicrometeoriteGardening,
        "micrometeorite_gardening",
        "Micrometeorite Gardening",
        ImpactCrater,
        66,
        [
            ModifierEffect::Multiplier {
                target: "regolith_depth",
                factor: 1.46
            },
            ModifierEffect::Multiplier {
                target: "small_crater_sharpness",
                factor: 0.72
            },
            ModifierEffect::Tag("powdered_regolith"),
        ]
    ),
    modifier!(
        ActivePlateMosaic,
        "active_plate_mosaic",
        "Active Plate Mosaic",
        Tectonic,
        64,
        [
            ModifierEffect::Multiplier {
                target: "plate_count",
                factor: 1.45
            },
            ModifierEffect::Delta {
                target: "tectonic_activity",
                amount: 0.34
            },
            ModifierEffect::Tag("plate_boundaries"),
        ]
    ),
    modifier!(
        StagnantLid,
        "stagnant_lid",
        "Stagnant Lid",
        Tectonic,
        58,
        [
            ModifierEffect::Multiplier {
                target: "plate_count",
                factor: 0.20
            },
            ModifierEffect::Multiplier {
                target: "volcanic_activity",
                factor: 1.22
            },
            ModifierEffect::Tag("single_lithosphere_shell"),
        ]
    ),
    modifier!(
        SupercontinentCycle,
        "supercontinent_cycle",
        "Supercontinent Cycle",
        Tectonic,
        46,
        [
            ModifierEffect::Delta {
                target: "continent_clustering",
                amount: 0.44
            },
            ModifierEffect::Multiplier {
                target: "coastline_complexity",
                factor: 0.82
            },
            ModifierEffect::Tag("supercontinent"),
        ]
    ),
    modifier!(
        RiftValleys,
        "rift_valleys",
        "Rift Valleys",
        Tectonic,
        60,
        [
            ModifierEffect::Delta {
                target: "rift_density",
                amount: 0.31
            },
            ModifierEffect::Multiplier {
                target: "linear_valley_depth",
                factor: 1.42
            },
            ModifierEffect::Tag("continental_rifting"),
        ]
    ),
    modifier!(
        SubductionArcs,
        "subduction_arcs",
        "Subduction Arcs",
        Tectonic,
        56,
        [
            ModifierEffect::Delta {
                target: "arc_volcano_density",
                amount: 0.29
            },
            ModifierEffect::Multiplier {
                target: "trench_depth",
                factor: 1.36
            },
            ModifierEffect::Tag("oceanic_subduction"),
        ]
    ),
    modifier!(
        TransformFaultWeb,
        "transform_fault_web",
        "Transform Fault Web",
        Tectonic,
        50,
        [
            ModifierEffect::Delta {
                target: "fault_line_density",
                amount: 0.36
            },
            ModifierEffect::Multiplier {
                target: "linear_surface_scars",
                factor: 1.47
            },
            ModifierEffect::Tag("strike_slip_faults"),
        ]
    ),
    modifier!(
        UpliftedPlateaus,
        "uplifted_plateaus",
        "Uplifted Plateaus",
        Tectonic,
        54,
        [
            ModifierEffect::Delta {
                target: "plateau_fraction",
                amount: 0.26
            },
            ModifierEffect::Multiplier {
                target: "mean_land_elevation",
                factor: 1.18
            },
            ModifierEffect::Tag("uplifted_crust"),
        ]
    ),
    modifier!(
        FloodBasaltProvince,
        "flood_basalt_province",
        "Flood Basalt Province",
        Volcanic,
        48,
        [
            ModifierEffect::Delta {
                target: "basalt_fraction",
                amount: 0.46
            },
            ModifierEffect::Multiplier {
                target: "surface_albedo",
                factor: 0.74
            },
            ModifierEffect::Tag("large_igneous_province"),
        ]
    ),
    modifier!(
        StratovolcanoChains,
        "stratovolcano_chains",
        "Stratovolcano Chains",
        Volcanic,
        58,
        [
            ModifierEffect::Delta {
                target: "volcano_cone_density",
                amount: 0.32
            },
            ModifierEffect::Multiplier {
                target: "mountain_shadow_contrast",
                factor: 1.18
            },
            ModifierEffect::Tag("volcanic_arcs"),
        ]
    ),
    modifier!(
        LavaLakes,
        "lava_lakes",
        "Lava Lakes",
        Volcanic,
        28,
        [
            ModifierEffect::Delta {
                target: "lava_emissive_intensity",
                amount: 0.58
            },
            ModifierEffect::Multiplier {
                target: "volcanic_activity",
                factor: 1.72
            },
            ModifierEffect::Tag("incandescent_lava"),
        ]
    ),
    modifier!(
        AshfallFields,
        "ashfall_fields",
        "Ashfall Fields",
        Volcanic,
        50,
        [
            ModifierEffect::Delta {
                target: "ash_coverage",
                amount: 0.36
            },
            ModifierEffect::Multiplier {
                target: "vegetation_fraction",
                factor: 0.74
            },
            ModifierEffect::Tag("gray_pyroclastic_surface"),
        ]
    ),
    modifier!(
        FumaroleBelts,
        "fumarole_belts",
        "Fumarole Belts",
        Volcanic,
        44,
        [
            ModifierEffect::Delta {
                target: "steam_plume_density",
                amount: 0.28
            },
            ModifierEffect::Multiplier {
                target: "sulfur_stain_visibility",
                factor: 1.36
            },
            ModifierEffect::Tag("hydrothermal_fields"),
        ]
    ),
    modifier!(
        CryovolcanoFields,
        "cryovolcano_fields",
        "Cryovolcano Fields",
        Volcanic,
        34,
        [
            ModifierEffect::Delta {
                target: "cryovolcano_density",
                amount: 0.33
            },
            ModifierEffect::Multiplier {
                target: "ice_fracture_visibility",
                factor: 1.44
            },
            ModifierEffect::Tag("volatile_eruptions"),
        ]
    ),
    modifier!(
        DormantCalderaScars,
        "dormant_caldera_scars",
        "Dormant Caldera Scars",
        Volcanic,
        62,
        [
            ModifierEffect::Delta {
                target: "caldera_density",
                amount: 0.25
            },
            ModifierEffect::Multiplier {
                target: "volcanic_activity",
                factor: 0.76
            },
            ModifierEffect::Tag("eroded_calderas"),
        ]
    ),
    modifier!(
        CyanobacteriaBloom,
        "cyanobacteria_bloom",
        "Cyanobacteria Bloom",
        Biological,
        54,
        [
            ModifierEffect::Delta {
                target: "ocean_bio_tint",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "oxygen_signal",
                factor: 1.28
            },
            ModifierEffect::Tag("blue_green_bloom"),
        ]
    ),
    modifier!(
        ForestContinents,
        "forest_continents",
        "Forest Continents",
        Biological,
        70,
        [
            ModifierEffect::Delta {
                target: "vegetation_fraction",
                amount: 0.42
            },
            ModifierEffect::Multiplier {
                target: "land_green_tint",
                factor: 1.38
            },
            ModifierEffect::Clamp {
                target: "temperature_c",
                min: -8.0,
                max: 42.0
            },
        ]
    ),
    modifier!(
        CoralReefBelts,
        "coral_reef_belts",
        "Coral Reef Belts",
        Biological,
        46,
        [
            ModifierEffect::Delta {
                target: "reef_density",
                amount: 0.31
            },
            ModifierEffect::Multiplier {
                target: "tropical_shallow_water_color",
                factor: 1.45
            },
            ModifierEffect::Tag("reef_rings"),
        ]
    ),
    modifier!(
        DesertLichenCrust,
        "desert_lichen_crust",
        "Desert Lichen Crust",
        Biological,
        44,
        [
            ModifierEffect::Delta {
                target: "dryland_biofilm_fraction",
                amount: 0.24
            },
            ModifierEffect::Multiplier {
                target: "desert_saturation",
                factor: 0.88
            },
            ModifierEffect::Tag("cryptobiotic_soil"),
        ]
    ),
    modifier!(
        BioluminescentOceans,
        "bioluminescent_oceans",
        "Bioluminescent Oceans",
        Biological,
        26,
        [
            ModifierEffect::Delta {
                target: "night_ocean_emission",
                amount: 0.46
            },
            ModifierEffect::Multiplier {
                target: "coastal_glow_visibility",
                factor: 1.62
            },
            ModifierEffect::Tag("bioluminescent_waves"),
        ]
    ),
    modifier!(
        SeasonalVegetationPulse,
        "seasonal_vegetation_pulse",
        "Seasonal Vegetation Pulse",
        Biological,
        58,
        [
            ModifierEffect::Multiplier {
                target: "vegetation_seasonal_shift",
                factor: 1.78
            },
            ModifierEffect::Delta {
                target: "land_color_variance",
                amount: 0.21
            },
            ModifierEffect::Tag("seasonal_biosphere"),
        ]
    ),
    modifier!(
        MicrobialMats,
        "microbial_mats",
        "Microbial Mats",
        Biological,
        52,
        [
            ModifierEffect::Delta {
                target: "wetland_biofilm_fraction",
                amount: 0.29
            },
            ModifierEffect::Multiplier {
                target: "shoreline_color_complexity",
                factor: 1.18
            },
            ModifierEffect::Tag("microbial_shores"),
        ]
    ),
    modifier!(
        MegacityNightside,
        "megacity_nightside",
        "Megacity Nightside",
        Civilization,
        28,
        [
            ModifierEffect::Delta {
                target: "city_light_intensity",
                amount: 0.62
            },
            ModifierEffect::Multiplier {
                target: "urban_coverage",
                factor: 1.82
            },
            ModifierEffect::Tag("ecumenopolis_lights"),
        ]
    ),
    modifier!(
        OrbitalElevatorGlints,
        "orbital_elevator_glints",
        "Orbital Elevator Glints",
        Civilization,
        18,
        [
            ModifierEffect::Delta {
                target: "orbital_structure_visibility",
                amount: 0.38
            },
            ModifierEffect::Multiplier {
                target: "specular_glint_intensity",
                factor: 1.54
            },
            ModifierEffect::Tag("orbital_elevator"),
        ]
    ),
    modifier!(
        AgriculturalPatchwork,
        "agricultural_patchwork",
        "Agricultural Patchwork",
        Civilization,
        56,
        [
            ModifierEffect::Delta {
                target: "farmland_grid_visibility",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "vegetation_uniformity",
                factor: 1.24
            },
            ModifierEffect::Tag("cultivated_land"),
        ]
    ),
    modifier!(
        TerraformingMirrors,
        "terraforming_mirrors",
        "Terraforming Mirrors",
        Civilization,
        14,
        [
            ModifierEffect::Delta {
                target: "orbital_mirror_brightness",
                amount: 0.52
            },
            ModifierEffect::Multiplier {
                target: "polar_insolation",
                factor: 1.32
            },
            ModifierEffect::Tag("terraforming_infrastructure"),
        ]
    ),
    modifier!(
        RuinedInfrastructure,
        "ruined_infrastructure",
        "Ruined Infrastructure",
        Civilization,
        34,
        [
            ModifierEffect::Delta {
                target: "ruin_linework_density",
                amount: 0.27
            },
            ModifierEffect::Multiplier {
                target: "city_light_intensity",
                factor: 0.22
            },
            ModifierEffect::Tag("abandoned_cities"),
        ]
    ),
    modifier!(
        IndustrialAerosols,
        "industrial_aerosols",
        "Industrial Aerosols",
        Civilization,
        42,
        [
            ModifierEffect::Delta {
                target: "aerosol_opacity",
                amount: 0.30
            },
            ModifierEffect::Multiplier {
                target: "atmosphere_warm_tint",
                factor: 1.22
            },
            ModifierEffect::Tag("industrial_pollution"),
        ]
    ),
    modifier!(
        NavigationBeaconGrid,
        "navigation_beacon_grid",
        "Navigation Beacon Grid",
        Civilization,
        22,
        [
            ModifierEffect::Delta {
                target: "beacon_light_density",
                amount: 0.41
            },
            ModifierEffect::Multiplier {
                target: "night_side_point_lights",
                factor: 1.36
            },
            ModifierEffect::Tag("orbital_navigation"),
        ]
    ),
    modifier!(
        NuclearWinter,
        "nuclear_winter",
        "Nuclear Winter",
        CatastrophicPostApocalyptic,
        16,
        [
            ModifierEffect::Delta {
                target: "stratospheric_soot",
                amount: 0.68
            },
            ModifierEffect::Delta {
                target: "temperature_c",
                amount: -38.0
            },
            ModifierEffect::Tag("post_nuclear_haze"),
        ]
    ),
    modifier!(
        SterilizingFlareScars,
        "sterilizing_flare_scars",
        "Sterilizing Flare Scars",
        CatastrophicPostApocalyptic,
        18,
        [
            ModifierEffect::Delta {
                target: "radiation_scarring",
                amount: 0.48
            },
            ModifierEffect::Multiplier {
                target: "vegetation_fraction",
                factor: 0.18
            },
            ModifierEffect::Tag("flare_burned_surface"),
        ]
    ),
    modifier!(
        OceanBoiloff,
        "ocean_boiloff",
        "Ocean Boiloff",
        CatastrophicPostApocalyptic,
        12,
        [
            ModifierEffect::Multiplier {
                target: "ocean_fraction",
                factor: 0.12
            },
            ModifierEffect::Delta {
                target: "steam_atmosphere",
                amount: 0.54
            },
            ModifierEffect::Tag("desiccated_seabeds"),
        ]
    ),
    modifier!(
        GreyGooPatchwork,
        "grey_goo_patchwork",
        "Grey Goo Patchwork",
        CatastrophicPostApocalyptic,
        10,
        [
            ModifierEffect::Delta {
                target: "artificial_surface_fraction",
                amount: 0.63
            },
            ModifierEffect::Multiplier {
                target: "organic_biosignature",
                factor: 0.05
            },
            ModifierEffect::Tag("self_replicator_scars"),
        ]
    ),
    modifier!(
        ImpactWinter,
        "impact_winter",
        "Impact Winter",
        CatastrophicPostApocalyptic,
        20,
        [
            ModifierEffect::Delta {
                target: "impact_dust_opacity",
                amount: 0.58
            },
            ModifierEffect::Delta {
                target: "temperature_c",
                amount: -28.0
            },
            ModifierEffect::Tag("recent_global_impact"),
        ]
    ),
    modifier!(
        CollapsedEcumenopolis,
        "collapsed_ecumenopolis",
        "Collapsed Ecumenopolis",
        CatastrophicPostApocalyptic,
        14,
        [
            ModifierEffect::Delta {
                target: "ruin_coverage",
                amount: 0.74
            },
            ModifierEffect::Multiplier {
                target: "city_light_intensity",
                factor: 0.08
            },
            ModifierEffect::Tag("planetary_ruins"),
        ]
    ),
    modifier!(
        BiosphereDieback,
        "biosphere_dieback",
        "Biosphere Dieback",
        CatastrophicPostApocalyptic,
        24,
        [
            ModifierEffect::Multiplier {
                target: "vegetation_fraction",
                factor: 0.26
            },
            ModifierEffect::Delta {
                target: "dead_biomass_tint",
                amount: 0.39
            },
            ModifierEffect::Tag("dying_biosphere"),
        ]
    ),
    modifier!(
        AmmoniaClouds,
        "ammonia_clouds",
        "Ammonia Clouds",
        ExoticChemistry,
        44,
        [
            ModifierEffect::Delta {
                target: "ammonia_cloud_fraction",
                amount: 0.48
            },
            ModifierEffect::Multiplier {
                target: "cloud_brightness",
                factor: 1.22
            },
            ModifierEffect::Tag("ammonia_weather"),
        ]
    ),
    modifier!(
        HydrocarbonLakes,
        "hydrocarbon_lakes",
        "Hydrocarbon Lakes",
        ExoticChemistry,
        38,
        [
            ModifierEffect::Delta {
                target: "hydrocarbon_liquid_fraction",
                amount: 0.36
            },
            ModifierEffect::Multiplier {
                target: "surface_specular",
                factor: 1.20
            },
            ModifierEffect::Tag("methane_ethane_lakes"),
        ]
    ),
    modifier!(
        SulfurDioxideFrost,
        "sulfur_dioxide_frost",
        "Sulfur Dioxide Frost",
        ExoticChemistry,
        36,
        [
            ModifierEffect::Delta {
                target: "sulfur_frost_fraction",
                amount: 0.31
            },
            ModifierEffect::Multiplier {
                target: "yellow_white_surface_tint",
                factor: 1.28
            },
            ModifierEffect::Tag("volatile_sulfur_frost"),
        ]
    ),
    modifier!(
        SulfuricAcidRain,
        "sulfuric_acid_rain",
        "Sulfuric Acid Rain",
        ExoticChemistry,
        30,
        [
            ModifierEffect::Delta {
                target: "sulfuric_acid_rain_opacity",
                amount: 0.52
            },
            ModifierEffect::Multiplier {
                target: "cloud_yellow_tint",
                factor: 1.42
            },
            ModifierEffect::Tag("sulfur_rain"),
            ModifierEffect::Tag("acid_precipitation"),
        ]
    ),
    modifier!(
        HydrogenSulfideStormBands,
        "hydrogen_sulfide_storm_bands",
        "Hydrogen Sulfide Storm Bands",
        ExoticChemistry,
        28,
        [
            ModifierEffect::Delta {
                target: "hydrogen_sulfide_fraction",
                amount: 0.44
            },
            ModifierEffect::Multiplier {
                target: "storm_band_contrast",
                factor: 1.50
            },
            ModifierEffect::Tag("toxic_gas_giant_weather"),
            ModifierEffect::Tag("sulfur_belts"),
        ]
    ),
    modifier!(
        ChlorineAtmosphere,
        "chlorine_atmosphere",
        "Chlorine Atmosphere",
        ExoticChemistry,
        22,
        [
            ModifierEffect::Delta {
                target: "chlorine_haze",
                amount: 0.43
            },
            ModifierEffect::Multiplier {
                target: "green_atmosphere_tint",
                factor: 1.46
            },
            ModifierEffect::Tag("toxic_halogen_air"),
        ]
    ),
    modifier!(
        MetallicSnow,
        "metallic_snow",
        "Metallic Snow",
        ExoticChemistry,
        20,
        [
            ModifierEffect::Delta {
                target: "metallic_frost_fraction",
                amount: 0.28
            },
            ModifierEffect::Multiplier {
                target: "specular_microfacet",
                factor: 1.38
            },
            ModifierEffect::Tag("metal_vapor_cycle"),
        ]
    ),
    modifier!(
        SilicateMetalClouds,
        "silicate_metal_clouds",
        "Silicate Metal Clouds",
        ExoticChemistry,
        24,
        [
            ModifierEffect::Delta {
                target: "silicate_cloud_fraction",
                amount: 0.46
            },
            ModifierEffect::Multiplier {
                target: "alkali_absorption",
                factor: 1.36
            },
            ModifierEffect::Tag("metal_vapor_clouds"),
            ModifierEffect::Tag("hot_jupiter_weather"),
        ]
    ),
    modifier!(
        DiamondRainDeepLayer,
        "diamond_rain_deep_layer",
        "Diamond Rain Deep Layer",
        ExoticChemistry,
        18,
        [
            ModifierEffect::Delta {
                target: "deep_carbon_precipitation",
                amount: 0.55
            },
            ModifierEffect::Multiplier {
                target: "methane_absorption",
                factor: 1.28
            },
            ModifierEffect::Tag("diamond_rain"),
            ModifierEffect::Tag("high_pressure_carbon"),
        ]
    ),
    modifier!(
        HeliumRainLayering,
        "helium_rain_layering",
        "Helium Rain Layering",
        ExoticChemistry,
        18,
        [
            ModifierEffect::Delta {
                target: "helium_phase_separation",
                amount: 0.48
            },
            ModifierEffect::Multiplier {
                target: "subtle_band_layering",
                factor: 1.28
            },
            ModifierEffect::Tag("helium_rain"),
            ModifierEffect::Tag("stratified_giant"),
        ]
    ),
    modifier!(
        PeroxideIce,
        "peroxide_ice",
        "Peroxide Ice",
        ExoticChemistry,
        34,
        [
            ModifierEffect::Delta {
                target: "peroxide_ice_fraction",
                amount: 0.30
            },
            ModifierEffect::Multiplier {
                target: "uv_brightness",
                factor: 1.32
            },
            ModifierEffect::Tag("irradiated_ice"),
        ]
    ),
    modifier!(
        CarbonSootDeposits,
        "carbon_soot_deposits",
        "Carbon Soot Deposits",
        ExoticChemistry,
        40,
        [
            ModifierEffect::Delta {
                target: "soot_coverage",
                amount: 0.37
            },
            ModifierEffect::Multiplier {
                target: "surface_albedo",
                factor: 0.58
            },
            ModifierEffect::Tag("carbonaceous_residue"),
        ]
    ),
    modifier!(
        GraphiteDiamondCrust,
        "graphite_diamond_crust",
        "Graphite Diamond Crust",
        ExoticChemistry,
        26,
        [
            ModifierEffect::Delta {
                target: "diamond_facet_fraction",
                amount: 0.32
            },
            ModifierEffect::Multiplier {
                target: "carbon_crust_contrast",
                factor: 1.42
            },
            ModifierEffect::Tag("graphite_plains"),
            ModifierEffect::Tag("diamond_ridges"),
        ]
    ),
    modifier!(
        IrradiatedBlueIce,
        "irradiated_blue_ice",
        "Irradiated Blue Ice",
        ExoticChemistry,
        30,
        [
            ModifierEffect::Delta {
                target: "blue_ice_tint",
                amount: 0.42
            },
            ModifierEffect::Multiplier {
                target: "ice_fracture_visibility",
                factor: 1.36
            },
            ModifierEffect::Tag("methane_ice_weathering"),
            ModifierEffect::Tag("frozen_world_material"),
        ]
    ),
    modifier!(
        EquatorialJetBands,
        "equatorial_jet_bands",
        "Equatorial Jet Bands",
        GasGiantBandsStorms,
        66,
        [
            ModifierEffect::Delta {
                target: "band_count",
                amount: 7.0
            },
            ModifierEffect::Multiplier {
                target: "zonal_wind_contrast",
                factor: 1.45
            },
            ModifierEffect::Tag("gas_giant_bands"),
        ]
    ),
    modifier!(
        GreatOvalStorm,
        "great_oval_storm",
        "Great Oval Storm",
        GasGiantBandsStorms,
        36,
        [
            ModifierEffect::Delta {
                target: "major_storm_count",
                amount: 1.0
            },
            ModifierEffect::Multiplier {
                target: "storm_color_contrast",
                factor: 1.58
            },
            ModifierEffect::Tag("persistent_anticyclone"),
        ]
    ),
    modifier!(
        PolarHexagon,
        "polar_hexagon",
        "Polar Hexagon",
        GasGiantBandsStorms,
        24,
        [
            ModifierEffect::Delta {
                target: "polar_wave_symmetry",
                amount: 6.0
            },
            ModifierEffect::Multiplier {
                target: "polar_vortex_visibility",
                factor: 1.52
            },
            ModifierEffect::Tag("hexagonal_jet"),
        ]
    ),
    modifier!(
        BandShearFilaments,
        "band_shear_filaments",
        "Band Shear Filaments",
        GasGiantBandsStorms,
        54,
        [
            ModifierEffect::Delta {
                target: "filament_density",
                amount: 0.42
            },
            ModifierEffect::Multiplier {
                target: "band_edge_noise",
                factor: 1.36
            },
            ModifierEffect::Tag("sheared_cloud_filaments"),
        ]
    ),
    modifier!(
        LightningStormTowers,
        "lightning_storm_towers",
        "Lightning Storm Towers",
        GasGiantBandsStorms,
        34,
        [
            ModifierEffect::Delta {
                target: "lightning_flash_density",
                amount: 0.36
            },
            ModifierEffect::Multiplier {
                target: "convective_cloud_height",
                factor: 1.48
            },
            ModifierEffect::Tag("deep_convective_storms"),
        ]
    ),
    modifier!(
        AmmoniaStormCells,
        "ammonia_storm_cells",
        "Ammonia Storm Cells",
        GasGiantBandsStorms,
        42,
        [
            ModifierEffect::Delta {
                target: "small_storm_cell_density",
                amount: 0.39
            },
            ModifierEffect::Multiplier {
                target: "cream_cloud_tint",
                factor: 1.24
            },
            ModifierEffect::Tag("ammonia_cellular_weather"),
        ]
    ),
    modifier!(
        BrutalJetStreamStorms,
        "brutal_jet_stream_storms",
        "Brutal Jet Stream Storms",
        GasGiantBandsStorms,
        40,
        [
            ModifierEffect::Delta {
                target: "jet_stream_speed",
                amount: 0.58
            },
            ModifierEffect::Multiplier {
                target: "storm_shear_contrast",
                factor: 1.62
            },
            ModifierEffect::Tag("brutal_storms"),
            ModifierEffect::Tag("hyperstorm_chains"),
        ]
    ),
    modifier!(
        MethaneIceWeather,
        "methane_ice_weather",
        "Methane Ice Weather",
        GasGiantBandsStorms,
        32,
        [
            ModifierEffect::Delta {
                target: "methane_cloud_opacity",
                amount: 0.40
            },
            ModifierEffect::Multiplier {
                target: "deep_blue_absorption",
                factor: 1.44
            },
            ModifierEffect::Tag("ice_giant_weather"),
            ModifierEffect::Tag("dark_spots"),
        ]
    ),
    modifier!(
        DeepBlueNeptuneBands,
        "deep_blue_neptune_bands",
        "Deep Blue Neptune Bands",
        GasGiantBandsStorms,
        28,
        [
            ModifierEffect::Multiplier {
                target: "blue_atmosphere_tint",
                factor: 1.72
            },
            ModifierEffect::Delta {
                target: "high_altitude_methane_absorption",
                amount: 0.44
            },
            ModifierEffect::Tag("ice_giant_palette"),
        ]
    ),
    modifier!(
        BroadIceRings,
        "broad_ice_rings",
        "Broad Ice Rings",
        RingsMoons,
        32,
        [
            ModifierEffect::Delta {
                target: "ring_opacity",
                amount: 0.46
            },
            ModifierEffect::Multiplier {
                target: "ring_width",
                factor: 1.88
            },
            ModifierEffect::Tag("bright_ice_rings"),
            ModifierEffect::Tag("ringed_planet"),
        ]
    ),
    modifier!(
        ThinDustRings,
        "thin_dust_rings",
        "Thin Dust Rings",
        RingsMoons,
        46,
        [
            ModifierEffect::Delta {
                target: "ring_opacity",
                amount: 0.16
            },
            ModifierEffect::Multiplier {
                target: "ring_width",
                factor: 0.48
            },
            ModifierEffect::Tag("faint_dust_rings"),
        ]
    ),
    modifier!(
        ShepherdMoons,
        "shepherd_moons",
        "Shepherd Moons",
        RingsMoons,
        38,
        [
            ModifierEffect::Delta {
                target: "small_moon_count",
                amount: 2.0
            },
            ModifierEffect::Multiplier {
                target: "ring_gap_sharpness",
                factor: 1.68
            },
            ModifierEffect::Tag("shepherded_ring_gaps"),
        ]
    ),
    modifier!(
        CapturedAsteroidMoon,
        "captured_asteroid_moon",
        "Captured Asteroid Moon",
        RingsMoons,
        54,
        [
            ModifierEffect::Delta {
                target: "irregular_moon_count",
                amount: 1.0
            },
            ModifierEffect::Multiplier {
                target: "moon_albedo",
                factor: 0.62
            },
            ModifierEffect::Tag("captured_irregular_moon"),
        ]
    ),
    modifier!(
        TidalHeatingMoon,
        "tidal_heating_moon",
        "Tidal Heating Moon",
        RingsMoons,
        30,
        [
            ModifierEffect::Delta {
                target: "tidal_heating",
                amount: 0.42
            },
            ModifierEffect::Multiplier {
                target: "moon_volcanic_activity",
                factor: 1.74
            },
            ModifierEffect::Tag("heated_satellite"),
        ]
    ),
    modifier!(
        RingShadowStripes,
        "ring_shadow_stripes",
        "Ring Shadow Stripes",
        RingsMoons,
        34,
        [
            ModifierEffect::Delta {
                target: "ring_shadow_opacity",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "equatorial_shadow_contrast",
                factor: 1.44
            },
            ModifierEffect::Tag("ring_shadow_bands"),
        ]
    ),
    modifier!(
        BrokenMoonDebris,
        "broken_moon_debris",
        "Broken Moon Debris",
        RingsMoons,
        18,
        [
            ModifierEffect::Delta {
                target: "debris_arc_density",
                amount: 0.52
            },
            ModifierEffect::Multiplier {
                target: "ring_particle_size_variance",
                factor: 1.86
            },
            ModifierEffect::Tag("disrupted_satellite"),
        ]
    ),
    modifier!(
        RedOxidePalette,
        "red_oxide_palette",
        "Red Oxide Palette",
        SurfaceTextureColor,
        70,
        [
            ModifierEffect::Delta {
                target: "palette_red",
                amount: 0.34
            },
            ModifierEffect::Multiplier {
                target: "surface_green",
                factor: 0.82
            },
            ModifierEffect::Tag("red_oxide_surface"),
        ]
    ),
    modifier!(
        JadeMineralStreaks,
        "jade_mineral_streaks",
        "Jade Mineral Streaks",
        SurfaceTextureColor,
        38,
        [
            ModifierEffect::Delta {
                target: "green_mineral_streaks",
                amount: 0.28
            },
            ModifierEffect::Multiplier {
                target: "vein_pattern_contrast",
                factor: 1.32
            },
            ModifierEffect::Tag("jade_chromite_veins"),
        ]
    ),
    modifier!(
        SaltPanMosaics,
        "salt_pan_mosaics",
        "Salt Pan Mosaics",
        SurfaceTextureColor,
        56,
        [
            ModifierEffect::Delta {
                target: "salt_pan_fraction",
                amount: 0.29
            },
            ModifierEffect::Multiplier {
                target: "polygon_crack_visibility",
                factor: 1.44
            },
            ModifierEffect::Tag("white_salt_polygons"),
        ]
    ),
    modifier!(
        BlackGlassFields,
        "black_glass_fields",
        "Black Glass Fields",
        SurfaceTextureColor,
        28,
        [
            ModifierEffect::Delta {
                target: "obsidian_fraction",
                amount: 0.32
            },
            ModifierEffect::Multiplier {
                target: "surface_specular",
                factor: 1.28
            },
            ModifierEffect::Tag("glassy_black_plains"),
        ]
    ),
    modifier!(
        PaleAeolianRipples,
        "pale_aeolian_ripples",
        "Pale Aeolian Ripples",
        SurfaceTextureColor,
        64,
        [
            ModifierEffect::Delta {
                target: "ripple_density",
                amount: 0.36
            },
            ModifierEffect::Multiplier {
                target: "fine_height_noise",
                factor: 1.22
            },
            ModifierEffect::Tag("wind_rippled_surface"),
        ]
    ),
    modifier!(
        BlueIceFractures,
        "blue_ice_fractures",
        "Blue Ice Fractures",
        SurfaceTextureColor,
        44,
        [
            ModifierEffect::Delta {
                target: "blue_ice_tint",
                amount: 0.33
            },
            ModifierEffect::Multiplier {
                target: "ice_crack_contrast",
                factor: 1.51
            },
            ModifierEffect::Tag("fractured_blue_ice"),
        ]
    ),
    modifier!(
        GoldenSavannaBands,
        "golden_savanna_bands",
        "Golden Savanna Bands",
        SurfaceTextureColor,
        52,
        [
            ModifierEffect::Delta {
                target: "dry_grass_tint",
                amount: 0.31
            },
            ModifierEffect::Multiplier {
                target: "latitude_color_banding",
                factor: 1.20
            },
            ModifierEffect::Tag("savanna_latitude_bands"),
        ]
    ),
    modifier!(
        StrongAtmosphericRim,
        "strong_atmospheric_rim",
        "Strong Atmospheric Rim",
        RenderingHints,
        62,
        [
            ModifierEffect::Multiplier {
                target: "rim_light_intensity",
                factor: 1.75
            },
            ModifierEffect::Delta {
                target: "limb_haze",
                amount: 0.24
            },
            ModifierEffect::Tag("render_strong_atmosphere"),
        ]
    ),
    modifier!(
        SubtleAtmosphericRim,
        "subtle_atmospheric_rim",
        "Subtle Atmospheric Rim",
        RenderingHints,
        58,
        [
            ModifierEffect::Multiplier {
                target: "rim_light_intensity",
                factor: 0.62
            },
            ModifierEffect::Multiplier {
                target: "limb_haze",
                factor: 0.74
            },
            ModifierEffect::Tag("render_subtle_atmosphere"),
        ]
    ),
    modifier!(
        HighReliefShading,
        "high_relief_shading",
        "High Relief Shading",
        RenderingHints,
        60,
        [
            ModifierEffect::Multiplier {
                target: "normal_map_strength",
                factor: 1.55
            },
            ModifierEffect::Multiplier {
                target: "terrain_shadow_strength",
                factor: 1.30
            },
            ModifierEffect::Tag("render_high_relief"),
        ]
    ),
    modifier!(
        SmoothOceanSpecular,
        "smooth_ocean_specular",
        "Smooth Ocean Specular",
        RenderingHints,
        54,
        [
            ModifierEffect::Multiplier {
                target: "ocean_roughness",
                factor: 0.55
            },
            ModifierEffect::Multiplier {
                target: "water_specular_intensity",
                factor: 1.48
            },
            ModifierEffect::Tag("render_glossy_oceans"),
        ]
    ),
    modifier!(
        CityLightBloom,
        "city_light_bloom",
        "City Light Bloom",
        RenderingHints,
        34,
        [
            ModifierEffect::Multiplier {
                target: "city_light_bloom_radius",
                factor: 1.65
            },
            ModifierEffect::Multiplier {
                target: "night_exposure",
                factor: 1.18
            },
            ModifierEffect::Tag("render_city_bloom"),
        ]
    ),
    modifier!(
        CloudShadowEmphasis,
        "cloud_shadow_emphasis",
        "Cloud Shadow Emphasis",
        RenderingHints,
        50,
        [
            ModifierEffect::Multiplier {
                target: "cloud_shadow_strength",
                factor: 1.52
            },
            ModifierEffect::Multiplier {
                target: "cloud_opacity",
                factor: 1.12
            },
            ModifierEffect::Tag("render_cloud_shadows"),
        ]
    ),
    modifier!(
        LowAlbedoExposureCompensation,
        "low_albedo_exposure_compensation",
        "Low Albedo Exposure Compensation",
        RenderingHints,
        42,
        [
            ModifierEffect::Multiplier {
                target: "exposure",
                factor: 1.22
            },
            ModifierEffect::Clamp {
                target: "surface_albedo",
                min: 0.08,
                max: 0.72
            },
            ModifierEffect::Tag("render_dark_surface_compensation"),
        ]
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_more_than_100_modifiers() {
        assert_eq!(MODIFIER_CATALOG_COUNT, modifier_catalog().len());
        assert!(modifier_catalog().len() > 100);
    }

    #[test]
    fn catalog_covers_every_category() {
        let mut counts = [0_usize; ModifierCategory::ALL.len()];
        for modifier in modifier_catalog() {
            counts[modifier.category.index()] += 1;
        }
        for category in ModifierCategory::ALL {
            assert!(
                counts[category.index()] > 0,
                "missing category {}",
                category.as_str()
            );
        }
    }

    #[test]
    fn catalog_ids_and_kinds_are_unique() {
        let catalog = modifier_catalog();
        for (index, modifier) in catalog.iter().enumerate() {
            assert!(!modifier.id.is_empty());
            assert!(!modifier.name.is_empty());
            assert!(!modifier.effects.is_empty());
            assert!(modifier.weight > 0);
            for other in &catalog[index + 1..] {
                assert_ne!(modifier.id, other.id);
                assert_ne!(modifier.kind, other.kind);
            }
        }
    }

    #[test]
    fn numeric_effects_are_finite_and_clamps_are_ordered() {
        for modifier in modifier_catalog() {
            for effect in modifier.effects {
                match *effect {
                    ModifierEffect::Delta { amount, .. } => assert!(amount.is_finite()),
                    ModifierEffect::Multiplier { factor, .. } => assert!(factor.is_finite()),
                    ModifierEffect::Clamp { min, max, .. } => {
                        assert!(min.is_finite());
                        assert!(max.is_finite());
                        assert!(min <= max);
                    }
                    ModifierEffect::Tag(tag) => assert!(!tag.is_empty()),
                }
            }
        }
    }
}
