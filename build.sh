#!/bin/bash

echo "Building backend..."
cd backend
pnpm run build

if [ $? -eq 0 ]; then
    echo "Build successful!"
    echo "Backend compiled to dist/"
else
    echo "Build failed!"
    exit 1
fi
