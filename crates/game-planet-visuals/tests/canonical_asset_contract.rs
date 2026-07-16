use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf};

const MANIFEST: &str = "new-terra-rust-480p-manifest.json";
const PROFILE: &str = "new-terra-rust-480p-profile.json";
const PREVIEW: &str = "preview-rust-480p.html";

fn canonical_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("game-planet-visuals should live under workspace/crates")
        .join("assets")
        .join("planet-rust-prototype")
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn canonical_480p_asset_family_matches_its_integrity_manifest() {
    let root = canonical_dir();
    let manifest_bytes = fs::read(root.join(MANIFEST)).expect("canonical manifest should exist");
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).expect("canonical manifest should be valid JSON");

    assert_eq!(
        manifest["schema"], "universus.planet-render-manifest.v1",
        "canonical assets should use the renderer manifest contract"
    );
    assert_eq!(manifest["preset"], "480p");
    assert_eq!(manifest["rendererMode"], "raster");
    assert_eq!(manifest["selectedBackend"], "cpu");
    assert_eq!(manifest["renderSupersample"], 3);
    assert_eq!(manifest["effectiveSupersample"]["icon"], 4);
    assert_eq!(manifest["effectiveSupersample"]["banner"], 3);
    assert!(
        manifest.get("outputDir").is_none(),
        "canonical manifest must not commit a machine-specific output directory"
    );

    let expected_dimensions = BTreeMap::from([
        ("new-terra-rust-480p-icon.png", (480, 480)),
        ("new-terra-rust-480p-night-icon.png", (480, 480)),
        ("new-terra-rust-480p-overview-banner.png", (854, 480)),
        ("new-terra-rust-480p-night-overview-banner.png", (854, 480)),
        ("new-terra-rust-480p-surface-map.png", (960, 480)),
        ("new-terra-rust-480p-reflection-map.png", (960, 480)),
    ]);
    let expected_files = [
        "new-terra-rust-480p-icon.png",
        "new-terra-rust-480p-night-icon.png",
        "new-terra-rust-480p-overview-banner.png",
        "new-terra-rust-480p-night-overview-banner.png",
        "new-terra-rust-480p-surface-map.png",
        "new-terra-rust-480p-reflection-map.png",
        PROFILE,
        PREVIEW,
    ];
    let outputs = manifest["outputs"]
        .as_array()
        .expect("manifest outputs should be an array");
    assert_eq!(
        outputs.len(),
        expected_files.len(),
        "the complete canonical family must be promoted together"
    );

    for expected in expected_files {
        let output = outputs
            .iter()
            .find(|entry| entry["fileName"] == expected)
            .unwrap_or_else(|| panic!("manifest should list {expected}"));
        let bytes = fs::read(root.join(expected))
            .unwrap_or_else(|error| panic!("canonical artifact {expected} should exist: {error}"));
        assert!(
            output.get("path").is_none(),
            "canonical output entries must be portable, not machine-specific paths"
        );
        assert_eq!(
            output["byteLength"].as_u64(),
            Some(bytes.len() as u64),
            "byte length drift for {expected}"
        );
        let digest = sha256_hex(&bytes);
        assert_eq!(
            output["sha256"].as_str(),
            Some(digest.as_str()),
            "SHA-256 drift for {expected}"
        );
        if let Some(dimensions) = expected_dimensions.get(expected) {
            assert_eq!(
                image::image_dimensions(root.join(expected)).unwrap_or_else(|error| panic!(
                    "reading PNG dimensions for {expected}: {error}"
                )),
                *dimensions,
                "canonical PNG dimensions drifted for {expected}"
            );
        }
    }

    let profile: Value = serde_json::from_slice(
        &fs::read(root.join(PROFILE)).expect("canonical profile should exist"),
    )
    .expect("canonical profile should be valid JSON");
    assert_eq!(manifest["profile"], profile);

    let preview = fs::read_to_string(root.join(PREVIEW)).expect("canonical preview should exist");
    for asset in expected_dimensions.keys() {
        assert!(
            preview.contains(asset),
            "canonical preview should reference {asset}"
        );
    }
}
