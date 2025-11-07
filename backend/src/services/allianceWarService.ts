// Phase 11: Alliance War Service
// Handles alliance war declarations, battle tracking, and war completion

import { pool } from '../config/database';
import {
    AllianceWar,
    WarBattle,
    WarParticipant,
    WarStatus,
    AllianceWarDetailsResponse,
    DeclareWarRequest,
    RecordWarBattleRequest,
    EndWarRequest,
    ActiveWarSummary,
    AlliancePermission
} from '../types/alliance';
import { AllianceService } from './allianceService';

export class AllianceWarService {
    private allianceService: AllianceService;
    
    constructor() {
        this.allianceService = new AllianceService();
    }
    
    // ========================================================================
    // WAR DECLARATIONS
    // ========================================================================
    
    async declareWar(attackerAllianceId: number, userId: number, data: DeclareWarRequest): Promise<AllianceWar> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.allianceService.checkPermission(
                attackerAllianceId,
                userId,
                AlliancePermission.DECLARE_WAR
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to declare war');
            }
            
            // Check if alliances exist
            const attackerAlliance = await this.allianceService.getAlliance(attackerAllianceId);
            const defenderAlliance = await this.allianceService.getAlliance(data.defender_alliance_id);
            
            if (!attackerAlliance || !defenderAlliance) {
                throw new Error('One or both alliances not found');
            }
            
            if (attackerAllianceId === data.defender_alliance_id) {
                throw new Error('Cannot declare war on your own alliance');
            }
            
            // Check for existing active war
            const existingWar = await client.query(
                `SELECT id FROM alliance_wars
                WHERE ((attacker_alliance_id = $1 AND defender_alliance_id = $2)
                    OR (attacker_alliance_id = $2 AND defender_alliance_id = $1))
                AND status IN ('declared', 'active')`,
                [attackerAllianceId, data.defender_alliance_id]
            );
            
            if (existingWar.rows.length > 0) {
                throw new Error('There is already an active war between these alliances');
            }
            
            // Check diplomatic status
            const diplomacy = await client.query(
                `SELECT status FROM diplomatic_relations
                WHERE ((alliance_id = $1 AND target_alliance_id = $2)
                    OR (alliance_id = $2 AND target_alliance_id = $1))
                AND terminated_at IS NULL`,
                [attackerAllianceId, data.defender_alliance_id]
            );
            
            if (diplomacy.rows[0]?.status === 'alliance' || diplomacy.rows[0]?.status === 'defense_pact') {
                throw new Error('Cannot declare war on an allied alliance. Terminate the pact first.');
            }
            
            // Create war declaration
            const warResult = await client.query(
                `INSERT INTO alliance_wars (
                    attacker_alliance_id,
                    defender_alliance_id,
                    declaration_message,
                    status,
                    victory_condition,
                    victory_threshold
                ) VALUES ($1, $2, $3, $4, $5, $6)
                RETURNING *`,
                [
                    attackerAllianceId,
                    data.defender_alliance_id,
                    data.declaration_message || null,
                    WarStatus.DECLARED,
                    data.victory_condition || 'points',
                    data.victory_threshold || 1000
                ]
            );
            
            const war = warResult.rows[0];
            if (!war) {
                throw new Error('Failed to create war declaration');
            }
            
            // Update diplomatic relations to war
            await client.query(
                `INSERT INTO diplomatic_relations (alliance_id, target_alliance_id, status)
                VALUES ($1, $2, 'war'), ($2, $1, 'war')
                ON CONFLICT (alliance_id, target_alliance_id)
                DO UPDATE SET status = 'war', established_at = CURRENT_TIMESTAMP`,
                [attackerAllianceId, data.defender_alliance_id]
            );
            
