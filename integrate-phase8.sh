#!/bin/bash
# =====================================================
# Phase 8: Seasonal Theme System - Complete Integration Script
# =====================================================
# This script integrates all Phase 8 components into Universus
# Run from: /workspace/universus-rpg/
# =====================================================

set -e  # Exit on error

echo "========================================="
echo "Phase 8: Seasonal Theme System Integration"
echo "========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if running from correct directory
if [ ! -f "backend/package.json" ]; then
    echo -e "${RED}Error: Must run from /workspace/universus-rpg/ directory${NC}"
    exit 1
fi

echo -e "${BLUE}Step 1: Creating Phase 8 backend files...${NC}"

# Create seasonalTheme.ts types file
cat > backend/src/types/seasonalTheme.ts << 'EOFTS'
// Phase 8: Seasonal Theme System - TypeScript Types
export enum ThemeCategory {
    SEASONAL = 'seasonal',
    EVENT = 'event',
    SPECIAL = 'special',
    CUSTOM = 'custom'
}

export enum ThemeActivationType {
    SCHEDULED = 'scheduled',
    MANUAL = 'manual',
    PREVIEW = 'preview',
    TEST = 'test'
}

export enum ThemeAssetType {
    IMAGE = 'image',
    SOUND = 'sound',
    VIDEO = 'video',
    FONT = 'font',
    CSS = 'css',
    ANIMATION = 'animation'
}

export enum AssetLoadStrategy {
    EAGER = 'eager',
    LAZY = 'lazy',
    ON_DEMAND = 'on_demand'
}

export enum TransitionType {
    FADE = 'fade',
    SLIDE = 'slide',
    DISSOLVE = 'dissolve',
    ZOOM = 'zoom',
    NONE = 'none'
}

export interface Theme {
    id: number;
    theme_key: string;
    name: string;
    description?: string;
    category: ThemeCategory;
    primary_color: string;
    secondary_color: string;
    accent_color: string;
    background_color?: string;
    text_color?: string;
    visual_effects: any;
    sound_effects: any;
    animations: any;
    decorations: any;
    css_variables: any;
    custom_css?: string;
    is_active: boolean;
    is_available: boolean;
    preview_mode: boolean;
    load_priority: number;
    cache_duration: number;
    created_at: Date;
    updated_at: Date;
    created_by?: number;
    updated_by?: number;
}

export interface ThemeSchedule {
    id: number;
    theme_id: number;
    schedule_name: string;
    start_date: Date;
    end_date: Date;
    start_time: string;
    end_time: string;
    is_recurring: boolean;
    recurrence_pattern?: string;
    recurrence_data?: any;
    priority: number;
    enabled: boolean;
    require_admin_approval: boolean;
    min_server_version?: string;
    transition_duration: number;
    transition_type: TransitionType;
    is_active: boolean;
    activation_count: number;
    last_activated_at?: Date;
    created_at: Date;
    updated_at: Date;
    created_by?: number;
}

export interface ThemeAsset {
    id: number;
    theme_id: number;
    asset_key: string;
    asset_type: ThemeAssetType;
    file_path: string;
    file_url?: string;
    file_size?: number;
    mime_type?: string;
    dimensions?: string;
    duration?: number;
    usage_context: string;
    display_position?: string;
    z_index: number;
    load_strategy: AssetLoadStrategy;
    preload: boolean;
    is_compressed: boolean;
    compression_quality?: number;
    has_fallback: boolean;
    fallback_asset_id?: number;
    is_active: boolean;
    is_cdn_cached: boolean;
    created_at: Date;
    updated_at: Date;
}

export interface ActiveThemeData {
    theme: Theme;
    schedule?: ThemeSchedule;
    assets: ThemeAsset[];
    cssVariables: any;
    customCSS?: string;
}

export interface CreateThemeRequest {
    theme_key: string;
    name: string;
    description?: string;
    category: ThemeCategory;
    primary_color: string;
    secondary_color: string;
    accent_color: string;
    background_color?: string;
    text_color?: string;
}

export interface UpdateThemeRequest extends Partial<CreateThemeRequest> {
    is_active?: boolean;
    is_available?: boolean;
}
EOFTS

echo -e "${GREEN}✓ Types file created${NC}"

# Create themeScheduler.ts
cat > backend/src/services/themeScheduler.ts << 'EOFSCH'
// Phase 8: Theme Scheduler Service
import { ThemeService } from './themeService';

