// Phase 11: Alliance Diplomacy Service
// Handles diplomatic relations, treaties, and agreements between alliances

import { pool } from '../config/database';
import {
    DiplomaticRelation,
    DiplomaticProposal,
    DiplomaticStatus,
    ProposeDiplomacyRequest,
    RespondToProposalRequest,
    AlliancePermission
} from '../types/alliance';
import { AllianceService } from './allianceService';

export class AllianceDiplomacyService {
    private allianceService: AllianceService;
    
    constructor() {
        this.allianceService = new AllianceService();
    }
    
    // ========================================================================
    // DIPLOMATIC PROPOSALS
    // ========================================================================
    
    async proposeDiplomacy(allianceId: number, userId: number, data: ProposeDiplomacyRequest): Promise<DiplomaticProposal> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.allianceService.checkPermission(
                allianceId,
                userId,
                AlliancePermission.MANAGE_DIPLOMACY
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to manage diplomacy');
            }
            
            // Validate alliances exist
            const sourceAlliance = await this.allianceService.getAlliance(allianceId);
            const targetAlliance = await this.allianceService.getAlliance(data.target_alliance_id);
            
            if (!sourceAlliance || !targetAlliance) {
                throw new Error('One or both alliances not found');
            }
            
            if (allianceId === data.target_alliance_id) {
                throw new Error('Cannot propose diplomacy with your own alliance');
            }
            
            // Check for existing relation
            const existingRelation = await client.query(
                `SELECT * FROM diplomatic_relations
                WHERE alliance_id = $1 AND target_alliance_id = $2 AND terminated_at IS NULL`,
                [allianceId, data.target_alliance_id]
            );
            
            if (existingRelation.rows[0]) {
                throw new Error(`You already have a ${existingRelation.rows[0].status} relation with this alliance`);
            }
            
            // Check for pending proposal
            const pendingProposal = await client.query(
                `SELECT id FROM diplomatic_proposals
                WHERE ((from_alliance_id = $1 AND to_alliance_id = $2)
                    OR (from_alliance_id = $2 AND to_alliance_id = $1))
                AND status = 'pending'
                AND expires_at > CURRENT_TIMESTAMP`,
                [allianceId, data.target_alliance_id]
            );
            
            if (pendingProposal.rows.length > 0) {
                throw new Error('There is already a pending proposal between these alliances');
            }
            
            // Validate proposed status
            if (data.proposed_status === DiplomaticStatus.WAR) {
                throw new Error('Cannot propose war through diplomacy. Use war declaration instead.');
            }
            
            // Create proposal
            const proposalResult = await client.query(
                `INSERT INTO diplomatic_proposals (
                    from_alliance_id,
                    to_alliance_id,
                    proposed_status,
                    terms,
                    duration_days,
                    proposed_by
                ) VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING *`,
                [
                    allianceId,
                    data.target_alliance_id,
                    data.proposed_status,
                    data.terms || null,
                    data.duration_days || null,
                    userId
                ]
            );
            
