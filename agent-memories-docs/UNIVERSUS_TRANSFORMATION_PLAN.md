# UNIVERSUS TRANSFORMATION PROJECT

## Project Overview
Comprehensive transformation of SpaceEmpire RPG into "Universus" - a modern, templated, CMS-managed, visually stunning space empire game.

**Start Date:** 2025-11-06  
**Current Status:** Planning Complete - Implementation Starting

---

## PHASE 1: TEMPLATING ENGINE IMPLEMENTATION

### 1.1 Setup Nunjucks
- [x] Install Nunjucks and dependencies
- [ ] Configure Express middleware for Nunjucks
- [ ] Create views directory structure
- [ ] Set up template configuration

### 1.2 Create Base Templates
- [ ] `layouts/base.njk` - Master layout template
- [ ] `layouts/game.njk` - Game page layout (extends base)
- [ ] `partials/nav.njk` - Top navigation component
- [ ] `partials/sidebar.njk` - Sidebar navigation component
- [ ] `partials/footer.njk` - Footer component
- [ ] `partials/resource-display.njk` - Resource counter component

### 1.3 Convert HTML Pages to Templates
- [ ] `pages/index.njk` - Login/home page
- [ ] `pages/overview.njk` - Empire overview
- [ ] `pages/buildings.njk` - Buildings management
- [ ] `pages/research.njk` - Research technologies
- [ ] `pages/shipyard.njk` - Shipyard/fleet construction
- [ ] `pages/fleet.njk` - Fleet management
- [ ] `pages/galaxy.njk` - Galaxy exploration
- [ ] `pages/leaderboard.njk` - Rankings
- [ ] `pages/messages.njk` - Message inbox
- [ ] `pages/shop.njk` - In-game shop
- [ ] `pages/admin.njk` - Admin panel
- [ ] `pages/admin/bots.njk` - Bot management

### 1.4 Template Routes
- [ ] Create template rendering routes
- [ ] Update Express index.ts for template serving
- [ ] Implement context data injection
- [ ] Add template helpers/filters

**Estimated Time:** 6-8 hours  
**Lines of Code:** ~2,000

---

## PHASE 2: CMS MANAGEMENT SYSTEM

### 2.1 Database Schema Design
- [ ] `cms_pages` - Page metadata and configurations
- [ ] `cms_content_blocks` - Reusable content blocks
- [ ] `cms_navigation` - Navigation menu structures
- [ ] `cms_themes` - Visual themes configuration
- [ ] `cms_page_layouts` - Page layout templates
- [ ] `cms_assets` - Asset management table (for Phase 6)

### 2.2 CMS Backend Services
- [ ] PageService - CRUD operations for pages
- [ ] ContentBlockService - Content block management
- [ ] NavigationService - Menu structure management
- [ ] ThemeService - Theme configuration management
- [ ] LayoutService - Layout template management

### 2.3 CMS API Endpoints
- [ ] GET/POST/PUT/DELETE `/api/cms/pages`
- [ ] GET/POST/PUT/DELETE `/api/cms/content-blocks`
- [ ] GET/POST/PUT/DELETE `/api/cms/navigation`
- [ ] GET/POST/PUT/DELETE `/api/cms/themes`
- [ ] GET/POST/PUT/DELETE `/api/cms/layouts`

### 2.4 CMS Admin Interface
- [ ] `pages/cms/dashboard.njk` - CMS dashboard
- [ ] `pages/cms/pages-manager.njk` - Page management UI
- [ ] `pages/cms/content-editor.njk` - Content block editor
- [ ] `pages/cms/navigation-editor.njk` - Navigation editor
- [ ] `pages/cms/theme-editor.njk` - Theme customization
- [ ] `js/cms-admin.js` - CMS admin JavaScript

**Estimated Time:** 10-12 hours  
**Lines of Code:** ~3,500

---

## PHASE 3: SPECIFICATION COMPLIANCE REVIEW

### 3.1 Universus Specification Audit
- [ ] Review original Universus building specifications
- [ ] Review original Universus research tree
- [ ] Review original Universus ship/defense types
- [ ] Review original Universus fleet mechanics
- [ ] Review original Universus alliance features
- [ ] Review original Universus combat system

### 3.2 Missing Features Implementation
- [ ] Additional building types (if missing)
- [ ] Missing research technologies
- [ ] Missing ship/defense types
- [ ] Enhanced combat mechanics
- [ ] Alliance diplomacy features
- [ ] Galaxy exploration enhancements

### 3.3 Compliance Documentation
- [ ] Create specification compliance checklist
- [ ] Document all implemented features
- [ ] Note intentional deviations

