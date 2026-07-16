//! Account repository boundary for authentication routes.
//!
//! PostgreSQL is the durable backend whenever `DATABASE_URL` is configured.
//! The in-memory backend is intentionally limited to non-production local
//! development and deterministic tests.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use platform_db::{
    AccountCreateError, AccountCreateInput, AccountRow, AuthPrincipal, AuthRefreshRotateInput,
    AuthRegistrationThrottlePolicy, AuthSessionCreateInput, AuthSessionError, AuthSessionIssue,
    AuthSessionView, AuthThrottlePolicy, AuthThrottleStatus, Database,
};

#[derive(Clone)]
pub struct AccountRepository {
    backend: AccountBackend,
}

#[derive(Clone)]
enum AccountBackend {
    Postgres(Database),
    Memory(Arc<Mutex<MemoryAccounts>>),
    Unavailable(String),
}

#[derive(Default)]
struct MemoryAccounts {
    next_id: u64,
    by_email: HashMap<String, AccountRow>,
    usernames: HashSet<String>,
    sessions: HashMap<String, MemorySession>,
    refresh_tokens: HashMap<Vec<u8>, MemoryRefreshToken>,
    throttles: HashMap<(String, Vec<u8>), MemoryThrottle>,
}

#[derive(Clone)]
struct MemorySession {
    session_id: String,
    family_id: String,
    user_id: String,
    device_label: Option<String>,
    created_at_unix: i64,
    last_used_at_unix: i64,
    expires_at_unix: i64,
    revoked_at_unix: Option<i64>,
    auth_epoch: i64,
    generation: i64,
}

#[derive(Clone)]
struct MemoryRefreshToken {
    session_id: String,
    consumed: bool,
    revoked: bool,
}

#[derive(Clone, Copy)]
struct MemoryThrottle {
    window_started_at_unix: i64,
    failure_count: i32,
    blocked_until_unix: Option<i64>,
}

/// Bounded runtime policy for durable sessions and normalized login throttles.
#[derive(Clone)]
pub struct SessionSecurityConfig {
    pub refresh_expiry_seconds: i64,
    pub max_sessions_per_user: usize,
    pub throttle_policy: AuthThrottlePolicy,
    pub registration_throttle_policy: AuthRegistrationThrottlePolicy,
    digest_key: Vec<u8>,
}

impl std::fmt::Debug for SessionSecurityConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionSecurityConfig")
            .field("refresh_expiry_seconds", &self.refresh_expiry_seconds)
            .field("max_sessions_per_user", &self.max_sessions_per_user)
            .field("throttle_policy", &self.throttle_policy)
            .field(
                "registration_throttle_policy",
                &self.registration_throttle_policy,
            )
            .field("digest_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    Duplicate,
    Unavailable(String),
    Storage(String),
}

