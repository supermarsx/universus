//! Durable non-fleet gameplay persistence.
//!
//! This module intentionally contains no request-time DDL and no in-memory
//! fallback. Callers choose that policy at their application boundary. Every
//! mutating operation verifies ownership and performs affordability checks,
//! resource deduction, and queue insertion in one PostgreSQL transaction.

use std::collections::BTreeMap;

use tokio_postgres::{error::SqlState, Transaction};

use super::{AccountCreateError, AccountCreateInput, AccountRow, Database, DbResult};

pub const STARTING_METAL: i64 = 125_000;
pub const STARTING_CRYSTAL: i64 = 94_500;
pub const STARTING_DEUTERIUM: i64 = 40_250;
pub const STARTING_DARK_MATTER: i64 = 1_500;

const MAX_QUEUE_DURATION_SECONDS: i64 = 10 * 365 * 24 * 60 * 60;
const MAX_SHIP_QUEUE_QUANTITY: i64 = 1_000_000_000;
const MAX_PROCESS_BATCH: usize = 1_000;
const STALE_PROCESSING_SECONDS: i64 = 15 * 60;
pub const COORDINATE_ALLOCATION_MAX_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayResourcesRow {
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
    pub dark_matter: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayPlanetRow {
    pub id: String,
    pub user_id: String,
    pub universe_id: i64,
    pub name: String,
    pub galaxy: i32,
    pub system: i32,
    pub position: i32,
    pub temperature: i32,
    pub metal: i64,
    pub crystal: i64,
    pub deuterium: i64,
    pub energy: i64,
    pub buildings: BTreeMap<String, i32>,
    pub ships: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayResearchRow {
    pub user_id: String,
    pub levels: BTreeMap<String, i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayScoreRow {
    pub user_id: String,
    pub total_score: i64,
    pub economy_score: i64,
    pub research_score: i64,
    pub military_score: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayQueueRow {
    pub id: String,
    pub planet_id: String,
    pub item_type: String,
    pub target_level: Option<i32>,
    pub quantity: Option<i64>,
    pub finishes_in_seconds: i64,
    pub status: String,
}

/// A fully priced queue request produced by the game-domain/economy layer.
/// The repository revalidates ownership, current level, affordability, and
/// the single-active-queue invariant atomically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayQueueInput {
    pub user_id: String,
    pub planet_id: String,
    pub item_type: String,
    pub target_level: Option<i32>,
    pub quantity: Option<i64>,
    /// Trusted server-side callers must derive these costs and the duration
    /// from canonical game-domain/game-economy definitions. The repository
    /// validates structural invariants and non-negativity, but deliberately
    /// does not duplicate economic formulas and must never receive raw client
    /// prices.
    pub metal_cost: i64,
    pub crystal_cost: i64,
    pub deuterium_cost: i64,
    /// Energy is a capacity requirement and is not deducted.
    pub energy_required: i64,
    pub duration_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameplayWriteError {
    NotFound,
    UniverseFull,
    QueueBusy,
    InsufficientResources,
    StaleState,
    Invalid(String),
    /// A serialization failure or deadlock aborted the whole transaction.
    /// No resources or queue state committed, so the operation may be retried.
    Retryable(String),
    Database(String),
}

impl GameplayWriteError {
    /// Callers may safely retry only errors for which the complete transaction
    /// was rolled back. Invalid domain state and a full universe are terminal.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Retryable(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameplayCompletionKind {
    Building,
    Research,
    Shipyard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameplayCompletion {
    pub kind: GameplayCompletionKind,
    pub queue_id: String,
    pub user_id: String,
    pub planet_id: String,
    pub item_type: String,
    pub target_level: Option<i32>,
    pub quantity: Option<i64>,
    pub score_delta: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameplayProcessResult {
    pub buildings: usize,
    pub research: usize,
    pub ships: usize,
    pub failed: usize,
    /// Durable completion facts committed in the same transaction as the
    /// inventory and score changes. Workers can publish these after commit;
    /// replaying the worker cannot apply the queue twice.
    pub completions: Vec<GameplayCompletion>,
}

impl Database {
    /// Create an account, allocate the first free universe coordinate, create
    /// its homeworld, research row, and score row in one transaction. Universe
    /// selection is server-owned: the first open universe is selected under a
    /// row lock and no universe identifier is accepted from the client.
    /// A transaction-scoped advisory lock serializes coordinate allocation;
    /// the planets unique constraint remains the final integrity boundary.
    pub async fn register_account_with_starting_state(
        &self,
        input: AccountCreateInput,
    ) -> Result<AccountRow, AccountCreateError> {
        let input = input.normalized();
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?;
        let transaction = client
            .transaction()
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?;

        let universe_id = transaction
            .query_opt(
                "SELECT id
                 FROM universes
                 WHERE registration_open
                 ORDER BY id
                 FOR SHARE
                 LIMIT 1",
                &[],
            )
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?
            .map(|row| row.get::<_, i64>("id"))
            .ok_or_else(|| {
                AccountCreateError::Database(
                    "registration is unavailable: no universe is open".to_string(),
                )
            })?;

        let account_row = match transaction
            .query_one(
                "INSERT INTO users
                    (username, email, password_hash, dark_matter, universe_id,
                     is_admin, is_banned)
                 VALUES ($1, $2, $3, $4, $5, FALSE, FALSE)
                 RETURNING id::TEXT AS id, username, email, password_hash,
                           CASE WHEN is_admin THEN 'admin' ELSE 'player' END AS role,
                           universe_id, is_banned",
                &[
                    &input.username,
                    &input.email,
                    &input.password_hash,
                    &(STARTING_DARK_MATTER as i32),
                    &universe_id,
                ],
            )
            .await
        {
            Ok(row) => row,
            Err(error) if error.code() == Some(&SqlState::UNIQUE_VIOLATION) => {
                return Err(AccountCreateError::Duplicate);
            }
            Err(error) => return Err(AccountCreateError::Database(error.to_string())),
        };
        let account = map_account_row(&account_row);
        let account_id = account
            .id
            .parse::<i32>()
            .map_err(|_| AccountCreateError::Database("invalid created account id".to_string()))?;

        let coordinate = next_available_coordinate(&transaction, universe_id)
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?
            .ok_or_else(|| {
                AccountCreateError::Database("universe has no free planet coordinates".to_string())
            })?;

        transaction
            .execute(
                "INSERT INTO planets
                    (user_id, universe_id, name, galaxy, system, position, metal, crystal,
                     deuterium, energy, last_resource_update)
                 VALUES ($1, $2, 'New Terra', $3, $4, $5, $6, $7, $8, 0, now())",
                &[
                    &account_id,
                    &universe_id,
                    &coordinate.0,
                    &coordinate.1,
                    &coordinate.2,
                    &STARTING_METAL,
                    &STARTING_CRYSTAL,
                    &STARTING_DEUTERIUM,
                ],
            )
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO player_scores (user_id)
                 VALUES ($1)
                 ON CONFLICT (user_id) DO NOTHING",
                &[&account_id],
            )
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?;
        transaction
            .execute(
                "INSERT INTO research (user_id) VALUES ($1)
                 ON CONFLICT (user_id) DO NOTHING",
                &[&account_id],
            )
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| AccountCreateError::Database(error.to_string()))?;
        Ok(account)
    }

    /// Verify every table, column, index, and integer width required by this
    /// repository. No schema is created from a request path.
    pub async fn gameplay_repository_ready(&self) -> DbResult<()> {
        self.account_repository_ready().await?;
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_one(
                "SELECT
                    to_regclass('public.planets') IS NOT NULL
                    AND to_regclass('public.research') IS NOT NULL
                    AND to_regclass('public.construction_queue') IS NOT NULL
                    AND to_regclass('public.research_queue') IS NOT NULL
                    AND to_regclass('public.shipyard_queue') IS NOT NULL
                    AND to_regclass('public.universes') IS NOT NULL
                    AND to_regclass('public.player_scores') IS NOT NULL
                    AND to_regclass('public.planets_universe_coordinates_unique') IS NOT NULL
                    AND to_regclass('public.uq_construction_queue_active_planet') IS NOT NULL
                    AND to_regclass('public.uq_research_queue_active_user') IS NOT NULL
                    AND to_regclass('public.uq_shipyard_queue_active_planet') IS NOT NULL
                    AND to_regclass('public.uq_construction_queue_active_moon') IS NOT NULL
                    AND to_regclass('public.uq_shipyard_queue_active_moon') IS NOT NULL
                    AND EXISTS (
                        SELECT 1
                        FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'users'
                          AND column_name = 'universe_id' AND data_type = 'bigint'
                          AND is_nullable = 'NO'
                    )
                    AND EXISTS (
                        SELECT 1
                        FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'planets'
                          AND column_name = 'universe_id' AND data_type = 'bigint'
                          AND is_nullable = 'NO'
                    )
                    AND (
                        SELECT COUNT(*) = 30
                        FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'planets'
                          AND column_name IN (
                            'metal_mine', 'crystal_mine', 'deuterium_synthesizer',
                            'solar_plant', 'fusion_reactor', 'metal_storage',
                            'crystal_storage', 'deuterium_tank', 'robotics_factory',
                            'shipyard', 'research_lab', 'nanite_factory', 'terraformer',
                            'missile_silo', 'alliance_depot', 'space_dock',
                            'small_cargo', 'large_cargo', 'light_fighter', 'heavy_fighter',
                            'cruiser', 'battleship', 'battlecruiser', 'bomber', 'destroyer',
                            'deathstar', 'recycler', 'espionage_probe', 'solar_satellite',
                            'colony_ship'
                          )
                    )
                    AND (
                        SELECT COUNT(*) = 16
                        FROM information_schema.columns
                        WHERE table_schema = 'public' AND table_name = 'research'
                          AND column_name IN (
                            'energy_technology', 'laser_technology', 'ion_technology',
                            'hyperspace_technology', 'plasma_technology', 'combustion_drive',
                            'impulse_drive', 'hyperspace_drive', 'espionage_technology',
                            'computer_technology', 'astrophysics',
                            'intergalactic_research_network', 'graviton_technology',
                            'weapons_technology', 'shielding_technology', 'armor_technology'
                          )
                    )
                    AND (
                        SELECT COUNT(*) = 12
                        FROM information_schema.columns
                        WHERE table_schema = 'public'
                          AND table_name IN (
                            'construction_queue', 'research_queue', 'shipyard_queue'
                          )
                          AND column_name IN (
                            'energy_required', 'status', 'completed_at',
                            'processing_started_at'
                          )
                    )
                    AND (
                        SELECT COUNT(*) = 15
                        FROM information_schema.columns
                        WHERE table_schema = 'public'
                          AND data_type = 'bigint'
                          AND (
                            (table_name = 'planets' AND column_name IN (
                              'small_cargo', 'large_cargo', 'light_fighter', 'heavy_fighter',
                              'cruiser', 'battleship', 'battlecruiser', 'bomber', 'destroyer',
                              'deathstar', 'recycler', 'espionage_probe', 'solar_satellite',
                              'colony_ship'
                            )) OR (table_name = 'shipyard_queue' AND column_name = 'quantity')
                          )
                    ) AS ready",
                &[],
            )
            .await
            .map_err(|error| error.to_string())?;
        if !row.get::<_, bool>("ready") {
            return Err(
                "durable gameplay schema is missing; run ordered database migrations".to_string(),
            );
        }
        validate_gameplay_schema_definitions(&client).await
    }

    pub async fn gameplay_score_for_user(
        &self,
        user_id: &str,
    ) -> DbResult<Option<GameplayScoreRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(None);
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT user_id::TEXT AS user_id,
                        total_score::BIGINT AS total_score,
                        economy_score::BIGINT AS economy_score,
                        research_score::BIGINT AS research_score,
                        military_score::BIGINT AS military_score
                 FROM player_scores
                 WHERE user_id = $1",
                &[&user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| GameplayScoreRow {
            user_id: row.get("user_id"),
            total_score: row.get("total_score"),
            economy_score: row.get("economy_score"),
            research_score: row.get("research_score"),
            military_score: row.get("military_score"),
        }))
    }

    /// Trusted provisioning primitive shared by registration and future
    /// colonization orchestration. It derives the universe from the persisted
    /// owner, serializes allocation per universe, and retries only whole
    /// transactions that PostgreSQL confirms did not commit. It is not an HTTP
    /// handler and deliberately does not accept a client-selected coordinate.
    pub async fn gameplay_provision_planet_at_next_coordinate(
        &self,
        user_id: &str,
        name: &str,
    ) -> Result<GameplayPlanetRow, GameplayWriteError> {
        let user_id = parse_id(user_id)?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(GameplayWriteError::Invalid(
                "planet name must contain 1-100 characters".to_string(),
            ));
        }

        let mut last_retry = None;
        for _ in 0..COORDINATE_ALLOCATION_MAX_ATTEMPTS {
            match self.try_provision_planet(user_id, name).await {
                Err(GameplayWriteError::Retryable(message)) => last_retry = Some(message),
                result => return result,
            }
        }
        Err(GameplayWriteError::Retryable(last_retry.unwrap_or_else(
            || "coordinate allocation retry budget exhausted".to_string(),
        )))
    }

    async fn try_provision_planet(
        &self,
        user_id: i32,
        name: &str,
    ) -> Result<GameplayPlanetRow, GameplayWriteError> {
        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| GameplayWriteError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(map_write_db_error)?;
        let universe_id = transaction
            .query_opt(
                "SELECT universe_id FROM users WHERE id = $1 FOR SHARE",
                &[&user_id],
            )
            .await
            .map_err(map_write_db_error)?
            .map(|row| row.get::<_, i64>("universe_id"))
            .ok_or(GameplayWriteError::NotFound)?;
        let coordinate = next_available_coordinate(&transaction, universe_id)
            .await
            .map_err(map_write_db_error)?
            .ok_or(GameplayWriteError::UniverseFull)?;

        let inserted = transaction
            .query_one(
                "INSERT INTO planets
                    (user_id, universe_id, name, galaxy, system, position,
                     metal, crystal, deuterium, energy, last_resource_update)
                 VALUES ($1, $2, $3, $4, $5, $6, 0, 0, 0, 0, now())
                 RETURNING id",
                &[
                    &user_id,
                    &universe_id,
                    &name,
                    &coordinate.0,
                    &coordinate.1,
                    &coordinate.2,
                ],
            )
            .await;
        let planet_id = match inserted {
            Ok(row) => row.get::<_, i32>("id"),
            Err(error) if error.code() == Some(&SqlState::UNIQUE_VIOLATION) => {
                return Err(GameplayWriteError::Retryable(
                    "coordinate was claimed concurrently".to_string(),
                ));
            }
            Err(error) => return Err(map_write_db_error(error)),
        };
        let row = transaction
            .query_one(
                &format!("{} WHERE id = $1 AND user_id = $2", planet_select_sql()),
                &[&planet_id, &user_id],
            )
            .await
            .map_err(map_write_db_error)?;
        let planet = map_planet_row(&row);
        transaction.commit().await.map_err(map_write_db_error)?;
        Ok(planet)
    }

    pub async fn gameplay_account_resources(
        &self,
        user_id: &str,
    ) -> DbResult<Option<GameplayResourcesRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(None);
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                "SELECT COALESCE(SUM(p.metal), 0)::BIGINT AS metal,
                        COALESCE(SUM(p.crystal), 0)::BIGINT AS crystal,
                        COALESCE(SUM(p.deuterium), 0)::BIGINT AS deuterium,
                        COALESCE(SUM(p.energy), 0)::BIGINT AS energy,
                        COALESCE(u.dark_matter, 0)::BIGINT AS dark_matter
                 FROM users u
                 LEFT JOIN planets p ON p.user_id = u.id
                 WHERE u.id = $1
                 GROUP BY u.id, u.dark_matter",
                &[&user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.map(|row| GameplayResourcesRow {
            metal: row.get("metal"),
            crystal: row.get("crystal"),
            deuterium: row.get("deuterium"),
            energy: row.get("energy"),
            dark_matter: row.get("dark_matter"),
        }))
    }

    pub async fn gameplay_planets_for_user(
        &self,
        user_id: &str,
    ) -> DbResult<Vec<GameplayPlanetRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(Vec::new());
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(
                &format!("{} WHERE user_id = $1 ORDER BY id", planet_select_sql()),
                &[&user_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.iter().map(map_planet_row).collect())
    }

    pub async fn gameplay_planet_for_user(
        &self,
        user_id: &str,
        planet_id: &str,
    ) -> DbResult<Option<GameplayPlanetRow>> {
        let (Some(user_id), Some(planet_id)) =
            (parse_optional_id(user_id), parse_optional_id(planet_id))
        else {
            return Ok(None);
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(
                &format!("{} WHERE user_id = $1 AND id = $2", planet_select_sql()),
                &[&user_id, &planet_id],
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.as_ref().map(map_planet_row))
    }

    pub async fn gameplay_research_for_user(
        &self,
        user_id: &str,
    ) -> DbResult<Option<GameplayResearchRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(None);
        };
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let row = client
            .query_opt(&research_select_sql(), &[&user_id])
            .await
            .map_err(|error| error.to_string())?;
        Ok(row.as_ref().map(map_research_row))
    }

    pub async fn gameplay_rename_planet(
        &self,
        user_id: &str,
        planet_id: &str,
        new_name: &str,
    ) -> Result<(String, String), GameplayWriteError> {
        let user_id = parse_id(user_id)?;
        let planet_id = parse_id(planet_id)?;
        let new_name = new_name.trim();
        if new_name.is_empty() || new_name.chars().count() > 100 {
            return Err(GameplayWriteError::Invalid(
                "planet name must contain 1-100 characters".to_string(),
            ));
        }
        let client = self
            .pool
            .get()
            .await
            .map_err(|error| GameplayWriteError::Database(error.to_string()))?;
        let row = client
            .query_opt(
                "WITH existing AS (
                    SELECT id, name FROM planets
                    WHERE id = $1 AND user_id = $2
                    FOR UPDATE
                 ), updated AS (
                    UPDATE planets p SET name = $3
                    FROM existing e WHERE p.id = e.id
                    RETURNING e.name AS old_name, p.name AS new_name
                 )
                 SELECT old_name, new_name FROM updated",
                &[&planet_id, &user_id, &new_name],
            )
            .await
            .map_err(|error| GameplayWriteError::Database(error.to_string()))?
            .ok_or(GameplayWriteError::NotFound)?;
        Ok((row.get("old_name"), row.get("new_name")))
    }

    pub async fn gameplay_construction_queue_for_user(
        &self,
        user_id: &str,
    ) -> DbResult<Vec<GameplayQueueRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(Vec::new());
        };
        self.query_gameplay_queue(
            "SELECT q.id::TEXT AS id, q.planet_id::TEXT AS planet_id,
                    q.building_type AS item_type, q.level AS target_level,
                    NULL::BIGINT AS quantity,
                    GREATEST(0, CEIL(EXTRACT(EPOCH FROM q.end_time - now())))::BIGINT
                      AS finishes_in_seconds,
                    q.status
             FROM construction_queue q
             JOIN planets p ON p.id = q.planet_id
             WHERE p.user_id = $1 AND q.location_type = 'planet'
               AND q.status IN ('queued', 'processing')
             ORDER BY q.end_time, q.id",
            user_id,
        )
        .await
    }

    pub async fn gameplay_research_queue_for_user(
        &self,
        user_id: &str,
    ) -> DbResult<Vec<GameplayQueueRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(Vec::new());
        };
        self.query_gameplay_queue(
            "SELECT q.id::TEXT AS id, q.planet_id::TEXT AS planet_id,
                    q.research_type AS item_type, q.level AS target_level,
                    NULL::BIGINT AS quantity,
                    GREATEST(0, CEIL(EXTRACT(EPOCH FROM q.end_time - now())))::BIGINT
                      AS finishes_in_seconds,
                    q.status
             FROM research_queue q
             WHERE q.user_id = $1 AND q.status IN ('queued', 'processing')
             ORDER BY q.end_time, q.id",
            user_id,
        )
        .await
    }

    pub async fn gameplay_shipyard_queue_for_user(
        &self,
        user_id: &str,
    ) -> DbResult<Vec<GameplayQueueRow>> {
        let Some(user_id) = parse_optional_id(user_id) else {
            return Ok(Vec::new());
        };
        self.query_gameplay_queue(
            "SELECT q.id::TEXT AS id, q.planet_id::TEXT AS planet_id,
                    q.unit_type AS item_type, NULL::INTEGER AS target_level,
                    q.quantity::BIGINT AS quantity,
                    GREATEST(0, CEIL(EXTRACT(EPOCH FROM q.end_time - now())))::BIGINT
                      AS finishes_in_seconds,
                    q.status
             FROM shipyard_queue q
             JOIN planets p ON p.id = q.planet_id
             WHERE p.user_id = $1 AND q.location_type = 'planet'
               AND q.status IN ('queued', 'processing')
             ORDER BY q.end_time, q.id",
            user_id,
        )
        .await
    }

    async fn query_gameplay_queue(
        &self,
        sql: &str,
        user_id: i32,
    ) -> DbResult<Vec<GameplayQueueRow>> {
        let client = self.pool.get().await.map_err(|error| error.to_string())?;
        let rows = client
            .query(sql, &[&user_id])
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.iter().map(map_queue_row).collect())
    }

    pub async fn gameplay_enqueue_building(
        &self,
        input: &GameplayQueueInput,
    ) -> Result<GameplayQueueRow, GameplayWriteError> {
        validate_queue_input(input)?;
        let user_id = parse_id(&input.user_id)?;
        let planet_id = parse_id(&input.planet_id)?;
        let column = lookup_column(&building_columns(), &input.item_type).ok_or_else(|| {
            GameplayWriteError::Invalid(format!("unknown building type: {}", input.item_type))
        })?;
        let target_level = input.target_level.ok_or_else(|| {
            GameplayWriteError::Invalid("building target level is required".to_string())
        })?;
        if input.quantity.is_some() {
            return Err(GameplayWriteError::Invalid(
                "building queue cannot include quantity".to_string(),
            ));
        }

        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| GameplayWriteError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(map_write_db_error)?;
        ensure_planet_queue_slot(&transaction, "construction_queue", planet_id, user_id, true)
            .await?;
        let planet = lock_planet(&transaction, user_id, planet_id, Some(column)).await?;
        let current_level = planet.current_level.ok_or_else(|| {
            GameplayWriteError::Database("building level was not selected".to_string())
        })?;
        if current_level.checked_add(1) != Some(target_level) {
            return Err(GameplayWriteError::StaleState);
        }
        ensure_planet_queue_slot(
            &transaction,
            "construction_queue",
            planet_id,
            user_id,
            false,
        )
        .await?;
        ensure_affordable(&planet, input)?;
        deduct_resources(&transaction, planet_id, input).await?;

        let duration = input.duration_seconds as f64;
        let row = transaction
            .query_one(
                "INSERT INTO construction_queue
                    (planet_id, building_type, level, end_time, metal_cost,
                     crystal_cost, deuterium_cost, energy_required, status)
                 VALUES ($1, $2, $3,
                         now() + ($8::DOUBLE PRECISION * INTERVAL '1 second'),
                         $4, $5, $6, $7, 'queued')
                 RETURNING id::TEXT AS id, planet_id::TEXT AS planet_id,
                           building_type AS item_type, level AS target_level,
                           NULL::BIGINT AS quantity,
                           GREATEST(0, CEIL(EXTRACT(EPOCH FROM end_time - now())))::BIGINT
                             AS finishes_in_seconds,
                           status",
                &[
                    &planet_id,
                    &input.item_type,
                    &target_level,
                    &input.metal_cost,
                    &input.crystal_cost,
                    &input.deuterium_cost,
                    &input.energy_required,
                    &duration,
                ],
            )
            .await
            .map_err(map_queue_insert_error)?;
        let queued = map_queue_row(&row);
        transaction.commit().await.map_err(map_write_db_error)?;
        Ok(queued)
    }

    pub async fn gameplay_enqueue_research(
        &self,
        input: &GameplayQueueInput,
    ) -> Result<GameplayQueueRow, GameplayWriteError> {
        validate_queue_input(input)?;
        let user_id = parse_id(&input.user_id)?;
        let planet_id = parse_id(&input.planet_id)?;
        let column = lookup_column(&research_columns(), &input.item_type).ok_or_else(|| {
            GameplayWriteError::Invalid(format!("unknown research type: {}", input.item_type))
        })?;
        let target_level = input.target_level.ok_or_else(|| {
            GameplayWriteError::Invalid("research target level is required".to_string())
        })?;
        if input.quantity.is_some() {
            return Err(GameplayWriteError::Invalid(
                "research queue cannot include quantity".to_string(),
            ));
        }

        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| GameplayWriteError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(map_write_db_error)?;
        ensure_user_queue_slot(&transaction, "research_queue", user_id, true).await?;
        let row = transaction
            .query_opt(
                &format!(
                    "SELECT COALESCE(r.{column}, 0) AS current_level,
                            COALESCE(p.metal, 0)::BIGINT AS metal,
                            COALESCE(p.crystal, 0)::BIGINT AS crystal,
                            COALESCE(p.deuterium, 0)::BIGINT AS deuterium,
                            COALESCE(p.energy, 0)::BIGINT AS energy
                     FROM research r
                     JOIN planets p ON p.id = $2 AND p.user_id = r.user_id
                     WHERE r.user_id = $1
                     FOR UPDATE OF r, p"
                ),
                &[&user_id, &planet_id],
            )
            .await
            .map_err(map_write_db_error)?
            .ok_or(GameplayWriteError::NotFound)?;
        let current_level = row.get::<_, i32>("current_level");
        if current_level.checked_add(1) != Some(target_level) {
            return Err(GameplayWriteError::StaleState);
        }
        ensure_user_queue_slot(&transaction, "research_queue", user_id, false).await?;
        let planet = LockedPlanet::from_row(&row, None);
        ensure_affordable(&planet, input)?;
        deduct_resources(&transaction, planet_id, input).await?;

        let duration = input.duration_seconds as f64;
        let row = transaction
            .query_one(
                "INSERT INTO research_queue
                    (user_id, planet_id, research_type, level, end_time,
                     metal_cost, crystal_cost, deuterium_cost, energy_required, status)
                 VALUES ($1, $2, $3, $4,
                         now() + ($9::DOUBLE PRECISION * INTERVAL '1 second'),
                         $5, $6, $7, $8, 'queued')
                 RETURNING id::TEXT AS id, planet_id::TEXT AS planet_id,
                           research_type AS item_type, level AS target_level,
                           NULL::BIGINT AS quantity,
                           GREATEST(0, CEIL(EXTRACT(EPOCH FROM end_time - now())))::BIGINT
                             AS finishes_in_seconds,
                           status",
                &[
                    &user_id,
                    &planet_id,
                    &input.item_type,
                    &target_level,
                    &input.metal_cost,
                    &input.crystal_cost,
                    &input.deuterium_cost,
                    &input.energy_required,
                    &duration,
                ],
            )
            .await
            .map_err(map_queue_insert_error)?;
        let queued = map_queue_row(&row);
        transaction.commit().await.map_err(map_write_db_error)?;
        Ok(queued)
    }

    pub async fn gameplay_enqueue_ships(
        &self,
        input: &GameplayQueueInput,
    ) -> Result<GameplayQueueRow, GameplayWriteError> {
        validate_queue_input(input)?;
        let user_id = parse_id(&input.user_id)?;
        let planet_id = parse_id(&input.planet_id)?;
        let ship_column = lookup_column(&ship_columns(), &input.item_type).ok_or_else(|| {
            GameplayWriteError::Invalid(format!("unknown ship type: {}", input.item_type))
        })?;
        let quantity = input
            .quantity
            .filter(|quantity| (1..=MAX_SHIP_QUEUE_QUANTITY).contains(quantity))
            .ok_or_else(|| {
                GameplayWriteError::Invalid(format!(
                    "ship quantity must be between 1 and {MAX_SHIP_QUEUE_QUANTITY}"
                ))
            })?;
        if input.target_level.is_some() {
            return Err(GameplayWriteError::Invalid(
                "shipyard queue cannot include target level".to_string(),
            ));
        }

        let mut client = self
            .pool
            .get()
            .await
            .map_err(|error| GameplayWriteError::Database(error.to_string()))?;
        let transaction = client.transaction().await.map_err(map_write_db_error)?;
        ensure_planet_queue_slot(&transaction, "shipyard_queue", planet_id, user_id, true).await?;
        let planet = lock_planet(&transaction, user_id, planet_id, None).await?;
        let inventory = transaction
            .query_one(
                &format!(
                    "SELECT COALESCE({ship_column}, 0)::BIGINT AS inventory
                     FROM planets WHERE id = $1"
                ),
                &[&planet_id],
            )
            .await
            .map_err(map_write_db_error)?
            .get::<_, i64>("inventory");
        if inventory > i64::MAX - quantity {
            return Err(GameplayWriteError::Invalid(
                "ship inventory would overflow".to_string(),
            ));
        }
        ensure_planet_queue_slot(&transaction, "shipyard_queue", planet_id, user_id, false).await?;
        ensure_affordable(&planet, input)?;
        deduct_resources(&transaction, planet_id, input).await?;

        let duration = input.duration_seconds as f64;
        let row = transaction
            .query_one(
                "INSERT INTO shipyard_queue
                    (planet_id, unit_type, quantity, end_time, metal_cost,
                     crystal_cost, deuterium_cost, energy_required, status)
                 VALUES ($1, $2, $3,
                         now() + ($8::DOUBLE PRECISION * INTERVAL '1 second'),
                         $4, $5, $6, $7, 'queued')
                 RETURNING id::TEXT AS id, planet_id::TEXT AS planet_id,
                           unit_type AS item_type, NULL::INTEGER AS target_level,
                           quantity::BIGINT AS quantity,
                           GREATEST(0, CEIL(EXTRACT(EPOCH FROM end_time - now())))::BIGINT
                             AS finishes_in_seconds,
                           status",
                &[
                    &planet_id,
                    &input.item_type,
                    &quantity,
                    &input.metal_cost,
                    &input.crystal_cost,
                    &input.deuterium_cost,
                    &input.energy_required,
                    &duration,
                ],
            )
            .await
            .map_err(map_queue_insert_error)?;
        let queued = map_queue_row(&row);
        transaction.commit().await.map_err(map_write_db_error)?;
        Ok(queued)
    }

    /// Claim and apply at most `limit_per_queue` due rows from each of the
    /// building, research, and ship queues under `FOR UPDATE SKIP LOCKED`.
    /// Equal per-kind bounds prevent a busy building queue from starving the
    /// other two kinds; a call processes at most `3 * limit_per_queue` rows.
    /// Inventory changes and terminal statuses commit together, making retries
    /// and concurrent workers exactly-once from the database's perspective.
    /// PostgreSQL transaction conflicts abort without partial effects and are
    /// returned as retryable errors for a scheduler to retry safely.
    pub async fn process_due_gameplay_queues(
        &self,
        limit_per_queue: usize,
    ) -> DbResult<GameplayProcessResult> {
        let limit_per_queue = limit_per_queue.clamp(1, MAX_PROCESS_BATCH);
        let mut client = self.pool.get().await.map_err(|error| error.to_string())?;
        let transaction = client.transaction().await.map_err(map_process_db_error)?;
        let mut result = GameplayProcessResult::default();

        quarantine_stale_processing(&transaction).await?;
        process_due_buildings(&transaction, limit_per_queue, &mut result).await?;
        process_due_research(&transaction, limit_per_queue, &mut result).await?;
        process_due_ships(&transaction, limit_per_queue, &mut result).await?;

        transaction.commit().await.map_err(map_process_db_error)?;
        Ok(result)
    }
}

