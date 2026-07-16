//! Account repository boundary for authentication routes.
//!
//! PostgreSQL is the durable backend whenever `DATABASE_URL` is configured.
//! The in-memory backend is intentionally limited to non-production local
//! development and deterministic tests.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use platform_db::{AccountCreateError, AccountCreateInput, AccountRow, Database};

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    Duplicate,
    Unavailable(String),
    Storage(String),
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

    pub async fn ready(&self) -> Result<(), RepositoryError> {
        match &self.backend {
            AccountBackend::Postgres(database) => database
                .gameplay_repository_ready()
                .await
                .map_err(RepositoryError::Storage),
            AccountBackend::Memory(_) => Ok(()),
            AccountBackend::Unavailable(message) => {
                Err(RepositoryError::Unavailable(message.clone()))
            }
        }
    }

    pub fn is_durable(&self) -> bool {
        matches!(&self.backend, AccountBackend::Postgres(_))
    }
}

pub async fn validate_runtime_configuration() -> Result<(), String> {
    platform_auth::AuthConfig::from_env()
        .validate_runtime()
        .map_err(|error| error.to_string())?;
    let environment = runtime_environment();
    match Database::try_from_env()? {
        Some(database) => database.gameplay_repository_ready().await?,
        None if development_environment(&environment) => {}
        None => {
            return Err(format!(
                "DATABASE_URL is required for account persistence in {environment}"
            ))
        }
    }
    Ok(())
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
    }
}