impl SessionSecurityConfig {
    pub fn from_environment() -> Result<Self, String> {
        let jwt_expiry_seconds = bounded_env_i64("JWT_EXPIRY_SECONDS", 86_400, 60, 86_400)?;
        let refresh_expiry_seconds =
            bounded_env_i64("REFRESH_EXPIRY_SECONDS", 604_800, 300, 7_776_000)?;
        if refresh_expiry_seconds <= jwt_expiry_seconds {
            return Err(
                "REFRESH_EXPIRY_SECONDS must be between 300 and 7776000 and exceed JWT_EXPIRY_SECONDS"
                    .to_string(),
            );
        }
        let max_sessions_per_user = bounded_env_i64("MAX_SESSIONS_PER_USER", 5, 1, 100)? as usize;

        let environment = runtime_environment();
        let digest_key = std::env::var("AUTH_SESSION_DIGEST_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(String::into_bytes)
            .unwrap_or_else(|| {
                if development_environment(&environment) {
                    b"universus-local-session-digest-key-v1".to_vec()
                } else {
                    Vec::new()
                }
            });
        if digest_key.len() < 32 {
            return Err(
                "AUTH_SESSION_DIGEST_KEY must contain at least 32 bytes outside local development"
                    .to_string(),
            );
        }

        let throttle_policy = AuthThrottlePolicy {
            window_seconds: bounded_env_i64("AUTH_LOGIN_WINDOW_SECONDS", 900, 60, 86_400)?,
            account_failure_limit: bounded_env_i32(
                "AUTH_LOGIN_ACCOUNT_FAILURE_LIMIT",
                5,
                1,
                1_000,
            )?,
            ip_failure_limit: bounded_env_i32("AUTH_LOGIN_IP_FAILURE_LIMIT", 20, 1, 1_000)?,
            block_seconds: bounded_env_i64("AUTH_LOGIN_BLOCK_SECONDS", 900, 60, 86_400)?,
        };
        let registration_throttle_policy = AuthRegistrationThrottlePolicy {
            window_seconds: bounded_env_i64(
                "AUTH_REGISTRATION_IP_WINDOW_SECONDS",
                3_600,
                60,
                86_400,
            )?,
            attempt_limit: bounded_env_i32("AUTH_REGISTRATION_IP_ATTEMPT_LIMIT", 5, 1, 1_000)?,
            block_seconds: bounded_env_i64(
                "AUTH_REGISTRATION_IP_BLOCK_SECONDS",
                3_600,
                60,
                86_400,
            )?,
        };

        Ok(Self {
            refresh_expiry_seconds,
            max_sessions_per_user,
            throttle_policy,
            registration_throttle_policy,
            digest_key,
        })
    }

    pub fn digest_account(&self, normalized_email: &str) -> Vec<u8> {
        platform_auth::keyed_metadata_digest(&self.digest_key, normalized_email.as_bytes()).to_vec()
    }

    pub fn digest_ip(&self, ip: Option<&str>) -> Option<Vec<u8>> {
        ip.filter(|value| !value.trim().is_empty()).map(|value| {
            platform_auth::keyed_metadata_digest(&self.digest_key, value.trim().as_bytes()).to_vec()
        })
    }

    pub fn digest_user_agent(&self, user_agent: Option<&str>) -> Option<Vec<u8>> {
        user_agent
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                platform_auth::keyed_metadata_digest(&self.digest_key, value.trim().as_bytes())
                    .to_vec()
            })
    }

    pub fn digest_registration_ip(&self, ip: Option<&str>) -> Vec<u8> {
        let normalized = ip
            .filter(|value| !value.trim().is_empty())
            .map(str::trim)
            .unwrap_or("unavailable-peer");
        platform_auth::keyed_metadata_digest(&self.digest_key, normalized.as_bytes()).to_vec()
    }
}

impl AccountRepository {
    pub fn from_environment(database: Option<Database>) -> Self {
        match database {
            Some(database) => Self {
                backend: AccountBackend::Postgres(database),
            },
            None if development_environment(&runtime_environment()) => Self::in_memory(),
            None if production_like_environment(&runtime_environment()) => Self {
                backend: AccountBackend::Unavailable(
                    "DATABASE_URL is required for account persistence in production-like environments"
                        .to_string(),
                ),
            },
            None => Self {
                backend: AccountBackend::Unavailable(
                    "DATABASE_URL is required outside explicit development/test environments"
                        .to_string(),
                ),
            },
        }
    }

    pub fn in_memory() -> Self {
        Self {
            backend: AccountBackend::Memory(Arc::new(Mutex::new(MemoryAccounts {
                next_id: 1,
                ..MemoryAccounts::default()
            }))),
        }
    }

