#![forbid(unsafe_code)]

//! HTTP compatibility adapters for translating between legacy (Node.js) and
//! modern (Rust) API formats during the Universus migration.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// AdaptError
// ---------------------------------------------------------------------------

/// Errors that can occur during request/response adaptation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AdaptError {
    InvalidJson(String),
    UnsupportedPath(String),
    TransformFailed(String),
    MissingField(String),
}

impl fmt::Display for AdaptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "invalid json: {msg}"),
            Self::UnsupportedPath(msg) => write!(f, "unsupported path: {msg}"),
            Self::TransformFailed(msg) => write!(f, "transform failed: {msg}"),
            Self::MissingField(msg) => write!(f, "missing field: {msg}"),
        }
    }
}

impl std::error::Error for AdaptError {}

// ---------------------------------------------------------------------------
// HttpCompatAdapter trait
// ---------------------------------------------------------------------------

/// Abstraction for HTTP compatibility adapters.
pub trait HttpCompatAdapter: Send + Sync {
    /// Adapt an incoming request from one format to another.
    fn adapt_request(&self, input: &str) -> Result<String, AdaptError>;
    /// Adapt an outgoing response from one format to another.
    fn adapt_response(&self, output: &str) -> Result<String, AdaptError>;
    /// Human-readable name of this adapter.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

/// Legacy request format (Node.js / v1 API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Value>,
    pub query_params: HashMap<String, String>,
}

/// Modern request format (Rust / v2 API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Value>,
    pub query_params: HashMap<String, String>,
    pub version: String,
}

/// Legacy response format (Node.js / v1 API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyResponse {
    pub status: u16,
    pub body: Value,
    pub headers: HashMap<String, String>,
}

/// Structured error payload used in modern responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

/// Metadata attached to every modern response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub request_id: String,
    pub timestamp: i64,
    pub version: String,
}

/// Modern response format (Rust / v2 API).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernResponse {
    pub status: u16,
    pub data: Option<Value>,
    pub error: Option<ErrorPayload>,
    pub meta: ResponseMeta,
}

// ---------------------------------------------------------------------------
// ApiVersion & middleware helpers
// ---------------------------------------------------------------------------

/// Detected API version from request headers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiVersion {
    V1Legacy,
    V2Modern,
    Unknown,
}

/// Inspect headers to determine the API version.
///
/// Checks for `x-api-version` (or `X-Api-Version`) header values `"v1"` / `"v2"`.
pub fn extract_api_version(headers: &HashMap<String, String>) -> ApiVersion {
    // Normalise lookup — check both lowercase and mixed-case keys.
    let value = headers
        .get("x-api-version")
        .or_else(|| headers.get("X-Api-Version"))
        .map(|v| v.to_lowercase());

    match value.as_deref() {
        Some("v1") => ApiVersion::V1Legacy,
        Some("v2") => ApiVersion::V2Modern,
        _ => ApiVersion::Unknown,
    }
}

/// Returns `true` when the request should be run through the compatibility adapter.
pub fn should_adapt(version: &ApiVersion) -> bool {
    matches!(version, ApiVersion::V1Legacy)
}

// ---------------------------------------------------------------------------
// Case-conversion helpers
// ---------------------------------------------------------------------------

/// Convert a `camelCase` string to `snake_case`.
pub fn camel_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }
    result
}