            // Log history
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id, related_user_id)
                VALUES ($1, 'diplomacy_proposed', 'Proposed ' || $2 || ' with alliance', $3, $4)`,
                [allianceId, data.proposed_status, data.target_alliance_id, userId]
            );
            
            await client.query('COMMIT');
            if (!proposalResult.rows[0]) {
                throw new Error('Failed to create diplomatic proposal');
            }
            return proposalResult.rows[0];
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async respondToProposal(proposalId: number, userId: number, data: RespondToProposalRequest): Promise<DiplomaticRelation | null> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Get proposal
            const proposalResult = await client.query(
                `SELECT * FROM diplomatic_proposals
                WHERE id = $1 AND status = 'pending' AND expires_at > CURRENT_TIMESTAMP`,
                [proposalId]
            );
            
            if (!proposalResult.rows[0]) {
                throw new Error('Proposal not found or expired');
            }
            
            const proposal = proposalResult.rows[0];
            
            // Check permission - must be from target alliance
            const hasPermission = await this.allianceService.checkPermission(
                proposal.to_alliance_id,
                userId,
                AlliancePermission.MANAGE_DIPLOMACY
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to respond to this proposal');
            }
            
            if (data.accept) {
                // Accept proposal - create diplomatic relations
                
                // Create bidirectional relations
                await client.query(
                    `INSERT INTO diplomatic_relations (
                        alliance_id,
                        target_alliance_id,
                        status,
                        treaty_terms,
                        treaty_duration_days,
                        proposed_by,
                        approved_by,
                        expires_at
                    ) VALUES 
                        ($1, $2, $3, $4, $5, $6, $7, $8),
                        ($2, $1, $3, $4, $5, $6, $7, $8)`,
                    [
                        proposal.from_alliance_id,
                        proposal.to_alliance_id,
                        proposal.proposed_status,
                        proposal.terms,
                        proposal.duration_days,
                        proposal.proposed_by,
                        userId,
                        proposal.duration_days 
                            ? new Date(Date.now() + proposal.duration_days * 24 * 60 * 60 * 1000)
                            : null
                    ]
                );
                
                // Update proposal status
                await client.query(
                    `UPDATE diplomatic_proposals
                    SET status = 'accepted', reviewed_by = $1, reviewed_at = CURRENT_TIMESTAMP
                    WHERE id = $2`,
                    [userId, proposalId]
                );
                
                // Log history for both alliances
                await client.query(
                    `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id, related_user_id)
                    VALUES 
                        ($1, 'diplomacy_accepted', 'Established ' || $4 || ' relation', $2, $3),
                        ($2, 'diplomacy_accepted', 'Established ' || $4 || ' relation', $1, $3)`,
                    [proposal.from_alliance_id, proposal.to_alliance_id, userId, proposal.proposed_status]
                );
                
                await client.query('COMMIT');
                
                // Return the new relation
                const relation = await this.getDiplomaticRelation(proposal.from_alliance_id, proposal.to_alliance_id);
                return relation;
                
            } else {
                // Reject proposal
                await client.query(
                    `UPDATE diplomatic_proposals
                    SET status = 'rejected', reviewed_by = $1, reviewed_at = CURRENT_TIMESTAMP
                    WHERE id = $2`,
                    [userId, proposalId]
                );
                
                // Log history
                await client.query(
                    `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id, related_user_id)
                    VALUES ($1, 'diplomacy_rejected', 'Rejected diplomatic proposal', $2, $3)`,
                    [proposal.to_alliance_id, proposal.from_alliance_id, userId]
                );
                
                await client.query('COMMIT');
                return null;
            }
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async cancelProposal(proposalId: number, userId: number): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            const proposal = await client.query(
                'SELECT * FROM diplomatic_proposals WHERE id = $1 AND status = $2',
                [proposalId, 'pending']
            );
            
            if (!proposal.rows[0]) {
                throw new Error('Proposal not found or already processed');
            }
            
            const proposalData = proposal.rows[0];
            
            // Check if user is the proposer or has permission
            const hasPermission = await this.allianceService.checkPermission(
                proposalData.from_alliance_id,
                userId,
                AlliancePermission.MANAGE_DIPLOMACY
            );
            
            if (!hasPermission && proposalData.proposed_by !== userId) {
                throw new Error('You do not have permission to cancel this proposal');
            }
            
            await client.query(
                `UPDATE diplomatic_proposals SET status = 'expired' WHERE id = $1`,
                [proposalId]
            );
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    // ========================================================================
    // DIPLOMATIC RELATIONS MANAGEMENT
    // ========================================================================
    
    async terminateRelation(allianceId: number, targetAllianceId: number, userId: number, reason?: string): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.allianceService.checkPermission(
                allianceId,
                userId,
                AlliancePermission.MANAGE_DIPLOMACY
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to terminate diplomatic relations');
            }
            
            // Get existing relation
            const relation = await client.query(
                `SELECT * FROM diplomatic_relations
                WHERE alliance_id = $1 AND target_alliance_id = $2 AND terminated_at IS NULL`,
                [allianceId, targetAllianceId]
            );
            
            if (!relation.rows[0]) {
                throw new Error('No active diplomatic relation found');
            }
            
            const relationData = relation.rows[0];
            
            // Cannot terminate war relation this way
            if (relationData.status === DiplomaticStatus.WAR) {
                throw new Error('Cannot terminate war relation. Use ceasefire or war end instead.');
            }
            
            // Terminate bidirectional relations
            await client.query(
                `UPDATE diplomatic_relations
                SET terminated_at = CURRENT_TIMESTAMP
                WHERE (alliance_id = $1 AND target_alliance_id = $2)
                    OR (alliance_id = $2 AND target_alliance_id = $1)`,
                [allianceId, targetAllianceId]
            );
            
            // Log history for both alliances
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id, related_user_id, metadata)
                VALUES 
                    ($1, 'diplomacy_terminated', 'Terminated ' || $4 || ' relation', $2, $3, $5),
                    ($2, 'diplomacy_terminated', 'Relation terminated by other alliance', $1, $3, $5)`,
                [
                    allianceId,
                    targetAllianceId,
                    userId,
                    relationData.status,
                    reason ? JSON.stringify({ reason }) : null
                ]
            );
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async updateRelationTerms(allianceId: number, targetAllianceId: number, userId: number, newTerms: string): Promise<DiplomaticRelation> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.allianceService.checkPermission(
                allianceId,
                userId,
                AlliancePermission.MANAGE_DIPLOMACY
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to update diplomatic terms');
            }
            
            // Update terms for both directions
            await client.query(
                `UPDATE diplomatic_relations
                SET treaty_terms = $1
                WHERE (alliance_id = $2 AND target_alliance_id = $3)
                    OR (alliance_id = $3 AND target_alliance_id = $2)`,
                [newTerms, allianceId, targetAllianceId]
            );
            
            await client.query('COMMIT');
            
            const relation = await this.getDiplomaticRelation(allianceId, targetAllianceId);
            if (!relation) {
                throw new Error('Relation not found after update');
            }
            
            return relation;
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    // ========================================================================
    // QUERIES
    // ========================================================================
    
    async getDiplomaticRelation(allianceId: number, targetAllianceId: number): Promise<DiplomaticRelation | null> {
        const result = await pool.query(
            `SELECT dr.*, a.name as target_name, a.tag as target_tag
            FROM diplomatic_relations dr
            JOIN alliances a ON a.id = dr.target_alliance_id
            WHERE dr.alliance_id = $1 AND dr.target_alliance_id = $2 AND dr.terminated_at IS NULL`,
            [allianceId, targetAllianceId]
        );
        
        return result.rows[0] || null;
    }
    
    async getAllianceDiplomaticRelations(allianceId: number, status?: DiplomaticStatus): Promise<DiplomaticRelation[]> {
        let query = `
            SELECT dr.*, a.name as target_name, a.tag as target_tag
            FROM diplomatic_relations dr
            JOIN alliances a ON a.id = dr.target_alliance_id
            WHERE dr.alliance_id = $1 AND dr.terminated_at IS NULL
        `;
        
        const params: any[] = [allianceId];
        
        if (status) {
            query += ' AND dr.status = $2';
            params.push(status);
        }
        
        query += ' ORDER BY dr.established_at DESC';
        
        const result = await pool.query(query, params);
        return result.rows;
    }
    
    async getPendingProposals(allianceId: number): Promise<DiplomaticProposal[]> {
        const result = await pool.query(
            `SELECT dp.*,
                fa.name as from_alliance_name, fa.tag as from_alliance_tag,
                ta.name as to_alliance_name, ta.tag as to_alliance_tag,
                u.username as proposer_username
            FROM diplomatic_proposals dp
            JOIN alliances fa ON fa.id = dp.from_alliance_id
            JOIN alliances ta ON ta.id = dp.to_alliance_id
            JOIN users u ON u.id = dp.proposed_by
            WHERE dp.to_alliance_id = $1 
            AND dp.status = 'pending'
            AND dp.expires_at > CURRENT_TIMESTAMP
            ORDER BY dp.created_at DESC`,
            [allianceId]
        );
        
        return result.rows;
    }
    
    async getSentProposals(allianceId: number): Promise<DiplomaticProposal[]> {
        const result = await pool.query(
            `SELECT dp.*,
                fa.name as from_alliance_name, fa.tag as from_alliance_tag,
                ta.name as to_alliance_name, ta.tag as to_alliance_tag,
                u.username as proposer_username
            FROM diplomatic_proposals dp
            JOIN alliances fa ON fa.id = dp.from_alliance_id
            JOIN alliances ta ON ta.id = dp.to_alliance_id
            JOIN users u ON u.id = dp.proposed_by
            WHERE dp.from_alliance_id = $1 
            AND dp.status = 'pending'
            AND dp.expires_at > CURRENT_TIMESTAMP
            ORDER BY dp.created_at DESC`,
            [allianceId]
        );
        
        return result.rows;
    }
    
    async getAlliances(): Promise<DiplomaticRelation[]> {
        const result = await pool.query(
            `SELECT dr.*, a.name as target_name, a.tag as target_tag
            FROM diplomatic_relations dr
            JOIN alliances a ON a.id = dr.target_alliance_id
            WHERE dr.status = 'alliance' AND dr.terminated_at IS NULL
            ORDER BY dr.established_at DESC`
        );
        
        return result.rows;
    }
    
    async getDefensePacts(): Promise<DiplomaticRelation[]> {
        const result = await pool.query(
            `SELECT dr.*, a.name as target_name, a.tag as target_tag
            FROM diplomatic_relations dr
            JOIN alliances a ON a.id = dr.target_alliance_id
            WHERE dr.status = 'defense_pact' AND dr.terminated_at IS NULL
            ORDER BY dr.established_at DESC`
        );
        
        return result.rows;
    }
    
    async checkRelationStatus(allianceId1: number, allianceId2: number): Promise<DiplomaticStatus> {
        const result = await pool.query(
            `SELECT status FROM diplomatic_relations
            WHERE alliance_id = $1 AND target_alliance_id = $2 AND terminated_at IS NULL`,
            [allianceId1, allianceId2]
        );
        
        return result.rows[0]?.status || DiplomaticStatus.NEUTRAL;
    }
    
    async canAttack(attackerAllianceId: number, defenderAllianceId: number): Promise<boolean> {
        const status = await this.checkRelationStatus(attackerAllianceId, defenderAllianceId);
        
        // Cannot attack allies or defense pact members
        if (status === DiplomaticStatus.ALLIANCE || status === DiplomaticStatus.DEFENSE_PACT) {
            return false;
        }
        
        // Can attack in war
        if (status === DiplomaticStatus.WAR) {
            return true;
        }
        
        // Can attack NAP, but with consequences
        // Can attack neutral, hostile, trade
        return true;
    }
    
    async getDiplomaticHistory(allianceId: number, limit: number = 50): Promise<any[]> {
        const result = await pool.query(
            `SELECT 
                ah.*,
                a.name as related_alliance_name,
                a.tag as related_alliance_tag,
                u.username as related_username
            FROM alliance_history ah
            LEFT JOIN alliances a ON a.id = ah.related_alliance_id
            LEFT JOIN users u ON u.id = ah.related_user_id
            WHERE ah.alliance_id = $1 
            AND ah.event_type LIKE 'diplomacy_%'
            ORDER BY ah.created_at DESC
            LIMIT $2`,
            [allianceId, limit]
        );
        
        return result.rows;
    }
}
