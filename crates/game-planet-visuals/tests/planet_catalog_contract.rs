#![allow(unexpected_cfgs)]

// When catalog/profile expansion modules land, this cfg lets the integration
// test compile them directly even before lib.rs wires them into the crate API:
//
// RUSTFLAGS="--cfg planet_catalog_contract_direct" cargo test -p game-planet-visuals --test planet_catalog_contract
#[rustfmt::skip]
#[cfg(planet_catalog_contract_direct)]
#[path = "../src/catalog.rs"]
#[allow(dead_code)]
mod catalog;

#[rustfmt::skip]
#[cfg(planet_catalog_contract_direct)]
#[path = "../src/modifiers.rs"]
#[allow(dead_code)]
mod modifiers;

#[rustfmt::skip]
#[cfg(planet_catalog_contract_direct)]
#[path = "../src/profile.rs"]
#[allow(dead_code)]
mod profile;

use game_planet_visuals::{GeneratedPlanetProfile, PlanetVisualProfile};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MIN_CATALOG_ENTRIES: usize = 60;
const MIN_MODIFIER_ENTRIES: usize = 100;

const REQUIRED_CATALOG_FAMILIES: &[&[&str]] = &[
    &["terrestrial"],
    &["ocean"],
    &["desert"],
    &["ice"],
    &["volcanic"],
    &["gas giant", "gas_giant", "jovian"],
    &["barren"],
    &["forest", "jungle", "lush"],
];

const REQUIRED_MODIFIERS: &[&[&str]] = &[
    &["ringed"],
    &["temperate"],
    &["super earth", "super-earth", "super_earth"],
    &["cloud", "clouded"],
    &["atmosphere", "atmospheric"],
    &["ocean", "oceanic"],
    &["ice", "icy"],
    &["volcanic"],
    &["storm", "stormy"],
    &["tidal", "tidally locked"],
    &["metal rich", "metal-rich", "metal_rich"],
    &["toxic"],
    &["radiated", "irradiated"],
    &["habitable", "life bearing", "life-bearing"],
];

const REQUIRED_REPRESENTATIVE_TYPES: &[&[&str]] = &[
    &["mercury like", "mercury-like", "mercury"],
    &["mars like", "mars-like", "mars"],
    &["venus like", "venus-like", "venus"],
    &["earth like", "earth-like", "earth"],
    &["ocean", "ocean world"],
    &["low water", "low-water", "arid"],
    &["megacontinent", "mega continent", "mega-continent"],
    &["post apocalyptic", "post-apocalyptic", "ruined"],
    &["active volcanic", "active-volcanic", "volcanic"],
    &["dense atmosphere", "dense-atmosphere", "thick atmosphere"],
    &["gas giant", "gas_giant", "jovian"],
    &["hot jupiter", "hot-jupiter"],
    &["saturn like", "saturn-like", "ring giant", "ring-giant"],
    &["ice giant", "ice-giant", "neptune like", "neptune-like"],
    &["mini neptune", "mini-neptune", "sub neptune", "sub-neptune"],
    &["titan like", "titan-like", "hydrocarbon"],
    &["europa", "ice shell", "ice-shell"],
    &["lava", "magma"],
    &["sulfur", "sulphur", "io like", "io-like"],
    &["carbon"],
    &["iron"],
    &["rogue"],
    &["tidally locked", "tidal", "eyeball"],
    &["circumbinary", "binary star", "binary-star"],
    &["proto planet", "proto-planet", "protoplanet"],
    &["exomoon", "exo moon", "exo-moon"],
];

#[derive(Debug)]
struct ExpansionSources {
    catalog: String,
    modifiers: String,
    profile: String,
}

fn expansion_source_paths() -> [(&'static str, PathBuf); 3] {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    [
        ("catalog.rs", manifest_dir.join("src").join("catalog.rs")),
        (
            "modifiers.rs",
            manifest_dir.join("src").join("modifiers.rs"),
        ),
        ("profile.rs", manifest_dir.join("src").join("profile.rs")),
    ]
}

fn read_expansion_sources_if_present() -> Option<ExpansionSources> {
    let paths = expansion_source_paths();
    let missing = paths
        .iter()
        .filter(|(_, path)| !path.exists())
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();

    if !missing.is_empty() {
        eprintln!(
            "planet catalog expansion contract skipped until source modules land: {}",
            missing.join(", ")
        );
        return None;
    }

    Some(ExpansionSources {
        catalog: read_source(&paths[0].1),
        modifiers: read_source(&paths[1].1),
        profile: read_source(&paths[2].1),
    })
}