    /// Construct an explicitly unavailable repository for readiness checks and
    /// deterministic failure-path tests.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            backend: AccountBackend::Unavailable(reason.into()),
        }
    }

    pub async fn create(&self, input: AccountCreateInput) -> Result<AccountRow, RepositoryError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database
                .register_account_with_starting_state(input)
                .await
                .map_err(|error| match error {
                    AccountCreateError::Duplicate => RepositoryError::Duplicate,
                    AccountCreateError::Database(message) => RepositoryError::Storage(message),
                }),
            AccountBackend::Memory(memory) => {
                let input = input.normalized();
                let normalized_username = input.username.to_ascii_lowercase();
                let mut state = memory
                    .lock()
                    .map_err(|_| RepositoryError::Storage("account store poisoned".to_string()))?;
                if state.by_email.contains_key(&input.email)
                    || state.usernames.contains(&normalized_username)
                {
                    return Err(RepositoryError::Duplicate);
                }

                let account = AccountRow {
                    id: format!("dev-{}", state.next_id),
                    username: input.username,
                    email: input.email.clone(),
                    password_hash: input.password_hash,
                    role: "player".to_string(),
                    universe_id: Some(1),
                    is_banned: false,
                };
                state.next_id = state.next_id.saturating_add(1);
                state.usernames.insert(normalized_username);
                state.by_email.insert(input.email, account.clone());
                Ok(account)
            }
            AccountBackend::Unavailable(message) => {
                Err(RepositoryError::Unavailable(message.clone()))
            }
        }
    }

    pub async fn find_by_email(
        &self,
        normalized_email: &str,
    ) -> Result<Option<AccountRow>, RepositoryError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database
                .account_by_normalized_email(normalized_email)
                .await
                .map_err(RepositoryError::Storage),
            AccountBackend::Memory(memory) => memory
                .lock()
                .map_err(|_| RepositoryError::Storage("account store poisoned".to_string()))
                .map(|state| state.by_email.get(normalized_email).cloned()),
            AccountBackend::Unavailable(message) => {
                Err(RepositoryError::Unavailable(message.clone()))
            }
        }
    }

    pub async fn find_by_id(
        &self,
        account_id: &str,
    ) -> Result<Option<AccountRow>, RepositoryError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database
                .account_by_id(account_id)
                .await
                .map_err(RepositoryError::Storage),
            AccountBackend::Memory(memory) => memory
                .lock()
                .map_err(|_| RepositoryError::Storage("account store poisoned".to_string()))
                .map(|state| {
                    state
                        .by_email
                        .values()
                        .find(|account| account.id == account_id)
                        .cloned()
                }),
            AccountBackend::Unavailable(message) => {
                Err(RepositoryError::Unavailable(message.clone()))
            }
        }
    }

    pub async fn record_login(&self, account_id: &str) -> Result<(), RepositoryError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database
                .update_account_last_login(account_id)
                .await
                .map_err(RepositoryError::Storage),
            AccountBackend::Memory(_) => Ok(()),
            AccountBackend::Unavailable(message) => {
                Err(RepositoryError::Unavailable(message.clone()))
            }
        }
    }

    pub async fn create_auth_session(
        &self,
        input: AuthSessionCreateInput,
    ) -> Result<AuthSessionIssue, AuthSessionError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database.create_auth_session(input).await,
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                memory_create_session(&mut state, input)
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn rotate_auth_refresh_token(
        &self,
        input: AuthRefreshRotateInput,
    ) -> Result<AuthSessionIssue, AuthSessionError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database.rotate_auth_refresh_token(input).await,
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                memory_rotate_refresh(&mut state, input)
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn validate_auth_session(
        &self,
        account_id: &str,
        session_id: &str,
        auth_epoch: i64,
        universe_id: Option<i64>,
    ) -> Result<bool, AuthSessionError> {
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database
                    .validate_auth_session(account_id, session_id, auth_epoch, universe_id)
                    .await
            }
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                let now = unix_timestamp();
                let account_enabled = state
                    .by_email
                    .values()
                    .find(|account| account.id == account_id)
                    .is_some_and(|account| {
                        !account.is_banned && account.universe_id == universe_id
                    });
                let valid = state.sessions.get_mut(session_id).is_some_and(|session| {
                    let valid = account_enabled
                        && session.user_id == account_id
                        && session.auth_epoch == auth_epoch
                        && session.revoked_at_unix.is_none()
                        && session.expires_at_unix > now;
                    if valid && session.last_used_at_unix <= now.saturating_sub(60) {
                        session.last_used_at_unix = now;
                    }
                    valid
                });
                Ok(valid)
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn list_auth_sessions(
        &self,
        account_id: &str,
    ) -> Result<Vec<AuthSessionView>, AuthSessionError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database.list_auth_sessions(account_id).await,
            AccountBackend::Memory(memory) => {
                let state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                let mut sessions: Vec<_> = state
                    .sessions
                    .values()
                    .filter(|session| session.user_id == account_id)
                    .map(|session| AuthSessionView {
                        session_id: session.session_id.clone(),
                        device_label: session.device_label.clone(),
                        created_at_unix: session.created_at_unix,
                        last_used_at_unix: session.last_used_at_unix,
                        expires_at_unix: session.expires_at_unix,
                        revoked_at_unix: session.revoked_at_unix,
                    })
                    .collect();
                sessions.sort_by(|left, right| {
                    right
                        .created_at_unix
                        .cmp(&left.created_at_unix)
                        .then_with(|| right.session_id.cmp(&left.session_id))
                });
                sessions.truncate(100);
                Ok(sessions)
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn revoke_auth_session(
        &self,
        account_id: &str,
        session_id: &str,
    ) -> Result<bool, AuthSessionError> {
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database.revoke_auth_session(account_id, session_id).await
            }
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                Ok(memory_revoke_session(
                    &mut state,
                    account_id,
                    session_id,
                    unix_timestamp(),
                ))
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn revoke_all_auth_sessions(
        &self,
        account_id: &str,
    ) -> Result<u64, AuthSessionError> {
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database.revoke_all_auth_sessions(account_id).await
            }
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                let session_ids: Vec<_> = state
                    .sessions
                    .values()
                    .filter(|session| {
                        session.user_id == account_id && session.revoked_at_unix.is_none()
                    })
                    .map(|session| session.session_id.clone())
                    .collect();
                let now = unix_timestamp();
                for session_id in &session_ids {
                    memory_revoke_session(&mut state, account_id, session_id, now);
                }
                Ok(session_ids.len() as u64)
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn auth_login_throttle_status(
        &self,
        account_digest: &[u8],
        ip_digest: Option<&[u8]>,
    ) -> Result<AuthThrottleStatus, AuthSessionError> {
        if account_digest.len() != 32 || ip_digest.is_some_and(|digest| digest.len() != 32) {
            return Err(AuthSessionError::InvalidInput);
        }
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database
                    .auth_login_throttle_status(account_digest, ip_digest)
                    .await
            }
            AccountBackend::Memory(memory) => {
                let state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                Ok(memory_throttle_status(&state, account_digest, ip_digest))
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn record_auth_login_failure(
        &self,
        account_digest: &[u8],
        ip_digest: Option<&[u8]>,
        policy: AuthThrottlePolicy,
    ) -> Result<AuthThrottleStatus, AuthSessionError> {
        if account_digest.len() != 32
            || ip_digest.is_some_and(|digest| digest.len() != 32)
            || !(60..=86_400).contains(&policy.window_seconds)
            || !(1..=1_000).contains(&policy.account_failure_limit)
            || !(1..=1_000).contains(&policy.ip_failure_limit)
            || !(60..=86_400).contains(&policy.block_seconds)
        {
            return Err(AuthSessionError::InvalidInput);
        }
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database
                    .record_auth_login_failure(account_digest, ip_digest, policy)
                    .await
            }
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                memory_record_failure(&mut state, account_digest, ip_digest, policy);
                Ok(memory_throttle_status(&state, account_digest, ip_digest))
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn admit_auth_registration(
        &self,
        ip_digest: &[u8],
        policy: AuthRegistrationThrottlePolicy,
    ) -> Result<AuthThrottleStatus, AuthSessionError> {
        if ip_digest.len() != 32
            || !(60..=86_400).contains(&policy.window_seconds)
            || !(1..=1_000).contains(&policy.attempt_limit)
            || !(60..=86_400).contains(&policy.block_seconds)
        {
            return Err(AuthSessionError::InvalidInput);
        }
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database.admit_auth_registration(ip_digest, policy).await
            }
            AccountBackend::Memory(memory) => {
                let mut state = memory.lock().map_err(|_| {
                    AuthSessionError::Database("account store poisoned".to_string())
                })?;
                let existing = memory_scope_throttle_status(&state, "registration_ip", ip_digest);
                if existing.blocked {
                    return Ok(existing);
                }
                memory_increment_throttle(
                    &mut state,
                    "registration_ip",
                    ip_digest,
                    policy.attempt_limit,
                    AuthThrottlePolicy {
                        window_seconds: policy.window_seconds,
                        account_failure_limit: policy.attempt_limit,
                        ip_failure_limit: policy.attempt_limit,
                        block_seconds: policy.block_seconds,
                    },
                );
                Ok(AuthThrottleStatus::default())
            }
            AccountBackend::Unavailable(message) => {
                Err(AuthSessionError::Database(message.clone()))
            }
        }
    }

    pub async fn ready(&self) -> Result<(), RepositoryError> {
        match &self.backend {
            AccountBackend::Postgres(database) => {
                database
                    .auth_repository_ready()
                    .await
                    .map_err(RepositoryError::Storage)?;
                database
                    .gameplay_repository_ready()
                    .await
                    .map_err(RepositoryError::Storage)
            }
            AccountBackend::Memory(_) => Ok(()),
            AccountBackend::Unavailable(message) => {
                Err(RepositoryError::Unavailable(message.clone()))
            }
        }
    }

    pub fn is_durable(&self) -> bool {
        matches!(&self.backend, AccountBackend::Postgres(_))
    }

    /// Legacy session-less human tokens remain accepted only by the explicit
    /// in-memory development/test repository.
    pub fn allows_legacy_sessionless_tokens(&self) -> bool {
        matches!(&self.backend, AccountBackend::Memory(_))
    }
}

