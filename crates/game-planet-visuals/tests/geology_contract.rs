use game_planet_visuals::{
    geology::{
        density_layers_for_profile, radioactive_heat_budget, CrustDomain, DensityLayerKind,
        GeologyModel, RadioactiveIsotope,
    },
    PlanetVisualProfile, ProfileSeedInput,
};

const SEED: u64 = 0x5EED_1208_6E01;
const OCEAN_ARCHETYPE: &str = "catalog.archetype.global-ocean";
const TEMPERATE_ARCHETYPE: &str = "catalog.archetype.temperate-continents";
const BARREN_ARCHETYPE: &str = "catalog.archetype.barren-basalt";

#[test]
fn rift_chains_are_deterministic_and_geographically_valid() {
    let profile = forced_profile(TEMPERATE_ARCHETYPE);
    let first = GeologyModel::from_profile(&profile);
    let second = GeologyModel::from_profile(&profile);

    assert_eq!(first, second);
    assert!(first.rift_chains.len() >= 2);
    assert!(
        first.rift_chains.iter().any(|chain| chain.intensity > 0.35),
        "at least one rift chain should carry renderable strength"
    );

    for chain in &first.rift_chains {
        assert!((0.010..=0.060).contains(&chain.width));
        assert!((0.0..=1.0).contains(&chain.intensity));
        for node in chain.nodes {
            assert!((0.0..1.0).contains(&node.u));
            assert!((0.04..=0.96).contains(&node.v));
        }
    }

    let changed_seed =
        GeologyModel::from_profile(&forced_profile_with_seed(TEMPERATE_ARCHETYPE, SEED + 1));
    assert_ne!(
        first.rift_chains, changed_seed.rift_chains,
        "rift geometry should vary across seeds"
    );
}

#[test]
fn ocean_world_exposes_stronger_oceanic_rifts_and_deeper_basins() {
    let ocean = GeologyModel::from_profile(&forced_profile(OCEAN_ARCHETYPE));
    let barren = GeologyModel::from_profile(&forced_profile(BARREN_ARCHETYPE));

    let ocean_stats = sampled_stats(&ocean);
    let barren_stats = sampled_stats(&barren);

    assert!(
        ocean_stats.oceanic_samples > 40,
        "ocean world should expose many oceanic crust samples; got {ocean_stats:?}"
    );
    assert!(
        ocean_stats.mean_oceanic_rift > barren_stats.mean_oceanic_rift + 0.025,
        "ocean worlds should have stronger oceanic rift signal; ocean={ocean_stats:?}, barren={barren_stats:?}"
    );
    assert!(
        ocean_stats.max_ocean_depth_m > 4_500.0,
        "oceanic basin field should produce deep bathymetry; got {ocean_stats:?}"
    );
    assert!(
        ocean_stats.mean_basin > barren_stats.mean_basin,
        "ocean basin field should dominate dry worlds; ocean={ocean_stats:?}, barren={barren_stats:?}"
    );
}

#[test]
fn radioactive_decay_and_heat_budget_are_monotonic() {
    let young = forced_profile_with_seed(TEMPERATE_ARCHETYPE, SEED + 10);
    let young_budget = radioactive_heat_budget(0.7, &young);
    let old_budget = radioactive_heat_budget(4.6, &young);

    assert!(young_budget.present_fraction > old_budget.present_fraction);
    assert!(young_budget.heat_flow_mw_m2 > old_budget.heat_flow_mw_m2);
    assert!(
        RadioactiveIsotope::Uranium235.remaining_parent_fraction(4.6)
            < RadioactiveIsotope::Uranium238.remaining_parent_fraction(4.6)
    );
    for isotope in RadioactiveIsotope::ALL {
        let now = isotope.remaining_parent_fraction(0.0);
        let later = isotope.remaining_parent_fraction(2.0);
        assert_eq!(now, 1.0);
        assert!(later.is_finite());
        assert!((0.0..=1.0).contains(&later));
    }
}

