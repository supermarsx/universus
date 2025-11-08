// Phase 11: Enhanced Alliance Management System - TypeScript Types
// Complete type definitions for alliance system

// ============================================================================
// ENUMS
// ============================================================================

export enum AllianceRank {
    FOUNDER = 'founder',
    LEADER = 'leader',
    OFFICER = 'officer',
    MEMBER = 'member',
    RECRUIT = 'recruit'
}

export enum AlliancePermission {
    MANAGE_MEMBERS = 'manage_members',
    MANAGE_RANKS = 'manage_ranks',
    DECLARE_WAR = 'declare_war',
    MANAGE_DIPLOMACY = 'manage_diplomacy',
    MANAGE_RESOURCES = 'manage_resources',
    SEND_ANNOUNCEMENTS = 'send_announcements',
    VIEW_TREASURY = 'view_treasury',
    WITHDRAW_RESOURCES = 'withdraw_resources',
    MANAGE_TERRITORY = 'manage_territory',
    KICK_MEMBERS = 'kick_members'
}

export enum WarStatus {
    DECLARED = 'declared',
    ACTIVE = 'active',
    CEASEFIRE = 'ceasefire',
    ENDED = 'ended'
}

export enum DiplomaticStatus {
    NEUTRAL = 'neutral',
    NAP = 'nap', // Non-Aggression Pact
    ALLIANCE = 'alliance',
    TRADE = 'trade',
    DEFENSE_PACT = 'defense_pact',
    WAR = 'war',
    HOSTILE = 'hostile'
}

export enum ApplicationStatus {
    PENDING = 'pending',
    ACCEPTED = 'accepted',
    REJECTED = 'rejected'
}

export enum EventType {
    COMPETITION = 'competition',
    RAID = 'raid',
    DEFENSE = 'defense',
    TOURNAMENT = 'tournament'
}

export enum EventStatus {
    UPCOMING = 'upcoming',
    ACTIVE = 'active',
    COMPLETED = 'completed',
    CANCELLED = 'cancelled'
}

// ============================================================================
// CORE ALLIANCE INTERFACES
// ============================================================================

export interface Alliance {
    id: number;
    tag: string;
    name: string;
    description?: string;
    founder_id?: number;
    logo_url?: string;
    banner_url?: string;
    color_primary: string;
    color_secondary: string;
    
    // Settings
    is_open: boolean;
    is_recruiting: boolean;
    min_score_requirement: number;
    auto_accept_min_score?: number | null;
    auto_reject_below_score?: number | null;
    auto_application_notes?: string | null;
    depot_settings?: {
        refuel_rate: number;
        max_docked_fleets: number;
        allow_allies: boolean;
    };
    
    // Statistics
    total_members: number;
    total_score: number;
    total_planets: number;
    total_fleets: number;
    
    // Treasury
    metal_treasury: number;
    crystal_treasury: number;
    deuterium_treasury: number;
    
    // Metadata
    created_at: Date;
    updated_at: Date;
    disbanded_at?: Date;
}

export interface AllianceMember {
    id: number;
    alliance_id: number;
    user_id: number;
    rank: AllianceRank;
    
    // Contributions
    metal_contributed: number;
    crystal_contributed: number;
    deuterium_contributed: number;
    wars_participated: number;
    battles_won: number;
    
    // Metadata
    joined_at: Date;
    promoted_at?: Date;
    last_contribution_at?: Date;
    
    // Joined data (when fetched with relations)
    username?: string;
    user_score?: number;
    alliance?: Alliance;
}

export interface AllianceRankPermission {
    id: number;
    alliance_id: number;
    rank: AllianceRank;
    permission: AlliancePermission;
    granted: boolean;
}

export interface AllianceApplication {
    id: number;
    alliance_id: number;
    user_id: number;
    message?: string;
    status: ApplicationStatus;
    