/// Convert a `snake_case` string to `camelCase`.
pub fn snake_to_camel(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_uppercase().next().unwrap());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Recursively transform all object keys in a JSON [`Value`] using `transform`.
pub fn transform_keys_recursive(value: &Value, transform: fn(&str) -> String) -> Value {
    match value {
        Value::Object(map) => {
            let new_map: serde_json::Map<String, Value> = map
                .iter()
                .map(|(k, v)| (transform(k), transform_keys_recursive(v, transform)))
                .collect();
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(
            arr.iter()
                .map(|v| transform_keys_recursive(v, transform))
                .collect(),
        ),
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// Body transformation
// ---------------------------------------------------------------------------

/// Transform a legacy (camelCase) request body to modern (snake_case).
pub fn transform_request_body(legacy_body: &Value, _endpoint: &str) -> Result<Value, AdaptError> {
    Ok(transform_keys_recursive(legacy_body, camel_to_snake))
}

/// Transform a modern (snake_case) response body to legacy (camelCase).
pub fn transform_response_body(modern_body: &Value, _endpoint: &str) -> Result<Value, AdaptError> {
    Ok(transform_keys_recursive(modern_body, snake_to_camel))
}

// ---------------------------------------------------------------------------
// Header adaptation
// ---------------------------------------------------------------------------

/// Translate legacy headers to modern format.
///
/// * Rewrites `Authorization: Token <tok>` → `Authorization: Bearer <tok>`
/// * Adds `x-api-version: v2`
pub fn adapt_headers_to_modern(
    legacy_headers: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut modern = legacy_headers.clone();

    // Rewrite auth scheme.
    if let Some(auth) = modern.get("Authorization").cloned() {
        if let Some(token) = auth.strip_prefix("Token ") {
            modern.insert("Authorization".to_string(), format!("Bearer {token}"));
        }
    }

    modern.insert("x-api-version".to_string(), "v2".to_string());
    modern
}

/// Translate modern headers back to legacy format.
///
/// * Rewrites `Authorization: Bearer <tok>` → `Authorization: Token <tok>`
/// * Removes `x-api-version`
pub fn adapt_headers_to_legacy(
    modern_headers: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut legacy = modern_headers.clone();

    if let Some(auth) = legacy.get("Authorization").cloned() {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            legacy.insert("Authorization".to_string(), format!("Token {token}"));
        }
    }

    legacy.remove("x-api-version");
    legacy
}

// ---------------------------------------------------------------------------
// PathMapper
// ---------------------------------------------------------------------------

/// Bidirectional path mapper that translates between legacy and modern URL paths.
#[derive(Debug, Clone)]
pub struct PathMapper {
    /// legacy → modern
    mappings: Vec<(String, String)>,
}

impl PathMapper {
    /// Create a new [`PathMapper`] pre-loaded with the default OGame-style
    /// Universus endpoint mappings.
    pub fn new() -> Self {
        let mut mapper = Self {
            mappings: Vec::new(),
        };

        // Default mappings: legacy path → modern path
        mapper.add_mapping("/api/player/overview", "/api/v2/players/{id}/overview");
        mapper.add_mapping("/api/player/resources", "/api/v2/players/{id}/resources");
        mapper.add_mapping("/api/fleet", "/api/v2/players/{id}/fleet");
        mapper.add_mapping("/api/galaxy", "/api/v2/galaxy");
        mapper.add_mapping("/api/research", "/api/v2/players/{id}/research");
        mapper.add_mapping("/api/shipyard", "/api/v2/players/{id}/shipyard");
        mapper.add_mapping("/api/defense", "/api/v2/players/{id}/defense");
        mapper.add_mapping("/api/messages", "/api/v2/players/{id}/messages");
        mapper.add_mapping("/api/alliance", "/api/v2/alliances/{id}");
        mapper.add_mapping("/api/marketplace", "/api/v2/marketplace");

        mapper
    }

    /// Register an additional legacy ↔ modern path mapping.
    pub fn add_mapping(&mut self, legacy: &str, modern: &str) {
        self.mappings.push((legacy.to_string(), modern.to_string()));
    }

    /// Translate a legacy path to its modern equivalent, if a mapping exists.
    pub fn map_path(&self, legacy_path: &str) -> Option<String> {
        self.mappings
            .iter()
            .find(|(l, _)| l == legacy_path)
            .map(|(_, m)| m.clone())
    }

    /// Translate a modern path back to its legacy equivalent, if a mapping exists.
    pub fn reverse_map(&self, modern_path: &str) -> Option<String> {
        self.mappings
            .iter()
            .find(|(_, m)| m == modern_path)
            .map(|(l, _)| l.clone())
    }
}

impl Default for PathMapper {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LegacyCompatAdapter
// ---------------------------------------------------------------------------

/// Full adapter that converts legacy (Node.js v1) requests to modern (Rust v2)
/// format and vice-versa for responses.
#[derive(Debug, Clone)]
pub struct LegacyCompatAdapter {
    pub path_mapper: PathMapper,
}

impl LegacyCompatAdapter {
    pub fn new() -> Self {
        Self {
            path_mapper: PathMapper::new(),
        }
    }
}

impl Default for LegacyCompatAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpCompatAdapter for LegacyCompatAdapter {
    fn adapt_request(&self, input: &str) -> Result<String, AdaptError> {
        let legacy: LegacyRequest =
            serde_json::from_str(input).map_err(|e| AdaptError::InvalidJson(e.to_string()))?;

        let modern_path = self
            .path_mapper
            .map_path(&legacy.path)
            .ok_or_else(|| AdaptError::UnsupportedPath(legacy.path.clone()))?;

        let modern_headers = adapt_headers_to_modern(&legacy.headers);

        let modern_body = match &legacy.body {
            Some(b) => Some(
                transform_request_body(b, &legacy.path)
                    .map_err(|e| AdaptError::TransformFailed(e.to_string()))?,
            ),
            None => None,
        };

        let modern = ModernRequest {
            method: legacy.method,
            path: modern_path,
            headers: modern_headers,
            body: modern_body,
            query_params: legacy.query_params,
            version: "v2".to_string(),
        };

        serde_json::to_string(&modern).map_err(|e| AdaptError::TransformFailed(e.to_string()))
    }

    fn adapt_response(&self, output: &str) -> Result<String, AdaptError> {
        let modern: ModernResponse =
            serde_json::from_str(output).map_err(|e| AdaptError::InvalidJson(e.to_string()))?;

        let legacy_body = match &modern.data {
            Some(d) => transform_response_body(d, "")
                .map_err(|e| AdaptError::TransformFailed(e.to_string()))?,
            None => match &modern.error {
                Some(err) => serde_json::json!({
                    "error": err.message,
                    "code": err.code,
                }),
                None => Value::Null,
            },
        };

        let legacy_headers = adapt_headers_to_legacy(
            &modern
                .meta
                .request_id
                .clone()
                .len()
                // Build a minimal header map from the modern response.
                .eq(&0)
                .then(HashMap::new)
                .unwrap_or_default(),
        );

        let legacy = LegacyResponse {
            status: modern.status,
            body: legacy_body,
            headers: legacy_headers,
        };

        serde_json::to_string(&legacy).map_err(|e| AdaptError::TransformFailed(e.to_string()))
    }

    fn name(&self) -> &str {
        "LegacyCompatAdapter"
    }
}

// ---------------------------------------------------------------------------
// PassthroughAdapter
// ---------------------------------------------------------------------------

/// No-op adapter that returns input/output unchanged.
/// Useful for testing and for post-migration traffic that no longer needs translation.
#[derive(Debug, Clone, Default)]
pub struct PassthroughAdapter;

impl PassthroughAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl HttpCompatAdapter for PassthroughAdapter {
    fn adapt_request(&self, input: &str) -> Result<String, AdaptError> {
        Ok(input.to_string())
    }

    fn adapt_response(&self, output: &str) -> Result<String, AdaptError> {
        Ok(output.to_string())
    }

    fn name(&self) -> &str {
        "PassthroughAdapter"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- camel_to_snake -------------------------------------------------------

    #[test]
    fn test_camel_to_snake_basic() {
        assert_eq!(camel_to_snake("metalMine"), "metal_mine");
        assert_eq!(camel_to_snake("crystalStorage"), "crystal_storage");
        assert_eq!(camel_to_snake("solarPlant"), "solar_plant");
    }

    #[test]
    fn test_camel_to_snake_already_snake() {
        assert_eq!(camel_to_snake("already_snake"), "already_snake");
    }

    #[test]
    fn test_camel_to_snake_multiple_uppercase() {
        assert_eq!(camel_to_snake("galaxyViewURL"), "galaxy_view_u_r_l");
        assert_eq!(
            camel_to_snake("deuteriumSynthesizer"),
            "deuterium_synthesizer"
        );
    }

    // -- snake_to_camel -------------------------------------------------------

    #[test]
    fn test_snake_to_camel_basic() {
        assert_eq!(snake_to_camel("metal_mine"), "metalMine");
        assert_eq!(snake_to_camel("crystal_storage"), "crystalStorage");
    }

    #[test]
    fn test_snake_to_camel_no_underscores() {
        assert_eq!(snake_to_camel("fleet"), "fleet");
    }

    // -- transform_keys_recursive ---------------------------------------------

    #[test]
    fn test_transform_keys_recursive_object() {
        let input = json!({"metalMine": 5, "crystalMine": 3});
        let output = transform_keys_recursive(&input, camel_to_snake);

        assert_eq!(output, json!({"metal_mine": 5, "crystal_mine": 3}));
    }

    #[test]
    fn test_transform_keys_recursive_nested() {
        let input = json!({
            "playerInfo": {
                "metalStorage": 1000,
                "fleetStatus": [
                    {"shipCount": 42, "shipType": "lightFighter"}
                ]
            }
        });

        let output = transform_keys_recursive(&input, camel_to_snake);

        assert_eq!(
            output,
            json!({
                "player_info": {
                    "metal_storage": 1000,
                    "fleet_status": [
                        {"ship_count": 42, "ship_type": "lightFighter"}
                    ]
                }
            })
        );
        // Note: values (strings) are NOT transformed, only keys.
    }

    #[test]
    fn test_transform_keys_roundtrip() {
        let original = json!({"metal_mine": 5, "crystal_mine": 3});
        let camel = transform_keys_recursive(&original, snake_to_camel);
        let back = transform_keys_recursive(&camel, camel_to_snake);
        assert_eq!(back, original);
    }

    // -- PathMapper -----------------------------------------------------------

    #[test]
    fn test_path_mapper_default_mappings() {
        let mapper = PathMapper::new();

        assert_eq!(
            mapper.map_path("/api/player/resources"),
            Some("/api/v2/players/{id}/resources".to_string())
        );
        assert_eq!(
            mapper.map_path("/api/fleet"),
            Some("/api/v2/players/{id}/fleet".to_string())
        );
        assert_eq!(
            mapper.map_path("/api/galaxy"),
            Some("/api/v2/galaxy".to_string())
        );
        assert_eq!(mapper.map_path("/api/nonexistent"), None);
    }

    #[test]
    fn test_path_mapper_reverse() {
        let mapper = PathMapper::new();

        assert_eq!(
            mapper.reverse_map("/api/v2/players/{id}/fleet"),
            Some("/api/fleet".to_string())
        );
        assert_eq!(mapper.reverse_map("/api/v2/unknown"), None);
    }

    #[test]
    fn test_path_mapper_custom_mapping() {
        let mut mapper = PathMapper::new();
        mapper.add_mapping("/api/custom/legacy", "/api/v2/custom/modern");

        assert_eq!(
            mapper.map_path("/api/custom/legacy"),
            Some("/api/v2/custom/modern".to_string())
        );
        assert_eq!(
            mapper.reverse_map("/api/v2/custom/modern"),
            Some("/api/custom/legacy".to_string())
        );
    }

    // -- Header adaptation ----------------------------------------------------

    #[test]
    fn test_adapt_headers_to_modern() {
        let mut legacy = HashMap::new();
        legacy.insert("Authorization".to_string(), "Token abc123".to_string());
        legacy.insert("Content-Type".to_string(), "application/json".to_string());

        let modern = adapt_headers_to_modern(&legacy);

        assert_eq!(modern.get("Authorization").unwrap(), "Bearer abc123");
        assert_eq!(modern.get("x-api-version").unwrap(), "v2");
        assert_eq!(modern.get("Content-Type").unwrap(), "application/json");
    }

    #[test]
    fn test_adapt_headers_to_legacy() {
        let mut modern = HashMap::new();
        modern.insert("Authorization".to_string(), "Bearer abc123".to_string());
        modern.insert("x-api-version".to_string(), "v2".to_string());

        let legacy = adapt_headers_to_legacy(&modern);

        assert_eq!(legacy.get("Authorization").unwrap(), "Token abc123");
        assert!(!legacy.contains_key("x-api-version"));
    }

    // -- Version detection ----------------------------------------------------

    #[test]
    fn test_extract_api_version() {
        let mut h = HashMap::new();
        assert_eq!(extract_api_version(&h), ApiVersion::Unknown);

        h.insert("x-api-version".to_string(), "v1".to_string());
        assert_eq!(extract_api_version(&h), ApiVersion::V1Legacy);

        h.insert("x-api-version".to_string(), "v2".to_string());
        assert_eq!(extract_api_version(&h), ApiVersion::V2Modern);
    }

    #[test]
    fn test_should_adapt() {
        assert!(should_adapt(&ApiVersion::V1Legacy));
        assert!(!should_adapt(&ApiVersion::V2Modern));
        assert!(!should_adapt(&ApiVersion::Unknown));
    }

    // -- LegacyCompatAdapter full round-trip ----------------------------------

    #[test]
    fn test_legacy_compat_adapt_request() {
        let adapter = LegacyCompatAdapter::new();

        let legacy = LegacyRequest {
            method: "GET".to_string(),
            path: "/api/player/resources".to_string(),
            headers: {
                let mut h = HashMap::new();
                h.insert("Authorization".to_string(), "Token secret".to_string());
                h
            },
            body: Some(json!({"metalMine": 5})),
            query_params: HashMap::new(),
        };

        let input = serde_json::to_string(&legacy).unwrap();
        let output = adapter.adapt_request(&input).unwrap();
        let modern: ModernRequest = serde_json::from_str(&output).unwrap();

        assert_eq!(modern.path, "/api/v2/players/{id}/resources");
        assert_eq!(modern.version, "v2");
        assert_eq!(
            modern.headers.get("Authorization").unwrap(),
            "Bearer secret"
        );
        assert_eq!(modern.body, Some(json!({"metal_mine": 5})));
    }

    #[test]
    fn test_legacy_compat_adapt_request_unsupported_path() {
        let adapter = LegacyCompatAdapter::new();

        let legacy = LegacyRequest {
            method: "GET".to_string(),
            path: "/api/nonexistent".to_string(),
            headers: HashMap::new(),
            body: None,
            query_params: HashMap::new(),
        };

        let input = serde_json::to_string(&legacy).unwrap();
        let result = adapter.adapt_request(&input);
        assert!(matches!(result, Err(AdaptError::UnsupportedPath(_))));
    }

    #[test]
    fn test_legacy_compat_adapt_request_invalid_json() {
        let adapter = LegacyCompatAdapter::new();
        let result = adapter.adapt_request("not json at all");
        assert!(matches!(result, Err(AdaptError::InvalidJson(_))));
    }

    #[test]
    fn test_legacy_compat_adapt_response() {
        let adapter = LegacyCompatAdapter::new();

        let modern = ModernResponse {
            status: 200,
            data: Some(json!({"metal_mine": 10, "crystal_mine": 7})),
            error: None,
            meta: ResponseMeta {
                request_id: "req-123".to_string(),
                timestamp: 1700000000,
                version: "v2".to_string(),
            },
        };

        let input = serde_json::to_string(&modern).unwrap();
        let output = adapter.adapt_response(&input).unwrap();
        let legacy: LegacyResponse = serde_json::from_str(&output).unwrap();

        assert_eq!(legacy.status, 200);
        assert_eq!(legacy.body, json!({"metalMine": 10, "crystalMine": 7}));
    }

    #[test]
    fn test_legacy_compat_adapt_response_error() {
        let adapter = LegacyCompatAdapter::new();

        let modern = ModernResponse {
            status: 404,
            data: None,
            error: Some(ErrorPayload {
                code: "NOT_FOUND".to_string(),
                message: "Planet not found".to_string(),
                details: None,
            }),
            meta: ResponseMeta {
                request_id: "req-456".to_string(),
                timestamp: 1700000000,
                version: "v2".to_string(),
            },
        };

        let input = serde_json::to_string(&modern).unwrap();
        let output = adapter.adapt_response(&input).unwrap();
        let legacy: LegacyResponse = serde_json::from_str(&output).unwrap();

        assert_eq!(legacy.status, 404);
        assert_eq!(legacy.body["error"], "Planet not found");
        assert_eq!(legacy.body["code"], "NOT_FOUND");
    }

    // -- PassthroughAdapter ---------------------------------------------------

    #[test]
    fn test_passthrough_adapter_request() {
        let adapter = PassthroughAdapter::new();
        let input = r#"{"anything":"goes"}"#;
        assert_eq!(adapter.adapt_request(input).unwrap(), input);
    }

    #[test]
    fn test_passthrough_adapter_response() {
        let adapter = PassthroughAdapter::new();
        let output = r#"{"status":200}"#;
        assert_eq!(adapter.adapt_response(output).unwrap(), output);
    }

    #[test]
    fn test_passthrough_adapter_name() {
        let adapter = PassthroughAdapter::new();
        assert_eq!(adapter.name(), "PassthroughAdapter");
    }

    // -- Body transformation --------------------------------------------------

    #[test]
    fn test_transform_request_body() {
        let body = json!({"fleetSpeed": 10, "targetGalaxy": 3});
        let result = transform_request_body(&body, "/api/fleet").unwrap();
        assert_eq!(result, json!({"fleet_speed": 10, "target_galaxy": 3}));
    }

    #[test]
    fn test_transform_response_body() {
        let body = json!({"fleet_speed": 10, "target_galaxy": 3});
        let result = transform_response_body(&body, "/api/v2/fleet").unwrap();
        assert_eq!(result, json!({"fleetSpeed": 10, "targetGalaxy": 3}));
    }

    // -- Adapter name ---------------------------------------------------------

    #[test]
    fn test_legacy_adapter_name() {
        let adapter = LegacyCompatAdapter::new();
        assert_eq!(adapter.name(), "LegacyCompatAdapter");
    }
}
