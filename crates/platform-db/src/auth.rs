use deadpool_postgres::{GenericClient, Transaction};
use tokio_postgres::Row;

use crate::{Database, DbResult};

const SESSION_TOUCH_INTERVAL_SECONDS: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPrincipal {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub universe_id: i64,
    pub auth_epoch: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionIssue {
    pub principal: AuthPrincipal,
    pub session_id: String,
    pub family_id: String,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionCreateInput {
    pub account_id: String,
    pub session_id: String,
    pub family_id: String,
    pub refresh_token_digest: Vec<u8>,
    pub refresh_expiry_seconds: i64,
    pub max_active_sessions: usize,
    pub device_label: Option<String>,
    pub ip_digest: Option<Vec<u8>>,
    pub user_agent_digest: Option<Vec<u8>>,
    pub account_throttle_digest: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRefreshRotateInput {
    pub consumed_token_digest: Vec<u8>,
    pub replacement_token_digest: Vec<u8>,
    pub ip_digest: Option<Vec<u8>>,
    pub user_agent_digest: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionView {
    pub session_id: String,
    pub device_label: Option<String>,
    pub created_at_unix: i64,
    pub last_used_at_unix: i64,
    pub expires_at_unix: i64,
    pub revoked_at_unix: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthThrottlePolicy {
    pub window_seconds: i64,
    pub account_failure_limit: i32,
    pub ip_failure_limit: i32,
    pub block_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthRegistrationThrottlePolicy {
    pub window_seconds: i64,
    pub attempt_limit: i32,
    pub block_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuthThrottleStatus {
    pub blocked: bool,
    pub retry_after_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSessionError {
    InvalidToken,
    ReplayDetected,
    AccountDisabled,
    InvalidInput,
    Database(String),
}

impl Database {
    /// Verifies the complete durable authentication schema used at runtime.
    pub async fn auth_repository_ready(&self) -> DbResult<()> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT to_regclass('public.users') IS NOT NULL
                    AND to_regclass('public.auth_sessions') IS NOT NULL
                    AND to_regclass('public.auth_refresh_tokens') IS NOT NULL
                    AND to_regclass('public.auth_login_throttles') IS NOT NULL
                    AND to_regclass('public.idx_auth_sessions_family_id') IS NOT NULL
                    AND EXISTS (
                        SELECT 1 FROM pg_constraint
                        WHERE conrelid = to_regclass('public.auth_login_throttles')
                          AND contype = 'c'
                          AND pg_get_constraintdef(oid) LIKE '%registration_ip%'
                    )
                    AND (SELECT COUNT(*) = 5
                         FROM information_schema.columns
                         WHERE table_schema = 'public'
                           AND table_name = 'users'
                           AND column_name IN ('auth_epoch', 'is_banned',
                                               'privacy_restriction_active',
                                               'privacy_erasure_pending', 'password_hash'))
                    AND (SELECT COUNT(*) = 6
                         FROM information_schema.columns
                         WHERE table_schema = 'public'
                           AND table_name = 'auth_sessions'
                           AND column_name IN ('session_id', 'family_id', 'user_id',
                                               'auth_epoch_at_issue', 'expires_at', 'revoked_at'))
                    AS ready",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        if row.get::<_, bool>("ready") {
            Ok(())
        } else {
            Err(
                "durable authentication schema is missing; run ordered database migrations"
                    .to_string(),
            )
        }
    }

    /// Creates a durable session and its first one-time refresh token.
    /// The oldest live sessions beyond the configured cap are revoked.
    pub async fn create_auth_session(
        &self,
        input: AuthSessionCreateInput,
    ) -> Result<AuthSessionIssue, AuthSessionError> {
        validate_create_input(&input)?;
        let account_id = parse_account_id(&input.account_id)?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(database_error)?;

        let account = locked_principal(&transaction, account_id).await?;
        if principal_disabled(&transaction, account_id).await? {
            return Err(AuthSessionError::AccountDisabled);
        }

        revoke_expired_sessions(&transaction, account_id).await?;
        cap_active_sessions(
            &transaction,
            account_id,
            input.max_active_sessions.saturating_sub(1),
        )
        .await?;

        let row = transaction
            .query_one(
                "INSERT INTO auth_sessions (
                    session_id, family_id, user_id, universe_id, auth_epoch_at_issue,
                    device_label, ip_digest, user_agent_digest, expires_at
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                           now() + ($9::BIGINT * interval '1 second'))
                 RETURNING EXTRACT(EPOCH FROM expires_at)::BIGINT AS expires_at_unix",
                &[
                    &input.session_id,
                    &input.family_id,
                    &account_id,
                    &account.universe_id,
                    &account.auth_epoch,
                    &input.device_label,
                    &input.ip_digest,
                    &input.user_agent_digest,
                    &input.refresh_expiry_seconds,
                ],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "INSERT INTO auth_refresh_tokens (
                    token_digest, session_id, generation, expires_at
                 ) SELECT $1, $2, 0, expires_at
                   FROM auth_sessions WHERE session_id = $2",
                &[&input.refresh_token_digest, &input.session_id],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "UPDATE users SET last_login = now() WHERE id = $1",
                &[&account_id],
            )
            .await
            .map_err(database_error)?;
        reset_throttles(&transaction, &input.account_throttle_digest).await?;
        transaction.commit().await.map_err(database_error)?;

        Ok(AuthSessionIssue {
            principal: account,
            session_id: input.session_id,
            family_id: input.family_id,
            expires_at_unix: row.get("expires_at_unix"),
        })
    }

    /// Atomically consumes one refresh token and installs its replacement.
    /// A consumed-token replay commits family-wide revocation before returning.
    pub async fn rotate_auth_refresh_token(
        &self,
        input: AuthRefreshRotateInput,
    ) -> Result<AuthSessionIssue, AuthSessionError> {
        validate_digest(&input.consumed_token_digest)?;
        validate_digest(&input.replacement_token_digest)?;
        validate_optional_digest(input.ip_digest.as_deref())?;
        validate_optional_digest(input.user_agent_digest.as_deref())?;
        if input.consumed_token_digest == input.replacement_token_digest {
            return Err(AuthSessionError::InvalidInput);
        }

        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let row = transaction
            .query_opt(
                "SELECT token.session_id, token.generation, token.consumed_at IS NOT NULL AS consumed,
                        token.revoked_at IS NOT NULL AS token_revoked,
                        token.expires_at <= now() AS token_expired,
                        session.family_id, session.user_id,
                        EXTRACT(EPOCH FROM session.expires_at)::BIGINT AS expires_at_unix,
                        session.revoked_at IS NOT NULL AS session_revoked,
                        session.expires_at <= now() AS session_expired,
                        session.auth_epoch_at_issue,
                        users.id::TEXT AS principal_id, users.username, users.email,
                        CASE WHEN users.is_admin THEN 'admin' ELSE 'player' END AS role,
                        users.universe_id, users.auth_epoch, users.is_banned,
                        users.privacy_restriction_active, users.privacy_erasure_pending
                 FROM auth_refresh_tokens AS token
                 JOIN auth_sessions AS session ON session.session_id = token.session_id
                 JOIN users ON users.id = session.user_id
                           AND users.universe_id = session.universe_id
                 WHERE token.token_digest = $1
                 FOR UPDATE OF token, session, users",
                &[&input.consumed_token_digest],
            )
            .await
            .map_err(database_error)?;
        let Some(row) = row else {
            transaction.commit().await.map_err(database_error)?;
            return Err(AuthSessionError::InvalidToken);
        };

        let family_id: String = row.get("family_id");
        if row.get::<_, bool>("consumed") {
            revoke_family(&transaction, &family_id, "refresh_token_replay").await?;
            transaction.commit().await.map_err(database_error)?;
            return Err(AuthSessionError::ReplayDetected);
        }
        let invalid = row.get::<_, bool>("token_revoked")
            || row.get::<_, bool>("token_expired")
            || row.get::<_, bool>("session_revoked")
            || row.get::<_, bool>("session_expired")
            || row.get::<_, bool>("is_banned")
            || row.get::<_, bool>("privacy_restriction_active")
            || row.get::<_, bool>("privacy_erasure_pending")
            || row.get::<_, i64>("auth_epoch_at_issue") != row.get::<_, i64>("auth_epoch");
        if invalid {
            revoke_family(&transaction, &family_id, "refresh_token_invalidated").await?;
            transaction.commit().await.map_err(database_error)?;
            return Err(AuthSessionError::InvalidToken);
        }

        let session_id: String = row.get("session_id");
        let generation = row.get::<_, i64>("generation");
        transaction
            .execute(
                "UPDATE auth_refresh_tokens
                 SET consumed_at = now()
                 WHERE token_digest = $1 AND consumed_at IS NULL",
                &[&input.consumed_token_digest],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "INSERT INTO auth_refresh_tokens (
                    token_digest, session_id, generation, parent_token_digest, expires_at
                 ) SELECT $1, $2, $3, $4, expires_at
                   FROM auth_sessions WHERE session_id = $2",
                &[
                    &input.replacement_token_digest,
                    &session_id,
                    &generation.saturating_add(1),
                    &input.consumed_token_digest,
                ],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "UPDATE auth_refresh_tokens SET replaced_by_digest = $2
                 WHERE token_digest = $1",
                &[
                    &input.consumed_token_digest,
                    &input.replacement_token_digest,
                ],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "UPDATE auth_sessions
                 SET rotation_counter = $2, last_used_at = now(),
                     ip_digest = COALESCE($3, ip_digest),
                     user_agent_digest = COALESCE($4, user_agent_digest)
                 WHERE session_id = $1",
                &[
                    &session_id,
                    &generation.saturating_add(1),
                    &input.ip_digest,
                    &input.user_agent_digest,
                ],
            )
            .await
            .map_err(database_error)?;

        let issue = AuthSessionIssue {
            principal: map_principal(&row),
            session_id,
            family_id,
            expires_at_unix: row.get("expires_at_unix"),
        };
        transaction.commit().await.map_err(database_error)?;
        Ok(issue)
    }

    /// Enforces account security flags, epoch equality, and a live session.
    pub async fn validate_auth_session(
        &self,
        account_id: &str,
        session_id: &str,
        claim_auth_epoch: i64,
        claim_universe_id: Option<i64>,
    ) -> Result<bool, AuthSessionError> {
        let account_id = parse_account_id(account_id)?;
        let Some(claim_universe_id) = claim_universe_id else {
            return Ok(false);
        };
        if session_id.trim().is_empty() || claim_auth_epoch < 0 || claim_universe_id <= 0 {
            return Ok(false);
        }
        let client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let updated = client
            .execute(
                "UPDATE auth_sessions AS session
                 SET last_used_at = CASE
                     WHEN session.last_used_at <= now() - ($5::BIGINT * interval '1 second')
                     THEN now() ELSE session.last_used_at END
                 FROM users
                 WHERE session.session_id = $1
                   AND session.user_id = $2
                   AND users.id = session.user_id
                   AND users.universe_id = session.universe_id
                   AND users.auth_epoch = $3
                   AND session.auth_epoch_at_issue = $3
                   AND session.universe_id = $4
                   AND NOT users.is_banned
                   AND NOT users.privacy_restriction_active
                   AND NOT users.privacy_erasure_pending
                   AND session.revoked_at IS NULL
                   AND session.expires_at > now()",
                &[
                    &session_id,
                    &account_id,
                    &claim_auth_epoch,
                    &claim_universe_id,
                    &SESSION_TOUCH_INTERVAL_SECONDS,
                ],
            )
            .await
            .map_err(database_error)?;
        Ok(updated == 1)
    }

    pub async fn list_auth_sessions(
        &self,
        account_id: &str,
    ) -> Result<Vec<AuthSessionView>, AuthSessionError> {
        let account_id = parse_account_id(account_id)?;
        let client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let rows = client
            .query(
                "SELECT session_id, device_label,
                        EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at_unix,
                        EXTRACT(EPOCH FROM last_used_at)::BIGINT AS last_used_at_unix,
                        EXTRACT(EPOCH FROM expires_at)::BIGINT AS expires_at_unix,
                        EXTRACT(EPOCH FROM revoked_at)::BIGINT AS revoked_at_unix
                 FROM auth_sessions
                 WHERE user_id = $1
                 ORDER BY created_at DESC, session_id DESC
                 LIMIT 100",
                &[&account_id],
            )
            .await
            .map_err(database_error)?;
        Ok(rows.iter().map(map_session_view).collect())
    }

    pub async fn revoke_auth_session(
        &self,
        account_id: &str,
        session_id: &str,
    ) -> Result<bool, AuthSessionError> {
        let account_id = parse_account_id(account_id)?;
        if session_id.trim().is_empty() {
            return Err(AuthSessionError::InvalidInput);
        }
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let updated = transaction
            .execute(
                "UPDATE auth_sessions
                 SET revoked_at = COALESCE(revoked_at, now()),
                     revoke_reason = COALESCE(revoke_reason, 'user_revoked')
                 WHERE session_id = $1 AND user_id = $2 AND revoked_at IS NULL",
                &[&session_id, &account_id],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "UPDATE auth_refresh_tokens
                 SET revoked_at = COALESCE(revoked_at, now())
                 WHERE session_id = $1 AND revoked_at IS NULL",
                &[&session_id],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(updated == 1)
    }

    pub async fn revoke_all_auth_sessions(
        &self,
        account_id: &str,
    ) -> Result<u64, AuthSessionError> {
        let account_id = parse_account_id(account_id)?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(database_error)?;
        let updated = transaction
            .execute(
                "UPDATE auth_sessions
                 SET revoked_at = COALESCE(revoked_at, now()),
                     revoke_reason = COALESCE(revoke_reason, 'user_revoked_all')
                 WHERE user_id = $1 AND revoked_at IS NULL",
                &[&account_id],
            )
            .await
            .map_err(database_error)?;
        transaction
            .execute(
                "UPDATE auth_refresh_tokens AS token
                 SET revoked_at = COALESCE(token.revoked_at, now())
                 FROM auth_sessions AS session
                 WHERE session.user_id = $1
                   AND token.session_id = session.session_id
                   AND token.revoked_at IS NULL",
                &[&account_id],
            )
            .await
            .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        Ok(updated)
    }

    pub async fn auth_login_throttle_status(
        &self,
        account_digest: &[u8],
        ip_digest: Option<&[u8]>,
    ) -> Result<AuthThrottleStatus, AuthSessionError> {
        validate_digest(account_digest)?;
        validate_optional_digest(ip_digest)?;
        let client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let rows = client
            .query(
                "SELECT GREATEST(1,
                        CEIL(EXTRACT(EPOCH FROM blocked_until - now())))::BIGINT AS retry_after
                 FROM auth_login_throttles
                 WHERE blocked_until > now()
                   AND ((scope = 'account' AND subject_digest = $1)
                     OR (scope = 'ip' AND subject_digest = $2))",
                &[&account_digest, &ip_digest],
            )
            .await
            .map_err(database_error)?;
        let retry_after_seconds = rows
            .iter()
            .map(|row| row.get::<_, i64>("retry_after"))
            .max()
            .unwrap_or(0);
        Ok(AuthThrottleStatus {
            blocked: retry_after_seconds > 0,
            retry_after_seconds,
        })
    }

    pub async fn record_auth_login_failure(
        &self,
        account_digest: &[u8],
        ip_digest: Option<&[u8]>,
        policy: AuthThrottlePolicy,
    ) -> Result<AuthThrottleStatus, AuthSessionError> {
        validate_digest(account_digest)?;
        validate_optional_digest(ip_digest)?;
        validate_throttle_policy(policy)?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(database_error)?;
        upsert_throttle(
            &transaction,
            "account",
            account_digest,
            policy.account_failure_limit,
            policy,
        )
        .await?;
        if let Some(ip_digest) = ip_digest {
            upsert_throttle(
                &transaction,
                "ip",
                ip_digest,
                policy.ip_failure_limit,
                policy,
            )
            .await?;
        }
        transaction.commit().await.map_err(database_error)?;
        self.auth_login_throttle_status(account_digest, ip_digest)
            .await
    }

    /// Atomically admits at most `attempt_limit` registration requests for a
    /// digest-only client IP in each bounded window. The attempt that reaches
    /// the limit is admitted; subsequent attempts are rejected.
    pub async fn admit_auth_registration(
        &self,
        ip_digest: &[u8],
        policy: AuthRegistrationThrottlePolicy,
    ) -> Result<AuthThrottleStatus, AuthSessionError> {
        validate_digest(ip_digest)?;
        validate_registration_throttle_policy(policy)?;
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AuthSessionError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(database_error)?;
        // This lock also serializes the initially-absent row case, where
        // SELECT FOR UPDATE alone cannot prevent parallel admissions.
        transaction
            .execute(
                "SELECT pg_advisory_xact_lock(
                    hashtextextended(encode($1::BYTEA, 'hex'), 0))",
                &[&ip_digest],
            )
            .await
            .map_err(database_error)?;
        let existing_block = transaction
            .query_opt(
                "SELECT GREATEST(1,
                        CEIL(EXTRACT(EPOCH FROM blocked_until - now())))::BIGINT AS retry_after
                 FROM auth_login_throttles
                 WHERE scope = 'registration_ip'
                   AND subject_digest = $1
                   AND blocked_until > now()
                 FOR UPDATE",
                &[&ip_digest],
            )
            .await
            .map_err(database_error)?;
        if let Some(row) = existing_block {
            let status = AuthThrottleStatus {
                blocked: true,
                retry_after_seconds: row.get("retry_after"),
            };
            transaction.commit().await.map_err(database_error)?;
            return Ok(status);
        }

        upsert_throttle(
            &transaction,
            "registration_ip",
            ip_digest,
            policy.attempt_limit,
            AuthThrottlePolicy {
                window_seconds: policy.window_seconds,
                account_failure_limit: policy.attempt_limit,
                ip_failure_limit: policy.attempt_limit,
                block_seconds: policy.block_seconds,
            },
        )
        .await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(AuthThrottleStatus::default())
    }
}

async fn locked_principal(
    transaction: &Transaction<'_>,
    account_id: i32,
) -> Result<AuthPrincipal, AuthSessionError> {
    transaction
        .query_opt(
            "SELECT id::TEXT AS principal_id, username, email,
                    CASE WHEN is_admin THEN 'admin' ELSE 'player' END AS role,
                    universe_id, auth_epoch
             FROM users WHERE id = $1 FOR UPDATE",
            &[&account_id],
        )
        .await
        .map_err(database_error)?
        .map(|row| map_principal(&row))
        .ok_or(AuthSessionError::AccountDisabled)
}

async fn principal_disabled(
    transaction: &Transaction<'_>,
    account_id: i32,
) -> Result<bool, AuthSessionError> {
    let row = transaction
        .query_one(
            "SELECT is_banned OR privacy_restriction_active OR privacy_erasure_pending
                    OR universe_id IS NULL AS disabled
             FROM users WHERE id = $1",
            &[&account_id],
        )
        .await
        .map_err(database_error)?;
    Ok(row.get("disabled"))
}

async fn revoke_expired_sessions(
    transaction: &Transaction<'_>,
    account_id: i32,
) -> Result<(), AuthSessionError> {
    transaction
        .execute(
            "UPDATE auth_refresh_tokens AS token
             SET revoked_at = COALESCE(token.revoked_at, now())
             FROM auth_sessions AS session
             WHERE session.user_id = $1
               AND session.expires_at <= now()
               AND token.session_id = session.session_id
               AND token.revoked_at IS NULL",
            &[&account_id],
        )
        .await
        .map_err(database_error)?;
    transaction
        .execute(
            "UPDATE auth_sessions
             SET revoked_at = COALESCE(revoked_at, now()),
                 revoke_reason = COALESCE(revoke_reason, 'expired')
             WHERE user_id = $1 AND expires_at <= now() AND revoked_at IS NULL",
            &[&account_id],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn cap_active_sessions(
    transaction: &Transaction<'_>,
    account_id: i32,
    sessions_to_keep: usize,
) -> Result<(), AuthSessionError> {
    let keep = i64::try_from(sessions_to_keep).unwrap_or(i64::MAX);
    let rows = transaction
        .query(
            "SELECT session_id FROM auth_sessions
             WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
             ORDER BY last_used_at DESC, created_at DESC, session_id DESC
             OFFSET $2",
            &[&account_id, &keep],
        )
        .await
        .map_err(database_error)?;
    let victims: Vec<String> = rows.iter().map(|row| row.get("session_id")).collect();
    if victims.is_empty() {
        return Ok(());
    }
    transaction
        .execute(
            "UPDATE auth_refresh_tokens SET revoked_at = COALESCE(revoked_at, now())
             WHERE session_id = ANY($1) AND revoked_at IS NULL",
            &[&victims],
        )
        .await
        .map_err(database_error)?;
    transaction
        .execute(
            "UPDATE auth_sessions
             SET revoked_at = COALESCE(revoked_at, now()),
                 revoke_reason = COALESCE(revoke_reason, 'session_limit')
             WHERE session_id = ANY($1) AND revoked_at IS NULL",
            &[&victims],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn revoke_family(
    transaction: &Transaction<'_>,
    family_id: &str,
    reason: &str,
) -> Result<(), AuthSessionError> {
    transaction
        .execute(
            "UPDATE auth_refresh_tokens AS token
             SET revoked_at = COALESCE(token.revoked_at, now())
             FROM auth_sessions AS session
             WHERE session.family_id = $1
               AND token.session_id = session.session_id
               AND token.revoked_at IS NULL",
            &[&family_id],
        )
        .await
        .map_err(database_error)?;
    transaction
        .execute(
            "UPDATE auth_sessions
             SET revoked_at = COALESCE(revoked_at, now()),
                 revoke_reason = COALESCE(revoke_reason, $2)
             WHERE family_id = $1 AND revoked_at IS NULL",
            &[&family_id, &reason],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn reset_throttles<C: GenericClient + Sync>(
    client: &C,
    account_digest: &[u8],
) -> Result<(), AuthSessionError> {
    client
        .execute(
            "DELETE FROM auth_login_throttles
             WHERE scope = 'account' AND subject_digest = $1",
            &[&account_digest],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn upsert_throttle(
    transaction: &Transaction<'_>,
    scope: &str,
    digest: &[u8],
    limit: i32,
    policy: AuthThrottlePolicy,
) -> Result<(), AuthSessionError> {
    transaction
        .execute(
            "INSERT INTO auth_login_throttles (
                scope, subject_digest, window_started_at, failure_count, blocked_until, updated_at
             ) VALUES (
                $1, $2, now(), 1,
                CASE WHEN $3::INTEGER <= 1
                     THEN now() + ($4::BIGINT * interval '1 second') END,
                now()
             )
             ON CONFLICT (scope, subject_digest) DO UPDATE SET
                window_started_at = CASE
                    WHEN auth_login_throttles.window_started_at
                         <= now() - ($5::BIGINT * interval '1 second')
                    THEN now() ELSE auth_login_throttles.window_started_at END,
                failure_count = CASE
                    WHEN auth_login_throttles.window_started_at
                         <= now() - ($5::BIGINT * interval '1 second')
                    THEN 1 ELSE LEAST($3, auth_login_throttles.failure_count + 1) END,
                blocked_until = CASE
                    WHEN auth_login_throttles.blocked_until > now()
                    THEN auth_login_throttles.blocked_until
                    WHEN (CASE
                        WHEN auth_login_throttles.window_started_at
                             <= now() - ($5::BIGINT * interval '1 second')
                        THEN 1 ELSE auth_login_throttles.failure_count + 1 END) >= $3
                    THEN now() + ($4::BIGINT * interval '1 second')
                    ELSE NULL END,
                updated_at = now()",
            &[
                &scope,
                &digest,
                &limit,
                &policy.block_seconds,
                &policy.window_seconds,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

fn map_principal(row: &Row) -> AuthPrincipal {
    AuthPrincipal {
        user_id: row.get("principal_id"),
        username: row.get("username"),
        email: row.get("email"),
        role: row.get("role"),
        universe_id: row.get("universe_id"),
        auth_epoch: row.get("auth_epoch"),
    }
}

fn map_session_view(row: &Row) -> AuthSessionView {
    AuthSessionView {
        session_id: row.get("session_id"),
        device_label: row.get("device_label"),
        created_at_unix: row.get("created_at_unix"),
        last_used_at_unix: row.get("last_used_at_unix"),
        expires_at_unix: row.get("expires_at_unix"),
        revoked_at_unix: row.get("revoked_at_unix"),
    }
}

fn parse_account_id(account_id: &str) -> Result<i32, AuthSessionError> {
    account_id
        .parse::<i32>()
        .map_err(|_| AuthSessionError::InvalidInput)
}

fn validate_create_input(input: &AuthSessionCreateInput) -> Result<(), AuthSessionError> {
    if !(32..=128).contains(&input.session_id.len())
        || input.session_id.trim() != input.session_id
        || !(32..=128).contains(&input.family_id.len())
        || input.family_id.trim() != input.family_id
        || !(60..=7_776_000).contains(&input.refresh_expiry_seconds)
        || !(1..=100).contains(&input.max_active_sessions)
        || input
            .device_label
            .as_ref()
            .is_some_and(|label| !(1..=128).contains(&label.len()) || label.trim() != label)
    {
        return Err(AuthSessionError::InvalidInput);
    }
    validate_digest(&input.refresh_token_digest)?;
    validate_digest(&input.account_throttle_digest)?;
    validate_optional_digest(input.ip_digest.as_deref())?;
    validate_optional_digest(input.user_agent_digest.as_deref())
}

fn validate_digest(digest: &[u8]) -> Result<(), AuthSessionError> {
    if digest.len() == 32 {
        Ok(())
    } else {
        Err(AuthSessionError::InvalidInput)
    }
}

fn validate_optional_digest(digest: Option<&[u8]>) -> Result<(), AuthSessionError> {
    digest.map_or(Ok(()), validate_digest)
}

fn validate_throttle_policy(policy: AuthThrottlePolicy) -> Result<(), AuthSessionError> {
    if (60..=86_400).contains(&policy.window_seconds)
        && (1..=1_000).contains(&policy.account_failure_limit)
        && (1..=1_000).contains(&policy.ip_failure_limit)
        && (60..=86_400).contains(&policy.block_seconds)
    {
        Ok(())
    } else {
        Err(AuthSessionError::InvalidInput)
    }
}

fn validate_registration_throttle_policy(
    policy: AuthRegistrationThrottlePolicy,
) -> Result<(), AuthSessionError> {
    if (60..=86_400).contains(&policy.window_seconds)
        && (1..=1_000).contains(&policy.attempt_limit)
        && (60..=86_400).contains(&policy.block_seconds)
    {
        Ok(())
    } else {
        Err(AuthSessionError::InvalidInput)
    }
}

fn database_error(error: tokio_postgres::Error) -> AuthSessionError {
    AuthSessionError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unbounded_session_and_throttle_inputs() {
        let mut input = AuthSessionCreateInput {
            account_id: "1".to_string(),
            session_id: "s".repeat(32),
            family_id: "f".repeat(32),
            refresh_token_digest: vec![1; 32],
            refresh_expiry_seconds: 604_800,
            max_active_sessions: 5,
            device_label: Some("Firefox on Linux".to_string()),
            ip_digest: Some(vec![2; 32]),
            user_agent_digest: Some(vec![3; 32]),
            account_throttle_digest: vec![4; 32],
        };
        assert_eq!(validate_create_input(&input), Ok(()));
        input.max_active_sessions = 0;
        assert_eq!(
            validate_create_input(&input),
            Err(AuthSessionError::InvalidInput)
        );
        assert_eq!(
            validate_throttle_policy(AuthThrottlePolicy {
                window_seconds: 10,
                account_failure_limit: 5,
                ip_failure_limit: 20,
                block_seconds: 900,
            }),
            Err(AuthSessionError::InvalidInput)
        );
    }
}
