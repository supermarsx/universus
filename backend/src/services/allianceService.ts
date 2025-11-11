// Phase 11: Alliance Service - Core Alliance Management
// Handles alliance creation, membership, permissions, and general management

import { pool } from '../config/database';
import {
    Alliance,
    AllianceMember,
    AllianceApplication,
    AllianceRank,
    AlliancePermission,
    ApplicationStatus,
    AllianceDetailsResponse,
    AllianceDashboardResponse,
    AllianceAnnouncement,
    CreateAllianceRequest,
    UpdateAllianceRequest,
    JoinAllianceRequest,
    ManageMemberRequest,
    ContributeResourcesRequest,
    WithdrawResourcesRequest,
    AllianceStatistics,
    AllianceLeaderboard
} from '../types/alliance';
import { MessagingService } from './messagingService';

const messagingService = new MessagingService(pool);

export class AllianceService {
    
    // ========================================================================
    // ALLIANCE CREATION & MANAGEMENT
    // ========================================================================
    
    async createAlliance(userId: number, data: CreateAllianceRequest): Promise<Alliance> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check if user is already in an alliance
            const existingMember = await client.query(
                'SELECT alliance_id FROM alliance_members WHERE user_id = $1',
                [userId]
            );
            
            if (existingMember.rows.length > 0) {
                throw new Error('You are already in an alliance');
            }
            
            // Validate tag (3-6 characters, alphanumeric)
            if (!/^[A-Z0-9]{3,6}$/.test(data.tag)) {
                throw new Error('Alliance tag must be 3-6 uppercase alphanumeric characters');
            }
            
            // Create alliance
            const allianceResult = await client.query(
                `INSERT INTO alliances (
                    tag, name, description, founder_id,
                    is_open, is_recruiting, min_score_requirement,
                    auto_accept_min_score, auto_reject_below_score, auto_application_notes,
                    color_primary, color_secondary
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                RETURNING *`,
                [
                    data.tag.toUpperCase(),
                    data.name,
                    data.description || null,
                    userId,
                    data.is_open !== undefined ? data.is_open : false,
                    data.is_recruiting !== undefined ? data.is_recruiting : true,
                    data.min_score_requirement || 0,
                    data.auto_accept_min_score || null,
                    data.auto_reject_below_score || null,
                    data.auto_application_notes || null,
                    data.color_primary || '#00ff41',
                    data.color_secondary || '#008f11'
                ]
            );
            
            const alliance = allianceResult.rows[0];
            if (!alliance) {
                throw new Error('Failed to create alliance');
            }
            
            // Add founder as first member
            await client.query(
                `INSERT INTO alliance_members (alliance_id, user_id, rank)
                VALUES ($1, $2, $3)`,
                [alliance.id, userId, AllianceRank.FOUNDER]
            );
            
            // Create default permissions for officers
            const defaultPermissions = [
                AlliancePermission.MANAGE_MEMBERS,
                AlliancePermission.SEND_ANNOUNCEMENTS,
                AlliancePermission.VIEW_TREASURY,
                AlliancePermission.MANAGE_TERRITORY
            ];
            
            for (const permission of defaultPermissions) {
                await client.query(
                    `INSERT INTO alliance_rank_permissions (alliance_id, rank, permission, granted)
                    VALUES ($1, $2, $3, true)`,
                    [alliance.id, AllianceRank.OFFICER, permission]
                );
            }
            
