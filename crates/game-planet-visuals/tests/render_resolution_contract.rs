use game_planet_visuals::{
    BackendCapabilities, PlanetVisualProfile, RenderBackend, RenderOptions, RenderOutputKind,
    RenderQuality, RenderRequest, RenderSize,
};

const SIZE_480P: RenderSize = RenderSize {
    width: 854,
    height: 480,
};
const SIZE_720P: RenderSize = RenderSize {
    width: 1280,
    height: 720,
};
const SIZE_1080P: RenderSize = RenderSize {
    width: 1920,
    height: 1080,
};
const SIZE_4K: RenderSize = RenderSize {
    width: 3840,
    height: 2160,
};
const SIZE_8K: RenderSize = RenderSize {
    width: 7680,
    height: 4320,
};
const SIZE_SQUARE_1K: RenderSize = RenderSize {
    width: 1024,
    height: 1024,
};
const SIZE_SQUARE_2K: RenderSize = RenderSize {
    width: 2048,
    height: 2048,
};
const SIZE_SQUARE_4K: RenderSize = RenderSize {
    width: 4096,
    height: 4096,
};
const SIZE_VERTICAL_720P: RenderSize = RenderSize {
    width: 720,
    height: 1280,
};
const SIZE_VERTICAL_1080P: RenderSize = RenderSize {
    width: 1080,
    height: 1920,
};
const SIZE_VERTICAL_4K: RenderSize = RenderSize {
    width: 2160,
    height: 3840,
};

