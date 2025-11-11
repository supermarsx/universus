// Phase 11: Alliance Management Routes
// Complete REST API for alliance system

import { Router, Response } from 'express';
import { AuthRequest } from '../types';
import { authenticateToken } from '../middleware/auth';
import { getUserId } from '../utils/authHelpers';
import { AllianceService } from '../services/allianceService';
import allianceLogisticsService from '../services/allianceLogisticsService';
import { AllianceWarService } from '../services/allianceWarService';
import { AllianceDiplomacyService } from '../services/allianceDiplomacyService';

const router = Router();

// Initialize services
const allianceService = new AllianceService();
const warService = new AllianceWarService();
const diplomacyService = new AllianceDiplomacyService();

// All routes require authentication
router.use(authenticateToken);


// ============================================================================
// ALLIANCE MANAGEMENT ROUTES
// ============================================================================

// Create alliance
router.post('/create', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const alliance = await allianceService.createAlliance(userId, req.body);
        res.json({ success: true, data: alliance });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get current user's alliance dashboard
router.get('/my-alliance', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const dashboard = await allianceService.getMyAllianceDashboard(userId);
        res.json({ success: true, data: dashboard });
    } catch (error: any) {
        if (error.message === 'NOT_IN_ALLIANCE') {
            return res.status(404).json({ success: false, error: { message: 'You are not part of an alliance' } });
        }
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get alliance details
router.get('/:allianceId', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        const allianceId = parseInt(req.params.allianceId);
        const details = await allianceService.getAllianceDetails(allianceId, userId ?? undefined);
        res.json({ success: true, data: details });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get alliance by tag
router.get('/tag/:tag', async (req: AuthRequest, res: Response) => {
    try {
        const alliance = await allianceService.getAllianceByTag(req.params.tag);
        if (!alliance) {
            return res.status(404).json({ success: false, error: { message: 'Alliance not found' } });
        }
        res.json({ success: true, data: alliance });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Update alliance
router.put('/:allianceId', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const alliance = await allianceService.updateAlliance(allianceId, userId, req.body);
        res.json({ success: true, data: alliance });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Disband alliance
router.delete('/:allianceId', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        await allianceService.disbandAlliance(allianceId, userId);
        res.json({ success: true, message: 'Alliance disbanded successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get alliance leaderboard
router.get('/leaderboard/rankings', async (req: AuthRequest, res: Response) => {
    try {
        const limit = parseInt(req.query.limit as string) || 100;
        const offset = parseInt(req.query.offset as string) || 0;
        const leaderboard = await allianceService.getAllianceLeaderboard(limit, offset);
        res.json({ success: true, data: leaderboard });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get alliance statistics
router.get('/:allianceId/statistics', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const stats = await allianceService.getAllianceStatistics(allianceId);
        res.json({ success: true, data: stats });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Search alliances
router.get('/search/query', async (req: AuthRequest, res: Response) => {
    try {
        const query = req.query.q as string;
        const limit = parseInt(req.query.limit as string) || 20;
        const alliances = await allianceService.searchAlliances(query, limit);
        res.json({ success: true, data: alliances });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// ============================================================================
// MEMBERSHIP ROUTES
// ============================================================================

// Apply to alliance
router.post('/:allianceId/apply', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const application = await allianceService.applyToAlliance(userId, {
            alliance_id: allianceId,
            message: req.body.message
        });
        res.json({ success: true, data: application });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Process application
router.post('/:allianceId/applications/:applicationId/process', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const applicationId = parseInt(req.params.applicationId);
        const accept = req.body.accept === true;
        
        await allianceService.processApplication(allianceId, applicationId, userId, accept);
        res.json({ success: true, message: accept ? 'Application accepted' : 'Application rejected' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// List applications
router.get('/:allianceId/applications', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const applications = await allianceService.getAllianceApplications(allianceId, userId);
        res.json({ success: true, data: applications });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Leave alliance
router.post('/:allianceId/leave', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        await allianceService.leaveAlliance(allianceId, userId);
        res.json({ success: true, message: 'Left alliance successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Manage member (promote/demote/kick)
router.post('/:allianceId/members/manage', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        await allianceService.manageMember(allianceId, userId, req.body);
        res.json({ success: true, message: 'Member managed successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// ============================================================================
// ANNOUNCEMENTS & COMMUNICATION
// ============================================================================

router.get('/:allianceId/announcements', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const limit = parseInt(req.query.limit as string) || 10;
        const announcements = await allianceService.getAnnouncements(allianceId, limit);
        res.json({ success: true, data: announcements });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

router.post('/:allianceId/announcements', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const announcement = await allianceService.createAnnouncement(allianceId, userId, req.body);
        res.json({ success: true, data: announcement });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// ============================================================================
// RESOURCE MANAGEMENT ROUTES
// ============================================================================

// Contribute resources
router.post('/:allianceId/resources/contribute', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        await allianceService.contributeResources(allianceId, userId, req.body);
        res.json({ success: true, message: 'Resources contributed successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Withdraw resources
router.post('/:allianceId/resources/withdraw', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        await allianceService.withdrawResources(allianceId, userId, req.body);
        res.json({ success: true, message: 'Resources withdrawn successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// ============================================================================
// LOGISTICS ROUTES (Depot & Shared Transport)
// ============================================================================

router.post('/:allianceId/depot/request', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const session = await allianceLogisticsService.requestDepotDock(allianceId, userId, req.body);
        res.json({ success: true, data: session });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

router.post('/:allianceId/depot/:sessionId/approve', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const sessionId = parseInt(req.params.sessionId);
        const result = await allianceLogisticsService.approveDepotDock(allianceId, userId, {
            sessionId,
            approvedAmount: Number(req.body.amount),
        });
        res.json({ success: true, data: result });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

router.post('/:allianceId/depot/:sessionId/cancel', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const sessionId = parseInt(req.params.sessionId);
        await allianceLogisticsService.cancelDepotSession(allianceId, userId, sessionId);
        res.json({ success: true, message: 'Depot request cancelled' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

router.post('/:allianceId/shared-transport', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        await allianceLogisticsService.createSharedTransport(allianceId, userId, req.body);
        res.json({ success: true, message: 'Shared transport completed' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

router.get('/:allianceId/depot/sessions', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const status = req.query.status as string | undefined;
        const sessions = await allianceLogisticsService.getDepotSessions(allianceId, userId, status);
        res.json({ success: true, data: sessions });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// ============================================================================
// WAR MANAGEMENT ROUTES
// ============================================================================

// Declare war
router.post('/:allianceId/wars/declare', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const war = await warService.declareWar(allianceId, userId, req.body);
        res.json({ success: true, data: war });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Accept war declaration
router.post('/wars/:warId/accept', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const warId = parseInt(req.params.warId);
        const war = await warService.acceptWarDeclaration(warId, userId);
        res.json({ success: true, data: war });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get war details
router.get('/wars/:warId', async (req: AuthRequest, res: Response) => {
    try {
        const warId = parseInt(req.params.warId);
        const details = await warService.getWarDetails(warId);
        res.json({ success: true, data: details });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get alliance wars
router.get('/:allianceId/wars', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const status = req.query.status as any;
        const wars = await warService.getAllianceWars(allianceId, status);
        res.json({ success: true, data: wars });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Record war battle
router.post('/wars/:warId/battles', async (req: AuthRequest, res: Response) => {
    try {
        const warId = parseInt(req.params.warId);
        const battle = await warService.recordWarBattle({
            war_id: warId,
            ...req.body
        });
        res.json({ success: true, data: battle });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// End war
router.post('/wars/:warId/end', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const warId = parseInt(req.params.warId);
        await warService.endWar(warId, userId, req.body);
        res.json({ success: true, message: 'War ended successfully' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Propose ceasefire
router.post('/wars/:warId/ceasefire', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const warId = parseInt(req.params.warId);
        await warService.proposeCeasefire(warId, userId);
        res.json({ success: true, message: 'Ceasefire proposed' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get active wars
router.get('/wars/active/all', async (req: AuthRequest, res: Response) => {
    try {
        const wars = await warService.getActiveWars();
        res.json({ success: true, data: wars });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get war leaderboard
router.get('/wars/:warId/leaderboard', async (req: AuthRequest, res: Response) => {
    try {
        const warId = parseInt(req.params.warId);
        const leaderboard = await warService.getWarLeaderboard(warId);
        res.json({ success: true, data: leaderboard });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// ============================================================================
// DIPLOMACY ROUTES
// ============================================================================

// Propose diplomacy
router.post('/:allianceId/diplomacy/propose', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const proposal = await diplomacyService.proposeDiplomacy(allianceId, userId, req.body);
        res.json({ success: true, data: proposal });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Respond to proposal
router.post('/diplomacy/proposals/:proposalId/respond', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const proposalId = parseInt(req.params.proposalId);
        const relation = await diplomacyService.respondToProposal(proposalId, userId, req.body);
        res.json({ success: true, data: relation });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Cancel proposal
router.delete('/diplomacy/proposals/:proposalId', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const proposalId = parseInt(req.params.proposalId);
        await diplomacyService.cancelProposal(proposalId, userId);
        res.json({ success: true, message: 'Proposal cancelled' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get diplomatic relations
router.get('/:allianceId/diplomacy/relations', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const status = req.query.status as any;
        const relations = await diplomacyService.getAllianceDiplomaticRelations(allianceId, status);
        res.json({ success: true, data: relations });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get pending proposals
router.get('/:allianceId/diplomacy/proposals/pending', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const proposals = await diplomacyService.getPendingProposals(allianceId);
        res.json({ success: true, data: proposals });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get sent proposals
router.get('/:allianceId/diplomacy/proposals/sent', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const proposals = await diplomacyService.getSentProposals(allianceId);
        res.json({ success: true, data: proposals });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Terminate diplomatic relation
router.post('/:allianceId/diplomacy/terminate/:targetAllianceId', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const targetAllianceId = parseInt(req.params.targetAllianceId);
        await diplomacyService.terminateRelation(allianceId, targetAllianceId, userId, req.body.reason);
        res.json({ success: true, message: 'Diplomatic relation terminated' });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Update relation terms
router.put('/:allianceId/diplomacy/terms/:targetAllianceId', async (req: AuthRequest, res: Response) => {
    try {
        const userId = getUserId(req);
        if (userId === null) return res.status(401).json({ success: false, message: 'Unauthorized' });
        const allianceId = parseInt(req.params.allianceId);
        const targetAllianceId = parseInt(req.params.targetAllianceId);
        const relation = await diplomacyService.updateRelationTerms(
            allianceId,
            targetAllianceId,
            userId,
            req.body.terms
        );
        res.json({ success: true, data: relation });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Get diplomatic history
router.get('/:allianceId/diplomacy/history', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const limit = parseInt(req.query.limit as string) || 50;
        const history = await diplomacyService.getDiplomaticHistory(allianceId, limit);
        res.json({ success: true, data: history });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

// Check if can attack
router.get('/:allianceId/diplomacy/can-attack/:targetAllianceId', async (req: AuthRequest, res: Response) => {
    try {
        const allianceId = parseInt(req.params.allianceId);
        const targetAllianceId = parseInt(req.params.targetAllianceId);
        const canAttack = await diplomacyService.canAttack(allianceId, targetAllianceId);
        res.json({ success: true, data: { can_attack: canAttack } });
    } catch (error: any) {
        res.status(400).json({ success: false, error: { message: error.message } });
    }
});

export default router;