            // Log history
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
                VALUES ($1, $2, $3, $4)`,
                [alliance.id, 'founded', 'Alliance founded', userId]
            );
            
            await client.query('COMMIT');
            return alliance;
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async getAlliance(allianceId: number): Promise<Alliance | null> {
        const result = await pool.query(
            'SELECT * FROM alliances WHERE id = $1 AND disbanded_at IS NULL',
            [allianceId]
        );
        
        return result.rows[0] || null;
    }
    
    async getAllianceByTag(tag: string): Promise<Alliance | null> {
        const result = await pool.query(
            'SELECT * FROM alliances WHERE UPPER(tag) = UPPER($1) AND disbanded_at IS NULL',
            [tag]
        );
        
        return result.rows[0] || null;
    }
    
    async updateAlliance(allianceId: number, userId: number, data: UpdateAllianceRequest): Promise<Alliance> {
        // Check permission
        const hasPermission = await this.checkPermission(allianceId, userId, AlliancePermission.MANAGE_RANKS);
        if (!hasPermission) {
            throw new Error('You do not have permission to update alliance settings');
        }
        
        const updateFields: string[] = [];
        const values: any[] = [];
        let paramIndex = 1;
        
        if (data.name !== undefined) {
            updateFields.push(`name = $${paramIndex++}`);
            values.push(data.name);
        }
        if (data.description !== undefined) {
            updateFields.push(`description = $${paramIndex++}`);
            values.push(data.description);
        }
        if (data.is_open !== undefined) {
            updateFields.push(`is_open = $${paramIndex++}`);
            values.push(data.is_open);
        }
        if (data.is_recruiting !== undefined) {
            updateFields.push(`is_recruiting = $${paramIndex++}`);
            values.push(data.is_recruiting);
        }
        if (data.min_score_requirement !== undefined) {
            updateFields.push(`min_score_requirement = $${paramIndex++}`);
            values.push(data.min_score_requirement);
        }
        if (data.auto_accept_min_score !== undefined) {
            updateFields.push(`auto_accept_min_score = $${paramIndex++}`);
            values.push(data.auto_accept_min_score);
        }
        if (data.auto_reject_below_score !== undefined) {
            updateFields.push(`auto_reject_below_score = $${paramIndex++}`);
            values.push(data.auto_reject_below_score);
        }
        if (data.auto_application_notes !== undefined) {
            updateFields.push(`auto_application_notes = $${paramIndex++}`);
            values.push(data.auto_application_notes);
        }
        if (data.logo_url !== undefined) {
            updateFields.push(`logo_url = $${paramIndex++}`);
            values.push(data.logo_url);
        }
        if (data.banner_url !== undefined) {
            updateFields.push(`banner_url = $${paramIndex++}`);
            values.push(data.banner_url);
        }
        if (data.color_primary !== undefined) {
            updateFields.push(`color_primary = $${paramIndex++}`);
            values.push(data.color_primary);
        }
        if (data.color_secondary !== undefined) {
            updateFields.push(`color_secondary = $${paramIndex++}`);
            values.push(data.color_secondary);
        }
        
        if (updateFields.length === 0) {
            const alliance = await this.getAlliance(allianceId);
            if (!alliance) throw new Error('Alliance not found');
            return alliance;
        }
        
        updateFields.push(`updated_at = CURRENT_TIMESTAMP`);
        values.push(allianceId);
        
        const result = await pool.query(
            `UPDATE alliances SET ${updateFields.join(', ')} WHERE id = $${paramIndex} RETURNING *`,
            values
        );
        
        if (!result.rows[0]) {
            throw new Error('Alliance not found after update');
        }
        return result.rows[0];
    }
    
    async disbandAlliance(allianceId: number, userId: number): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check if user is founder
            const member = await client.query(
                'SELECT rank FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
                [allianceId, userId]
            );
            
            if (!member.rows[0] || member.rows[0].rank !== AllianceRank.FOUNDER) {
                throw new Error('Only the founder can disband the alliance');
            }
            
            // Mark alliance as disbanded
            await client.query(
                'UPDATE alliances SET disbanded_at = CURRENT_TIMESTAMP WHERE id = $1',
                [allianceId]
            );
            
            // Remove all members
            await client.query(
                'DELETE FROM alliance_members WHERE alliance_id = $1',
                [allianceId]
            );
            
            // Log history
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
                VALUES ($1, $2, $3, $4)`,
                [allianceId, 'disbanded', 'Alliance disbanded by founder', userId]
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
    // MEMBERSHIP MANAGEMENT
    // ========================================================================
    