const NATIVE_RESOLUTION_PRESETS: [NativeResolutionPreset; 11] = [
    NativeResolutionPreset {
        key: "480p",
        label: "480p widescreen",
        size: SIZE_480P,
        quality: RenderQuality::Draft,
    },
    NativeResolutionPreset {
        key: "720p",
        label: "720p HD",
        size: SIZE_720P,
        quality: RenderQuality::Draft,
    },
    NativeResolutionPreset {
        key: "1080p",
        label: "1080p full HD",
        size: SIZE_1080P,
        quality: RenderQuality::Balanced,
    },
    NativeResolutionPreset {
        key: "4k",
        label: "4K UHD",
        size: SIZE_4K,
        quality: RenderQuality::High,
    },
    NativeResolutionPreset {
        key: "8k",
        label: "8K UHD",
        size: SIZE_8K,
        quality: RenderQuality::High,
    },
    NativeResolutionPreset {
        key: "square-1k",
        label: "Square 1K",
        size: SIZE_SQUARE_1K,
        quality: RenderQuality::Draft,
    },
    NativeResolutionPreset {
        key: "square-2k",
        label: "Square 2K",
        size: SIZE_SQUARE_2K,
        quality: RenderQuality::Balanced,
    },
    NativeResolutionPreset {
        key: "square-4k",
        label: "Square 4K",
        size: SIZE_SQUARE_4K,
        quality: RenderQuality::High,
    },
    NativeResolutionPreset {
        key: "vertical-720p",
        label: "Vertical 720p",
        size: SIZE_VERTICAL_720P,
        quality: RenderQuality::Draft,
    },
    NativeResolutionPreset {
        key: "vertical-1080p",
        label: "Vertical 1080p",
        size: SIZE_VERTICAL_1080P,
        quality: RenderQuality::Balanced,
    },
    NativeResolutionPreset {
        key: "vertical-4k",
        label: "Vertical 4K",
        size: SIZE_VERTICAL_4K,
        quality: RenderQuality::High,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeResolutionPreset {
    key: &'static str,
    label: &'static str,
    size: RenderSize,
    quality: RenderQuality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct QualityMetadata {
    key: &'static str,
    options: RenderOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeResolutionMetadata {
    preset_key: &'static str,
    preset_label: &'static str,
    width: u32,
    height: u32,
    pixels: u64,
    output: RenderOutputKind,
    quality: RenderQuality,
    backend: &'static str,
    max_dimension: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolutionValidationError {
    ZeroDimension {
        size: RenderSize,
    },
    ExceedsNativeLimit {
        size: RenderSize,
        max_size: RenderSize,
    },
    UnsupportedOutput {
        output: RenderOutputKind,
    },
    UnrepresentedPreset {
        size: RenderSize,
    },
}

#[derive(Debug, Clone, Copy, Default)]
struct MetadataOnlyBackend;

impl RenderBackend for MetadataOnlyBackend {
    type Output = NativeResolutionMetadata;
    type Error = ResolutionValidationError;

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            name: "metadata-only-native-resolution-contract",
            hardware_accelerated: false,
            supports_icon: true,
            supports_banner: true,
            supports_surface_map: true,
            max_dimension: Some(SIZE_8K.width),
        }
    }

    fn render(&self, request: RenderRequest<'_>) -> Result<Self::Output, Self::Error> {
        let capabilities = self.capabilities();
        if !supports_output(capabilities, request.output) {
            return Err(ResolutionValidationError::UnsupportedOutput {
                output: request.output,
            });
        }

        validate_native_resolution(request.size)?;
        let preset = preset_for_size(request.size)
            .ok_or(ResolutionValidationError::UnrepresentedPreset { size: request.size })?;

        Ok(NativeResolutionMetadata {
            preset_key: preset.key,
            preset_label: preset.label,
            width: request.size.width,
            height: request.size.height,
            pixels: pixel_count(request.size),
            output: request.output,
            quality: request.quality,
            backend: capabilities.name,
            max_dimension: capabilities.max_dimension,
        })
    }
}

#[test]
fn native_resolution_presets_represent_expected_dimensions() {
    assert_eq!(NATIVE_RESOLUTION_PRESETS.len(), 11);
    assert_eq!(
        preset_for_key("480p").map(|preset| preset.size),
        Some(SIZE_480P)
    );
    assert_eq!(
        preset_for_key("720p").map(|preset| preset.size),
        Some(SIZE_720P)
    );
    assert_eq!(
        preset_for_key("1080p").map(|preset| preset.size),
        Some(SIZE_1080P)
    );
    assert_eq!(
        preset_for_key("4k").map(|preset| preset.size),
        Some(SIZE_4K)
    );
    assert_eq!(
        preset_for_key("8k").map(|preset| preset.size),
        Some(SIZE_8K)
    );
    assert_eq!(
        preset_for_key("square-1k").map(|preset| preset.size),
        Some(SIZE_SQUARE_1K)
    );
    assert_eq!(
        preset_for_key("square-2k").map(|preset| preset.size),
        Some(SIZE_SQUARE_2K)
    );
    assert_eq!(
        preset_for_key("square-4k").map(|preset| preset.size),
        Some(SIZE_SQUARE_4K)
    );
    assert_eq!(
        preset_for_key("vertical-720p").map(|preset| preset.size),
        Some(SIZE_VERTICAL_720P)
    );
    assert_eq!(
        preset_for_key("vertical-1080p").map(|preset| preset.size),
        Some(SIZE_VERTICAL_1080P)
    );
    assert_eq!(
        preset_for_key("vertical-4k").map(|preset| preset.size),
        Some(SIZE_VERTICAL_4K)
    );

    let keys = NATIVE_RESOLUTION_PRESETS
        .iter()
        .map(|preset| preset.key)
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "480p",
            "720p",
            "1080p",
            "4k",
            "8k",
            "square-1k",
            "square-2k",
            "square-4k",
            "vertical-720p",
            "vertical-1080p",
            "vertical-4k",
        ]
    );

    let mut unique_keys = keys.clone();
    unique_keys.sort_unstable();
    unique_keys.dedup();
    assert_eq!(unique_keys.len(), keys.len());

    let mut sizes = NATIVE_RESOLUTION_PRESETS
        .iter()
        .map(|preset| (preset.size.width, preset.size.height))
        .collect::<Vec<_>>();
    sizes.sort_unstable();
    sizes.dedup();
    assert_eq!(sizes.len(), NATIVE_RESOLUTION_PRESETS.len());
}

#[test]
fn native_validation_accepts_8k_and_rejects_oversized_dimensions() {
    assert_eq!(validate_native_resolution(SIZE_8K), Ok(SIZE_8K));

    assert_eq!(
        validate_native_resolution(RenderSize {
            width: SIZE_8K.width + 1,
            height: SIZE_8K.height,
        }),
        Err(ResolutionValidationError::ExceedsNativeLimit {
            size: RenderSize {
                width: SIZE_8K.width + 1,
                height: SIZE_8K.height,
            },
            max_size: SIZE_8K,
        })
    );
    assert_eq!(
        validate_native_resolution(RenderSize {
            width: SIZE_8K.width,
            height: SIZE_8K.height + 1,
        }),
        Err(ResolutionValidationError::ExceedsNativeLimit {
            size: RenderSize {
                width: SIZE_8K.width,
                height: SIZE_8K.height + 1,
            },
            max_size: SIZE_8K,
        })
    );
}

#[test]
fn metadata_backend_accepts_native_requests_without_rendering() {
    let backend = MetadataOnlyBackend;
    let profile = PlanetVisualProfile::from_seed(0x5EED_1208_0001);

    for preset in NATIVE_RESOLUTION_PRESETS {
        let request = RenderRequest {
            profile: &profile,
            size: preset.size,
            output: RenderOutputKind::Banner,
            quality: preset.quality,
        };

        let metadata = backend
            .render(request)
            .expect("native preset metadata request should validate");

        assert_eq!(metadata.preset_key, preset.key);
        assert_eq!(metadata.preset_label, preset.label);
        assert_eq!(metadata.width, preset.size.width);
        assert_eq!(metadata.height, preset.size.height);
        assert_eq!(metadata.pixels, pixel_count(preset.size));
        assert_eq!(metadata.quality, preset.quality);
    }

    let request = RenderRequest {
        profile: &profile,
        size: SIZE_8K,
        output: RenderOutputKind::Banner,
        quality: RenderQuality::High,
    };

    let metadata = backend
        .render(request)
        .expect("8K native metadata request should validate");

    assert_eq!(
        metadata,
        NativeResolutionMetadata {
            preset_key: "8k",
            preset_label: "8K UHD",
            width: 7680,
            height: 4320,
            pixels: 33_177_600,
            output: RenderOutputKind::Banner,
            quality: RenderQuality::High,
            backend: "metadata-only-native-resolution-contract",
            max_dimension: Some(7680),
        }
    );

    let oversized = RenderRequest {
        size: RenderSize {
            width: SIZE_8K.width + 1,
            height: SIZE_8K.height,
        },
        ..request
    };
    assert_eq!(
        backend.render(oversized),
        Err(ResolutionValidationError::ExceedsNativeLimit {
            size: oversized.size,
            max_size: SIZE_8K,
        })
    );
}

#[test]
fn quality_and_preset_metadata_is_deterministic() {
    let first = preset_metadata();
    let second = preset_metadata();
    assert_eq!(first, second);
    assert_eq!(
        first,
        vec![
            (
                "480p",
                "480p widescreen",
                854,
                480,
                409_920,
                RenderQuality::Draft
            ),
            ("720p", "720p HD", 1280, 720, 921_600, RenderQuality::Draft),
            (
                "1080p",
                "1080p full HD",
                1920,
                1080,
                2_073_600,
                RenderQuality::Balanced
            ),
            ("4k", "4K UHD", 3840, 2160, 8_294_400, RenderQuality::High),
            ("8k", "8K UHD", 7680, 4320, 33_177_600, RenderQuality::High),
            (
                "square-1k",
                "Square 1K",
                1024,
                1024,
                1_048_576,
                RenderQuality::Draft
            ),
            (
                "square-2k",
                "Square 2K",
                2048,
                2048,
                4_194_304,
                RenderQuality::Balanced
            ),
            (
                "square-4k",
                "Square 4K",
                4096,
                4096,
                16_777_216,
                RenderQuality::High
            ),
            (
                "vertical-720p",
                "Vertical 720p",
                720,
                1280,
                921_600,
                RenderQuality::Draft
            ),
            (
                "vertical-1080p",
                "Vertical 1080p",
                1080,
                1920,
                2_073_600,
                RenderQuality::Balanced
            ),
            (
                "vertical-4k",
                "Vertical 4K",
                2160,
                3840,
                8_294_400,
                RenderQuality::High
            ),
        ]
    );

    assert_eq!(
        [
            quality_metadata(RenderQuality::Draft),
            quality_metadata(RenderQuality::Balanced),
            quality_metadata(RenderQuality::High),
        ],
        [
            QualityMetadata {
                key: "draft",
                options: RenderOptions::preview(),
            },
            QualityMetadata {
                key: "balanced",
                options: RenderOptions::standard(),
            },
            QualityMetadata {
                key: "high",
                options: RenderOptions::ultra(),
            },
        ]
    );
}

fn preset_for_key(key: &str) -> Option<NativeResolutionPreset> {
    NATIVE_RESOLUTION_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.key == key)
}

