#!/bin/bash

echo "Starting PostgreSQL..."

# Create run directory with proper permissions
mkdir -p /var/run/postgresql
chown -R postgres:postgres /var/run/postgresql 2>/dev/null || true

# Start PostgreSQL
su - postgres -c "pg_ctlcluster 15 main start" 2>&1

# Wait for startup
sleep 3

# Check status
echo ""
echo "PostgreSQL Status:"
pg_isready -h 127.0.0.1 -p 5432

echo ""
echo "Process Check:"
ps aux | grep postgres | grep -v grep || echo "No PostgreSQL processes found"
