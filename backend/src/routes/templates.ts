import { Router, Request, Response } from 'express';
import { TemplateService } from '../services/templateService';
import { authenticateToken } from '../middleware/auth';

const router = Router();

/**
 * Public routes (no authentication required)
 */

// Landing/Login page
router.get('/', (req: Request, res: Response) => {
  TemplateService.renderIndex(res);
});

// Index page (alias)
router.get('/index.html', (req: Request, res: Response) => {
  TemplateService.renderIndex(res);
});

/**
 * Protected routes (authentication required)
 * Note: In production, all game pages should be protected
 * For now, we'll render templates without strict auth enforcement
 * The client-side JavaScript will handle authentication checks
 */

// Overview page
router.get('/overview', (req: Request, res: Response) => {
  TemplateService.renderOverview(res);
});

router.get('/overview.html', (req: Request, res: Response) => {
  TemplateService.renderOverview(res);
});

// Buildings page
router.get('/buildings', (req: Request, res: Response) => {
  TemplateService.renderBuildings(res);
});

router.get('/buildings.html', (req: Request, res: Response) => {
  TemplateService.renderBuildings(res);
});

// Research page
router.get('/research', (req: Request, res: Response) => {
  TemplateService.renderResearch(res);
});

router.get('/research.html', (req: Request, res: Response) => {
  TemplateService.renderResearch(res);
});

// Shipyard page
router.get('/shipyard', (req: Request, res: Response) => {
  TemplateService.renderShipyard(res);
});

router.get('/shipyard.html', (req: Request, res: Response) => {
  TemplateService.renderShipyard(res);
});

// Fleet page
router.get('/fleet', (req: Request, res: Response) => {
  TemplateService.renderFleet(res);
});

router.get('/fleet.html', (req: Request, res: Response) => {
  TemplateService.renderFleet(res);
});

// Galaxy page
router.get('/galaxy', (req: Request, res: Response) => {
  TemplateService.renderGalaxy(res);
});

router.get('/galaxy.html', (req: Request, res: Response) => {
  TemplateService.renderGalaxy(res);
});

// Leaderboard page
router.get('/leaderboard', (req: Request, res: Response) => {
  TemplateService.renderLeaderboard(res);
});

router.get('/leaderboard.html', (req: Request, res: Response) => {
  TemplateService.renderLeaderboard(res);
});

// Messages page
router.get('/messages', (req: Request, res: Response) => {
  TemplateService.renderMessages(res);
});

router.get('/messages.html', (req: Request, res: Response) => {
  TemplateService.renderMessages(res);
});

// Shop page
router.get('/shop', (req: Request, res: Response) => {
  TemplateService.renderShop(res);
});

router.get('/shop.html', (req: Request, res: Response) => {
  TemplateService.renderShop(res);
});

// Notifications page
router.get('/notifications', (req: Request, res: Response) => {
  TemplateService.renderNotifications(res);
});

router.get('/notifications.html', (req: Request, res: Response) => {
  TemplateService.renderNotifications(res);
});

// Matrix Shop (Phase 10 Enhanced Shop)
router.get('/matrix-shop', (req: Request, res: Response) => {
  res.render('pages/matrix-shop.njk');
});

router.get('/matrix-shop.html', (req: Request, res: Response) => {
  res.render('pages/matrix-shop.njk');
});

// Admin page
router.get('/admin', (req: Request, res: Response) => {
  TemplateService.renderAdmin(res);
});

router.get('/admin.html', (req: Request, res: Response) => {
  TemplateService.renderAdmin(res);
});

// New Admin Dashboard
router.get('/admin/dashboard', (req: Request, res: Response) => {
  res.render('pages/admin/dashboard.njk');
});

// Admin Users Management
router.get('/admin/users', (req: Request, res: Response) => {
  res.render('pages/admin/users.njk');
});

// Admin Monitoring
router.get('/admin/monitoring', (req: Request, res: Response) => {
  res.render('pages/admin/monitoring.njk');
});

// Admin Settings
router.get('/admin/settings', (req: Request, res: Response) => {
  res.render('pages/admin/settings.njk');
});

