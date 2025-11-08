-- Migration 29: Persist leaderboard snapshots for SQL access

BEGIN;

CREATE TABLE IF NOT EXISTS player_leaderboard_snapshots (
    snapshot_at TIMESTAMP NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    username VARCHAR(64) NOT NULL,
    total_score BIGINT NOT NULL,
    building_score BIGINT NOT NULL,
    research_score BIGINT NOT NULL,
    fleet_score BIGINT NOT NULL,
    defense_score BIGINT NOT NULL,
    alliance_tag VARCHAR(16),
    PRIMARY KEY (snapshot_at, user_id)
);

CREATE INDEX IF NOT EXISTS idx_player_leaderboard_latest
    ON player_leaderboard_snapshots(user_id, snapshot_at DESC);

CREATE VIEW v_player_leaderboard AS
SELECT DISTINCT ON (pls.user_id)
    pls.user_id,
    pls.username,
    pls.total_score,
    pls.building_score,
    pls.research_score,
    pls.fleet_score,
    pls.defense_score,
    pls.alliance_tag,
    pls.snapshot_at
FROM player_leaderboard_snapshots pls
ORDER BY pls.user_id, pls.snapshot_at DESC;

CREATE TABLE IF NOT EXISTS alliance_leaderboard_snapshots (
    snapshot_at TIMESTAMP NOT NULL,
    alliance_id INTEGER NOT NULL REFERENCES alliances(id) ON DELETE CASCADE,
    alliance_name VARCHAR(100) NOT NULL,
    alliance_tag VARCHAR(16) NOT NULL,
    total_score BIGINT NOT NULL,
    member_count INTEGER NOT NULL,
    average_score BIGINT NOT NULL,
    PRIMARY KEY (snapshot_at, alliance_id)
);

CREATE INDEX IF NOT EXISTS idx_alliance_leaderboard_latest
    ON alliance_leaderboard_snapshots(alliance_id, snapshot_at DESC);

CREATE VIEW v_alliance_leaderboard AS
SELECT DISTINCT ON (als.alliance_id)
    als.alliance_id,
    als.alliance_name,
    als.alliance_tag,
    als.total_score,
    als.member_count,
    als.average_score,
    als.snapshot_at
FROM alliance_leaderboard_snapshots als
ORDER BY als.alliance_id, als.snapshot_at DESC;

COMMIT;