fn read_source(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    })
}

#[test]
fn catalog_and_modifier_sources_meet_expansion_contract_when_present() {
    let Some(sources) = read_expansion_sources_if_present() else {
        return;
    };

    let catalog_entries = probable_entry_count(
        &sources.catalog,
        &[
            "PlanetCatalogEntry",
            "CatalogEntry",
            "PlanetArchetype",
            "PlanetFamily",
            "PlanetClass",
        ],
    );
    assert!(
        catalog_entries >= MIN_CATALOG_ENTRIES,
        "catalog.rs should define at least {MIN_CATALOG_ENTRIES} planet catalog entries; detected {catalog_entries}"
    );

    let modifier_entries = probable_entry_count(
        &sources.modifiers,
        &[
            "PlanetModifier",
            "ModifierEntry",
            "CatalogModifier",
            "ProfileModifier",
            "Modifier",
        ],
    );
    assert!(
        modifier_entries >= MIN_MODIFIER_ENTRIES,
        "modifiers.rs should define at least {MIN_MODIFIER_ENTRIES} modifier entries; detected {modifier_entries}"
    );

    assert_source_has_terms(
        "catalog.rs",
        &sources.catalog,
        REQUIRED_CATALOG_FAMILIES,
        "planet family",
    );
    assert_source_has_terms(
        "modifiers.rs",
        &sources.modifiers,
        REQUIRED_MODIFIERS,
        "planet modifier",
    );

    let representative_type_source = format!(
        "{}\n{}\n{}",
        sources.catalog, sources.modifiers, sources.profile
    );
    assert_source_has_terms(
        "planet catalog expansion sources",
        &representative_type_source,
        REQUIRED_REPRESENTATIVE_TYPES,
        "representative requested type",
    );

    assert!(
        contains_any(
            &sources.profile,
            &[
                "PlanetVisualProfile",
                "GeneratedPlanetProfile",
                "VisualProfile",
                "PlanetProfile"
            ]
        ) && contains_any(
            &sources.profile,
            &[
                "from_seed",
                "generate_planet_profile",
                "generate",
                "from_identity"
            ]
        ),
        "profile.rs should expose a deterministic profile-generation entrypoint"
    );
}

#[test]
fn profile_generation_is_stable_for_same_seed_and_varies_for_different_seed() {
    let first = PlanetVisualProfile::from_seed(0x5EED_1208_0001);
    let second = PlanetVisualProfile::from_seed(0x5EED_1208_0001);
    let different = PlanetVisualProfile::from_seed(0x5EED_1208_0002);

    assert_eq!(first, second, "same seed should produce identical profiles");

    let first_json = serde_json::to_string(&first).expect("profile should serialize");
    let second_json = serde_json::to_string(&second).expect("profile should serialize");
    let different_json = serde_json::to_string(&different).expect("profile should serialize");

    assert_eq!(
        first_json, second_json,
        "same seed should serialize to identical profile metadata"
    );
    assert_ne!(
        first_json, different_json,
        "different seeds should not collapse to identical profile metadata"
    );

    let difference_score = usize::from(first.seed != different.seed)
        + usize::from(first.planet_class != different.planet_class)
        + usize::from(first.radius_km != different.radius_km)
        + usize::from(first.temperature_c != different.temperature_c)
        + usize::from((first.ocean_fraction - different.ocean_fraction).abs() > f32::EPSILON)
        + usize::from((first.ice_fraction - different.ice_fraction).abs() > f32::EPSILON)
        + usize::from((first.cloud_density - different.cloud_density).abs() > f32::EPSILON)
        + usize::from(
            (first.atmosphere_density - different.atmosphere_density).abs() > f32::EPSILON,
        )
        + usize::from((first.volcanic_activity - different.volcanic_activity).abs() > f32::EPSILON)
        + usize::from(first.ringed != different.ringed)
        + usize::from(first.palette != different.palette)
        + usize::from(first.render_model != different.render_model);

    assert!(
        difference_score >= 5,
        "different seed should alter several profile fields; difference score was {difference_score}"
    );
}

