-- 46_status_page.sql
-- Migration: Add status page incidents and maintenance windows tables

CREATE TABLE IF NOT EXISTS status_incidents (
  id SERIAL PRIMARY KEY,
  title TEXT NOT NULL,
  description TEXT,
  status VARCHAR(32) NOT NULL DEFAULT 'detected',
  severity VARCHAR(16) NOT NULL DEFAULT 'medium',
  affected_components JSONB NOT NULL DEFAULT '[]',
  start_time TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  end_time TIMESTAMP WITH TIME ZONE,
  created_by INTEGER,
  created_by_username TEXT,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS status_maintenance_windows (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  start_time TIMESTAMP WITH TIME ZONE NOT NULL,
  end_time TIMESTAMP WITH TIME ZONE NOT NULL,
  created_by INTEGER,
  created_by_username TEXT,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

-- Index for quick active incident lookup
CREATE INDEX IF NOT EXISTS idx_status_incidents_active ON status_incidents (start_time, end_time, status);

-- Index for maintenance windows
CREATE INDEX IF NOT EXISTS idx_status_maintenance_active ON status_maintenance_windows (start_time, end_time);