**Estimated Time:** 4-6 hours  
**Lines of Code:** ~1,500

---

## PHASE 4: COMPLETE REBRAND TO "UNIVERSUS"

### 4.1 Text Content Updates
- [ ] Replace all "SpaceEmpire" references
- [ ] Replace all "Universus" references
- [ ] Update page titles and metadata
- [ ] Update console logs and error messages

### 4.2 Configuration Updates
- [ ] package.json name and description
- [ ] Database name and references
- [ ] Environment variable names
- [ ] README and documentation files

### 4.3 Visual Identity
- [ ] Update logo/branding elements
- [ ] Update color scheme variables
- [ ] Update typography choices
- [ ] Create Universus style guide

**Estimated Time:** 3-4 hours  
**Files Modified:** ~50+

---

## PHASE 5: VISUAL ASSET GENERATION ENGINE

### 5.1 Asset Categories & Generation
**Total Assets Target: 200+**

#### 5.1.1 Planet Backgrounds (50+ assets)
- [ ] Terrestrial planets (10 variations)
- [ ] Gas giants (8 variations)
- [ ] Ice worlds (6 variations)
- [ ] Desert planets (6 variations)
- [ ] Lava planets (5 variations)
- [ ] Metal planets (5 variations)
- [ ] Artificial/station worlds (5 variations)
- [ ] Exotic/unique planets (5 variations)

#### 5.1.2 Spacecraft Designs (30+ assets)
- [ ] Light fighters (3 variations)
- [ ] Heavy fighters (3 variations)
- [ ] Cruisers (4 variations)
- [ ] Battleships (4 variations)
- [ ] Dreadnoughts (3 variations)
- [ ] Cargo ships (3 variations)
- [ ] Colony ships (2 variations)
- [ ] Recyclers (2 variations)
- [ ] Espionage probes (2 variations)
- [ ] Bombers (2 variations)
- [ ] Destroyers (2 variations)

#### 5.1.3 Planetary Structures (40+ assets)
- [ ] Metal mines (4 variations)
- [ ] Crystal mines (4 variations)
- [ ] Deuterium synthesizers (4 variations)
- [ ] Solar power plants (4 variations)
- [ ] Fusion reactors (4 variations)
- [ ] Research labs (4 variations)
- [ ] Shipyards (4 variations)
- [ ] Defense systems (4 variations)
- [ ] Storage facilities (4 variations)
- [ ] Terraforming structures (4 variations)

#### 5.1.4 Space Stations (20+ assets)
- [ ] Trading posts (4 variations)
- [ ] Military stations (4 variations)
- [ ] Research stations (4 variations)
- [ ] Mining stations (4 variations)
- [ ] Defense platforms (4 variations)

#### 5.1.5 Environmental Assets (30+ assets)
- [ ] Asteroid fields (6 variations)
- [ ] Nebulae (6 variations)
- [ ] Space debris (6 variations)
- [ ] Cosmic phenomena (6 variations)
- [ ] Star backgrounds (6 variations)

#### 5.1.6 UI Elements (30+ assets)
- [ ] Button styles (6 variations)
- [ ] Icon set (12 icons)
- [ ] Progress bars (4 variations)
- [ ] Panel backgrounds (4 variations)
- [ ] Tooltips (4 variations)

### 5.2 Asset Generation Process
- [ ] Define consistent art style and color palette
- [ ] Create generation prompts for each category
- [ ] Generate assets using AI image generation
- [ ] Quality control and selection
- [ ] Optimization and formatting

**Estimated Time:** 8-10 hours  
**Assets Generated:** 200+

---

## PHASE 6: ASSET MANAGEMENT SYSTEM

### 6.1 Asset Database Schema
- [ ] Extend `cms_assets` table with metadata
- [ ] Asset categories and tags
- [ ] Asset versions and dependencies
- [ ] Asset usage tracking

### 6.2 Asset Service Implementation
- [ ] AssetService - Asset CRUD operations
- [ ] Asset upload and storage
- [ ] Asset optimization pipeline
- [ ] Asset versioning system
- [ ] CDN integration preparation

### 6.3 Asset API Endpoints
- [ ] POST `/api/assets/upload`
- [ ] GET `/api/assets/:id`
- [ ] GET `/api/assets/category/:category`
- [ ] PUT `/api/assets/:id`
- [ ] DELETE `/api/assets/:id`

### 6.4 Asset Manager UI
- [ ] `pages/cms/asset-manager.njk`
- [ ] Asset upload interface
- [ ] Asset browser/gallery
- [ ] Asset metadata editor
- [ ] `js/asset-manager.js`

