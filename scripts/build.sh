#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if command -v git >/dev/null 2>&1; then
    REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
else
    REPO_ROOT="$SCRIPT_DIR"
    while [ "$REPO_ROOT" != "/" ] && [ ! -f "$REPO_ROOT/docker-compose.yml" ]; do
        REPO_ROOT="$(cd "$REPO_ROOT/.." && pwd)"
    done
fi

echo "Building backend..."
cd "$REPO_ROOT/backend"
pnpm run build

if [ $? -eq 0 ]; then
    echo "Build successful!"
    echo "Backend compiled to dist/"
else
    echo "Build failed!"
    exit 1
fi
