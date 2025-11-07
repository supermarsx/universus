#!/bin/bash

echo "=========================================="
echo "SpaceEmpire - Universus-Inspired Browser RPG"
echo "=========================================="
echo ""

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if command -v git >/dev/null 2>&1; then
    REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
else
    REPO_ROOT="$SCRIPT_DIR"
    while [ "$REPO_ROOT" != "/" ] && [ ! -f "$REPO_ROOT/docker-compose.yml" ]; do
        REPO_ROOT="$(cd "$REPO_ROOT/.." && pwd)"
    done
fi
cd "$REPO_ROOT"

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "Error: Docker is not installed"
    echo "Please install Docker and Docker Compose first"
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo "Error: Docker Compose is not installed"
    echo "Please install Docker Compose first"
    exit 1
fi

echo "Starting services with Docker Compose..."
echo ""

# Build and start containers
docker-compose up --build -d

# Wait for services to be ready
echo "Waiting for services to start..."
sleep 10

# Check if services are running
if docker-compose ps | grep -q "Up"; then
    echo ""
    echo "=========================================="
    echo "Services started successfully!"
    echo "=========================================="
    echo ""
    echo "Backend-rendered game UI: http://localhost:3000"
    echo "Standalone frontend bundle: http://localhost:8080"
    echo ""
    echo "To view logs: docker-compose logs -f"
    echo "To stop services: docker-compose down"
    echo ""
else
    echo "Error: Services failed to start"
    echo "Run 'docker-compose logs' to see error details"
    exit 1
fi