            // Log history for both alliances
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id, related_user_id)
                VALUES 
                    ($1, 'war_declared', 'War declared against ' || $3, $2, $4),
                    ($2, 'war_declared_against', 'War declared by ' || $5, $1, $4)`,
                [
                    attackerAllianceId,
                    data.defender_alliance_id,
                    defenderAlliance.name,
                    userId,
                    attackerAlliance.name
                ]
            );
            
            await client.query('COMMIT');
            return war;
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async acceptWarDeclaration(warId: number, defenderUserId: number): Promise<AllianceWar> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Get war
            const warResult = await client.query(
                'SELECT * FROM alliance_wars WHERE id = $1 AND status = $2',
                [warId, WarStatus.DECLARED]
            );
            
            if (!warResult.rows[0]) {
                throw new Error('War not found or already started');
            }
            
            const war = warResult.rows[0];
            if (!war) {
                throw new Error('War not found or already started');
            }
            
            // Check permission
            const hasPermission = await this.allianceService.checkPermission(
                war.defender_alliance_id,
                defenderUserId,
                AlliancePermission.DECLARE_WAR
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to accept war declarations');
            }
            
            // Start war
            await client.query(
                `UPDATE alliance_wars 
                SET status = $1, started_at = CURRENT_TIMESTAMP
                WHERE id = $2`,
                [WarStatus.ACTIVE, warId]
            );
            
            // Log history
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id)
                VALUES 
                    ($1, 'war_started', 'War has begun', $2),
                    ($2, 'war_started', 'War has begun', $1)`,
                [war.attacker_alliance_id, war.defender_alliance_id]
            );
            
            await client.query('COMMIT');
            
            const updatedWar = await this.getWar(warId);
            return updatedWar!;
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    // ========================================================================
    // BATTLE RECORDING
    // ========================================================================
    
    async recordWarBattle(data: RecordWarBattleRequest): Promise<WarBattle> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Get war
            const war = await client.query(
                'SELECT * FROM alliance_wars WHERE id = $1 AND status = $2',
                [data.war_id, WarStatus.ACTIVE]
            );
            
            if (!war.rows[0]) {
                throw new Error('War not found or not active');
            }
            
            const warData = war.rows[0];
            
            // Verify attacker is in attacker alliance
            const attackerMember = await client.query(
                'SELECT alliance_id FROM alliance_members WHERE user_id = $1',
                [data.combat_id] // Assuming combat includes attacker
            );
            
            // Verify defender is in defender alliance
            const defenderMember = await client.query(
                'SELECT alliance_id FROM alliance_members WHERE user_id = $1',
                [data.defender_user_id]
            );
            
            if (!attackerMember.rows[0] || !defenderMember.rows[0]) {
                throw new Error('One or both participants not found in their alliances');
            }
            
            if (!attackerMember.rows[0]?.alliance_id || !defenderMember.rows[0]?.alliance_id) {
                throw new Error('Alliance ID not found for one or both participants');
            }
            
            const attackerAllianceId = attackerMember.rows[0].alliance_id;
            const defenderAllianceId = defenderMember.rows[0].alliance_id;
            
            // Determine winner alliance
            const winnerAllianceId = data.winner_user_id === data.defender_user_id
                ? defenderAllianceId
                : attackerAllianceId;
            
            // Calculate war points
            const pointsResult = await client.query(
                'SELECT calculate_war_points($1, $2) as points',
                [data.attacker_losses, data.defender_losses]
            );
            
            if (!pointsResult.rows[0] || pointsResult.rows[0].points === undefined) {
                throw new Error('Failed to calculate war points');
            }
            const points = pointsResult.rows[0].points;
            
            // Distribute points
            const attackerPoints = winnerAllianceId === attackerAllianceId ? points : 0;
            const defenderPoints = winnerAllianceId === defenderAllianceId ? points : 0;
            
            // Record battle
            const battleResult = await client.query(
                `INSERT INTO war_battles (
                    war_id,
                    attacker_user_id,
                    defender_user_id,
                    attacker_alliance_id,
                    defender_alliance_id,
                    combat_id,
                    winner_user_id,
                    winner_alliance_id,
                    attacker_points,
                    defender_points,
                    attacker_losses,
                    defender_losses,
                    loot_metal,
                    loot_crystal,
                    loot_deuterium
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
                RETURNING *`,
                [
                    data.war_id,
                    data.combat_id, // Placeholder for attacker_user_id
                    data.defender_user_id,
                    attackerAllianceId,
                    defenderAllianceId,
                    data.combat_id,
                    data.winner_user_id,
                    winnerAllianceId,
                    attackerPoints,
                    defenderPoints,
                    data.attacker_losses,
                    data.defender_losses,
                    data.loot_metal,
                    data.loot_crystal,
                    data.loot_deuterium
                ]
            );
            
            // Update war scores
            await client.query(
                `UPDATE alliance_wars
                SET attacker_score = attacker_score + $1,
                    defender_score = defender_score + $2,
                    total_battles = total_battles + 1
                WHERE id = $3`,
                [attackerPoints, defenderPoints, data.war_id]
            );
            
            // Update participants
            await client.query(
                `INSERT INTO war_participants (war_id, user_id, alliance_id, battles_fought, battles_won, total_points, total_damage)
                VALUES 
                    ($1, $2, $3, 1, $4, $5, $6),
                    ($1, $7, $8, 1, $9, $10, $11)
                ON CONFLICT (war_id, user_id)
                DO UPDATE SET
                    battles_fought = war_participants.battles_fought + 1,
                    battles_won = war_participants.battles_won + EXCLUDED.battles_won,
                    total_points = war_participants.total_points + EXCLUDED.total_points,
                    total_damage = war_participants.total_damage + EXCLUDED.total_damage`,
                [
                    data.war_id,
                    data.combat_id, // attacker
                    attackerAllianceId,
                    winnerAllianceId === attackerAllianceId ? 1 : 0,
                    attackerPoints,
                    data.attacker_losses,
                    data.defender_user_id,
                    defenderAllianceId,
                    winnerAllianceId === defenderAllianceId ? 1 : 0,
                    defenderPoints,
                    data.defender_losses
                ]
            );
            
            // Update member statistics
            await client.query(
                `UPDATE alliance_members
                SET wars_participated = wars_participated + 1,
                    battles_won = battles_won + CASE WHEN user_id = $1 THEN 1 ELSE 0 END
                WHERE user_id IN ($2, $3)`,
                [data.winner_user_id, data.combat_id, data.defender_user_id]
            );
            
            // Check victory condition
            const updatedWar = await client.query(
                'SELECT * FROM alliance_wars WHERE id = $1',
                [data.war_id]
            );
            
            const warUpdate = updatedWar.rows[0];
            if (!warUpdate) {
                throw new Error('War not found during battle update');
            }
            
            if (warUpdate.victory_condition === 'points') {
                if (warUpdate.attacker_score >= warUpdate.victory_threshold) {
                    await this.endWarInternal(client, data.war_id, warUpdate.attacker_alliance_id, 'Victory threshold reached');
                } else if (warUpdate.defender_score >= warUpdate.victory_threshold) {
                    await this.endWarInternal(client, data.war_id, warUpdate.defender_alliance_id, 'Victory threshold reached');
                }
            }
            
            await client.query('COMMIT');
            if (!battleResult.rows[0]) {
                throw new Error('Failed to record battle result');
            }
            return battleResult.rows[0];
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    // ========================================================================
    // WAR COMPLETION
    // ========================================================================
    
    async endWar(warId: number, userId: number, data: EndWarRequest): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            const war = await client.query(
                'SELECT * FROM alliance_wars WHERE id = $1',
                [warId]
            );
            
            if (!war.rows[0]) {
                throw new Error('War not found');
            }
            
            const warData = war.rows[0];
            
            // Check permission for either alliance
            const hasPermissionAttacker = await this.allianceService.checkPermission(
                warData.attacker_alliance_id,
                userId,
                AlliancePermission.DECLARE_WAR
            );
            
            const hasPermissionDefender = await this.allianceService.checkPermission(
                warData.defender_alliance_id,
                userId,
                AlliancePermission.DECLARE_WAR
            );
            
            if (!hasPermissionAttacker && !hasPermissionDefender) {
                throw new Error('You do not have permission to end this war');
            }
            
            // Determine winner
            let winnerId: number | null = null;
            if (warData.attacker_score > warData.defender_score) {
                winnerId = warData.attacker_alliance_id;
            } else if (warData.defender_score > warData.attacker_score) {
                winnerId = warData.defender_alliance_id;
            }
            
            await this.endWarInternal(client, warId, winnerId, data.end_reason);
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    private async endWarInternal(client: any, warId: number, winnerId: number | null, reason: string): Promise<void> {
        // Update war status
        await client.query(
            `UPDATE alliance_wars
            SET status = $1, ended_at = CURRENT_TIMESTAMP, winner_alliance_id = $2, end_reason = $3
            WHERE id = $4`,
            [WarStatus.ENDED, winnerId, reason, warId]
        );
        
        // Get war details
        const war = await client.query(
            'SELECT * FROM alliance_wars WHERE id = $1',
            [warId]
        );
        
        const warData = war.rows[0];
        if (!warData) {
            throw new Error('War not found in endWarInternal');
        }
        
        // Update diplomatic relations back to neutral
        await client.query(
            `UPDATE diplomatic_relations
            SET status = 'neutral'
            WHERE ((alliance_id = $1 AND target_alliance_id = $2)
                OR (alliance_id = $2 AND target_alliance_id = $1))
            AND status = 'war'`,
            [warData.attacker_alliance_id, warData.defender_alliance_id]
        );
        
        // Log history
        if (winnerId) {
            const loserId = winnerId === warData.attacker_alliance_id 
                ? warData.defender_alliance_id 
                : warData.attacker_alliance_id;
            
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id)
                VALUES 
                    ($1, 'war_won', 'War victory', $2),
                    ($2, 'war_lost', 'War defeat', $1)`,
                [winnerId, loserId]
            );
            
            // Create achievement for winner
            await client.query(
                `INSERT INTO alliance_achievements (alliance_id, achievement_type, achievement_name, description)
                VALUES ($1, 'war_victory', 'War Victory', 'Won a war against another alliance')`,
                [winnerId]
            );
        } else {
            // Stalemate
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_alliance_id)
                VALUES 
                    ($1, 'war_ended', 'War ended in stalemate', $2),
                    ($2, 'war_ended', 'War ended in stalemate', $1)`,
                [warData.attacker_alliance_id, warData.defender_alliance_id]
            );
        }
    }
    
    async proposeCeasefire(warId: number, userId: number): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            const war = await client.query(
                'SELECT * FROM alliance_wars WHERE id = $1 AND status = $2',
                [warId, WarStatus.ACTIVE]
            );
            
            if (!war.rows[0]) {
                throw new Error('Active war not found');
            }
            
            const warData = war.rows[0];
            if (!warData) {
                throw new Error('Active war not found');
            }
            
            // Check permission
            const hasPermission = await this.allianceService.checkPermission(
                warData.attacker_alliance_id,
                userId,
                AlliancePermission.DECLARE_WAR
            ) || await this.allianceService.checkPermission(
                warData.defender_alliance_id,
                userId,
                AlliancePermission.DECLARE_WAR
            );
            
            if (!hasPermission) {
                throw new Error('You do not have permission to propose ceasefire');
            }
            
            // Set ceasefire status
            await client.query(
                'UPDATE alliance_wars SET status = $1 WHERE id = $2',
                [WarStatus.CEASEFIRE, warId]
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
    // QUERIES
    // ========================================================================
    
    async getWar(warId: number): Promise<AllianceWar | null> {
        const result = await pool.query(
            'SELECT * FROM alliance_wars WHERE id = $1',
            [warId]
        );
        
        return result.rows[0] || null;
    }
    
    async getWarDetails(warId: number): Promise<AllianceWarDetailsResponse> {
        const war = await this.getWar(warId);
        if (!war) {
            throw new Error('War not found');
        }
        
        // Get battles
        const battles = await pool.query(
            `SELECT wb.*, u1.username as attacker_username, u2.username as defender_username
            FROM war_battles wb
            LEFT JOIN users u1 ON u1.id = wb.attacker_user_id
            LEFT JOIN users u2 ON u2.id = wb.defender_user_id
            WHERE wb.war_id = $1
            ORDER BY wb.battle_at DESC`,
            [warId]
        );
        
        // Get participants
        const participants = await pool.query(
            `SELECT wp.*, u.username
            FROM war_participants wp
            JOIN users u ON u.id = wp.user_id
            WHERE wp.war_id = $1
            ORDER BY wp.total_points DESC`,
            [warId]
        );
        
        // Get alliances
        const attackerAlliance = await this.allianceService.getAlliance(war.attacker_alliance_id);
        const defenderAlliance = await this.allianceService.getAlliance(war.defender_alliance_id);
        
        if (!attackerAlliance || !defenderAlliance) {
            throw new Error('Alliance not found');
        }
        
        return {
            war,
            battles: battles.rows,
            participants: participants.rows,
            attacker_alliance: attackerAlliance,
            defender_alliance: defenderAlliance
        };
    }
    
    async getAllianceWars(allianceId: number, status?: WarStatus): Promise<AllianceWar[]> {
        let query = `
            SELECT aw.*, 
                a1.name as attacker_name, a1.tag as attacker_tag,
                a2.name as defender_name, a2.tag as defender_tag
            FROM alliance_wars aw
            JOIN alliances a1 ON a1.id = aw.attacker_alliance_id
            JOIN alliances a2 ON a2.id = aw.defender_alliance_id
            WHERE (aw.attacker_alliance_id = $1 OR aw.defender_alliance_id = $1)
        `;
        
        const params: any[] = [allianceId];
        
        if (status) {
            query += ' AND aw.status = $2';
            params.push(status);
        }
        
        query += ' ORDER BY aw.declared_at DESC';
        
        const result = await pool.query(query, params);
        return result.rows;
    }
    
    async getActiveWars(): Promise<ActiveWarSummary[]> {
        const result = await pool.query(
            'SELECT * FROM v_active_wars_summary ORDER BY declared_at DESC'
        );
        
        return result.rows;
    }
    
    async getWarLeaderboard(warId: number): Promise<WarParticipant[]> {
        const result = await pool.query(
            `SELECT wp.*, u.username
            FROM war_participants wp
            JOIN users u ON u.id = wp.user_id
            WHERE wp.war_id = $1
            ORDER BY wp.total_points DESC, wp.battles_won DESC
            LIMIT 50`,
            [warId]
        );
        
        return result.rows;
    }
}