#[test]
fn crust_and_mantle_density_layers_are_ordered_and_depth_queryable() {
    let temperate = forced_profile(TEMPERATE_ARCHETYPE);
    let ocean = forced_profile(OCEAN_ARCHETYPE);
    let temperate_layers = density_layers_for_profile(&temperate);
    let ocean_layers = density_layers_for_profile(&ocean);

    assert_layer_stack(&temperate_layers);
    assert_layer_stack(&ocean_layers);

    let continental_crust = layer_density(&temperate_layers, DensityLayerKind::ContinentalCrust);
    let oceanic_crust = layer_density(&ocean_layers, DensityLayerKind::OceanicCrust);
    let mantle = layer_density(&temperate_layers, DensityLayerKind::LithosphereMantle);
    let core = layer_density(&temperate_layers, DensityLayerKind::MetallicCore);

    assert!(oceanic_crust > continental_crust);
    assert!(mantle > continental_crust);
    assert!(core > mantle * 1.75);

    let model = GeologyModel::from_profile(&temperate);
    assert_eq!(
        model.density_at_depth_km(0.2).kind,
        DensityLayerKind::SurfaceSediment
    );
    assert!(matches!(
        model
            .density_at_depth_km(model.mean_crust_thickness_km + 10.0)
            .kind,
        DensityLayerKind::OceanicCrust | DensityLayerKind::LithosphereMantle
    ));
    assert_eq!(
        model.density_at_depth_km(temperate.radius_km as f32).kind,
        DensityLayerKind::MetallicCore
    );
}

#[test]
fn geology_samples_are_bounded_and_layer_aware() {
    let model = GeologyModel::from_profile(&forced_profile(TEMPERATE_ARCHETYPE));
    let sample = model.sample(0.37, 0.54);
    let same = model.sample(1.37, 0.54);

    assert_eq!(sample, same, "longitude should wrap deterministically");
    assert!((0.0..=1.0).contains(&sample.rift));
    assert!((0.0..=1.0).contains(&sample.basin));
    assert!((0.0..=1.0).contains(&sample.trench));
    assert!(sample.terrain_elevation_m.is_finite());
    assert!(sample.crust_thickness_km >= 3.5);
    assert!(sample.crust_density_kg_m3 < sample.mantle_density_kg_m3);
    assert!(sample.surface_heat_flow_mw_m2 >= 8.0);
    assert!((0.0..=1.0).contains(&sample.radioactive_heat_fraction));

    if sample.crust_domain == CrustDomain::Oceanic {
        assert!(sample.ocean_depth_m > 0.0);
        assert!(sample.oceanic_rift >= sample.continental_rift * 0.5);
    }
}

#[derive(Debug, Default)]
struct SampledStats {
    oceanic_samples: usize,
    mean_oceanic_rift: f32,
    mean_basin: f32,
    max_ocean_depth_m: f32,
}

fn sampled_stats(model: &GeologyModel) -> SampledStats {
    let mut stats = SampledStats::default();
    let mut rift_total = 0.0;
    let mut basin_total = 0.0;
    let mut samples = 0_usize;

    for y in 0..12 {
        for x in 0..18 {
            let sample = model.sample((x as f32 + 0.37) / 18.0, (y as f32 + 0.41) / 12.0);
            if sample.crust_domain == CrustDomain::Oceanic {
                stats.oceanic_samples += 1;
            }
            rift_total += sample.oceanic_rift;
            basin_total += sample.basin;
            stats.max_ocean_depth_m = stats.max_ocean_depth_m.max(sample.ocean_depth_m);
            samples += 1;
        }
    }

    stats.mean_oceanic_rift = rift_total / samples.max(1) as f32;
    stats.mean_basin = basin_total / samples.max(1) as f32;
    stats
}

fn forced_profile(archetype: &str) -> PlanetVisualProfile {
    forced_profile_with_seed(archetype, SEED)
}

fn forced_profile_with_seed(archetype: &str, seed: u64) -> PlanetVisualProfile {
    let profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(seed)
            .with_archetype_key(archetype)
            .with_modifier_budget(0)
            .without_rare_modifiers(),
    );
    assert_eq!(
        profile.archetype_key, archetype,
        "forced geology profile should not fall back to weighted selection"
    );
    profile
}

fn assert_layer_stack(layers: &[game_planet_visuals::geology::DensityLayer]) {
    assert!(layers.len() >= 6);
    assert_eq!(layers[0].top_depth_km, 0.0);
    for pair in layers.windows(2) {
        assert!(pair[0].bottom_depth_km > pair[0].top_depth_km);
        assert_eq!(pair[0].bottom_depth_km, pair[1].top_depth_km);
        assert!(
            pair[1].density_kg_m3 >= pair[0].density_kg_m3,
            "density should not decrease with depth: {pair:?}"
        );
    }
    assert!(layers.last().unwrap().bottom_depth_km > layers.last().unwrap().top_depth_km);
}

fn layer_density(
    layers: &[game_planet_visuals::geology::DensityLayer],
    kind: DensityLayerKind,
) -> f32 {
    layers
        .iter()
        .find(|layer| layer.kind == kind)
        .expect("expected density layer")
        .density_kg_m3
}
