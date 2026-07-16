use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, RequestExt};
use serde::Serialize;

use crate::auth_guard::BearerToken;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ActorPolicy {
    Public,
    Player,
    Admin,
    SuperAdmin,
    Service { scope: &'static str },
    AdminOrService { scope: &'static str },
    SelfOrAdminPath { parameter: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RouteAuthorization {
    pub method: &'static str,
    pub path: &'static str,
    pub policy: ActorPolicy,
}

macro_rules! rule {
    ($method:literal, $path:literal, $policy:expr) => {
        RouteAuthorization {
            method: $method,
            path: $path,
            policy: $policy,
        }
    };
}

/// Exhaustive authorization contract for every gateway route.
///
/// Runtime enforcement fails closed for any matched route absent from this
/// table. A source-coverage test independently scans every router declaration
/// and ensures this table cannot drift when routes are added or removed.
pub const ROUTE_AUTHORIZATION: &[RouteAuthorization] = &[
    // Liveness, readiness, authentication entry points and public discovery.
    rule!("GET", "/health", ActorPolicy::Public),
    rule!("GET", "/api/health", ActorPolicy::Public),
    rule!("GET", "/metrics", ActorPolicy::Public),
    rule!("GET", "/ready", ActorPolicy::Public),
    rule!("POST", "/api/auth/login", ActorPolicy::Public),
    rule!("POST", "/api/auth/register", ActorPolicy::Public),
    rule!("POST", "/api/auth/logout", ActorPolicy::Player),
    rule!("GET", "/api/auth/me", ActorPolicy::Player),
    rule!("GET", "/api/achievements", ActorPolicy::Public),
    rule!("GET", "/api/achievements/badges", ActorPolicy::Public),
    rule!("GET", "/api/achievements/hall-of-fame", ActorPolicy::Public),
    rule!("GET", "/api/achievements/ladders", ActorPolicy::Public),
    rule!("GET", "/api/achievements/rewards", ActorPolicy::Public),
    rule!("GET", "/api/alliance", ActorPolicy::Public),
    rule!("GET", "/api/alliance/diplomacy", ActorPolicy::Public),
    rule!("GET", "/api/alliance/members", ActorPolicy::Public),
    rule!("GET", "/api/alliances", ActorPolicy::Public),
    rule!("GET", "/api/alliances/diplomacy", ActorPolicy::Public),
    rule!("GET", "/api/alliances/members", ActorPolicy::Public),
    rule!("GET", "/api/galaxy", ActorPolicy::Public),
    rule!("GET", "/api/galaxy/:galaxy/:system", ActorPolicy::Public),
    rule!(
        "GET",
        "/api/galaxy/:galaxy/:system/:position",
        ActorPolicy::Public
    ),
    rule!("GET", "/api/leaderboard", ActorPolicy::Public),
    rule!("GET", "/api/leaderboard/alliances", ActorPolicy::Public),
    rule!("GET", "/api/leaderboard/players", ActorPolicy::Public),
    rule!("GET", "/api/moons/public/:moon_id", ActorPolicy::Public),
    rule!("GET", "/api/shop/offers", ActorPolicy::Public),
    rule!("GET", "/api/shop/packages", ActorPolicy::Public),
    rule!("POST", "/api/shop/purchase-preview", ActorPolicy::Public),
    rule!("GET", "/api/shop-enhanced/cosmetics", ActorPolicy::Public),
    rule!("GET", "/api/shop-enhanced/flash-sales", ActorPolicy::Public),
    rule!("GET", "/api/shop-enhanced/promotions", ActorPolicy::Public),
    rule!("GET", "/api/themes", ActorPolicy::Public),
    rule!("GET", "/api/themes/:id", ActorPolicy::Public),
    rule!("GET", "/api/themes/current", ActorPolicy::Public),
    rule!("GET", "/api/universe", ActorPolicy::Public),
    rule!("GET", "/api/universe/:id", ActorPolicy::Public),
    rule!("GET", "/api/users/leaderboard", ActorPolicy::Public),
    // Signed human actors. Moderator/admin tiers retain normal player access;
    // service identities do not inherit player data access.
    rule!("GET", "/api/account/profile", ActorPolicy::Player),
    rule!("GET", "/api/account/resources", ActorPolicy::Player),
    rule!("GET", "/api/acs", ActorPolicy::Player),
    rule!("POST", "/api/acs", ActorPolicy::Player),
    rule!("POST", "/api/acs/:id/join", ActorPolicy::Player),
    rule!("DELETE", "/api/acs/:id/leave", ActorPolicy::Player),
    rule!("POST", "/api/analytics/events", ActorPolicy::Player),
    rule!("POST", "/api/combat/simulate", ActorPolicy::Player),
    rule!("GET", "/api/debris", ActorPolicy::Player),
    rule!("GET", "/api/debris/:id", ActorPolicy::Player),
    rule!("POST", "/api/debris/:id/claim", ActorPolicy::Player),
    rule!("GET", "/api/debris/claims/my", ActorPolicy::Player),
    rule!(
        "GET",
        "/api/debris/location/:galaxy/:system/:position",
        ActorPolicy::Player
    ),
    rule!("POST", "/api/debris/search", ActorPolicy::Player),
    rule!("GET", "/api/debris/system/stats", ActorPolicy::Player),
    rule!("GET", "/api/fleet", ActorPolicy::Player),
    rule!("GET", "/api/fleet/:fleet_id", ActorPolicy::Player),
    rule!(
        "POST",
        "/api/fleet/helpers/combat/attacker-distribution",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/fleet/helpers/combat/defense-rebuild",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/fleet/helpers/espionage-outcome",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/fleet/helpers/harvest-collection",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/fleet/helpers/mission-cargo-transfer",
        ActorPolicy::Player
    ),
    rule!("POST", "/api/fleet/helpers/movement", ActorPolicy::Player),
    rule!("POST", "/api/fleet/move", ActorPolicy::Player),
    rule!("POST", "/api/fleet/movement", ActorPolicy::Player),
    rule!("POST", "/api/fleet/send", ActorPolicy::Player),
    rule!("GET", "/api/marketplace/listings", ActorPolicy::Player),
    rule!("POST", "/api/marketplace/listings", ActorPolicy::Player),
    rule!(
        "DELETE",
        "/api/marketplace/listings/:id",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/marketplace/listings/:id", ActorPolicy::Player),
    rule!(
        "POST",
        "/api/marketplace/listings/:id/accept",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/marketplace/my-history", ActorPolicy::Player),
    rule!("GET", "/api/marketplace/my-listings", ActorPolicy::Player),
    rule!("GET", "/api/messages", ActorPolicy::Player),
    rule!("GET", "/api/messages/:message_id", ActorPolicy::Player),
    rule!("POST", "/api/messages/send", ActorPolicy::Player),
    rule!("GET", "/api/messages/unread-count", ActorPolicy::Player),
    rule!("GET", "/api/moons", ActorPolicy::Player),
    rule!("POST", "/api/moons/:moon_id/destroy", ActorPolicy::Player),
    rule!("POST", "/api/moons/:moon_id/jump-gate", ActorPolicy::Player),
    rule!("POST", "/api/moons/:moon_id/phalanx", ActorPolicy::Player),
    rule!("GET", "/api/moons/:planet_id", ActorPolicy::Player),
    rule!("GET", "/api/moons/id/:moon_id", ActorPolicy::Player),
    rule!("POST", "/moons/:moon_id/destroy", ActorPolicy::Player),
    rule!("GET", "/api/notifications", ActorPolicy::Player),
    rule!(
        "POST",
        "/api/notifications/:notification_id/read",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/notifications/preferences", ActorPolicy::Player),
    rule!(
        "PUT",
        "/api/notifications/preferences/:category",
        ActorPolicy::Player
    ),
    rule!("POST", "/api/notifications/read-all", ActorPolicy::Player),
    rule!(
        "GET",
        "/api/notifications/unread-count",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/planets", ActorPolicy::Player),
    rule!("GET", "/api/planets/:planet_id", ActorPolicy::Player),
    rule!("POST", "/api/planets/:planet_id/build", ActorPolicy::Player),
    rule!(
        "GET",
        "/api/planets/:planet_id/buildings",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/planets/:planet_id/build-quote",
        ActorPolicy::Player
    ),
    rule!(
        "GET",
        "/api/planets/:planet_id/build-queue",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/planets/:planet_id/rename",
        ActorPolicy::Player
    ),
    rule!(
        "GET",
        "/api/planets/:planet_id/resources",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/player-blocks", ActorPolicy::Player),
    rule!("POST", "/api/player-blocks", ActorPolicy::Player),
    rule!(
        "DELETE",
        "/api/player-blocks/:target_identifier",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/research", ActorPolicy::Player),
    rule!("POST", "/api/research/:tech_id/cost", ActorPolicy::Player),
    rule!("GET", "/api/research/queue", ActorPolicy::Player),
    rule!("POST", "/api/research/start", ActorPolicy::Player),
    rule!("POST", "/api/rips/destroyMoon", ActorPolicy::Player),
    rule!(
        "GET",
        "/api/shipyard/:planet_id/build-options",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/shipyard/:planet_id/build-preview",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/shipyard/:planet_id/queue", ActorPolicy::Player),
    rule!("POST", "/api/shipyard/build", ActorPolicy::Player),
    rule!(
        "GET",
        "/api/shop-enhanced/matrix/progress",
        ActorPolicy::Player
    ),
    rule!(
        "GET",
        "/api/shop-enhanced/my-cosmetics",
        ActorPolicy::Player
    ),
    rule!(
        "POST",
        "/api/shop-enhanced/promotions/validate",
        ActorPolicy::Player
    ),
    rule!("GET", "/api/themes/user/custom-css", ActorPolicy::Player),
    rule!("PUT", "/api/themes/user/custom-css", ActorPolicy::Player),
    rule!("GET", "/api/themes/user/preferences", ActorPolicy::Player),
    rule!("PUT", "/api/themes/user/preferences", ActorPolicy::Player),
    rule!("GET", "/api/users/me", ActorPolicy::Player),
    // Signed-subject ownership. The numeric bridge supports both legacy
    // numeric subjects and current string account ids without trusting a
    // caller-supplied user id.
    rule!(
        "GET",
        "/api/achievements/user/:user_id/achievements",
        ActorPolicy::SelfOrAdminPath {
            parameter: "user_id"
        }
    ),
    rule!(
        "GET",
        "/api/achievements/user/:user_id/badges",
        ActorPolicy::SelfOrAdminPath {
            parameter: "user_id"
        }
    ),
    rule!(
        "GET",
        "/api/achievements/user/:user_id/rewards",
        ActorPolicy::SelfOrAdminPath {
            parameter: "user_id"
        }
    ),
    // Administrative and operational data.
    rule!("GET", "/api/analytics/usage", ActorPolicy::Admin),
    rule!("GET", "/api/config/categories", ActorPolicy::Admin),
    rule!(
        "GET",
        "/api/config/categories/:category",
        ActorPolicy::Admin
    ),
    rule!("GET", "/api/config/game-config", ActorPolicy::Admin),
    rule!(
        "POST",
        "/api/config/game-config/refresh",
        ActorPolicy::Admin
    ),
    rule!("GET", "/api/config/history", ActorPolicy::Admin),
    rule!("GET", "/api/config/parameters", ActorPolicy::Admin),
    rule!("GET", "/api/config/parameters/:key", ActorPolicy::Admin),
    rule!("PUT", "/api/config/parameters/:key", ActorPolicy::Admin),
    rule!("POST", "/api/debris/generate", ActorPolicy::Player),
    rule!("GET", "/api/shards/health/overview", ActorPolicy::Admin),
    rule!(
        "GET",
        "/api/shards/leaderboards/:category",
        ActorPolicy::Admin
    ),
    rule!("GET", "/api/shards/messages/failed", ActorPolicy::Admin),
    rule!("GET", "/api/shards/messages/status", ActorPolicy::Admin),
    rule!("POST", "/api/shards/routing/calculate", ActorPolicy::Admin),
    rule!("GET", "/api/shards/routing/player/:id", ActorPolicy::Admin),
    rule!(
        "GET",
        "/api/shards/routing/servers/available",
        ActorPolicy::Admin
    ),
    rule!("GET", "/api/shards/routing/stats", ActorPolicy::Admin),
    rule!("GET", "/api/shards/servers", ActorPolicy::Admin),
    rule!("GET", "/api/shards/servers/:id/health", ActorPolicy::Admin),
    rule!("GET", "/api/shards/servers/stats", ActorPolicy::Admin),
    rule!("POST", "/api/universe/create", ActorPolicy::Admin),
    rule!("POST", "/api/universe/:id/seed", ActorPolicy::Admin),
    rule!("POST", "/api/universe/:id/place-player", ActorPolicy::Admin),
    rule!("GET", "/api/universe/:id/stats", ActorPolicy::Admin),
    rule!(
        "POST",
        "/api/universe/:id/maintenance/start",
        ActorPolicy::Admin
    ),
    rule!(
        "POST",
        "/api/universe/:id/maintenance/population-balance",
        ActorPolicy::Admin
    ),
    rule!(
        "PATCH",
        "/api/universe/:id/registration",
        ActorPolicy::Admin
    ),
    rule!("PATCH", "/api/universe/:id/speed", ActorPolicy::Admin),
    rule!(
        "PATCH",
        "/api/universe/:id/announcement",
        ActorPolicy::Admin
    ),
    // Irreversible or broad operational changes require the top human tier.
    rule!(
        "POST",
        "/api/shards/routing/migrate",
        ActorPolicy::SuperAdmin
    ),
    rule!(
        "PATCH",
        "/api/universe/:id/lifecycle",
        ActorPolicy::SuperAdmin
    ),
    rule!("PATCH", "/api/universe/:id/merge", ActorPolicy::SuperAdmin),
    rule!(
        "PATCH",
        "/api/universe/:id/end-event",
        ActorPolicy::SuperAdmin
    ),
    // Trusted service writers may grant outcomes and enqueue internal work;
    // admins retain an intentional manual recovery path.
    rule!(
        "POST",
        "/api/achievements/user/:user_id/achievements/:achievement_id",
        ActorPolicy::AdminOrService {
            scope: "achievements.write"
        }
    ),
    rule!(
        "POST",
        "/api/achievements/user/:user_id/badges/:badge_id",
        ActorPolicy::AdminOrService {
            scope: "achievements.write"
        }
    ),
    rule!(
        "POST",
        "/api/achievements/user/:user_id/rewards/:reward_id",
        ActorPolicy::AdminOrService {
            scope: "achievements.write"
        }
    ),
    rule!(
        "POST",
        "/api/notifications",
        ActorPolicy::AdminOrService {
            scope: "notifications.write"
        }
    ),
    rule!(
        "POST",
        "/api/shards/messages/broadcast",
        ActorPolicy::AdminOrService {
            scope: "shards.messages.write"
        }
    ),
    rule!(
        "POST",
        "/api/shards/messages/enqueue",
        ActorPolicy::AdminOrService {
            scope: "shards.messages.write"
        }
    ),
    rule!(
        "POST",
        "/api/shards/messages/requeue-failed",
        ActorPolicy::AdminOrService {
            scope: "shards.messages.write"
        }
    ),
    rule!(
        "POST",
        "/api/shards/servers/register",
        ActorPolicy::AdminOrService {
            scope: "shards.servers.register"
        }
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActorRole {
    Player,
    Moderator,
    Admin,
    SuperAdmin,
    Service,
}

impl ActorRole {
    fn from_claim(role: &str) -> Option<Self> {
        match role.trim().to_ascii_lowercase().as_str() {
            "player" => Some(Self::Player),
            "moderator" => Some(Self::Moderator),
            "admin" => Some(Self::Admin),
            "superadmin" => Some(Self::SuperAdmin),
            "service" => Some(Self::Service),
            _ => None,
        }
    }

    fn is_human(self) -> bool {
        !matches!(self, Self::Service)
    }

    fn is_admin(self) -> bool {
        matches!(self, Self::Admin | Self::SuperAdmin)
    }
}

#[derive(Serialize)]
struct AuthorizationError {
    success: bool,
    error: &'static str,
}

pub async fn enforce_route_authorization(mut request: Request<Body>, next: Next<Body>) -> Response {
    let Some(matched_path) = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str().to_string())
    else {
        return next.run(request).await;
    };

    let method = effective_method(request.method());
    let Some(rule) = ROUTE_AUTHORIZATION
        .iter()
        .find(|rule| rule.method == method && rule.path == matched_path.as_str())
    else {
        return authorization_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Route authorization policy is not configured",
        );
    };

    if rule.policy == ActorPolicy::Public {
        return next.run(request).await;
    }

    if request.extract_parts::<BearerToken>().await.is_err() {
        return authorization_response(StatusCode::UNAUTHORIZED, "Unauthorized");
    }

    let Some(claims) = request.extensions().get::<platform_auth::Claims>() else {
        return authorization_response(StatusCode::UNAUTHORIZED, "Unauthorized");
    };
    let Some(role) = ActorRole::from_claim(&claims.role) else {
        return authorization_response(StatusCode::FORBIDDEN, "Forbidden");
    };

    let allowed = match rule.policy {
        ActorPolicy::Public => true,
        ActorPolicy::Player => claims.is_access_token() && role.is_human(),
        ActorPolicy::Admin => claims.is_access_token() && role.is_admin(),
        ActorPolicy::SuperAdmin => claims.is_access_token() && role == ActorRole::SuperAdmin,
        ActorPolicy::Service { scope } => claims.is_service_token() && claims.has_scope(scope),
        ActorPolicy::AdminOrService { scope } => {
            (claims.is_access_token() && role.is_admin())
                || (claims.is_service_token() && claims.has_scope(scope))
        }
        ActorPolicy::SelfOrAdminPath { parameter } => {
            claims.is_access_token()
                && (role.is_admin()
                    || (role.is_human()
                        && path_parameter(&matched_path, request.uri().path(), parameter)
                            .and_then(|value| value.parse::<i64>().ok())
                            .is_some_and(|requested| {
                                subject_matches_numeric(&claims.sub, requested)
                            })))
        }
    };

    if !allowed {
        return authorization_response(StatusCode::FORBIDDEN, "Forbidden");
    }

    next.run(request).await
}

fn effective_method(method: &Method) -> &str {
    if *method == Method::HEAD {
        "GET"
    } else {
        method.as_str()
    }
}

fn path_parameter<'a>(template: &str, actual: &'a str, parameter: &str) -> Option<&'a str> {
    let wanted = format!(":{parameter}");
    template
        .trim_matches('/')
        .split('/')
        .zip(actual.trim_matches('/').split('/'))
        .find_map(|(template_segment, actual_segment)| {
            (template_segment == wanted).then_some(actual_segment)
        })
}

pub fn numeric_subject(subject: &str) -> i64 {
    subject
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or_else(|| platform_auth::stable_numeric_subject_id(subject) as i64)
}

pub fn subject_matches_numeric(subject: &str, requested: i64) -> bool {
    requested > 0 && numeric_subject(subject) == requested
}

/// Resolve an optional legacy numeric user selector without allowing a human
/// caller to switch away from the signed subject. Admins may intentionally
/// select another account; service identities must use a service-only writer.
pub fn effective_numeric_user_id(
    user: &platform_auth::AuthUser,
    requested: Option<i64>,
) -> Result<i64, Response> {
    let Some(role) = ActorRole::from_claim(&user.role) else {
        return Err(authorization_response(StatusCode::FORBIDDEN, "Forbidden"));
    };
    let own_user_id = numeric_subject(&user.user_id);
    let selected = requested.unwrap_or(own_user_id);

    if user.token_purpose == platform_auth::TOKEN_PURPOSE_ACCESS
        && (role.is_admin() || (role.is_human() && selected == own_user_id))
    {
        Ok(selected)
    } else {
        Err(authorization_response(StatusCode::FORBIDDEN, "Forbidden"))
    }
}

fn authorization_response(status: StatusCode, error: &'static str) -> Response {
    (
        status,
        Json(AuthorizationError {
            success: false,
            error,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn every_declared_route_has_exactly_one_authorization_policy() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut files = vec![manifest.join("src/routes.rs")];
        let routes_dir = manifest.join("src/routes");
        files.extend(
            fs::read_dir(routes_dir)
                .expect("route directory")
                .map(|entry| entry.expect("route file").path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "rs")),
        );

        let mut declared = BTreeSet::new();
        for file in files {
            let source = fs::read_to_string(&file).expect("route source");
            declared.extend(extract_route_declarations(&source));
        }

        let configured = ROUTE_AUTHORIZATION
            .iter()
            .map(|rule| (rule.method.to_string(), rule.path.to_string()))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            configured.len(),
            ROUTE_AUTHORIZATION.len(),
            "duplicate policy"
        );
        assert_eq!(declared, configured, "route authorization table drifted");
    }

    #[test]
    fn numeric_subjects_preserve_legacy_ids_and_bridge_string_ids() {
        assert_eq!(numeric_subject("42"), 42);
        assert!(subject_matches_numeric("42", 42));
        assert_eq!(
            numeric_subject("acct-string"),
            platform_auth::stable_numeric_subject_id("acct-string") as i64
        );
    }

    fn extract_route_declarations(source: &str) -> Vec<(String, String)> {
        let mut declarations = Vec::new();
        let mut remainder = source;
        while let Some(route_start) = remainder.find(".route(") {
            remainder = &remainder[route_start + ".route(".len()..];
            let Some(call_end) = matching_call_end(remainder) else {
                panic!("unclosed .route declaration");
            };
            let call = &remainder[..call_end];
            let path = first_string_literal(call).expect("route path literal");
            let method = ["get", "post", "put", "patch", "delete"]
                .into_iter()
                .find(|method| call.contains(&format!("{method}(")))
                .expect("route method");
            declarations.push((method.to_ascii_uppercase(), path));
            remainder = &remainder[call_end + 1..];
        }
        declarations
    }

    fn matching_call_end(source: &str) -> Option<usize> {
        let mut depth = 1usize;
        let mut in_string = false;
        let mut escaped = false;
        for (index, character) in source.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                continue;
            }
            match character {
                '"' => in_string = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn first_string_literal(source: &str) -> Option<String> {
        let start = source.find('"')? + 1;
        let tail = &source[start..];
        let end = tail.find('"')?;
        Some(tail[..end].to_string())
    }
}