async fn validate_gameplay_schema_definitions(client: &tokio_postgres::Client) -> DbResult<()> {
    let index_rows = client
        .query(
            "SELECT indexname, indexdef
             FROM pg_indexes
             WHERE schemaname = 'public'",
            &[],
        )
        .await
        .map_err(|error| error.to_string())?;
    let indexes = index_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<_, String>("indexname"),
                row.get::<_, String>("indexdef").to_ascii_lowercase(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let required_indexes: [(&str, &[&str]); 9] = [
        (
            "planets_universe_coordinates_unique",
            &[
                "create unique index",
                "universe_id",
                "galaxy",
                "system",
                "position",
            ],
        ),
        (
            "uq_construction_queue_active_planet",
            &[
                "create unique index",
                "(planet_id)",
                "location_type",
                "'planet'",
                "status",
                "'queued'",
                "'processing'",
            ],
        ),
        (
            "uq_research_queue_active_user",
            &[
                "create unique index",
                "(user_id)",
                "status",
                "'queued'",
                "'processing'",
            ],
        ),
        (
            "uq_shipyard_queue_active_planet",
            &[
                "create unique index",
                "(planet_id)",
                "location_type",
                "'planet'",
                "status",
                "'queued'",
                "'processing'",
            ],
        ),
        (
            "uq_construction_queue_active_moon",
            &[
                "create unique index",
                "(moon_id)",
                "location_type",
                "'moon'",
                "status",
                "'queued'",
                "'processing'",
            ],
        ),
        (
            "uq_shipyard_queue_active_moon",
            &[
                "create unique index",
                "(moon_id)",
                "location_type",
                "'moon'",
                "status",
                "'queued'",
                "'processing'",
            ],
        ),
        (
            "idx_construction_queue_due",
            &[
                "(end_time, id)",
                "location_type",
                "'planet'",
                "status",
                "'queued'",
            ],
        ),
        (
            "idx_research_queue_due",
            &["(end_time, id)", "status", "'queued'"],
        ),
        (
            "idx_shipyard_queue_due",
            &[
                "(end_time, id)",
                "location_type",
                "'planet'",
                "status",
                "'queued'",
            ],
        ),
    ];
    for (name, tokens) in required_indexes {
        let Some(definition) = indexes.get(name) else {
            return Err(format!("required gameplay index is missing: {name}"));
        };
        if !definition_contains_all(definition, tokens) {
            return Err(format!(
                "gameplay index has an unexpected column or predicate definition: {name}"
            ));
        }
    }

    let constraint_rows = client
        .query(
            "SELECT c.conname, pg_get_constraintdef(c.oid) AS definition, c.convalidated
             FROM pg_constraint c
             JOIN pg_class t ON t.oid = c.conrelid
             JOIN pg_namespace n ON n.oid = t.relnamespace
             WHERE n.nspname = 'public'
               AND t.relname IN (
                 'construction_queue', 'research_queue', 'shipyard_queue'
               )",
            &[],
        )
        .await
        .map_err(|error| error.to_string())?;
    let constraints = constraint_rows
        .into_iter()
        .map(|row| {
            (
                row.get::<_, String>("conname"),
                (
                    row.get::<_, String>("definition").to_ascii_lowercase(),
                    row.get::<_, bool>("convalidated"),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let required_constraints: [(&str, &[&str]); 8] = [
        (
            "construction_queue_level_positive",
            &["status", "queued", "processing", "level", "> 0"],
        ),
        (
            "research_queue_level_positive",
            &["status", "queued", "processing", "level", "> 0"],
        ),
        (
            "shipyard_queue_quantity_positive",
            &["status", "queued", "processing", "quantity", "> 0"],
        ),
        (
            "construction_queue_status_valid",
            &["status", "legacy_unclassified", "stale_processing"],
        ),
        (
            "research_queue_status_valid",
            &["status", "legacy_unclassified", "stale_processing"],
        ),
        (
            "shipyard_queue_status_valid",
            &["status", "legacy_unclassified", "stale_processing"],
        ),
        (
            "construction_queue_active_location_valid",
            &["status", "location_type", "planet_id", "moon_id"],
        ),
        (
            "shipyard_queue_active_location_valid",
            &["status", "location_type", "planet_id", "moon_id"],
        ),
    ];
    for (name, tokens) in required_constraints {
        let Some((definition, validated)) = constraints.get(name) else {
            return Err(format!("required gameplay constraint is missing: {name}"));
        };
        if !validated || !definition_contains_all(definition, tokens) {
            return Err(format!(
                "gameplay constraint is unvalidated or has an unexpected definition: {name}"
            ));
        }
    }
    Ok(())
}

fn definition_contains_all(definition: &str, tokens: &[&str]) -> bool {
    tokens.iter().all(|token| definition.contains(token))
}

async fn quarantine_stale_processing(transaction: &Transaction<'_>) -> DbResult<()> {
    let stale_after = STALE_PROCESSING_SECONDS as f64;
    for table in ["construction_queue", "research_queue", "shipyard_queue"] {
        transaction
            .execute(
                &format!(
                    "UPDATE {table}
                     SET status = 'stale_processing', completed_at = COALESCE(completed_at, now())
                     WHERE status = 'processing'
                       AND COALESCE(processing_started_at, start_time) <=
                           now() - ($1::DOUBLE PRECISION * INTERVAL '1 second')"
                ),
                &[&stale_after],
            )
            .await
            .map_err(map_process_db_error)?;
    }
    Ok(())
}

#[derive(Debug)]
struct LockedPlanet {
    metal: i64,
    crystal: i64,
    deuterium: i64,
    energy: i64,
    current_level: Option<i32>,
}

impl LockedPlanet {
    fn from_row(row: &tokio_postgres::Row, current_level: Option<i32>) -> Self {
        Self {
            metal: row.get("metal"),
            crystal: row.get("crystal"),
            deuterium: row.get("deuterium"),
            energy: row.get("energy"),
            current_level,
        }
    }
}

async fn lock_planet(
    transaction: &Transaction<'_>,
    user_id: i32,
    planet_id: i32,
    level_column: Option<&str>,
) -> Result<LockedPlanet, GameplayWriteError> {
    let level = level_column
        .map(|column| format!("COALESCE({column}, 0) AS current_level,"))
        .unwrap_or_default();
    let row = transaction
        .query_opt(
            &format!(
                "SELECT {level}
                        COALESCE(metal, 0)::BIGINT AS metal,
                        COALESCE(crystal, 0)::BIGINT AS crystal,
                        COALESCE(deuterium, 0)::BIGINT AS deuterium,
                        COALESCE(energy, 0)::BIGINT AS energy
                 FROM planets
                 WHERE id = $1 AND user_id = $2
                 FOR UPDATE"
            ),
            &[&planet_id, &user_id],
        )
        .await
        .map_err(map_write_db_error)?
        .ok_or(GameplayWriteError::NotFound)?;
    let current_level = level_column.map(|_| row.get::<_, i32>("current_level"));
    Ok(LockedPlanet::from_row(&row, current_level))
}

async fn ensure_planet_queue_slot(
    transaction: &Transaction<'_>,
    table: &str,
    planet_id: i32,
    user_id: i32,
    lock_existing: bool,
) -> Result<(), GameplayWriteError> {
    // `table` is a private call-site constant, never request input. The first
    // check locks an existing queue row before planet state, matching the due
    // processor's queue -> state lock order. The post-state check is lock-free
    // to close the no-row insertion race without creating an inverse lock edge.
    let lock = if lock_existing { "FOR UPDATE OF q" } else { "" };
    let active = transaction
        .query_opt(
            &format!(
                "SELECT q.id FROM {table} q
                 JOIN planets p ON p.id = q.planet_id
                 WHERE q.location_type = 'planet' AND q.planet_id = $1
                   AND p.user_id = $2
                   AND q.status IN ('queued', 'processing')
                 {lock}
                 LIMIT 1"
            ),
            &[&planet_id, &user_id],
        )
        .await
        .map_err(map_write_db_error)?;
    if active.is_some() {
        Err(GameplayWriteError::QueueBusy)
    } else {
        Ok(())
    }
}

async fn ensure_user_queue_slot(
    transaction: &Transaction<'_>,
    table: &str,
    user_id: i32,
    lock_existing: bool,
) -> Result<(), GameplayWriteError> {
    let lock = if lock_existing { "FOR UPDATE OF q" } else { "" };
    let active = transaction
        .query_opt(
            &format!(
                "SELECT q.id FROM {table} q
                 WHERE q.user_id = $1 AND q.status IN ('queued', 'processing')
                 {lock}
                 LIMIT 1"
            ),
            &[&user_id],
        )
        .await
        .map_err(map_write_db_error)?;
    if active.is_some() {
        Err(GameplayWriteError::QueueBusy)
    } else {
        Ok(())
    }
}

fn ensure_affordable(
    planet: &LockedPlanet,
    input: &GameplayQueueInput,
) -> Result<(), GameplayWriteError> {
    if planet.metal < input.metal_cost
        || planet.crystal < input.crystal_cost
        || planet.deuterium < input.deuterium_cost
        || planet.energy < input.energy_required
    {
        Err(GameplayWriteError::InsufficientResources)
    } else {
        Ok(())
    }
}

async fn deduct_resources(
    transaction: &Transaction<'_>,
    planet_id: i32,
    input: &GameplayQueueInput,
) -> Result<(), GameplayWriteError> {
    let updated = transaction
        .execute(
            "UPDATE planets
             SET metal = COALESCE(metal, 0) - $2,
                 crystal = COALESCE(crystal, 0) - $3,
                 deuterium = COALESCE(deuterium, 0) - $4
             WHERE id = $1",
            &[
                &planet_id,
                &input.metal_cost,
                &input.crystal_cost,
                &input.deuterium_cost,
            ],
        )
        .await
        .map_err(map_write_db_error)?;
    if updated == 1 {
        Ok(())
    } else {
        Err(GameplayWriteError::NotFound)
    }
}

fn validate_queue_input(input: &GameplayQueueInput) -> Result<(), GameplayWriteError> {
    if input.item_type.trim().is_empty() {
        return Err(GameplayWriteError::Invalid(
            "queue item type is required".to_string(),
        ));
    }
    if input.metal_cost < 0
        || input.crystal_cost < 0
        || input.deuterium_cost < 0
        || input.energy_required < 0
    {
        return Err(GameplayWriteError::Invalid(
            "queue costs cannot be negative".to_string(),
        ));
    }
    if !(0..=MAX_QUEUE_DURATION_SECONDS).contains(&input.duration_seconds) {
        return Err(GameplayWriteError::Invalid(
            "queue duration is outside the supported range".to_string(),
        ));
    }
    Ok(())
}

fn map_queue_insert_error(error: tokio_postgres::Error) -> GameplayWriteError {
    if error.code() == Some(&SqlState::UNIQUE_VIOLATION) {
        GameplayWriteError::QueueBusy
    } else {
        map_write_db_error(error)
    }
}

fn map_process_db_error(error: tokio_postgres::Error) -> String {
    if is_retryable_transaction_error(&error) {
        format!("retryable gameplay transaction conflict: {error}")
    } else {
        error.to_string()
    }
}

fn map_write_db_error(error: tokio_postgres::Error) -> GameplayWriteError {
    if is_retryable_transaction_error(&error) {
        GameplayWriteError::Retryable(error.to_string())
    } else {
        GameplayWriteError::Database(error.to_string())
    }
}

fn is_retryable_transaction_error(error: &tokio_postgres::Error) -> bool {
    matches!(
        error.code(),
        Some(&SqlState::T_R_DEADLOCK_DETECTED | &SqlState::T_R_SERIALIZATION_FAILURE)
    )
}

async fn next_available_coordinate(
    transaction: &Transaction<'_>,
    universe_id: i64,
) -> Result<Option<(i32, i32, i32)>, tokio_postgres::Error> {
    // The hashed advisory key namespaces the lock by universe while avoiding
    // a lossy BIGINT-to-INTEGER cast. The unique constraint remains the final
    // integrity boundary for writers that do not use this shared allocator.
    transaction
        .query_one(
            "SELECT pg_advisory_xact_lock(
                hashtextextended('universus:coordinate:' || $1::BIGINT::TEXT, 0)
             )",
            &[&universe_id],
        )
        .await?;
    transaction
        .query_opt(
            "SELECT coordinates.galaxy, coordinates.system, coordinates.position
             FROM (
                SELECT galaxy, system, position
                FROM generate_series(1, 9) AS galaxy
                CROSS JOIN generate_series(1, 499) AS system
                CROSS JOIN generate_series(1, 15) AS position
             ) AS coordinates
             LEFT JOIN planets p
               ON p.universe_id = $1
              AND p.galaxy = coordinates.galaxy
              AND p.system = coordinates.system
              AND p.position = coordinates.position
             WHERE p.id IS NULL
             ORDER BY coordinates.galaxy, coordinates.system, coordinates.position
             LIMIT 1",
            &[&universe_id],
        )
        .await
        .map(|row| {
            row.map(|row| {
                (
                    row.get::<_, i32>("galaxy"),
                    row.get::<_, i32>("system"),
                    row.get::<_, i32>("position"),
                )
            })
        })
}

async fn process_due_buildings(
    transaction: &Transaction<'_>,
    limit: usize,
    result: &mut GameplayProcessResult,
) -> DbResult<()> {
    if limit == 0 {
        return Ok(());
    }
    let rows = transaction
        .query(
            "SELECT q.id, q.planet_id, p.user_id, q.building_type, q.level,
                    q.metal_cost, q.crystal_cost, q.deuterium_cost
             FROM construction_queue q
             JOIN planets p ON p.id = q.planet_id
             WHERE q.location_type = 'planet' AND q.planet_id IS NOT NULL
               AND q.status = 'queued' AND q.end_time <= now()
             ORDER BY q.end_time, q.id
             FOR UPDATE OF q SKIP LOCKED
             LIMIT $1",
            &[&(limit as i64)],
        )
        .await
        .map_err(map_process_db_error)?;
    for row in rows {
        let id = row.get::<_, i32>("id");
        let planet_id = row.get::<_, i32>("planet_id");
        let user_id = row.get::<_, i32>("user_id");
        let item = row.get::<_, String>("building_type");
        let target = row.get::<_, i32>("level");
        if target <= 0 {
            mark_queue_failed(transaction, "construction_queue", id).await?;
            result.failed += 1;
            continue;
        }
        let Some(column) = lookup_column(&building_columns(), &item) else {
            mark_queue_failed(transaction, "construction_queue", id).await?;
            result.failed += 1;
            continue;
        };
        let updated = transaction
            .execute(
                &format!(
                    "UPDATE planets SET {column} = GREATEST(COALESCE({column}, 0), $2)
                     WHERE id = $1"
                ),
                &[&planet_id, &target],
            )
            .await
            .map_err(map_process_db_error)?;
        if updated == 1 {
            let score_delta = queue_score_delta(&row);
            apply_score_delta(
                transaction,
                user_id,
                GameplayCompletionKind::Building,
                score_delta,
            )
            .await?;
            mark_queue_completed(transaction, "construction_queue", id).await?;
            result.buildings += 1;
            result.completions.push(GameplayCompletion {
                kind: GameplayCompletionKind::Building,
                queue_id: id.to_string(),
                user_id: user_id.to_string(),
                planet_id: planet_id.to_string(),
                item_type: item,
                target_level: Some(target),
                quantity: None,
                score_delta,
            });
        } else {
            mark_queue_failed(transaction, "construction_queue", id).await?;
            result.failed += 1;
        }
    }
    Ok(())
}

async fn process_due_research(
    transaction: &Transaction<'_>,
    limit: usize,
    result: &mut GameplayProcessResult,
) -> DbResult<()> {
    if limit == 0 {
        return Ok(());
    }
    let rows = transaction
        .query(
            "SELECT id, user_id, planet_id, research_type, level,
                    metal_cost, crystal_cost, deuterium_cost
             FROM research_queue
             WHERE status = 'queued' AND end_time <= now()
             ORDER BY end_time, id
             FOR UPDATE SKIP LOCKED
             LIMIT $1",
            &[&(limit as i64)],
        )
        .await
        .map_err(map_process_db_error)?;
    for row in rows {
        let id = row.get::<_, i32>("id");
        let user_id = row.get::<_, i32>("user_id");
        let planet_id = row.get::<_, i32>("planet_id");
        let item = row.get::<_, String>("research_type");
        let target = row.get::<_, i32>("level");
        if target <= 0 {
            mark_queue_failed(transaction, "research_queue", id).await?;
            result.failed += 1;
            continue;
        }
        let Some(column) = lookup_column(&research_columns(), &item) else {
            mark_queue_failed(transaction, "research_queue", id).await?;
            result.failed += 1;
            continue;
        };
        let updated = transaction
            .execute(
                &format!(
                    "UPDATE research SET {column} = GREATEST(COALESCE({column}, 0), $2)
                     WHERE user_id = $1"
                ),
                &[&user_id, &target],
            )
            .await
            .map_err(map_process_db_error)?;
        if updated == 1 {
            let score_delta = queue_score_delta(&row);
            apply_score_delta(
                transaction,
                user_id,
                GameplayCompletionKind::Research,
                score_delta,
            )
            .await?;
            mark_queue_completed(transaction, "research_queue", id).await?;
            result.research += 1;
            result.completions.push(GameplayCompletion {
                kind: GameplayCompletionKind::Research,
                queue_id: id.to_string(),
                user_id: user_id.to_string(),
                planet_id: planet_id.to_string(),
                item_type: item,
                target_level: Some(target),
                quantity: None,
                score_delta,
            });
        } else {
            mark_queue_failed(transaction, "research_queue", id).await?;
            result.failed += 1;
        }
    }
    Ok(())
}

async fn process_due_ships(
    transaction: &Transaction<'_>,
    limit: usize,
    result: &mut GameplayProcessResult,
) -> DbResult<()> {
    if limit == 0 {
        return Ok(());
    }
    let rows = transaction
        .query(
            "SELECT q.id, q.planet_id, p.user_id, q.unit_type,
                    q.quantity::BIGINT AS quantity,
                    q.metal_cost, q.crystal_cost, q.deuterium_cost
             FROM shipyard_queue q
             JOIN planets p ON p.id = q.planet_id
             WHERE q.location_type = 'planet' AND q.planet_id IS NOT NULL
               AND q.status = 'queued' AND q.end_time <= now()
             ORDER BY q.end_time, q.id
             FOR UPDATE OF q SKIP LOCKED
             LIMIT $1",
            &[&(limit as i64)],
        )
        .await
        .map_err(map_process_db_error)?;
    for row in rows {
        let id = row.get::<_, i32>("id");
        let planet_id = row.get::<_, i32>("planet_id");
        let user_id = row.get::<_, i32>("user_id");
        let item = row.get::<_, String>("unit_type");
        let quantity = row.get::<_, i64>("quantity");
        if quantity <= 0 {
            mark_queue_failed(transaction, "shipyard_queue", id).await?;
            result.failed += 1;
            continue;
        }
        let Some(column) = lookup_column(&ship_columns(), &item) else {
            mark_queue_failed(transaction, "shipyard_queue", id).await?;
            result.failed += 1;
            continue;
        };
        let updated = transaction
            .execute(
                &format!(
                    "UPDATE planets SET {column} = COALESCE({column}, 0) + $2
                     WHERE id = $1 AND COALESCE({column}, 0) <= $3"
                ),
                &[&planet_id, &quantity, &(i64::MAX - quantity)],
            )
            .await
            .map_err(map_process_db_error)?;
        if updated == 1 {
            let score_delta = queue_score_delta(&row);
            apply_score_delta(
                transaction,
                user_id,
                GameplayCompletionKind::Shipyard,
                score_delta,
            )
            .await?;
            mark_queue_completed(transaction, "shipyard_queue", id).await?;
            result.ships += 1;
            result.completions.push(GameplayCompletion {
                kind: GameplayCompletionKind::Shipyard,
                queue_id: id.to_string(),
                user_id: user_id.to_string(),
                planet_id: planet_id.to_string(),
                item_type: item,
                target_level: None,
                quantity: Some(quantity),
                score_delta,
            });
        } else {
            mark_queue_failed(transaction, "shipyard_queue", id).await?;
            result.failed += 1;
        }
    }
    Ok(())
}

fn queue_score_delta(row: &tokio_postgres::Row) -> i64 {
    let spent = i128::from(row.get::<_, i64>("metal_cost"))
        + i128::from(row.get::<_, i64>("crystal_cost"))
        + i128::from(row.get::<_, i64>("deuterium_cost"));
    (spent.max(0) / 1_000).min(i128::from(i64::MAX)) as i64
}

async fn apply_score_delta(
    transaction: &Transaction<'_>,
    user_id: i32,
    kind: GameplayCompletionKind,
    score_delta: i64,
) -> DbResult<()> {
    let score_column = match kind {
        GameplayCompletionKind::Building => "economy_score",
        GameplayCompletionKind::Research => "research_score",
        GameplayCompletionKind::Shipyard => "military_score",
    };
    transaction
        .execute(
            &format!(
                "INSERT INTO player_scores
                    (user_id, total_score, {score_column}, last_updated)
                 VALUES ($1, $2, $2, now())
                 ON CONFLICT (user_id) DO UPDATE
                 SET total_score = LEAST(
                        9223372036854775807::NUMERIC,
                        player_scores.total_score::NUMERIC + EXCLUDED.total_score
                     )::BIGINT,
                     {score_column} = LEAST(
                        9223372036854775807::NUMERIC,
                        player_scores.{score_column}::NUMERIC + EXCLUDED.{score_column}
                     )::BIGINT,
                     last_updated = now()"
            ),
            &[&user_id, &score_delta],
        )
        .await
        .map(|_| ())
        .map_err(map_process_db_error)
}

async fn mark_queue_completed(transaction: &Transaction<'_>, table: &str, id: i32) -> DbResult<()> {
    transaction
        .execute(
            &format!(
                "UPDATE {table}
                 SET status = 'completed', completed_at = now()
                 WHERE id = $1 AND status = 'queued'"
            ),
            &[&id],
        )
        .await
        .map(|_| ())
        .map_err(map_process_db_error)
}

async fn mark_queue_failed(transaction: &Transaction<'_>, table: &str, id: i32) -> DbResult<()> {
    transaction
        .execute(
            &format!(
                "UPDATE {table}
                 SET status = 'failed', completed_at = now()
                 WHERE id = $1 AND status = 'queued'"
            ),
            &[&id],
        )
        .await
        .map(|_| ())
        .map_err(map_process_db_error)
}

fn planet_select_sql() -> &'static str {
    "SELECT id::TEXT AS id, user_id::TEXT AS user_id, universe_id::BIGINT AS universe_id,
            name, galaxy, system,
            position, COALESCE(temperature, 20) AS temperature,
            COALESCE(metal, 0)::BIGINT AS metal,
            COALESCE(crystal, 0)::BIGINT AS crystal,
            COALESCE(deuterium, 0)::BIGINT AS deuterium,
            COALESCE(energy, 0)::BIGINT AS energy,
            COALESCE(metal_mine, 0) AS metal_mine,
            COALESCE(crystal_mine, 0) AS crystal_mine,
            COALESCE(deuterium_synthesizer, 0) AS deuterium_synthesizer,
            COALESCE(solar_plant, 0) AS solar_plant,
            COALESCE(fusion_reactor, 0) AS fusion_reactor,
            COALESCE(metal_storage, 0) AS metal_storage,
            COALESCE(crystal_storage, 0) AS crystal_storage,
            COALESCE(deuterium_tank, 0) AS deuterium_tank,
            COALESCE(robotics_factory, 0) AS robotics_factory,
            COALESCE(shipyard, 0) AS shipyard,
            COALESCE(research_lab, 0) AS research_lab,
            COALESCE(nanite_factory, 0) AS nanite_factory,
            COALESCE(terraformer, 0) AS terraformer,
            COALESCE(missile_silo, 0) AS missile_silo,
            COALESCE(alliance_depot, 0) AS alliance_depot,
            COALESCE(space_dock, 0) AS space_dock,
            COALESCE(small_cargo, 0)::BIGINT AS small_cargo,
            COALESCE(large_cargo, 0)::BIGINT AS large_cargo,
            COALESCE(light_fighter, 0)::BIGINT AS light_fighter,
            COALESCE(heavy_fighter, 0)::BIGINT AS heavy_fighter,
            COALESCE(cruiser, 0)::BIGINT AS cruiser,
            COALESCE(battleship, 0)::BIGINT AS battleship,
            COALESCE(battlecruiser, 0)::BIGINT AS battlecruiser,
            COALESCE(bomber, 0)::BIGINT AS bomber,
            COALESCE(destroyer, 0)::BIGINT AS destroyer,
            COALESCE(deathstar, 0)::BIGINT AS deathstar,
            COALESCE(recycler, 0)::BIGINT AS recycler,
            COALESCE(espionage_probe, 0)::BIGINT AS espionage_probe,
            COALESCE(solar_satellite, 0)::BIGINT AS solar_satellite,
            COALESCE(colony_ship, 0)::BIGINT AS colony_ship
     FROM planets"
}

fn research_select_sql() -> String {
    let levels = research_columns()
        .into_iter()
        .map(|(name, column)| format!("COALESCE({column}, 0) AS {name}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("SELECT user_id::TEXT AS user_id, {levels} FROM research WHERE user_id = $1")
}

fn map_planet_row(row: &tokio_postgres::Row) -> GameplayPlanetRow {
    let buildings = building_columns()
        .into_iter()
        .map(|(name, column)| (name.to_string(), row.get::<_, i32>(column)))
        .collect();
    let ships = ship_columns()
        .into_iter()
        .map(|(name, column)| (name.to_string(), row.get::<_, i64>(column)))
        .collect();
    GameplayPlanetRow {
        id: row.get("id"),
        user_id: row.get("user_id"),
        universe_id: row.get("universe_id"),
        name: row.get("name"),
        galaxy: row.get("galaxy"),
        system: row.get("system"),
        position: row.get("position"),
        temperature: row.get("temperature"),
        metal: row.get("metal"),
        crystal: row.get("crystal"),
        deuterium: row.get("deuterium"),
        energy: row.get("energy"),
        buildings,
        ships,
    }
}

fn map_research_row(row: &tokio_postgres::Row) -> GameplayResearchRow {
    let levels = research_columns()
        .into_iter()
        .map(|(name, _)| (name.to_string(), row.get::<_, i32>(name)))
        .collect();
    GameplayResearchRow {
        user_id: row.get("user_id"),
        levels,
    }
}

fn map_queue_row(row: &tokio_postgres::Row) -> GameplayQueueRow {
    GameplayQueueRow {
        id: row.get("id"),
        planet_id: row.get("planet_id"),
        item_type: row.get("item_type"),
        target_level: row.get("target_level"),
        quantity: row.get("quantity"),
        finishes_in_seconds: row.get("finishes_in_seconds"),
        status: row.get("status"),
    }
}

fn map_account_row(row: &tokio_postgres::Row) -> AccountRow {
    AccountRow {
        id: row.get("id"),
        username: row.get("username"),
        email: row.get("email"),
        password_hash: row.get("password_hash"),
        role: row.get("role"),
        universe_id: row.get("universe_id"),
        is_banned: row.get("is_banned"),
    }
}

fn parse_optional_id(value: &str) -> Option<i32> {
    value.parse::<i32>().ok()
}

fn parse_id(value: &str) -> Result<i32, GameplayWriteError> {
    value
        .parse::<i32>()
        .map_err(|_| GameplayWriteError::NotFound)
}

fn lookup_column(
    entries: &[(&'static str, &'static str)],
    item_type: &str,
) -> Option<&'static str> {
    entries
        .iter()
        .find_map(|(name, column)| (*name == item_type).then_some(*column))
}

pub fn building_columns() -> [(&'static str, &'static str); 16] {
    [
        ("metal_mine", "metal_mine"),
        ("crystal_mine", "crystal_mine"),
        ("deuterium_synthesizer", "deuterium_synthesizer"),
        ("solar_plant", "solar_plant"),
        ("fusion_reactor", "fusion_reactor"),
        ("metal_storage", "metal_storage"),
        ("crystal_storage", "crystal_storage"),
        ("deuterium_tank", "deuterium_tank"),
        ("robotics_factory", "robotics_factory"),
        ("shipyard", "shipyard"),
        ("research_lab", "research_lab"),
        ("nanite_factory", "nanite_factory"),
        ("terraformer", "terraformer"),
        ("missile_silo", "missile_silo"),
        ("alliance_depot", "alliance_depot"),
        ("space_dock", "space_dock"),
    ]
}

pub fn research_columns() -> [(&'static str, &'static str); 16] {
    [
        ("energy_technology", "energy_technology"),
        ("laser_technology", "laser_technology"),
        ("ion_technology", "ion_technology"),
        ("hyperspace_technology", "hyperspace_technology"),
        ("plasma_technology", "plasma_technology"),
        ("combustion_drive", "combustion_drive"),
        ("impulse_drive", "impulse_drive"),
        ("hyperspace_drive", "hyperspace_drive"),
        ("espionage_technology", "espionage_technology"),
        ("computer_technology", "computer_technology"),
        ("astrophysics", "astrophysics"),
        (
            "intergalactic_research_network",
            "intergalactic_research_network",
        ),
        ("graviton_technology", "graviton_technology"),
        ("weapons_technology", "weapons_technology"),
        ("shielding_technology", "shielding_technology"),
        ("armour_technology", "armor_technology"),
    ]
}

pub fn ship_columns() -> [(&'static str, &'static str); 14] {
    [
        ("small_cargo", "small_cargo"),
        ("large_cargo", "large_cargo"),
        ("light_fighter", "light_fighter"),
        ("heavy_fighter", "heavy_fighter"),
        ("cruiser", "cruiser"),
        ("battleship", "battleship"),
        ("battlecruiser", "battlecruiser"),
        ("bomber", "bomber"),
        ("destroyer", "destroyer"),
        ("deathstar", "deathstar"),
        ("recycler", "recycler"),
        ("espionage_probe", "espionage_probe"),
        ("solar_satellite", "solar_satellite"),
        ("colony_ship", "colony_ship"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn canonical_column_maps_are_complete_and_unique() {
        for entries in [
            building_columns().to_vec(),
            research_columns().to_vec(),
            ship_columns().to_vec(),
        ] {
            let names = entries.iter().map(|entry| entry.0).collect::<BTreeSet<_>>();
            let columns = entries.iter().map(|entry| entry.1).collect::<BTreeSet<_>>();
            assert_eq!(names.len(), entries.len());
            assert_eq!(columns.len(), entries.len());
        }
        assert_eq!(building_columns().len(), 16);
        assert_eq!(research_columns().len(), 16);
        assert_eq!(ship_columns().len(), 14);
    }

    #[test]
    fn read_queries_cover_every_canonical_column() {
        let planet_sql = planet_select_sql();
        for (_, column) in building_columns().into_iter().chain(ship_columns()) {
            assert!(
                planet_sql.contains(column),
                "missing planet column {column}"
            );
        }
        let research_sql = research_select_sql();
        for (_, column) in research_columns() {
            assert!(
                research_sql.contains(column),
                "missing research column {column}"
            );
        }
    }

    #[test]
    fn queue_validation_rejects_negative_costs_and_unbounded_duration() {
        let mut input = GameplayQueueInput {
            user_id: "1".to_string(),
            planet_id: "1".to_string(),
            item_type: "metal_mine".to_string(),
            target_level: Some(1),
            quantity: None,
            metal_cost: -1,
            crystal_cost: 0,
            deuterium_cost: 0,
            energy_required: 0,
            duration_seconds: 1,
        };
        assert!(matches!(
            validate_queue_input(&input),
            Err(GameplayWriteError::Invalid(_))
        ));
        input.metal_cost = 0;
        input.duration_seconds = MAX_QUEUE_DURATION_SECONDS + 1;
        assert!(matches!(
            validate_queue_input(&input),
            Err(GameplayWriteError::Invalid(_))
        ));
    }

    #[test]
    fn retry_contract_only_marks_rolled_back_transaction_conflicts() {
        assert!(GameplayWriteError::Retryable("serialization".to_string()).is_retryable());
        assert!(!GameplayWriteError::UniverseFull.is_retryable());
        assert!(!GameplayWriteError::InsufficientResources.is_retryable());
    }
}