fn memory_create_session(
    state: &mut MemoryAccounts,
    input: AuthSessionCreateInput,
) -> Result<AuthSessionIssue, AuthSessionError> {
    if input.refresh_token_digest.len() != 32
        || input.account_throttle_digest.len() != 32
        || !(32..=128).contains(&input.session_id.len())
        || input.session_id.trim() != input.session_id
        || !(32..=128).contains(&input.family_id.len())
        || input.family_id.trim() != input.family_id
        || !(300..=7_776_000).contains(&input.refresh_expiry_seconds)
        || !(1..=100).contains(&input.max_active_sessions)
        || input.device_label.as_ref().is_some_and(|label| {
            label.trim() != label || !(1..=128).contains(&label.chars().count())
        })
        || input
            .ip_digest
            .as_ref()
            .is_some_and(|digest| digest.len() != 32)
        || input
            .user_agent_digest
            .as_ref()
            .is_some_and(|digest| digest.len() != 32)
        || state.sessions.contains_key(&input.session_id)
        || state
            .refresh_tokens
            .contains_key(&input.refresh_token_digest)
    {
        return Err(AuthSessionError::InvalidInput);
    }
    let account = state
        .by_email
        .values()
        .find(|account| account.id == input.account_id)
        .cloned()
        .ok_or(AuthSessionError::AccountDisabled)?;
    if account.is_banned || account.universe_id.is_none() {
        return Err(AuthSessionError::AccountDisabled);
    }

    let now = unix_timestamp();
    let mut active: Vec<_> = state
        .sessions
        .values()
        .filter(|session| {
            session.user_id == input.account_id
                && session.revoked_at_unix.is_none()
                && session.expires_at_unix > now
        })
        .map(|session| {
            (
                session.session_id.clone(),
                session.last_used_at_unix,
                session.created_at_unix,
            )
        })
        .collect();
    active.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| right.0.cmp(&left.0))
    });
    for (session_id, _, _) in active
        .into_iter()
        .skip(input.max_active_sessions.saturating_sub(1))
    {
        memory_revoke_session(state, &input.account_id, &session_id, now);
    }

    let expires_at_unix = now.saturating_add(input.refresh_expiry_seconds);
    state.sessions.insert(
        input.session_id.clone(),
        MemorySession {
            session_id: input.session_id.clone(),
            family_id: input.family_id.clone(),
            user_id: input.account_id.clone(),
            device_label: input.device_label,
            created_at_unix: now,
            last_used_at_unix: now,
            expires_at_unix,
            revoked_at_unix: None,
            auth_epoch: 0,
            generation: 0,
        },
    );
    state.refresh_tokens.insert(
        input.refresh_token_digest,
        MemoryRefreshToken {
            session_id: input.session_id.clone(),
            consumed: false,
            revoked: false,
        },
    );
    state
        .throttles
        .remove(&("account".to_string(), input.account_throttle_digest));

    Ok(AuthSessionIssue {
        principal: memory_principal(&account),
        session_id: input.session_id,
        family_id: input.family_id,
        expires_at_unix,
    })
}