    async applyToAlliance(userId: number, data: JoinAllianceRequest): Promise<AllianceApplication> {
        // Check if user is already in an alliance
        const existingMember = await pool.query(
            'SELECT alliance_id FROM alliance_members WHERE user_id = $1',
            [userId]
        );
        
        if (existingMember.rows.length > 0) {
            throw new Error('You are already in an alliance');
        }
        
        // Check if alliance exists and is recruiting
        const alliance = await this.getAlliance(data.alliance_id);
        if (!alliance) {
            throw new Error('Alliance not found');
        }
        const applicantScore = await this.getPlayerScore(userId);

        if (alliance.auto_reject_below_score !== null && alliance.auto_reject_below_score !== undefined) {
            if (applicantScore < alliance.auto_reject_below_score) {
                throw new Error('Your score is below this alliance\'s requirements');
            }
        }
        
        const autoAccept =
            alliance.is_open ||
            (alliance.auto_accept_min_score !== null &&
                alliance.auto_accept_min_score !== undefined &&
                applicantScore >= alliance.auto_accept_min_score);

        if (autoAccept) {
            await this.addMember(data.alliance_id, userId, AllianceRank.RECRUIT);
            return {
                id: 0,
                alliance_id: data.alliance_id,
                user_id: userId,
                status: ApplicationStatus.ACCEPTED,
                created_at: new Date(),
                username: undefined,
                user_score: applicantScore
            };
        }
        
        // Check for existing pending application
        const existingApp = await pool.query(
            'SELECT id FROM alliance_applications WHERE alliance_id = $1 AND user_id = $2 AND status = $3',
            [data.alliance_id, userId, ApplicationStatus.PENDING]
        );
        
        if (existingApp.rows.length > 0) {
            throw new Error('You already have a pending application to this alliance');
        }
        
        // Create application
        const result = await pool.query(
            `INSERT INTO alliance_applications (alliance_id, user_id, message, status)
            VALUES ($1, $2, $3, $4)
            RETURNING *`,
            [data.alliance_id, userId, data.message || null, ApplicationStatus.PENDING]
        );
        
        if (!result.rows[0]) {
            throw new Error('Failed to process application');
        }
        return result.rows[0];
    }
    
    async processApplication(allianceId: number, applicationId: number, reviewerId: number, accept: boolean): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.checkPermission(allianceId, reviewerId, AlliancePermission.MANAGE_MEMBERS);
            if (!hasPermission) {
                throw new Error('You do not have permission to manage applications');
            }
            
            // Get application
            const appResult = await client.query(
                'SELECT * FROM alliance_applications WHERE id = $1 AND alliance_id = $2 AND status = $3',
                [applicationId, allianceId, ApplicationStatus.PENDING]
            );
            
            if (appResult.rows.length === 0) {
                throw new Error('Application not found or already processed');
            }
            
            const application = appResult.rows[0];
            
            if (accept) {
                // Add member
                await client.query(
                    `INSERT INTO alliance_members (alliance_id, user_id, rank)
                    VALUES ($1, $2, $3)`,
                    [allianceId, application.user_id, AllianceRank.RECRUIT]
                );
                
                // Update application
                await client.query(
                    `UPDATE alliance_applications 
                    SET status = $1, reviewed_by = $2, reviewed_at = CURRENT_TIMESTAMP
                    WHERE id = $3`,
                    [ApplicationStatus.ACCEPTED, reviewerId, applicationId]
                );
            } else {
                // Reject application
                await client.query(
                    `UPDATE alliance_applications 
                    SET status = $1, reviewed_by = $2, reviewed_at = CURRENT_TIMESTAMP
                    WHERE id = $3`,
                    [ApplicationStatus.REJECTED, reviewerId, applicationId]
                );
            }
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    async getAllianceApplications(allianceId: number, reviewerId: number): Promise<AllianceApplication[]> {
        const hasPermission = await this.checkPermission(
            allianceId,
            reviewerId,
            AlliancePermission.MANAGE_MEMBERS
        );
        if (!hasPermission) {
            throw new Error('You do not have permission to view applications');
        }

        const result = await pool.query(
            `SELECT aa.*, u.username, COALESCE(ps.total_score, 0) as user_score
             FROM alliance_applications aa
             JOIN users u ON u.id = aa.user_id
             LEFT JOIN player_scores ps ON ps.user_id = u.id
             WHERE aa.alliance_id = $1
             ORDER BY aa.status, aa.created_at ASC`,
            [allianceId]
        );

        return result.rows.map((row) => ({
            id: row.id,
            alliance_id: row.alliance_id,
            user_id: row.user_id,
            message: row.message,
            status: row.status,
            reviewed_by: row.reviewed_by,
            reviewed_at: row.reviewed_at,
            created_at: row.created_at,
            username: row.username,
            user_score: Number(row.user_score || 0)
        }));
    }
    
    async leaveAlliance(allianceId: number, userId: number): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check if user is founder
            const member = await client.query(
                'SELECT rank FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
                [allianceId, userId]
            );
            
            if (!member.rows[0]) {
                throw new Error('You are not a member of this alliance');
            }
            
            if (member.rows[0].rank === AllianceRank.FOUNDER) {
                throw new Error('Founder cannot leave. Transfer leadership or disband the alliance');
            }
            