    reviewed_by?: number;
    reviewed_at?: Date;
    created_at: Date;
    
    // Joined data
    username?: string;
    user_score?: number;
    alliance_name?: string;
}

// ============================================================================
// ALLIANCE WARS
// ============================================================================

export interface AllianceWar {
    id: number;
    attacker_alliance_id: number;
    defender_alliance_id: number;
    
    declaration_message?: string;
    status: WarStatus;
    
    // Victory conditions
    victory_condition: string;
    victory_threshold: number;
    
    // Statistics
    attacker_score: number;
    defender_score: number;
    total_battles: number;
    
    // Timestamps
    declared_at: Date;
    started_at?: Date;
    ended_at?: Date;
    
    // Result
    winner_alliance_id?: number;
    end_reason?: string;
    
    // Joined data
    attacker_alliance?: Alliance;
    defender_alliance?: Alliance;
    winner_alliance?: Alliance;
}

export interface WarBattle {
    id: number;
    war_id: number;
    
    attacker_user_id: number;
    defender_user_id: number;
    attacker_alliance_id: number;
    defender_alliance_id: number;
    
    // Battle details
    combat_id?: number;
    winner_user_id?: number;
    winner_alliance_id?: number;
    
    // Points awarded
    attacker_points: number;
    defender_points: number;
    
    // Loot and losses
    attacker_losses: number;
    defender_losses: number;
    loot_metal: number;
    loot_crystal: number;
    loot_deuterium: number;
    
    battle_at: Date;
}

export interface WarParticipant {
    id: number;
    war_id: number;
    user_id: number;
    alliance_id: number;
    
    battles_fought: number;
    battles_won: number;
    total_points: number;
    total_damage: number;
    
    // Joined data
    username?: string;
}

// ============================================================================
// DIPLOMATIC RELATIONS
// ============================================================================

export interface DiplomaticRelation {
    id: number;
    alliance_id: number;
    target_alliance_id: number;
    
    status: DiplomaticStatus;
    
    // Treaty details
    treaty_terms?: string;
    treaty_duration_days?: number;
    
    // Metadata
    proposed_by?: number;
    approved_by?: number;
    
    established_at: Date;
    expires_at?: Date;
    terminated_at?: Date;
    
    // Joined data
    alliance?: Alliance;
    target_alliance?: Alliance;
}

export interface DiplomaticProposal {
    id: number;
    from_alliance_id: number;
    to_alliance_id: number;
    
    proposed_status: DiplomaticStatus;
    terms?: string;
    duration_days?: number;
    
    status: string;
    
    proposed_by: number;
    reviewed_by?: number;
    
    created_at: Date;
    reviewed_at?: Date;
    expires_at: Date;
    
    // Joined data
    from_alliance?: Alliance;
    to_alliance?: Alliance;
    proposer_username?: string;
}

// ============================================================================
// RESOURCES & CONTRIBUTIONS
// ============================================================================

export interface AllianceContribution {
    id: number;
    alliance_id: number;
    user_id: number;
    
    contribution_type: string;
    amount: number;
    
    contributed_at: Date;
    
    // Joined data
    username?: string;
}

export interface AllianceResearch {
    id: number;
    alliance_id: number;
    
    research_name: string;
    level: number;
    
    // Cost and benefits
    total_cost_metal?: number;
    total_cost_crystal?: number;
    total_cost_deuterium?: number;
    
    bonus_description?: string;
    
    started_at?: Date;
    completed_at?: Date;
}

// ============================================================================
// TERRITORIES
// ============================================================================

export interface AllianceTerritory {
    id: number;
    alliance_id: number;
    
    galaxy: number;
    system: number;
    
    control_percentage: number;
    planets_controlled: number;
    total_planets: number;
    
    claimed_at: Date;
    updated_at: Date;
    
    // Joined data
    alliance?: Alliance;
}

export interface TerritoryControlLog {
    id: number;
    alliance_id?: number;
    