fn memory_rotate_refresh(
    state: &mut MemoryAccounts,
    input: AuthRefreshRotateInput,
) -> Result<AuthSessionIssue, AuthSessionError> {
    if input.consumed_token_digest.len() != 32
        || input.replacement_token_digest.len() != 32
        || input
            .ip_digest
            .as_ref()
            .is_some_and(|digest| digest.len() != 32)
        || input
            .user_agent_digest
            .as_ref()
            .is_some_and(|digest| digest.len() != 32)
        || input.consumed_token_digest == input.replacement_token_digest
        || state
            .refresh_tokens
            .contains_key(&input.replacement_token_digest)
    {
        return Err(AuthSessionError::InvalidInput);
    }
    let token = state
        .refresh_tokens
        .get(&input.consumed_token_digest)
        .cloned()
        .ok_or(AuthSessionError::InvalidToken)?;
    let session = state
        .sessions
        .get(&token.session_id)
        .cloned()
        .ok_or(AuthSessionError::InvalidToken)?;
    if token.consumed {
        memory_revoke_family(state, &session.family_id, unix_timestamp());
        return Err(AuthSessionError::ReplayDetected);
    }
    let account = state
        .by_email
        .values()
        .find(|account| account.id == session.user_id)
        .cloned()
        .ok_or(AuthSessionError::InvalidToken)?;
    let now = unix_timestamp();
    if token.revoked
        || session.revoked_at_unix.is_some()
        || session.expires_at_unix <= now
        || account.is_banned
    {
        memory_revoke_family(state, &session.family_id, now);
        return Err(AuthSessionError::InvalidToken);
    }

    if let Some(old) = state.refresh_tokens.get_mut(&input.consumed_token_digest) {
        old.consumed = true;
    }
    state.refresh_tokens.insert(
        input.replacement_token_digest,
        MemoryRefreshToken {
            session_id: session.session_id.clone(),
            consumed: false,
            revoked: false,
        },
    );
    if let Some(current) = state.sessions.get_mut(&session.session_id) {
        current.generation = current.generation.saturating_add(1);
        current.last_used_at_unix = now;
    }

    Ok(AuthSessionIssue {
        principal: memory_principal(&account),
        session_id: session.session_id,
        family_id: session.family_id,
        expires_at_unix: session.expires_at_unix,
    })
}

