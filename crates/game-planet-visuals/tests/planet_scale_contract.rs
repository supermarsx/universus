#[path = "../src/catalog.rs"]
#[allow(dead_code)]
mod catalog;

#[path = "../src/profile.rs"]
#[allow(dead_code)]
mod profile;

use game_planet_visuals::PlanetVisualProfile;
use profile::{generate_planet_profile, ProfileSeedInput};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

const CONTRACT_SEED: u64 = 0x5EED_1208_5001;

const SCALE_LABELS: &[&str] = &["small", "medium", "large"];
const SCALE_FIELD_NAMES: &[&str] = &[
    "scale",
    "scale_class",
    "planet_scale",
    "planet_size_class",
    "size_class",
    "size_key",
    "radius_class",
    "scale_band",
    "scale_key",
    "radius_scale",
];
const JSON_SCALE_KEYS: &[&str] = &[
    "sizeClass",
    "sizeKey",
    "planetSize",
    "planetSizeClass",
    "planetSizeKey",
    "scaleClass",
    "scaleBand",
    "scaleKey",
    "planetScale",
    "radiusClass",
    "scale",
    "radiusScale",
];
const FORCED_SCALE_TERMS: &[&str] = &[
    "forced_scale",
    "forced_scale_class",
    "forced_size_class",
    "forced_planet_scale",
    "with_scale",
    "with_scale_class",
    "with_size_class",
    "with_planet_scale",
    "with_forced_scale",
    "with_forced_size",
];

#[test]
fn catalog_entries_expose_small_medium_large_scale_metadata() {
    let source = read_source("catalog.rs");
    let entries = catalog::catalog_entries();
    let mut tiers = BTreeSet::new();
    let mut bands = BTreeSet::new();

    for entry in entries {
        let scale = entry.expected_scale();
        assert_eq!(
            scale.band,
            entry.scale_band(),
            "{} should expose a stable scale band",
            entry.id
        );
        assert_eq!(
            scale.tier,
            entry.scale_tier(),
            "{} should expose a stable small/medium/large tier",
            entry.id
        );
        assert_eq!(
            scale.radius_km, entry.radius_km,
            "{} scale metadata should preserve the catalog radius range",
            entry.id
        );
        assert!(
            scale.band_radius_km.contains(entry.radius_km.midpoint()),
            "{} radius midpoint should fit its exposed scale band",
            entry.id
        );

        tiers.insert(scale.tier);
        bands.insert(scale.band);
    }

    assert_eq!(
        tiers,
        catalog::PlanetScaleTier::ALL.into_iter().collect(),
        "catalog should expose small, medium, and large scale tiers"
    );
    assert_eq!(
        bands,
        catalog::PlanetScaleBand::ALL.into_iter().collect(),
        "catalog should expose all scale bands through entries"
    );

    assert!(
        source_mentions_any(&source, &["expected_scale", "scale_tier", "scale_band"]),
        "catalog source should expose scale data through explicit methods"
    );
}

#[test]
fn public_profile_metadata_serializes_scale_class_deterministically() {
    let first = PlanetVisualProfile::from_seed(CONTRACT_SEED);
    let second = PlanetVisualProfile::from_seed(CONTRACT_SEED);

    assert_eq!(first, second, "same seed should produce identical profile");

    let first_json = serde_json::to_value(&first).expect("profile should serialize");
    let second_json = serde_json::to_value(&second).expect("profile should serialize");
    assert_eq!(
        first_json, second_json,
        "same seed should serialize to identical metadata"
    );

    let (_, first_scale) = find_scale_metadata(&first_json).unwrap_or_else(|| {
        panic!(
            "PlanetVisualProfile JSON should expose explicit scale metadata under one of keys {JSON_SCALE_KEYS:?}; got {first_json}"
        )
    });
    let (_, second_scale) = find_scale_metadata(&second_json)
        .expect("second deterministic profile should expose scale metadata");

    assert_eq!(
        first_scale, second_scale,
        "same seed should preserve the same scale metadata"
    );

    let label = scale_label(first_scale).unwrap_or_else(|| {
        panic!("scale metadata should name one of {SCALE_LABELS:?}; got {first_scale}")
    });
    assert!(
        SCALE_LABELS.contains(&label.as_str()),
        "scale metadata should be small, medium, or large; got {label}"
    );
}