    galaxy: number;
    system: number;
    
    action: string;
    control_change?: number;
    
    created_at: Date;
}

// ============================================================================
// COMMUNICATIONS
// ============================================================================

export interface AllianceMessage {
    id: number;
    alliance_id: number;
    sender_id: number;
    
    message_type: string;
    subject?: string;
    content: string;
    
    is_pinned: boolean;
    min_rank?: AllianceRank;
    
    created_at: Date;
    updated_at: Date;
    
    // Joined data
    sender_username?: string;
    sender_rank?: AllianceRank;
    reactions_count?: number;
}

export interface AllianceMessageReaction {
    id: number;
    message_id: number;
    user_id: number;
    
    reaction_type: string;
    
    created_at: Date;
}

// ============================================================================
// EVENTS & COMPETITIONS
// ============================================================================

export interface AllianceEvent {
    id: number;
    
    event_type: string;
    event_name: string;
    description?: string;
    
    // Participants
    participating_alliance_ids?: number[];
    
    // Objectives
    objective_type?: string;
    objective_target?: number;
    
    // Rewards
    reward_metal: number;
    reward_crystal: number;
    reward_deuterium: number;
    reward_description?: string;
    
    // Status
    status: EventStatus;
    
    start_at: Date;
    end_at: Date;
    created_at: Date;
    
    winner_alliance_id?: number;
    
    // Joined data
    winner_alliance?: Alliance;
}

export interface AllianceEventParticipation {
    id: number;
    event_id: number;
    alliance_id: number;
    
    score: number;
    rank?: number;
    
    rewards_claimed: boolean;
    
    // Joined data
    alliance?: Alliance;
    event?: AllianceEvent;
}

// ============================================================================
// ACHIEVEMENTS & HISTORY
// ============================================================================

export interface AllianceAchievement {
    id: number;
    alliance_id: number;
    
    achievement_type: string;
    achievement_name: string;
    description?: string;
    
    achieved_at: Date;
}

export interface AllianceHistory {
    id: number;
    alliance_id: number;
    
    event_type: string;
    description: string;
    
    related_user_id?: number;
    related_alliance_id?: number;
    
    metadata?: any;
    
    created_at: Date;
    
    // Joined data
    related_username?: string;
    related_alliance_name?: string;
}

// ============================================================================
// VIEW INTERFACES
// ============================================================================

export interface AllianceLeaderboard {
    id: number;
    tag: string;
    name: string;
    total_members: number;
    total_score: number;
    total_planets: number;
    total_fleets: number;
    wars_won: number;
    active_wars: number;
    rank: number;
}

export interface AllianceMemberActivity {
    alliance_id: number;
    user_id: number;
    username: string;
    rank: AllianceRank;
    total_contributed: number;
    wars_participated: number;
    battles_won: number;
    joined_at: Date;
    days_in_alliance: number;
}