fn memory_revoke_session(
    state: &mut MemoryAccounts,
    account_id: &str,
    session_id: &str,
    now: i64,
) -> bool {
    let matches_actor = state
        .sessions
        .get(session_id)
        .is_some_and(|session| session.user_id == account_id);
    if !matches_actor {
        return false;
    }
    let newly_revoked = state.sessions.get_mut(session_id).is_some_and(|session| {
        if session.revoked_at_unix.is_none() {
            session.revoked_at_unix = Some(now);
            true
        } else {
            false
        }
    });
    for token in state
        .refresh_tokens
        .values_mut()
        .filter(|token| token.session_id == session_id)
    {
        token.revoked = true;
    }
    newly_revoked
}

fn memory_revoke_family(state: &mut MemoryAccounts, family_id: &str, now: i64) {
    let sessions: Vec<(String, String)> = state
        .sessions
        .values()
        .filter(|session| session.family_id == family_id)
        .map(|session| (session.user_id.clone(), session.session_id.clone()))
        .collect();
    for (account_id, session_id) in sessions {
        memory_revoke_session(state, &account_id, &session_id, now);
    }
}

fn memory_principal(account: &AccountRow) -> AuthPrincipal {
    AuthPrincipal {
        user_id: account.id.clone(),
        username: account.username.clone(),
        email: account.email.clone(),
        role: account.role.clone(),
        universe_id: account.universe_id.unwrap_or(1),
        auth_epoch: 0,
    }
}

fn memory_throttle_status(
    state: &MemoryAccounts,
    account_digest: &[u8],
    ip_digest: Option<&[u8]>,
) -> AuthThrottleStatus {
    let now = unix_timestamp();
    let account_key = ("account".to_string(), account_digest.to_vec());
    let account_retry = state
        .throttles
        .get(&account_key)
        .and_then(|throttle| throttle.blocked_until_unix)
        .map(|until| until.saturating_sub(now))
        .unwrap_or(0);
    let ip_retry = ip_digest
        .and_then(|digest| state.throttles.get(&("ip".to_string(), digest.to_vec())))
        .and_then(|throttle| throttle.blocked_until_unix)
        .map(|until| until.saturating_sub(now))
        .unwrap_or(0);
    let retry_after_seconds = account_retry.max(ip_retry).max(0);
    AuthThrottleStatus {
        blocked: retry_after_seconds > 0,
        retry_after_seconds,
    }
}

fn memory_scope_throttle_status(
    state: &MemoryAccounts,
    scope: &str,
    digest: &[u8],
) -> AuthThrottleStatus {
    let now = unix_timestamp();
    let retry_after_seconds = state
        .throttles
        .get(&(scope.to_string(), digest.to_vec()))
        .and_then(|throttle| throttle.blocked_until_unix)
        .map(|until| until.saturating_sub(now))
        .unwrap_or(0)
        .max(0);
    AuthThrottleStatus {
        blocked: retry_after_seconds > 0,
        retry_after_seconds,
    }
}

fn memory_record_failure(
    state: &mut MemoryAccounts,
    account_digest: &[u8],
    ip_digest: Option<&[u8]>,
    policy: AuthThrottlePolicy,
) {
    memory_increment_throttle(
        state,
        "account",
        account_digest,
        policy.account_failure_limit,
        policy,
    );
    if let Some(ip_digest) = ip_digest {
        memory_increment_throttle(state, "ip", ip_digest, policy.ip_failure_limit, policy);
    }
}