// Admin Events
router.get('/admin/events', (req: Request, res: Response) => {
  res.render('pages/admin/events.njk');
});

// Admin Analytics
router.get('/admin/analytics', (req: Request, res: Response) => {
  res.render('pages/admin/analytics.njk');
});

// Admin Audit Logs
router.get('/admin/audit', (req: Request, res: Response) => {
  res.render('pages/admin/audit.njk');
});

// Bot management page
router.get('/admin/bots', (req: Request, res: Response) => {
  TemplateService.renderBotManagement(res);
});

router.get('/admin/bots.html', (req: Request, res: Response) => {
  TemplateService.renderBotManagement(res);
});

export default router;


// Chat page (Phase 6)
router.get('/chat', (req: Request, res: Response) => {
  res.render('pages/chat.njk', {
    user: (req as any).user || null
  });
});

router.get('/chat.html', (req: Request, res: Response) => {
  res.render('pages/chat.njk', {
    user: (req as any).user || null
  });
});

/**
 * Account Management Pages (Phase 9)
 * All account management routes require authentication
 */

// Account Settings (main page)
router.get('/account/settings', (req: Request, res: Response) => {
  res.render('account/account-settings.njk', {
    user: (req as any).user || null,
    title: 'Account Settings - Universus'
  });
});

// Security Dashboard
router.get('/account/security', (req: Request, res: Response) => {
  res.render('account/security-dashboard.njk', {
    user: (req as any).user || null,
    title: 'Security Dashboard - Universus'
  });
});

// Two-Factor Authentication Setup
router.get('/account/2fa', (req: Request, res: Response) => {
  res.render('account/2fa-setup.njk', {
    user: (req as any).user || null,
    title: '2FA Setup - Universus'
  });
});

// Email Verification
router.get('/account/email', (req: Request, res: Response) => {
  res.render('account/email-verification.njk', {
    user: (req as any).user || null,
    title: 'Email Verification - Universus'
  });
});

// Password Recovery
router.get('/account/password', (req: Request, res: Response) => {
  res.render('account/password-recovery.njk', {
    user: (req as any).user || null,
    title: 'Password Recovery - Universus'
  });
});

// Privacy & GDPR Compliance
router.get('/account/privacy', (req: Request, res: Response) => {
  res.render('account/gdpr-compliance.njk', {
    user: (req as any).user || null,
    title: 'Privacy & Data Management - Universus'
  });
});

// Account Transfer
router.get('/account/transfer', (req: Request, res: Response) => {
  res.render('account/account-transfer.njk', {
    user: (req as any).user || null,
    title: 'Account Transfer - Universus'
  });
});

/**
 * Alliance Management Pages (Phase 11)
 * All alliance routes require authentication
 */

// Alliance Dashboard (main page)
router.get('/alliance', (req: Request, res: Response) => {
  res.render('pages/alliance-dashboard.njk', {
    user: (req as any).user || null,
    title: 'Alliance Dashboard - Universus',
    currentPage: 'alliance'
  });
});

router.get('/alliance/dashboard', (req: Request, res: Response) => {
  res.render('pages/alliance-dashboard.njk', {
    user: (req as any).user || null,
    title: 'Alliance Dashboard - Universus',
    currentPage: 'alliance'
  });
});

// Alliance Wars
router.get('/alliance/wars', (req: Request, res: Response) => {
  res.render('pages/alliance-wars.njk', {
    user: (req as any).user || null,
    title: 'Alliance Wars - Universus',
    currentPage: 'alliance'
  });
});

// Alliance Diplomacy
router.get('/alliance/diplomacy', (req: Request, res: Response) => {
  res.render('pages/alliance-diplomacy.njk', {
    user: (req as any).user || null,
    title: 'Alliance Diplomacy - Universus',
    currentPage: 'alliance'
  });
});

// Alliance Management (Leaders only)
router.get('/alliance/manage', (req: Request, res: Response) => {
  res.render('pages/alliance-management.njk', {
    user: (req as any).user || null,
    title: 'Alliance Management - Universus',
    currentPage: 'alliance'
  });
});