            // Remove member
            await client.query(
                'DELETE FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
                [allianceId, userId]
            );
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async manageMember(allianceId: number, managerId: number, data: ManageMemberRequest): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.checkPermission(allianceId, managerId, AlliancePermission.MANAGE_MEMBERS);
            if (!hasPermission) {
                throw new Error('You do not have permission to manage members');
            }
            
            if (data.action === 'kick') {
                // Check KICK_MEMBERS permission
                const canKick = await this.checkPermission(allianceId, managerId, AlliancePermission.KICK_MEMBERS);
                if (!canKick) {
                    throw new Error('You do not have permission to kick members');
                }
                
                // Cannot kick founder
                const targetMember = await client.query(
                    'SELECT rank FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
                    [allianceId, data.user_id]
                );
                
                if (targetMember.rows[0]?.rank === AllianceRank.FOUNDER) {
                    throw new Error('Cannot kick the founder');
                }
                
                await client.query(
                    'DELETE FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
                    [allianceId, data.user_id]
                );
                
            } else if (data.action === 'promote' || data.action === 'demote') {
                if (!data.new_rank) {
                    throw new Error('New rank is required for promotion/demotion');
                }
                
                // Check MANAGE_RANKS permission for rank changes
                const canManageRanks = await this.checkPermission(allianceId, managerId, AlliancePermission.MANAGE_RANKS);
                if (!canManageRanks) {
                    throw new Error('You do not have permission to change ranks');
                }
                
                await client.query(
                    `UPDATE alliance_members 
                    SET rank = $1, promoted_at = CURRENT_TIMESTAMP
                    WHERE alliance_id = $2 AND user_id = $3`,
                    [data.new_rank, allianceId, data.user_id]
                );
            }
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    private async addMember(allianceId: number, userId: number, rank: AllianceRank): Promise<AllianceMember> {
        const result = await pool.query(
            `INSERT INTO alliance_members (alliance_id, user_id, rank)
            VALUES ($1, $2, $3)
            RETURNING *`,
            [allianceId, userId, rank]
        );
        
        if (!result.rows[0]) {
            throw new Error('Failed to add member');
        }
        return result.rows[0];
    }
    
    // ========================================================================
    // RESOURCES & TREASURY
    // ========================================================================
    
    async contributeResources(allianceId: number, userId: number, data: ContributeResourcesRequest): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check if user is member
            const member = await client.query(
                'SELECT id FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
                [allianceId, userId]
            );
            
            if (!member.rows[0]) {
                throw new Error('You are not a member of this alliance');
            }
            
            // Deduct from user
            const columnName = `${data.contribution_type}`;
            await client.query(
                `UPDATE users SET ${columnName} = ${columnName} - $1 WHERE id = $2 AND ${columnName} >= $1`,
                [data.amount, userId]
            );
            
            // Add to alliance treasury
            const treasuryColumn = `${data.contribution_type}_treasury`;
            await client.query(
                `UPDATE alliances SET ${treasuryColumn} = ${treasuryColumn} + $1 WHERE id = $2`,
                [data.amount, allianceId]
            );
            
            // Update member contribution stats
            const contributionColumn = `${data.contribution_type}_contributed`;
            await client.query(
                `UPDATE alliance_members 
                SET ${contributionColumn} = ${contributionColumn} + $1, last_contribution_at = CURRENT_TIMESTAMP
                WHERE alliance_id = $2 AND user_id = $3`,
                [data.amount, allianceId, userId]
            );
            
            // Log contribution
            await client.query(
                `INSERT INTO alliance_contributions (alliance_id, user_id, contribution_type, amount)
                VALUES ($1, $2, $3, $4)`,
                [allianceId, userId, data.contribution_type, data.amount]
            );
            