fn memory_increment_throttle(
    state: &mut MemoryAccounts,
    scope: &str,
    digest: &[u8],
    limit: i32,
    policy: AuthThrottlePolicy,
) {
    let now = unix_timestamp();
    let throttle = state
        .throttles
        .entry((scope.to_string(), digest.to_vec()))
        .or_insert(MemoryThrottle {
            window_started_at_unix: now,
            failure_count: 0,
            blocked_until_unix: None,
        });
    if throttle.window_started_at_unix <= now.saturating_sub(policy.window_seconds) {
        throttle.window_started_at_unix = now;
        throttle.failure_count = 0;
        throttle.blocked_until_unix = None;
    }
    throttle.failure_count = throttle.failure_count.saturating_add(1).min(limit);
    if throttle.failure_count >= limit && throttle.blocked_until_unix.unwrap_or(0) <= now {
        throttle.blocked_until_unix = Some(now.saturating_add(policy.block_seconds));
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub async fn validate_runtime_configuration() -> Result<(), String> {
    let auth = platform_auth::AuthConfig::from_env();
    auth.validate_runtime().map_err(|error| error.to_string())?;
    let environment = runtime_environment();
    validate_gateway_issuer_role(&auth, &environment)?;
    SessionSecurityConfig::from_environment()?;
    match Database::try_from_env()? {
        Some(database) => {
            database.gameplay_repository_ready().await?;
            database.auth_repository_ready().await?;
        }
        None if development_environment(&environment) => {}
        None => {
            return Err(format!(
                "DATABASE_URL is required for account persistence in {environment}"
            ))
        }
    }
    Ok(())
}

fn validate_gateway_issuer_role(
    auth: &platform_auth::AuthConfig,
    environment: &str,
) -> Result<(), String> {
    if production_like_environment(environment) && !auth.token_issuer {
        Err("AUTH_TOKEN_ISSUER=true is required for the production API gateway".to_string())
    } else {
        Ok(())
    }
}

fn runtime_environment() -> String {
    ["UNIVERSUS_ENV", "APP_ENV", "ENVIRONMENT", "RUST_ENV"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok())
        .unwrap_or_else(|| "development".to_string())
}

pub(crate) fn production_like_environment(environment: &str) -> bool {
    matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "production" | "prod" | "staging" | "stage"
    )
}

fn development_environment(environment: &str) -> bool {
    matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "development" | "dev" | "test" | "testing" | "local"
    )
}

fn bounded_env_i64(name: &str, default: i64, minimum: i64, maximum: i64) -> Result<i64, String> {
    let value = match std::env::var(name) {
        Ok(raw) => raw
            .parse::<i64>()
            .map_err(|_| format!("{name} must be an integer"))?,
        Err(_) => default,
    };
    if (minimum..=maximum).contains(&value) {
        Ok(value)
    } else {
        Err(format!("{name} must be between {minimum} and {maximum}"))
    }
}

