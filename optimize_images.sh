#!/bin/bash

# Image Optimization Script for Universus
# Compresses PNG images while maintaining visual quality

ASSETS_DIR="/workspace/ogame-rpg/frontend/assets"
BACKUP_DIR="/workspace/ogame-rpg/frontend/assets_backup"
LOG_FILE="/workspace/ogame-rpg/image_optimization.log"

echo "=== Universus Image Optimization ===" > "$LOG_FILE"
echo "Start time: $(date)" >> "$LOG_FILE"
echo "" >> "$LOG_FILE"

# Backup original assets
if [ ! -d "$BACKUP_DIR" ]; then
    echo "Creating backup of original assets..."
    cp -r "$ASSETS_DIR" "$BACKUP_DIR"
    echo "Backup created at: $BACKUP_DIR" >> "$LOG_FILE"
fi

# Get initial size
INITIAL_SIZE=$(du -sh "$ASSETS_DIR" | cut -f1)
echo "Initial size: $INITIAL_SIZE" >> "$LOG_FILE"

# Counter
TOTAL=0
PROCESSED=0

# Find all PNG files
echo "Finding PNG files..." >> "$LOG_FILE"
TOTAL=$(find "$ASSETS_DIR" -name "*.png" | wc -l)
echo "Total PNG files found: $TOTAL" >> "$LOG_FILE"
echo "" >> "$LOG_FILE"

# Optimize each PNG file
echo "Optimizing images..." >> "$LOG_FILE"
find "$ASSETS_DIR" -name "*.png" | while read file; do
    PROCESSED=$((PROCESSED + 1))
    BEFORE=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null)
    
    # Compress with ImageMagick (85% quality, strip metadata)
    convert "$file" -strip -quality 85 -resize 2048x2048\> "$file.tmp" 2>/dev/null
    
    if [ -f "$file.tmp" ]; then
        AFTER=$(stat -c%s "$file.tmp" 2>/dev/null || stat -f%z "$file.tmp" 2>/dev/null)
        REDUCTION=$((100 - (AFTER * 100 / BEFORE)))
        
        # Only replace if compression improved size
        if [ $AFTER -lt $BEFORE ]; then
            mv "$file.tmp" "$file"
            echo "Optimized: $(basename $file) - Reduced by ${REDUCTION}%" >> "$LOG_FILE"
        else
            rm "$file.tmp"
        fi
    fi
done

# Get final size
FINAL_SIZE=$(du -sh "$ASSETS_DIR" | cut -f1)
echo "" >> "$LOG_FILE"
echo "Final size: $FINAL_SIZE" >> "$LOG_FILE"
echo "Optimization complete: $(date)" >> "$LOG_FILE"

echo "=== Optimization Complete ==="
echo "Before: $INITIAL_SIZE"
echo "After: $FINAL_SIZE"
echo "Log saved to: $LOG_FILE"