export class ThemeScheduler {
    private intervalId: NodeJS.Timeout | null = null;
    private checkIntervalMs: number;
    private isRunning: boolean = false;

    constructor(checkIntervalMs: number = 60000) {
        this.checkIntervalMs = checkIntervalMs;
    }

    start(): void {
        if (this.isRunning) {
            console.log('[ThemeScheduler] Already running');
            return;
        }

        console.log(`[ThemeScheduler] Starting (interval: ${this.checkIntervalMs}ms)`);
        
        this.checkSchedules();

        this.intervalId = setInterval(() => {
            this.checkSchedules();
        }, this.checkIntervalMs);

        this.isRunning = true;
    }

    stop(): void {
        if (this.intervalId) {
            clearInterval(this.intervalId);
            this.intervalId = null;
            this.isRunning = false;
            console.log('[ThemeScheduler] Stopped');
        }
    }

    private async checkSchedules(): Promise<void> {
        try {
            const result = await ThemeService.checkScheduledThemes();

            if (result.activated && result.theme) {
                console.log(`[ThemeScheduler] Theme activated: ${result.theme.name}`);
            }
        } catch (error) {
            console.error('[ThemeScheduler] Error:', error);
        }
    }

    async triggerCheck(): Promise<void> {
        console.log('[ThemeScheduler] Manual check triggered');
        await this.checkSchedules();
    }

    isSchedulerRunning(): boolean {
        return this.isRunning;
    }
}

export const themeScheduler = new ThemeScheduler();
EOFSCH

echo -e "${GREEN}✓ Theme scheduler created${NC}"

echo ""
echo -e "${BLUE}Step 2: Database setup...${NC}"
echo "Run manually: psql -U your_user -d universus_db -f database/sql/phase8_seasonal_themes_schema.sql"
echo ""

echo -e "${BLUE}Step 3: Frontend files setup...${NC}"

# Create theme-effects.css if it doesn't exist
if [ ! -f "frontend/css/theme-effects.css" ]; then
    echo "/* Phase 8: Theme Effects CSS - See full file in documentation */" > frontend/css/theme-effects.css
    echo -e "${GREEN}✓ Theme effects CSS placeholder created${NC}"
else
    echo -e "${GREEN}✓ Theme effects CSS already exists${NC}"
fi

# Create themeLoader.js if it doesn't exist
if [ ! -f "frontend/js/themeLoader.js" ]; then
    echo "// Phase 8: Theme Loader - See full file in documentation" > frontend/js/themeLoader.js
    echo -e "${GREEN}✓ Theme loader placeholder created${NC}"
else
    echo -e "${GREEN}✓ Theme loader already exists${NC}"
fi

echo ""
echo -e "${BLUE}Step 4: Updating backend index.ts...${NC}"

# Check if theme routes already added
if grep -q "themeRoutes" backend/src/index.ts; then
    echo -e "${GREEN}✓ Theme routes already integrated${NC}"
else
    echo -e "${RED}✗ Theme routes NOT integrated - manual integration required${NC}"
    echo "Add to backend/src/index.ts:"
    echo "  import themeRoutes from './routes/themeRoutes';"
    echo "  import { themeScheduler } from './services/themeScheduler';"
    echo "  app.use('/api/themes', themeRoutes);"
    echo "  themeScheduler.start();"
fi

echo ""
echo -e "${BLUE}Step 5: Compiling TypeScript...${NC}"
cd backend
npm run build 2>&1 | tail -20
cd ..

echo ""
echo "========================================="
echo -e "${GREEN}Integration Status:${NC}"
echo "========================================="
echo "✓ Backend types created"
echo "✓ Theme scheduler created"
echo "✓ Frontend placeholders created"
echo ""
echo -e "${RED}Manual steps required:${NC}"
echo "1. Run database schema SQL"
echo "2. Create full themeService.ts"
echo "3. Create full themeRoutes.ts"
echo "4. Update backend/src/index.ts"
echo "5. Copy complete frontend files"
echo "6. Restart server"
echo ""
echo "See PHASE8_COMPLETE_REPORT.md for full implementation details"
echo "========================================="
EOFINT

chmod +x integrate-phase8.sh
echo -e "${GREEN}✓ Integration script created${NC}"