fn bounded_env_i32(name: &str, default: i32, minimum: i32, maximum: i32) -> Result<i32, String> {
    let value = match std::env::var(name) {
        Ok(raw) => raw
            .parse::<i32>()
            .map_err(|_| format!("{name} must be an integer"))?,
        Err(_) => default,
    };
    if (minimum..=maximum).contains(&value) {
        Ok(value)
    } else {
        Err(format!("{name} must be between {minimum} and {maximum}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(username: &str, email: &str) -> AccountCreateInput {
        AccountCreateInput {
            username: username.to_string(),
            email: email.to_string(),
            password_hash: "hash".to_string(),
        }
    }

    #[tokio::test]
    async fn memory_repository_enforces_case_insensitive_uniqueness() {
        let repository = AccountRepository::in_memory();
        repository
            .create(input("Commander", "Commander@Example.com"))
            .await
            .unwrap();

        assert_eq!(
            repository
                .create(input("Other", " commander@example.COM "))
                .await,
            Err(RepositoryError::Duplicate)
        );
        assert_eq!(
            repository
                .create(input("COMMANDER", "other@example.com"))
                .await,
            Err(RepositoryError::Duplicate)
        );
    }

    #[tokio::test]
    async fn memory_repository_round_trips_identity() {
        let repository = AccountRepository::in_memory();
        let created = repository
            .create(input("Explorer", "EXPLORER@example.com"))
            .await
            .unwrap();

        assert_eq!(created.id, "dev-1");
        assert_eq!(created.email, "explorer@example.com");
        assert_eq!(
            repository.find_by_id(&created.id).await.unwrap(),
            Some(created.clone())
        );
        assert_eq!(
            repository
                .find_by_email("explorer@example.com")
                .await
                .unwrap(),
            Some(created)
        );
    }

    #[tokio::test]
    async fn successful_session_resets_account_throttle_but_not_shared_ip_bucket() {
        let repository = AccountRepository::in_memory();
        let account = repository
            .create(input("ThrottleUser", "throttle@example.test"))
            .await
            .unwrap();
        let account_digest = vec![1; 32];
        let ip_digest = vec![2; 32];
        let other_ip_digest = vec![3; 32];
        let policy = AuthThrottlePolicy {
            window_seconds: 900,
            account_failure_limit: 1,
            ip_failure_limit: 1,
            block_seconds: 900,
        };
        repository
            .record_auth_login_failure(&account_digest, Some(&ip_digest), policy)
            .await
            .unwrap();
        repository
            .create_auth_session(AuthSessionCreateInput {
                account_id: account.id,
                session_id: "s".repeat(32),
                family_id: "f".repeat(32),
                refresh_token_digest: vec![4; 32],
                refresh_expiry_seconds: 604_800,
                max_active_sessions: 5,
                device_label: None,
                ip_digest: Some(ip_digest.clone()),
                user_agent_digest: None,
                account_throttle_digest: account_digest.clone(),
            })
            .await
            .unwrap();

        assert!(
            repository
                .auth_login_throttle_status(&account_digest, Some(&ip_digest))
                .await
                .unwrap()
                .blocked
        );
        assert!(
            !repository
                .auth_login_throttle_status(&account_digest, Some(&other_ip_digest))
                .await
                .unwrap()
                .blocked
        );
    }

    #[tokio::test]
    async fn registration_and_login_ip_throttles_are_independent() {
        let repository = AccountRepository::in_memory();
        let account_digest = vec![10; 32];
        let registration_ip = vec![11; 32];
        let login_ip = vec![12; 32];
        let registration_policy = AuthRegistrationThrottlePolicy {
            window_seconds: 3_600,
            attempt_limit: 1,
            block_seconds: 3_600,
        };
        assert!(
            !repository
                .admit_auth_registration(&registration_ip, registration_policy)
                .await
                .unwrap()
                .blocked
        );
        assert!(
            repository
                .admit_auth_registration(&registration_ip, registration_policy)
                .await
                .unwrap()
                .blocked
        );
        assert!(
            !repository
                .auth_login_throttle_status(&account_digest, Some(&registration_ip))
                .await
                .unwrap()
                .blocked
        );

        repository
            .record_auth_login_failure(
                &account_digest,
                Some(&login_ip),
                AuthThrottlePolicy {
                    window_seconds: 900,
                    account_failure_limit: 1,
                    ip_failure_limit: 1,
                    block_seconds: 900,
                },
            )
            .await
            .unwrap();
        assert!(
            !repository
                .admit_auth_registration(&login_ip, registration_policy)
                .await
                .unwrap()
                .blocked
        );
    }

    #[tokio::test]
    async fn session_cap_revokes_the_oldest_live_session() {
        let repository = AccountRepository::in_memory();
        let account = repository
            .create(input("SessionCapUser", "session-cap@example.test"))
            .await
            .unwrap();
        let mut session_ids = Vec::new();
        for suffix in 1_u8..=3 {
            let session_id = format!("{suffix:032}");
            repository
                .create_auth_session(AuthSessionCreateInput {
                    account_id: account.id.clone(),
                    session_id: session_id.clone(),
                    family_id: format!("family-{suffix:025}"),
                    refresh_token_digest: vec![suffix; 32],
                    refresh_expiry_seconds: 604_800,
                    max_active_sessions: 2,
                    device_label: None,
                    ip_digest: None,
                    user_agent_digest: None,
                    account_throttle_digest: vec![99; 32],
                })
                .await
                .unwrap();
            session_ids.push(session_id);
        }

        assert!(!repository
            .validate_auth_session(&account.id, &session_ids[0], 0, Some(1))
            .await
            .unwrap());
        assert!(repository
            .validate_auth_session(&account.id, &session_ids[1], 0, Some(1))
            .await
            .unwrap());
        assert!(repository
            .validate_auth_session(&account.id, &session_ids[2], 0, Some(1))
            .await
            .unwrap());
    }

    #[test]
    fn session_security_debug_redacts_digest_key() {
        let config = SessionSecurityConfig {
            refresh_expiry_seconds: 604_800,
            max_sessions_per_user: 5,
            throttle_policy: AuthThrottlePolicy {
                window_seconds: 900,
                account_failure_limit: 5,
                ip_failure_limit: 20,
                block_seconds: 900,
            },
            registration_throttle_policy: AuthRegistrationThrottlePolicy {
                window_seconds: 3_600,
                attempt_limit: 5,
                block_seconds: 3_600,
            },
            digest_key: b"never-print-this-session-secret".to_vec(),
        };
        let debug = format!("{config:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("never-print-this-session-secret"));
    }

    #[test]
    fn production_environment_names_are_explicit() {
        assert!(production_like_environment("production"));
        assert!(production_like_environment("STAGING"));
        assert!(!production_like_environment("development"));
        assert!(!production_like_environment("test"));
        for environment in ["development", "dev", "test", "testing", "local"] {
            assert!(development_environment(environment));
        }
        assert!(!development_environment("qa"));

        let auth = platform_auth::AuthConfig::default();
        assert!(validate_gateway_issuer_role(&auth, "production").is_err());
        assert!(validate_gateway_issuer_role(&auth, "development").is_ok());
    }
}
