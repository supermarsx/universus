use game_planet_visuals::evolution::{
    evolved_seed, EvolutionTime, PlanetEvolutionModel, DEFAULT_TICKS_PER_DAY,
};
use game_planet_visuals::{PlanetVisualProfile, ProfileSeedInput};

#[test]
fn evolution_time_advances_seed_deterministically() {
    let seed = 0x5EED_1208_0C11;
    let start = EvolutionTime::ZERO;
    let after_ticks = start.advance_ticks(DEFAULT_TICKS_PER_DAY as i64 * 3);
    let after_days = start.advance_days(3.0);

    assert_eq!(after_ticks, after_days);
    assert_eq!(after_days.days(), 3.0);
    assert_eq!(
        evolved_seed(seed, after_days),
        evolved_seed(seed, after_ticks)
    );
    assert_ne!(evolved_seed(seed, start), evolved_seed(seed, after_days));

    let rebuilt = EvolutionTime::from_days_with_resolution(3.0, DEFAULT_TICKS_PER_DAY);
    assert_eq!(after_days, rebuilt);
}

#[test]
fn same_seed_and_time_produce_identical_snapshots_and_samples() {
    let model = PlanetEvolutionModel::from_seed(0x5EED_1208_0C12);
    let time = EvolutionTime::from_days(42.25);
    let first = model.snapshot_at(time);
    let second = model.snapshot_at(time);

    assert_eq!(first, second);
    assert_eq!(first.sample(0.23, 0.57), second.sample(0.23, 0.57));

    let first_json = serde_json::to_string(&first.sample(0.23, 0.57)).expect("sample serializes");
    let second_json = serde_json::to_string(&second.sample(0.23, 0.57)).expect("sample serializes");
    assert_eq!(first_json, second_json);
}

#[test]
fn climate_samples_are_bounded_and_expose_all_requested_fields() {
    let profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(0x5EED_1208_0C13)
            .with_archetype_key("catalog.archetype.global-ocean"),
    );
    let model = PlanetEvolutionModel::from_profile(profile);
    let sample = model.sample(0.37, 0.44, EvolutionTime::from_days(8.5));

    assert!(sample.flow.wind_speed_mps >= 0.0);
    assert!(sample.flow.wind_speed_mps <= model.wind_velocity_scale_mps * 1.55 + f32::EPSILON);
    assert!(sample.flow.ocean_current_speed_mps >= 0.0);
    assert!(
        sample.flow.ocean_current_speed_mps <= model.ocean_velocity_scale_mps * 1.45 + f32::EPSILON
    );
    assert!(sample.flow.cloud_speed_mps >= 0.0);
    assert!(sample.flow.cloud_speed_mps <= model.cloud_velocity_scale_mps * 1.65 + f32::EPSILON);
    assert!(sample.flow.wind_mps.length().is_finite());
    assert!(sample.flow.ocean_current_mps.length().is_finite());
    assert!(sample.flow.cloud_flow_mps.length().is_finite());

    assert!((900.0..=1_240.0).contains(&sample.density.water_density_kg_m3));
    assert!((0.001..=16.0).contains(&sample.density.atmosphere_density_kg_m3));
    assert!((0.0005..=32.0).contains(&sample.density.surface_pressure_bar));
    assert!((0.0..=1.0).contains(&sample.density.humidity));

    assert!((0.1..=180.0).contains(&sample.magnetism.magnetic_field_microtesla));
    assert!((0.0..=1.0).contains(&sample.magnetism.magnetosphere_strength));
    assert!((0.0..=1.0).contains(&sample.magnetism.aurora_power));

    assert!((0.0..=1.0).contains(&sample.cyclone.influence));
    assert!((0.0..=1.0).contains(&sample.cyclone.cloud_wall));
    assert!((0.0..=1.0).contains(&sample.cloud_density));
    assert!((0.0..=1.0).contains(&sample.humidity));
}

#[test]
fn cyclone_systems_are_deterministic_but_time_evolved() {
    let profile = PlanetVisualProfile::from_seed_input(
        ProfileSeedInput::new(0x5EED_1208_0C14)
            .with_archetype_key("catalog.archetype.storm-gas-giant"),
    );
    let model = PlanetEvolutionModel::from_profile(profile);
    let early = model.snapshot_at(EvolutionTime::from_days(5.0));
    let early_again = model.snapshot_at(EvolutionTime::from_days(5.0));
    let later = model.snapshot_at(EvolutionTime::from_days(11.0));

    assert_eq!(early.cyclone_systems, early_again.cyclone_systems);
    assert!(
        !early.cyclone_systems.is_empty(),
        "storm worlds should expose cyclone systems"
    );
    assert_ne!(
        early.cyclone_systems, later.cyclone_systems,
        "cyclone centers should drift as time advances"
    );

    for system in &early.cyclone_systems {
        assert!((0.0..=1.0).contains(&system.center_u));
        assert!((0.0..=1.0).contains(&system.center_v));
        assert!((0.0..=1.0).contains(&system.intensity));
        assert!(system.radius_uv > 0.0);
        assert!(system.pressure_drop_bar >= 0.0);
    }
}