#[test]
fn forced_size_classes_change_radius_and_metadata_without_rendering() {
    let small = forced_profile("catalog.archetype.dwarf-asteroid-like");
    let medium = forced_profile("catalog.archetype.ocean");
    let large = forced_profile("catalog.archetype.gas-giant");

    assert!(
        small.radius_km < medium.radius_km && medium.radius_km < large.radius_km,
        "forced size-class representative archetypes should order radius small < medium < large; got small={}, medium={}, large={}",
        small.radius_km,
        medium.radius_km,
        large.radius_km
    );

    assert_ne!(
        (
            small.radius_km,
            small.archetype_key.as_str(),
            small.class_key.as_str()
        ),
        (
            medium.radius_km,
            medium.archetype_key.as_str(),
            medium.class_key.as_str()
        ),
        "small and medium forced classes should differ in radius and metadata"
    );
    assert_ne!(
        (
            medium.radius_km,
            medium.archetype_key.as_str(),
            medium.class_key.as_str()
        ),
        (
            large.radius_km,
            large.archetype_key.as_str(),
            large.class_key.as_str()
        ),
        "medium and large forced classes should differ in radius and metadata"
    );
    assert_ne!(
        (small.size_key.as_str(), small.scale_key.as_str()),
        (large.size_key.as_str(), large.scale_key.as_str()),
        "small and large representative profiles should expose different scale metadata"
    );
    for (label, profile) in [("small", &small), ("medium", &medium), ("large", &large)] {
        assert!(
            SCALE_LABELS.contains(&profile.size_key.as_str()),
            "{label} representative should expose a small/medium/large size_key, got {}",
            profile.size_key
        );
        assert!(
            !profile.scale_key.is_empty(),
            "{label} representative should expose a scale_key"
        );
        assert!(
            profile.radius_scale > 0.0,
            "{label} representative should expose a positive radius_scale"
        );
    }

    let source = read_source("profile.rs");
    let cli_source = read_source("bin/render_planet.rs");
    let seed_input = extract_braced_item(&source, "pub struct ProfileSeedInput")
        .expect("profile.rs should define ProfileSeedInput");
    let generated_profile = extract_braced_item(&source, "pub struct GeneratedPlanetProfile")
        .expect("profile.rs should define GeneratedPlanetProfile");
    let generation_body = extract_braced_item(&source, "pub fn generate_planet_profile")
        .expect("profile.rs should define generate_planet_profile");

    assert!(
        source_mentions_any(&seed_input, FORCED_SCALE_TERMS)
            || source_mentions_any(&source, FORCED_SCALE_TERMS)
            || source_mentions_any(&cli_source, &["planet_size", "planet-size", "--planet-size"]),
        "ProfileSeedInput or CLI should expose a forced small/medium/large scale or size-class control"
    );
    assert!(
        has_named_field(&generated_profile, SCALE_FIELD_NAMES),
        "GeneratedPlanetProfile must carry explicit scale metadata; expected one of fields {SCALE_FIELD_NAMES:?}"
    );
    assert!(
        has_field_assignment(&generation_body, SCALE_FIELD_NAMES)
            && generation_body.contains("radius_km"),
        "generate_planet_profile should assign explicit scale metadata while deriving radius_km"
    );
}

#[test]
fn cli_help_exposes_scale_controls_without_rendering() {
    let output = Command::new(env!("CARGO_BIN_EXE_render_planet"))
        .arg("--help")
        .output()
        .expect("render_planet --help should run without rendering");

    assert!(
        output.status.success(),
        "render_planet --help should exit successfully; status={:?}",
        output.status.code()
    );

    let help = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        [
            "--scale",
            "--scale-class",
            "--size-class",
            "--planet-scale",
            "--planet-size",
        ]
        .iter()
        .any(|flag| help.contains(flag)),
        "CLI help should expose a non-rendering scale or size-class control; help was:\n{help}"
    );

    for label in SCALE_LABELS {
        assert!(
            contains_word(&help, label),
            "CLI help should document the {label} scale option"
        );
    }
}

fn forced_profile(archetype_key: &str) -> profile::GeneratedPlanetProfile {
    generate_planet_profile(
        ProfileSeedInput::new(CONTRACT_SEED)
            .with_archetype_key(archetype_key)
            .with_modifier_budget(0),
    )
}

fn read_source(file_name: &str) -> String {
    let path = manifest_dir().join("src").join(file_name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn extract_braced_item(source: &str, item_start: &str) -> Option<String> {
    let start = source.find(item_start)?;
    let rest = &source[start..];
    let open_offset = rest.find('{')?;
    let open = start + open_offset;
    let mut depth = 0_usize;

    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(source[start..=open + offset].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

fn has_named_field(source: &str, field_names: &[&str]) -> bool {
    source.lines().any(|line| {
        let trimmed = trim_code_line(line);
        field_names.iter().any(|field_name| {
            trimmed.starts_with(&format!("pub {field_name}:"))
                || trimmed.starts_with(&format!("{field_name}:"))
        })
    })
}

fn has_field_assignment(source: &str, field_names: &[&str]) -> bool {
    source.lines().any(|line| {
        let trimmed = trim_code_line(line);
        field_names.iter().any(|field_name| {
            trimmed.starts_with(&format!("{field_name}:"))
                || trimmed == *field_name
                || trimmed.starts_with(&format!("{field_name},"))
        })
    })
}

fn source_mentions_any(source: &str, terms: &[&str]) -> bool {
    let compact = compact_term(source);
    terms
        .iter()
        .any(|term| compact.contains(&compact_term(term)))
}

fn find_scale_metadata(value: &Value) -> Option<(&str, &Value)> {
    match value {
        Value::Object(map) => {
            for candidate in JSON_SCALE_KEYS {
                if let Some(child) = map.get(*candidate) {
                    return Some((*candidate, child));
                }
            }
            map.values().find_map(find_scale_metadata)
        }
        Value::Array(items) => items.iter().find_map(find_scale_metadata),
        _ => None,
    }
}

fn scale_label(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => Some(word_term(raw)),
        Value::Object(map) => [
            "class",
            "key",
            "label",
            "value",
            "scale",
            "sizeClass",
            "sizeKey",
            "scaleKey",
        ]
        .iter()
        .filter_map(|key| map.get(*key))
        .find_map(scale_label),
        _ => None,
    }
}

fn trim_code_line(line: &str) -> &str {
    line.split_once("//")
        .map_or(line, |(before_comment, _)| before_comment)
        .trim()
}

fn contains_word(source: &str, word: &str) -> bool {
    word_term(source)
        .split_whitespace()
        .any(|candidate| candidate == word)
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