export interface ActiveWarSummary {
    id: number;
    status: WarStatus;
    attacker_tag: string;
    attacker_name: string;
    defender_tag: string;
    defender_name: string;
    attacker_score: number;
    defender_score: number;
    total_battles: number;
    declared_at: Date;
    started_at?: Date;
    current_leader: string;
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

// Alliance Management
export interface CreateAllianceRequest {
    tag: string;
    name: string;
    description?: string;
    is_open?: boolean;
    is_recruiting?: boolean;
    min_score_requirement?: number;
    auto_accept_min_score?: number;
    auto_reject_below_score?: number;
    auto_application_notes?: string;
    color_primary?: string;
    color_secondary?: string;
}

export interface UpdateAllianceRequest {
    name?: string;
    description?: string;
    is_open?: boolean;
    is_recruiting?: boolean;
    min_score_requirement?: number;
    auto_accept_min_score?: number | null;
    auto_reject_below_score?: number | null;
    auto_application_notes?: string | null;
    logo_url?: string;
    banner_url?: string;
    color_primary?: string;
    color_secondary?: string;
}

export interface JoinAllianceRequest {
    alliance_id: number;
    message?: string;
}

export interface ManageMemberRequest {
    user_id: number;
    action: 'promote' | 'demote' | 'kick';
    new_rank?: AllianceRank;
    reason?: string;
}

export interface ContributeResourcesRequest {
    contribution_type: 'metal' | 'crystal' | 'deuterium';
    amount: number;
}

export interface WithdrawResourcesRequest {
    resource_type: 'metal' | 'crystal' | 'deuterium';
    amount: number;
    reason?: string;
}

// War Management
export interface DeclareWarRequest {
    defender_alliance_id: number;
    declaration_message?: string;
    victory_condition?: string;
    victory_threshold?: number;
}

export interface RecordWarBattleRequest {
    war_id: number;
    combat_id: number;
    defender_user_id: number;
    winner_user_id: number;
    attacker_losses: number;
    defender_losses: number;
    loot_metal: number;
    loot_crystal: number;
    loot_deuterium: number;
}

export interface EndWarRequest {
    war_id: number;
    end_reason: string;
}

// Diplomacy
export interface ProposeDiplomacyRequest {
    target_alliance_id: number;
    proposed_status: DiplomaticStatus;
    terms?: string;
    duration_days?: number;
}

export interface RespondToProposalRequest {
    proposal_id: number;
    accept: boolean;
    response_message?: string;
}

// Messages
export interface SendAllianceMessageRequest {
    message_type?: string;
    subject?: string;
    content: string;
    is_pinned?: boolean;
    min_rank?: AllianceRank;
}

// Territory
export interface ClaimTerritoryRequest {
    galaxy: number;
    system: number;
}

// ============================================================================
// RESPONSE TYPES
// ============================================================================

export interface AllianceDetailsResponse {
    alliance: Alliance;
    members: AllianceMember[];
    member_count: number;
    is_member: boolean;
    user_rank?: AllianceRank;
    user_permissions?: AlliancePermission[];
    active_wars: number;
    diplomatic_relations: DiplomaticRelation[];
}

export interface AllianceWarDetailsResponse {
    war: AllianceWar;
    battles: WarBattle[];
    participants: WarParticipant[];
    attacker_alliance: Alliance;
    defender_alliance: Alliance;
}

export interface AllianceDashboardResponse {
    alliance: Alliance;
    user_rank: AllianceRank;
    user_permissions: AlliancePermission[];
    recent_members: AllianceMember[];
    announcements: AllianceAnnouncement[];
    members: AllianceMember[];
    recent_activity: any[];
    treasury: {
        metal: number;
        crystal: number;
        deuterium: number;
    };
    active_wars: AllianceWar[];
    recent_messages: AllianceMessage[];
    territories: AllianceTerritory[];
    leaderboard_position: number;
    current_member_role: string;
    war_points?: number;
    territories_count?: number;
    diplomatic_relations_count?: number;
    tag?: string;
    name?: string;
    description?: string;
}

export interface AllianceLeaderboardResponse {
    leaderboard: AllianceLeaderboard[];
    total_alliances: number;
    user_alliance_rank?: number;
}

export interface AllianceStatistics {
    total_members: number;
    total_score: number;
    average_score_per_member: number;
    total_planets: number;
    wars_won: number;
    wars_lost: number;
    wars_active: number;
    total_contributions: {
        metal: number;
        crystal: number;
        deuterium: number;
    };
    territories_controlled: number;
    diplomatic_relations_count: number;
}

export interface AllianceAnnouncement {
    id: number;
    alliance_id: number;
    title: string;
    content: string;
    is_pinned: boolean;
    created_by?: number;
    author_name?: string;
    author_role?: string;
    created_by_username?: string;
    created_at: Date;
    pinned_at?: Date;
    metadata?: any;
}
