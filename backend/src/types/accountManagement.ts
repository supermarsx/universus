// Phase 9: Advanced Account Management System - TypeScript Types
// Complete type definitions for account security, sessions, verification,
// password recovery, 2FA, GDPR compliance, and audit logging

// =====================================================
// ENUMS
// =====================================================

export enum AccountStatus {
    ACTIVE = 'active',
    SUSPENDED = 'suspended',
    DELETED = 'deleted',
    LOCKED = 'locked'
}

export enum SuspensionReason {
    VIOLATION_TOS = 'violation_tos',
    VIOLATION_CONDUCT = 'violation_conduct',
    SPAM = 'spam',
    FRAUD = 'fraud',
    SECURITY_THREAT = 'security_threat',
    PAYMENT_DISPUTE = 'payment_dispute',
    ADMIN_ACTION = 'admin_action',
    OTHER = 'other'
}

export enum TransferStatus {
    PENDING = 'pending',
    VERIFIED = 'verified',
    COMPLETED = 'completed',
    CANCELLED = 'cancelled',
    EXPIRED = 'expired'
}

export enum VerificationStatus {
    PENDING = 'pending',
    VERIFIED = 'verified',
    EXPIRED = 'expired',
    FAILED = 'failed'
}

export enum ResetStatus {
    PENDING = 'pending',
    VALIDATED = 'validated',
    COMPLETED = 'completed',
    EXPIRED = 'expired',
    CANCELLED = 'cancelled'
}

export enum TwoFactorMethod {
    TOTP = 'totp',
    SMS = 'sms',
    EMAIL = 'email'
}

export enum SessionStatus {
    ACTIVE = 'active',
    EXPIRED = 'expired',
    TERMINATED = 'terminated',
    SUSPICIOUS = 'suspicious'
}

export enum SecurityEventType {
    LOGIN_SUCCESS = 'login_success',
    LOGIN_FAILED = 'login_failed',
    LOGOUT = 'logout',
    PASSWORD_CHANGE = 'password_change',
    PASSWORD_RESET = 'password_reset',
    EMAIL_CHANGE = 'email_change',
    EMAIL_VERIFIED = 'email_verified',
    TWO_FACTOR_ENABLED = 'two_factor_enabled',
    TWO_FACTOR_DISABLED = 'two_factor_disabled',
    ACCOUNT_SUSPENDED = 'account_suspended',
    ACCOUNT_DELETED = 'account_deleted',
    ACCOUNT_LOCKED = 'account_locked',
    SUSPICIOUS_ACTIVITY = 'suspicious_activity',
    GDPR_REQUEST = 'gdpr_request',
    DATA_EXPORT = 'data_export',
    DATA_DELETE = 'data_delete'
}

export enum SecurityEventSeverity {
    INFO = 'info',
    LOW = 'low',
    MEDIUM = 'medium',
    HIGH = 'high',
    CRITICAL = 'critical'
}

export enum GDPRRequestType {
    EXPORT_DATA = 'export_data',
    DELETE_DATA = 'delete_data',
    RESTRICT_PROCESSING = 'restrict_processing',
    DATA_PORTABILITY = 'data_portability',
    OBJECT_PROCESSING = 'object_processing',
    ACCESS_REQUEST = 'access_request'
}

export enum GDPRRequestStatus {
    PENDING = 'pending',
    PROCESSING = 'processing',
    COMPLETED = 'completed',
    FAILED = 'failed',
    CANCELLED = 'cancelled'
}

export enum BlockType {
    FULL = 'full',
    MUTE = 'mute',
    MESSAGES_ONLY = 'messages_only'
}

export enum ActivityType {
    LOGIN = 'login',
    LOGOUT = 'logout',
    PAGE_VIEW = 'page_view',
    RESOURCE_ACCESS = 'resource_access',
    SETTING_CHANGE = 'setting_change',
    PROFILE_UPDATE = 'profile_update'
}

// =====================================================
// CORE INTERFACES
// =====================================================

export interface AccountSuspension {
    id: number;
    user_id: number;
    reason: SuspensionReason;
    suspended_by: number;
    suspended_at: Date;
    expires_at?: Date;
    lifted_at?: Date;
    lifted_by?: number;
    notes?: string;
    is_active: boolean;
    created_at: Date;
    updated_at: Date;
}

export interface AccountTransfer {
    id: number;
    user_id: number;
    from_email: string;
    to_email: string;
    verification_token: string;
    status: TransferStatus;
    initiated_at: Date;
    verified_at?: Date;
    completed_at?: Date;
    cancelled_at?: Date;
    expires_at: Date;
    ip_address?: string;
    user_agent?: string;
    created_at: Date;
    updated_at: Date;
}

export interface EmailVerification {
    id: number;
    user_id: number;
    email: string;
    verification_token: string;
    status: VerificationStatus;
    sent_at: Date;
    verified_at?: Date;
    expires_at: Date;
    attempts: number;
    ip_address?: string;
    user_agent?: string;
    created_at: Date;
    updated_at: Date;
}

export interface PasswordReset {
    id: number;
    user_id: number;
    reset_token: string;
    status: ResetStatus;
    initiated_at: Date;
    validated_at?: Date;
    completed_at?: Date;
    expires_at: Date;
    ip_address?: string;
    user_agent?: string;
    created_at: Date;
    updated_at: Date;
}

export interface PasswordResetWithEmail extends PasswordReset {
    email: string;
}