#[test]
fn public_profile_from_seed_preserves_generated_metadata_in_json() {
    let generated = GeneratedPlanetProfile::from_seed(0x5EED_1208_00AF);
    let public = PlanetVisualProfile::from_seed(generated.seed);

    assert_eq!(
        public,
        PlanetVisualProfile::from(generated.clone()),
        "PlanetVisualProfile::from_seed should use generated profile wiring"
    );

    let json = serde_json::to_value(&public).expect("profile should serialize to JSON metadata");
    assert_eq!(json["catalogVersion"], generated.catalog_version);
    assert_eq!(json["archetypeKey"], generated.archetype_key);
    assert_eq!(json["classKey"], generated.class_key);
    assert_eq!(json["planetClass"], generated.legacy_planet_class_label());
    assert_eq!(json["sizeClass"], generated.size_key);
    assert_eq!(json["sizeKey"], generated.size_key);
    assert_eq!(json["scaleBand"], generated.scale_key);
    assert_eq!(json["scaleKey"], generated.scale_key);
    assert_eq!(
        json["radiusScale"]
            .as_f64()
            .expect("radiusScale should serialize as a number"),
        generated.radius_scale as f64
    );

    let modifier_keys = json["modifierKeys"]
        .as_array()
        .expect("modifierKeys should serialize as an array")
        .iter()
        .map(|value| {
            value
                .as_str()
                .expect("modifier key should serialize as a string")
        })
        .collect::<Vec<_>>();
    let generated_modifier_keys = generated
        .modifiers
        .iter()
        .map(|modifier| modifier.key.as_str())
        .collect::<Vec<_>>();

    assert_eq!(modifier_keys, generated_modifier_keys);
}

#[cfg(planet_catalog_contract_direct)]
#[test]
fn expansion_modules_compile_as_direct_test_imports() {
    // Reaching this test means catalog.rs, modifiers.rs, and profile.rs all
    // parsed as direct integration-test modules through the #[path] imports.
}

fn probable_entry_count(source: &str, constructor_markers: &[&str]) -> usize {
    let structured_entries = source
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("//")
                && !trimmed.starts_with("pub struct")
                && !trimmed.starts_with("struct")
                && trimmed.contains('{')
                && constructor_markers
                    .iter()
                    .any(|marker| trimmed.contains(marker))
        })
        .count();

    let named_literals = named_literal_values(source, &["id", "key", "slug", "name"]).len();

    structured_entries.max(named_literals)
}

fn named_literal_values(source: &str, field_names: &[&str]) -> BTreeSet<String> {
    let mut values = BTreeSet::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        for field_name in field_names {
            let Some(field_start) = trimmed.find(field_name) else {
                continue;
            };
            let after_field = &trimmed[field_start + field_name.len()..];
            if !after_field.trim_start().starts_with(':') {
                continue;
            }

            for literal in string_literals(after_field) {
                if looks_like_catalog_key(&literal) {
                    values.insert(literal);
                }
            }
        }
    }

    values
}

fn string_literals(source: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }

        let mut value = String::new();
        let mut escaped = false;
        for next in chars.by_ref() {
            if escaped {
                value.push(next);
                escaped = false;
                continue;
            }

            match next {
                '\\' => escaped = true,
                '"' => break,
                _ => value.push(next),
            }
        }
        literals.push(value);
    }

    literals
}

fn looks_like_catalog_key(value: &str) -> bool {
    let compact = compact_term(value);
    compact.len() >= 3
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '/' | ':'))
}

fn assert_source_has_terms(
    source_name: &str,
    source: &str,
    required_terms: &[&[&str]],
    label: &str,
) {
    for alternatives in required_terms {
        assert!(
            alternatives
                .iter()
                .any(|alternative| contains_term(source, alternative)),
            "{source_name} should include requested {label} term: one of {alternatives:?}"
        );
    }
}

fn contains_any(source: &str, alternatives: &[&str]) -> bool {
    alternatives
        .iter()
        .any(|alternative| contains_term(source, alternative))
}

fn contains_term(source: &str, term: &str) -> bool {
    let source_words = word_term(source);
    let term_words = word_term(term);
    source_words.contains(&term_words) || compact_term(source).contains(&compact_term(term))
}

fn word_term(input: &str) -> String {
    let mut out = String::new();
    let mut previous_was_space = true;

    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            previous_was_space = false;
        } else if !previous_was_space {
            out.push(' ');
            previous_was_space = true;
        }
    }

    out.trim().to_string()
}

fn compact_term(input: &str) -> String {
    input
        .chars()
        .flat_map(char::to_lowercase)
        .filter(char::is_ascii_alphanumeric)
        .collect()
}
