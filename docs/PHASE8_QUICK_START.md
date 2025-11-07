# Phase 8: Seasonal Theme System - Quick Start Guide

## 🚀 Backend Integration (5 Minutes)

### Step 1: Import Database Schema

```bash
# Connect to your PostgreSQL database
psql -U your_username -d universus_db

# Execute the schema file
\i backend/src/database/phase8_seasonal_themes_schema.sql

# Verify tables were created
\dt theme*
```

**Expected Output**:
```
 themes
 theme_schedules
 theme_assets
 theme_configurations
 theme_activations
 theme_preferences
```

### Step 2: Integrate Routes into Express Server

**Find your main server file** (e.g., `backend/src/index.ts` or `backend/src/server.ts`)

**Add these imports at the top**:
```typescript
import themeRoutes from './routes/themeRoutes';
import { themeScheduler } from './services/themeScheduler';
```

**Register the routes** (after other routes):
```typescript
// Theme system routes
app.use('/api/themes', themeRoutes);
```

**Start the scheduler** (before `app.listen()`):
```typescript
// Start theme scheduler (checks every minute)
themeScheduler.start();
console.log('✅ Theme scheduler started');
```

**Add graceful shutdown** (at the end of file):
```typescript
process.on('SIGTERM', () => {
    console.log('Shutting down gracefully...');
    themeScheduler.stop();
    // ... your other cleanup code
    process.exit(0);
});
```

### Step 3: Compile and Restart

```bash
cd backend
npm run build
# or
pnpm run build

# Restart your server
npm start
# or
pm2 restart universus
```

## ✅ Verification

### Test Theme API

```bash
# Get current active theme
curl http://localhost:3000/api/themes/current

# List all themes
curl http://localhost:3000/api/themes

# Get theme by key
curl http://localhost:3000/api/themes/key/christmas
```

### Expected Response (Current Theme):
```json
{
  "success": true,
  "theme": {
    "id": 1,
    "theme_key": "christmas",
    "name": "Christmas",
    "primary_color": "#c41e3a",
    "is_active": true,
    "visual_effects": { ... },
    "sound_effects": { ... }
  },
  "assets": [],
  "cssVariables": {
    "--theme-primary": "#c41e3a",
    "--theme-secondary": "#165b33",
    "--theme-accent": "#ffd700"
  }
}
```

### Check Scheduler Logs

Look for this in your server logs:
```
[ThemeScheduler] Starting theme scheduler (interval: 60000ms)
✅ Theme scheduler started
```

## 🎨 Frontend Integration (Quick Demo)

### Minimal Theme Loader

Create `frontend/js/themeLoader.js`:

```javascript
class ThemeLoader {
    constructor() {
        this.currentTheme = null;
    }

    async init() {
        await this.loadCurrentTheme();
        // Reload every 5 minutes to check for theme changes
        setInterval(() => this.loadCurrentTheme(), 5 * 60 * 1000);
    }

    async loadCurrentTheme() {
        try {
            const response = await fetch('/api/themes/current');
            const data = await response.json();

            if (data.success && data.theme) {
                this.applyTheme(data.theme, data.cssVariables);
            }
        } catch (error) {
            console.error('Failed to load theme:', error);
        }
    }

    applyTheme(theme, cssVariables) {
        // Apply CSS variables
        if (cssVariables) {
            Object.entries(cssVariables).forEach(([key, value]) => {
                document.documentElement.style.setProperty(key, value);
            });
        }

        // Store current theme
        this.currentTheme = theme;

        // Emit event for other components
        window.dispatchEvent(new CustomEvent('themeChanged', { 
            detail: { theme } 
        }));

        console.log(`Theme applied: ${theme.name}`);
    }
}

// Initialize on page load
const themeLoader = new ThemeLoader();
document.addEventListener('DOMContentLoaded', () => {
    themeLoader.init();
});
```

### Add to Your HTML

```html
<!-- In your base template (e.g., frontend/views/layouts/base.njk) -->
<script src="/js/themeLoader.js"></script>
```

### Use CSS Variables in Your Styles

```css
/* Your existing CSS can now use theme variables */
.primary-button {
    background-color: var(--theme-primary, #4a5568);
    color: var(--theme-text, #ffffff);
}

.accent-border {
    border-color: var(--theme-accent, #ffd700);
}

.page-background {
    background-color: var(--theme-background, #1a1f2e);
}
```

## 🔧 Admin Operations

### Manually Activate a Theme

```bash
# Get your admin JWT token first
TOKEN="your_admin_jwt_token"

# Activate Christmas theme
curl -X POST http://localhost:3000/api/themes/1/activate \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"reason": "Testing Christmas theme"}'
```

### Create a Schedule

```bash
curl -X POST http://localhost:3000/api/themes/admin/schedules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "theme_id": 1,
    "schedule_name": "Test Christmas",
    "start_date": "2025-12-15",
    "end_date": "2025-12-25",
    "priority": 90,
    "is_recurring": true
  }'
```

## 📊 Monitoring

### Check Active Schedules

```sql
-- In PostgreSQL
SELECT * FROM v_active_theme_schedules;
```

### View Theme Analytics

```bash
curl http://localhost:3000/api/themes/1/analytics \
  -H "Authorization: Bearer $TOKEN"
```

## 🎯 Testing Checklist

- [ ] Database schema loaded successfully
- [ ] Server starts without errors
- [ ] Theme scheduler logs appear
- [ ] `/api/themes/current` returns data
- [ ] `/api/themes` lists all themes
- [ ] CSS variables applied in browser
- [ ] Manual theme activation works
- [ ] Schedule creation works

## 🚨 Troubleshooting

### "Theme scheduler not starting"
**Check**: Make sure you called `themeScheduler.start()` in your server file

### "No themes returned"
**Check**: Run the SQL schema file to seed default themes

### "401 Unauthorized on admin endpoints"
**Check**: You need a valid admin JWT token for admin operations

### "TypeError: Cannot read property 'theme'"
**Check**: Frontend themeLoader.js is loaded and initialized

## 📝 Default Theme Schedule

By default, themes activate automatically on these dates:

- **Christmas**: December 1-31 (Priority: 90)
- **Halloween**: October 20 - November 2 (Priority: 85)
- **Easter**: April 1-21 (Priority: 80)
- **New Year**: December 31 - January 2 (Priority: 100)

The scheduler checks every minute and automatically switches themes.

## 🎉 Success!

If you can:
1. ✅ Get theme data from `/api/themes/current`
2. ✅ See CSS variables applied in DevTools
3. ✅ See scheduler logs in console

**Your backend is fully operational!**

Next: Implement visual effects and admin UI for full functionality.

---

**Need Help?**
- Check logs: `backend/logs/server.log`
- Database issues: Verify connection and schema
- API issues: Check authentication middleware
- Frontend issues: Check browser console

**Documentation**: See `PHASE8_SEASONAL_THEMES_IMPLEMENTATION.md` for complete details.