export interface TwoFactorAuth {
    id: number;
    user_id: number;
    method: TwoFactorMethod;
    secret: string;
    is_enabled: boolean;
    verified_at?: Date;
    backup_codes?: string[];
    recovery_email?: string;
    last_used_at?: Date;
    created_at: Date;
    updated_at: Date;
}

export interface UserSession {
    id: number;
    user_id: number;
    session_token: string;
    device_fingerprint?: string;
    device_name?: string;
    device_type?: string;
    browser?: string;
    os?: string;
    ip_address: string;
    location?: string;
    latitude?: number;
    longitude?: number;
    status: SessionStatus;
    is_trusted: boolean;
    last_activity: Date;
    created_at: Date;
    expires_at: Date;
}

export interface SecurityAuditLog {
    id: number;
    user_id?: number;
    event_type: SecurityEventType;
    event_description: string;
    severity: SecurityEventSeverity;
    ip_address?: string;
    user_agent?: string;
    metadata?: Record<string, any>;
    created_at: Date;
}

export interface GDPRRequest {
    id: number;
    user_id: number;
    request_type: GDPRRequestType;
    status: GDPRRequestStatus;
    requested_at: Date;
    processed_at?: Date;
    completed_at?: Date;
    data_url?: string;
    expires_at?: Date;
    notes?: string;
    created_at: Date;
    updated_at: Date;
}

export interface UserBlock {
    id: number;
    user_id: number;
    blocked_user_id: number;
    block_type: BlockType;
    reason?: string;
    created_at: Date;
}

export interface UserActivityLog {
    id: number;
    user_id: number;
    activity_type: ActivityType;
    description?: string;
    ip_address?: string;
    metadata?: Record<string, any>;
    created_at: Date;
}

export interface AccountDataBackup {
    id: number;
    user_id: number;
    backup_data: Record<string, any>;
    backup_size: number;
    created_at: Date;
    expires_at?: Date;
}

export interface BackupVerificationCode {
    id: number;
    backup_id: number;
    verification_code: string;
    is_used: boolean;
    used_at?: Date;
    created_at: Date;
    expires_at: Date;
}

// =====================================================
// REQUEST/RESPONSE TYPES
// =====================================================

export interface SuspendAccountRequest {
    user_id: number;
    reason: SuspensionReason;
    admin_id: number;
    expires_at?: Date;
    notes?: string;
}

export interface InitiateTransferRequest {
    user_id: number;
    to_email: string;
    ip_address?: string;
    user_agent?: string;
}

export interface VerifyEmailRequest {
    verification_token: string;
    ip_address?: string;
    user_agent?: string;
}

export interface InitiatePasswordResetRequest {
    email: string;
    ip_address?: string;
    user_agent?: string;
}

export interface CompletePasswordResetRequest {
    reset_token: string;
    new_password: string;
    ip_address?: string;
    user_agent?: string;
}

export interface Setup2FARequest {
    user_id: number;
    method: TwoFactorMethod;
    recovery_email?: string;
}

export interface Verify2FARequest {
    user_id: number;
    code: string;
}

export interface CreateSessionRequest {
    user_id: number;
    ip_address: string;
    user_agent?: string;
    device_fingerprint?: string;
    device_name?: string;
    location?: string;
    latitude?: number;
    longitude?: number;
}

export interface LogSecurityEventRequest {
    user_id?: number;
    event_type: SecurityEventType;
    event_description: string;
    severity: SecurityEventSeverity;
    ip_address?: string;
    user_agent?: string;
    metadata?: Record<string, any>;
}

export interface CreateGDPRRequestRequest {
    user_id: number;
    request_type: GDPRRequestType;
    notes?: string;
}

export interface BlockUserRequest {
    user_id: number;
    blocked_user_id: number;
    block_type: BlockType;
    reason?: string;
}

export interface LogActivityRequest {
    user_id: number;
    activity_type: ActivityType;
    description?: string;
    ip_address?: string;
    metadata?: Record<string, any>;
}

// =====================================================
// RESPONSE TYPES
// =====================================================

export interface Setup2FAResponse {
    secret: string;
    qr_code: string;
    backup_codes: string[];
}

export interface SecuritySummaryResponse {
    user_id: number;
    account_status: AccountStatus;
    is_locked: boolean;
    email_verified: boolean;
    has_2fa: boolean;
    active_sessions: number;
    recent_security_events: number;
    risk_level: 'low' | 'medium' | 'high';
    last_login: Date;
    last_login_ip: string;
}

export interface SessionListResponse {
    sessions: UserSession[];
    total: number;
    active_count: number;
}

export interface SecurityAuditLogResponse {
    logs: SecurityAuditLog[];
    total: number;
    page: number;
    limit: number;
}

export interface GDPRDataExportResponse {
    user_data: Record<string, any>;
    export_date: Date;
    data_size: number;
    download_url?: string;
}

export interface AccountAccessCheck {
    can_access: boolean;
    reason?: string;
    suspension_expires?: Date;
}

// =====================================================
// UTILITY TYPES
// =====================================================

export interface DeviceInfo {
    fingerprint: string;
    name: string;
    type: 'desktop' | 'mobile' | 'tablet' | 'unknown';
    browser: string;
    os: string;
}

export interface LocationInfo {
    ip_address: string;
    location: string;
    latitude?: number;
    longitude?: number;
    country?: string;
    city?: string;
}

export interface SuspiciousActivityAlert {
    user_id: number;
    alert_type: string;
    description: string;
    severity: SecurityEventSeverity;
    detected_at: Date;
    ip_address?: string;
    location?: string;
}