            await client.query('COMMIT');
            
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }
    
    async withdrawResources(allianceId: number, userId: number, data: WithdrawResourcesRequest): Promise<void> {
        const client = await pool.connect();
        
        try {
            await client.query('BEGIN');
            
            // Check permission
            const hasPermission = await this.checkPermission(allianceId, userId, AlliancePermission.WITHDRAW_RESOURCES);
            if (!hasPermission) {
                throw new Error('You do not have permission to withdraw resources');
            }
            
            // Deduct from alliance treasury
            const treasuryColumn = `${data.resource_type}_treasury`;
            const result = await client.query(
                `UPDATE alliances 
                SET ${treasuryColumn} = ${treasuryColumn} - $1 
                WHERE id = $2 AND ${treasuryColumn} >= $1
                RETURNING id`,
                [data.amount, allianceId]
            );
            
            if (!result.rows[0]) {
                throw new Error('Insufficient resources in alliance treasury');
            }
            
            // Add to user
            await client.query(
                `UPDATE users SET ${data.resource_type} = ${data.resource_type} + $1 WHERE id = $2`,
                [data.amount, userId]
            );
            
            // Log withdrawal
            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id, metadata)
                VALUES ($1, $2, $3, $4, $5)`,
                [
                    allianceId,
                    'resource_withdrawn',
                    `Withdrew ${data.amount} ${data.resource_type}`,
                    userId,
                    JSON.stringify({ reason: data.reason })
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

    async getMyAllianceDashboard(userId: number): Promise<AllianceDashboardResponse & { 
        alliance_id: number;
        tag: string;
        name: string;
        description?: string;
        founder_name?: string;
        founded_at?: Date;
        total_members: number;
        total_power: number;
        rank?: number | null;
        war_points?: number;
        territories_count?: number;
        diplomatic_relations_count?: number;
        members: any[];
        announcements: any[];
        recent_activity: any[];
        current_member_role: string;
    }> {
        const membershipResult = await pool.query(
            `SELECT alliance_id, rank FROM alliance_members WHERE user_id = $1`,
            [userId]
        );

        if (!membershipResult.rows.length) {
            throw new Error('NOT_IN_ALLIANCE');
        }

        const membership = membershipResult.rows[0];
        const allianceId = membership.alliance_id;

        const allianceResult = await pool.query(
            `SELECT a.*, u.username as founder_name
             FROM alliances a
             LEFT JOIN users u ON u.id = a.founder_id
             WHERE a.id = $1`,
            [allianceId]
        );

        if (!allianceResult.rows.length) {
            throw new Error('Alliance not found');
        }

        const allianceRow = allianceResult.rows[0];

        const rankResult = await pool.query(
            `SELECT rank_position FROM (
                SELECT id, RANK() OVER (ORDER BY total_score DESC) as rank_position
                FROM alliances
            ) ranked WHERE id = $1`,
            [allianceId]
        );

        const membersResult = await pool.query(
            `SELECT 
                am.user_id,
                am.rank,
                am.joined_at,
                am.metal_contributed,
                am.crystal_contributed,
                am.deuterium_contributed,
                COALESCE(ps.total_score, 0) as power,
                u.username,
                NULL::text as avatar_url,
                s.status = 'online' AS is_online
            FROM alliance_members am
            JOIN users u ON u.id = am.user_id
            LEFT JOIN player_scores ps ON ps.user_id = am.user_id
            LEFT JOIN player_status s ON s.user_id = am.user_id
            WHERE am.alliance_id = $1
            ORDER BY 
                CASE am.rank
                    WHEN 'founder' THEN 1
                    WHEN 'leader' THEN 2
                    WHEN 'officer' THEN 3
                    WHEN 'member' THEN 4
                    ELSE 5
                END,
                am.joined_at ASC`,
            [allianceId]
        );

        const historyResult = await pool.query(
            `SELECT id, event_type, description, related_user_id, created_at
             FROM alliance_history
             WHERE alliance_id = $1
             ORDER BY created_at DESC
             LIMIT 10`,
            [allianceId]
        );

        const diplomacyCountResult = await pool.query(
            `SELECT COUNT(*) as count FROM diplomatic_relations
             WHERE alliance_id = $1 AND terminated_at IS NULL`,
            [allianceId]
        );

        const territoryCountResult = await pool.query(
            `SELECT COUNT(*) as count FROM alliance_territories
             WHERE alliance_id = $1`,
            [allianceId]
        );

        const announcements = await this.getAnnouncements(allianceId, 10);

        const members = membersResult.rows.map((row) => ({
            id: row.user_id,
            alliance_id: allianceId,
            user_id: row.user_id,
            rank: row.rank,
            metal_contributed: Number(row.metal_contributed || 0),
            crystal_contributed: Number(row.crystal_contributed || 0),
            deuterium_contributed: Number(row.deuterium_contributed || 0),
            wars_participated: Number(row.wars_participated || 0),
            battles_won: Number(row.battles_won || 0),
            joined_at: row.joined_at,
            promoted_at: row.promoted_at,
            last_contribution_at: row.last_contribution_at,
            username: row.username,
            user_score: Number(row.power || 0),
            alliance: undefined
        }));

        const recentActivity = historyResult.rows.map((row) => ({
            id: row.id,
            type: row.event_type,
            message: row.description,
            related_user_id: row.related_user_id,
            timestamp: row.created_at,
            icon: this.getActivityIcon(row.event_type)
        }));

        const dashboard = {
            alliance: allianceRow,
            alliance_id: allianceRow.id,
            tag: allianceRow.tag,
            name: allianceRow.name,
            description: allianceRow.description,
            founder_name: allianceRow.founder_name || 'Unknown',
            founded_at: allianceRow.created_at,
            total_members: Number(allianceRow.total_members || members.length),
            total_power: Number(allianceRow.total_score || 0),
            rank: rankResult.rows[0]?.rank_position || null,
            leaderboard_position: Number(rankResult.rows[0]?.rank_position || 0),
            war_points: Number(allianceRow.war_points || 0),
            territories_count: Number(territoryCountResult.rows[0]?.count || 0),
            diplomatic_relations_count: Number(diplomacyCountResult.rows[0]?.count || 0),
            treasury: {
                metal: Number(allianceRow.metal_treasury || 0),
                crystal: Number(allianceRow.crystal_treasury || 0),
                deuterium: Number(allianceRow.deuterium_treasury || 0)
            },
            depot_settings: {
                refuel_rate: 1,
                max_docked_fleets: Math.max(1, Math.floor(members.length / 5) || 1),
                allow_allies: true,
            },
            members,
            recent_members: members.slice(0, 5),
            announcements: announcements.map((announcement) => ({
                ...announcement,
                message: announcement.content
            })),
            recent_activity: recentActivity,
            active_wars: [],
            recent_messages: [],
            territories: [],
            current_member_role: (membership.rank || '').toUpperCase(),
            user_rank: membership.rank,
            user_permissions: await this.getUserPermissions(allianceId, userId)
        };

        return dashboard;
    }

    async getAnnouncements(allianceId: number, limit: number = 10): Promise<AllianceAnnouncement[]> {
        const result = await pool.query(
            `SELECT 
                aa.*,
                u.username as creator_username,
                am.rank as creator_rank
             FROM alliance_announcements aa
             LEFT JOIN users u ON u.id = aa.created_by
             LEFT JOIN alliance_members am ON am.user_id = aa.created_by AND am.alliance_id = aa.alliance_id
             WHERE aa.alliance_id = $1
             ORDER BY aa.is_pinned DESC, aa.created_at DESC
             LIMIT $2`,
            [allianceId, limit]
        );

        return result.rows.map((row) => this.mapAnnouncementRow(row));
    }

    async createAnnouncement(
        allianceId: number,
        userId: number,
        data: { title: string; content: string; is_pinned?: boolean; broadcast?: boolean; metadata?: any }
    ): Promise<AllianceAnnouncement> {
        const title = (data.title || '').trim();
        const content = (data.content || '').trim();

        if (!title || !content) {
            throw new Error('Announcement title and content are required');
        }

        const hasPermission = await this.checkPermission(allianceId, userId, AlliancePermission.SEND_ANNOUNCEMENTS);
        if (!hasPermission) {
            throw new Error('You do not have permission to send alliance announcements');
        }

        const client = await pool.connect();

        try {
            await client.query('BEGIN');

            const result = await client.query(
                `INSERT INTO alliance_announcements (
                    alliance_id, title, content, is_pinned, created_by, metadata, pinned_at
                ) VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $4 THEN CURRENT_TIMESTAMP ELSE NULL END)
                RETURNING *`,
                [
                    allianceId,
                    title,
                    content,
                    data.is_pinned ?? false,
                    userId,
                    data.metadata ? JSON.stringify(data.metadata) : null
                ]
            );

            await client.query(
                `INSERT INTO alliance_history (alliance_id, event_type, description, related_user_id)
                 VALUES ($1, $2, $3, $4)`,
                [allianceId, 'announcement_posted', `New announcement: ${data.title}`, userId]
            );

            await client.query('COMMIT');

            const announcement = await this.getAnnouncementById(result.rows[0].id);

            if (announcement && data.broadcast !== false) {
                try {
                    await messagingService.sendAllianceCircular(allianceId, userId, title, content);
                } catch (err) {
                    console.error('Failed to broadcast alliance circular:', err);
                }
            }

            return announcement!;
        } catch (error) {
            await client.query('ROLLBACK');
            throw error;
        } finally {
            client.release();
        }
    }

    private async getAnnouncementById(announcementId: number): Promise<AllianceAnnouncement | null> {
        const result = await pool.query(
            `SELECT 
                aa.*,
                u.username as creator_username,
                am.rank as creator_rank
             FROM alliance_announcements aa
             LEFT JOIN users u ON u.id = aa.created_by
             LEFT JOIN alliance_members am ON am.user_id = aa.created_by AND am.alliance_id = aa.alliance_id
             WHERE aa.id = $1`,
            [announcementId]
        );

        if (!result.rows.length) {
            return null;
        }

        return this.mapAnnouncementRow(result.rows[0]);
    }

    private mapAnnouncementRow(row: any): AllianceAnnouncement {
        return {
            id: row.id,
            alliance_id: row.alliance_id,
            title: row.title,
            content: row.content,
            is_pinned: row.is_pinned,
            created_by: row.created_by,
            author_name: row.creator_username || null,
            author_role: row.creator_rank ? row.creator_rank.toUpperCase() : undefined,
            created_by_username: row.creator_username || null,
            created_at: row.created_at,
            pinned_at: row.pinned_at,
            metadata: row.metadata,
        };
    }

    private getActivityIcon(eventType: string): string {
        switch (eventType) {
            case 'member_joined':
                return 'user-add';
            case 'member_left':
                return 'user-remove';
            case 'announcement_posted':
                return 'broadcast';
            case 'resource_contribution':
                return 'crystal';
            case 'war_declared':
                return 'combat';
            default:
                return 'activity';
        }
    }

    private async getPlayerScore(userId: number): Promise<number> {
        const result = await pool.query(
            'SELECT total_score FROM player_scores WHERE user_id = $1',
            [userId]
        );
        if (result.rows.length) {
            return Number(result.rows[0].total_score || 0);
        }

        return 0;
    }
    
    // ========================================================================
    // PERMISSIONS
    // ========================================================================
    
    async checkPermission(allianceId: number, userId: number, permission: AlliancePermission): Promise<boolean> {
        const result = await pool.query(
            'SELECT check_alliance_permission($1, $2, $3) as has_permission',
            [allianceId, userId, permission]
        );
        
        return result.rows[0]?.has_permission || false;
    }
    
    async getUserPermissions(allianceId: number, userId: number): Promise<AlliancePermission[]> {
        const member = await pool.query(
            'SELECT rank FROM alliance_members WHERE alliance_id = $1 AND user_id = $2',
            [allianceId, userId]
        );
        
        if (!member.rows[0]) {
            return [];
        }
        
        const rank = member.rows[0].rank;
        
        // Founder and leaders have all permissions
        if (rank === AllianceRank.FOUNDER || rank === AllianceRank.LEADER) {
            return Object.values(AlliancePermission);
        }
        
        // Get rank-specific permissions
        const result = await pool.query(
            'SELECT permission FROM alliance_rank_permissions WHERE alliance_id = $1 AND rank = $2 AND granted = true',
            [allianceId, rank]
        );
        
        return result.rows.map(row => row.permission);
    }
    
    // ========================================================================
    // QUERIES & ANALYTICS
    // ========================================================================
    
    async getAllianceDetails(allianceId: number, viewerId?: number): Promise<AllianceDetailsResponse> {
        const alliance = await this.getAlliance(allianceId);
        if (!alliance) {
            throw new Error('Alliance not found');
        }
        
        // Get members
        const membersResult = await pool.query(
            `SELECT am.*, u.username, u.score as user_score
            FROM alliance_members am
            JOIN users u ON u.id = am.user_id
            WHERE am.alliance_id = $1
            ORDER BY 
                CASE am.rank
                    WHEN 'founder' THEN 1
                    WHEN 'leader' THEN 2
                    WHEN 'officer' THEN 3
                    WHEN 'member' THEN 4
                    WHEN 'recruit' THEN 5
                END,
                am.joined_at ASC`,
            [allianceId]
        );
        
        // Get diplomatic relations
        const relationsResult = await pool.query(
            `SELECT dr.*, a.tag as target_tag, a.name as target_name
            FROM diplomatic_relations dr
            JOIN alliances a ON a.id = dr.target_alliance_id
            WHERE dr.alliance_id = $1 AND dr.terminated_at IS NULL`,
            [allianceId]
        );
        
        // Get active wars count
        const warsResult = await pool.query(
            `SELECT COUNT(*) as count FROM alliance_wars
            WHERE (attacker_alliance_id = $1 OR defender_alliance_id = $1)
            AND status IN ('declared', 'active')`,
            [allianceId]
        );
        
        let userRank: AllianceRank | undefined;
        let userPermissions: AlliancePermission[] = [];
        let isMember = false;
        
        if (viewerId) {
            const memberCheck = membersResult.rows.find(m => m.user_id === viewerId);
            if (memberCheck) {
                isMember = true;
                userRank = memberCheck.rank;
                userPermissions = await this.getUserPermissions(allianceId, viewerId);
            }
        }
        
        return {
            alliance,
            members: membersResult.rows,
            member_count: membersResult.rows.length,
            is_member: isMember,
            user_rank: userRank,
            user_permissions: userPermissions,
            active_wars: warsResult.rows[0]?.count || 0,
            diplomatic_relations: relationsResult.rows
        };
    }
    
    async getAllianceLeaderboard(limit: number = 100, offset: number = 0): Promise<AllianceLeaderboard[]> {
        const result = await pool.query(
            'SELECT * FROM v_alliance_leaderboard LIMIT $1 OFFSET $2',
            [limit, offset]
        );
        
        return result.rows;
    }
    
    async getAllianceStatistics(allianceId: number): Promise<AllianceStatistics> {
        const alliance = await this.getAlliance(allianceId);
        if (!alliance) {
            throw new Error('Alliance not found');
        }
        
        // Get wars statistics
        const warsResult = await pool.query(
            `SELECT 
                COUNT(*) FILTER (WHERE winner_alliance_id = $1) as wars_won,
                COUNT(*) FILTER (WHERE (attacker_alliance_id = $1 OR defender_alliance_id = $1) AND winner_alliance_id != $1 AND winner_alliance_id IS NOT NULL) as wars_lost,
                COUNT(*) FILTER (WHERE (attacker_alliance_id = $1 OR defender_alliance_id = $1) AND status IN ('declared', 'active')) as wars_active
            FROM alliance_wars
            WHERE attacker_alliance_id = $1 OR defender_alliance_id = $1`,
            [allianceId]
        );
        
        // Get total contributions
        const contributionsResult = await pool.query(
            `SELECT 
                SUM(metal_contributed) as total_metal,
                SUM(crystal_contributed) as total_crystal,
                SUM(deuterium_contributed) as total_deuterium
            FROM alliance_members
            WHERE alliance_id = $1`,
            [allianceId]
        );
        
        // Get territories count
        const territoriesResult = await pool.query(
            'SELECT COUNT(*) as count FROM alliance_territories WHERE alliance_id = $1',
            [allianceId]
        );
        
        // Get diplomatic relations count
        const relationsResult = await pool.query(
            'SELECT COUNT(*) as count FROM diplomatic_relations WHERE alliance_id = $1 AND terminated_at IS NULL',
            [allianceId]
        );
        
        const wars = warsResult.rows[0];
        const contributions = contributionsResult.rows[0];
        
        return {
            total_members: alliance.total_members,
            total_score: Number(alliance.total_score),
            average_score_per_member: alliance.total_members > 0 
                ? Math.floor(Number(alliance.total_score) / alliance.total_members)
                : 0,
            total_planets: alliance.total_planets,
            wars_won: Number(wars.wars_won) || 0,
            wars_lost: Number(wars.wars_lost) || 0,
            wars_active: Number(wars.wars_active) || 0,
            total_contributions: {
                metal: Number(contributions.total_metal) || 0,
                crystal: Number(contributions.total_crystal) || 0,
                deuterium: Number(contributions.total_deuterium) || 0
            },
            territories_controlled: Number(territoriesResult.rows[0]?.count) || 0,
            diplomatic_relations_count: Number(relationsResult.rows[0]?.count) || 0
        };
    }
    
    async searchAlliances(query: string, limit: number = 20): Promise<Alliance[]> {
        const result = await pool.query(
            `SELECT * FROM alliances 
            WHERE disbanded_at IS NULL 
            AND (
                LOWER(tag) LIKE LOWER($1) 
                OR LOWER(name) LIKE LOWER($1)
            )
            ORDER BY total_score DESC
            LIMIT $2`,
            [`%${query}%`, limit]
        );
        
        return result.rows;
    }
}
