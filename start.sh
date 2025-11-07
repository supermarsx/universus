#!/bin/bash

echo "=========================================="
echo "SpaceEmpire - OGame-Inspired Browser RPG"
echo "=========================================="
echo ""

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
    echo "Game is available at: http://localhost:3000"
    echo ""
    echo "To view logs: docker-compose logs -f"
    echo "To stop services: docker-compose down"
    echo ""
else
    echo "Error: Services failed to start"
    echo "Run 'docker-compose logs' to see error details"
    exit 1
fi