fn preset_for_size(size: RenderSize) -> Option<NativeResolutionPreset> {
    NATIVE_RESOLUTION_PRESETS
        .iter()
        .copied()
        .find(|preset| preset.size == size)
}

fn validate_native_resolution(size: RenderSize) -> Result<RenderSize, ResolutionValidationError> {
    if size.width == 0 || size.height == 0 {
        return Err(ResolutionValidationError::ZeroDimension { size });
    }

    if size.width > SIZE_8K.width
        || size.height > SIZE_8K.height
        || pixel_count(size) > pixel_count(SIZE_8K)
    {
        return Err(ResolutionValidationError::ExceedsNativeLimit {
            size,
            max_size: SIZE_8K,
        });
    }

    Ok(size)
}

fn supports_output(capabilities: BackendCapabilities, output: RenderOutputKind) -> bool {
    match output {
        RenderOutputKind::Icon => capabilities.supports_icon,
        RenderOutputKind::Banner => capabilities.supports_banner,
        RenderOutputKind::SurfaceMap => capabilities.supports_surface_map,
    }
}

fn quality_metadata(quality: RenderQuality) -> QualityMetadata {
    match quality {
        RenderQuality::Draft => QualityMetadata {
            key: "draft",
            options: RenderOptions::preview(),
        },
        RenderQuality::Balanced => QualityMetadata {
            key: "balanced",
            options: RenderOptions::standard(),
        },
        RenderQuality::High => QualityMetadata {
            key: "high",
            options: RenderOptions::ultra(),
        },
    }
}

fn preset_metadata() -> Vec<(&'static str, &'static str, u32, u32, u64, RenderQuality)> {
    NATIVE_RESOLUTION_PRESETS
        .iter()
        .map(|preset| {
            (
                preset.key,
                preset.label,
                preset.size.width,
                preset.size.height,
                pixel_count(preset.size),
                preset.quality,
            )
        })
        .collect()
}

fn pixel_count(size: RenderSize) -> u64 {
    u64::from(size.width) * u64::from(size.height)
}