**Estimated Time:** 6-8 hours  
**Lines of Code:** ~2,000

---

## PHASE 7: VISUAL STYLE OVERHAUL & UI REDESIGN

### 7.1 Universus Visual Identity
- [ ] Define color palette (deep space theme)
- [ ] Define typography system
- [ ] Create design tokens
- [ ] Define spacing/layout system

### 7.2 CSS Architecture Update
- [ ] Refactor CSS with new design system
- [ ] Implement CSS custom properties
- [ ] Create component-based styles
- [ ] Add animations and transitions

### 7.3 Asset Integration
- [ ] Integrate planet backgrounds
- [ ] Integrate spacecraft images
- [ ] Integrate building images
- [ ] Integrate UI elements
- [ ] Integrate environmental assets

### 7.4 UI/UX Enhancements
- [ ] Improve navigation experience
- [ ] Enhanced visual feedback
- [ ] Smooth animations
- [ ] Responsive design improvements
- [ ] Accessibility enhancements

**Estimated Time:** 8-10 hours  
**Lines of Code:** ~2,500

---

## PHASE 8: INTEGRATION TESTING & QUALITY ASSURANCE

### 8.1 Template Testing
- [ ] Verify all templates render correctly
- [ ] Test template inheritance
- [ ] Test partial includes
- [ ] Test context data injection

### 8.2 CMS Functionality Testing
- [ ] Test page management
- [ ] Test content block editing
- [ ] Test navigation editing
- [ ] Test theme customization

### 8.3 Asset System Testing
- [ ] Test asset uploads
- [ ] Test asset optimization
- [ ] Test asset loading performance
- [ ] Test asset caching

### 8.4 Integration Testing
- [ ] Test all game features with new templates
- [ ] Test real-time updates with new UI
- [ ] Test mobile responsiveness
- [ ] Cross-browser compatibility testing

### 8.5 Performance Testing
- [ ] Page load time optimization
- [ ] Asset delivery performance
- [ ] Database query optimization
- [ ] WebSocket performance

**Estimated Time:** 6-8 hours  
**Test Cases:** 100+

---

## SUCCESS CRITERIA

- [ ] Complete templating engine with reusable components
- [ ] Full CMS system for content and layout management
- [ ] 100% specification compliance verified
- [ ] Complete rebrand to "Universus" throughout
- [ ] 200+ AI-generated game assets in consistent style
- [ ] Professional asset management system
- [ ] Modernized UI/UX with new visual identity
- [ ] Comprehensive testing and quality assurance
- [ ] All existing functionality preserved and enhanced
- [ ] Production-ready deployment

---

## TECHNICAL STACK

### New Dependencies
- **Templating:** Nunjucks
- **Image Processing:** Sharp (for asset optimization)
- **File Upload:** Multer
- **Asset Storage:** Local filesystem with CDN-ready structure

### Existing Stack (Preserved)
- Backend: Node.js + TypeScript + Express.js
- Database: PostgreSQL + Redis
- Frontend: Vanilla JavaScript + Canvas
- Real-time: Socket.io
- Deployment: Docker

---

## DELIVERABLES

1. Complete Nunjucks templating system
2. Full CMS administration interface
3. 200+ high-quality AI-generated assets
4. Rebranded "Universus" game
5. Updated documentation
6. Testing suite and QA reports
7. Deployment guide

---

## ESTIMATED TOTAL TIME: 50-65 hours
## ESTIMATED TOTAL CODE: 15,000+ lines

---

## VISUAL STYLE DIRECTION: "UNIVERSUS"

### Color Palette
- **Primary:** Deep Space Blue (#0A1929)
- **Secondary:** Nebula Purple (#6B2B84)
- **Accent:** Stellar Gold (#FFB923)
- **Highlight:** Cosmic Cyan (#00D9FF)
- **Text:** Star White (#E8F1F5)
- **Danger:** Supernova Red (#FF3366)
- **Success:** Alien Green (#00FF88)

### Art Style
- Modern, clean sci-fi aesthetic
- High-tech interfaces with glowing elements
- Atmospheric space environments
- Cinematic visual quality
- Consistent futuristic design language

### Typography
- Headers: Futuristic sans-serif (Orbitron, Exo 2)
- Body: Clean, readable sans-serif (Inter, Roboto)
- Monospace: Technical displays (JetBrains Mono)

### UI Design Principles
- Dark theme optimized for long play sessions
- High contrast for readability
- Subtle animations and transitions
- Glowing/neon accent elements
- Depth through layering and shadows
- Professional, polished interface elements

---

**Status:** Ready to begin implementation  
**Next Step:** Phase 1 - Templating Engine Setup
