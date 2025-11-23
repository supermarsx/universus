import Database from 'better-sqlite3';

export interface HistoryRecord {
    id: number;
    request_id: string;
    idempotency_key?: string | null;
    contact: string;
    destination: string;
    channel: string;
    status: 'success' | 'failed';
    error?: string | null;
    metadata?: Record<string, any> | null;
    created_at: string;
}

const dbPath = process.env.SMS_HISTORY_DB_PATH || 'sms-history.db';
const db = new Database(dbPath);

db.exec(`
    CREATE TABLE IF NOT EXISTS sms_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        request_id TEXT NOT NULL,
        idempotency_key TEXT,
        contact TEXT NOT NULL,
        destination TEXT NOT NULL,
        channel TEXT NOT NULL,
        status TEXT NOT NULL,
        error TEXT,
        metadata TEXT,
        created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
    );
`);

const tableInfo = db.prepare(`PRAGMA table_info(sms_history)`).all();
const hasIdempotencyColumn = tableInfo.some((column: any) => column.name === 'idempotency_key');
if (!hasIdempotencyColumn) {
    db.exec(`ALTER TABLE sms_history ADD COLUMN idempotency_key TEXT;`);
}

db.exec(`
    CREATE INDEX IF NOT EXISTS idx_sms_history_contact ON sms_history(contact);
`);

db.exec(`
    CREATE UNIQUE INDEX IF NOT EXISTS idx_sms_history_idempotency
    ON sms_history(idempotency_key)
    WHERE idempotency_key IS NOT NULL;
`);

const insertStmt = db.prepare(`
    INSERT INTO sms_history (request_id, idempotency_key, contact, destination, channel, status, error, metadata)
    VALUES (@request_id, @idempotency_key, @contact, @destination, @channel, @status, @error, @metadata)
`);

const recentStmt = db.prepare(`
    SELECT * FROM sms_history
    ORDER BY id DESC
    LIMIT ?
`);

const statsStmt = db.prepare(`
    SELECT channel, status, COUNT(*) as count
    FROM sms_history
    GROUP BY channel, status
`);

const findByIdempotencyStmt = db.prepare(`
    SELECT * FROM sms_history
    WHERE idempotency_key = ?
    ORDER BY id DESC
    LIMIT 1
`);

const rateLimitStmt = db.prepare(`
    SELECT COUNT(*) as count
    FROM sms_history
    WHERE contact = ?
      AND created_at >= datetime('now', ?)
`);

export interface RecordHistoryOptions {
    requestId: string;
    idempotencyKey?: string;
    contact: string;
    destination: string;
    channel: string;
    status: 'success' | 'failed';
    error?: string;
    metadata?: Record<string, any>;
}

export function recordHistory(entry: RecordHistoryOptions): void {
    insertStmt.run({
        request_id: entry.requestId,
        idempotency_key: entry.idempotencyKey || null,
        contact: entry.contact,
        destination: entry.destination,
        channel: entry.channel,
        status: entry.status,
        error: entry.error,
        metadata: entry.metadata ? JSON.stringify(entry.metadata) : null
    });
}

export function getRecentHistory(limit: number = 50): HistoryRecord[] {
    const rows = recentStmt.all(limit);
    return rows.map((row: any) => ({
        ...row,
        metadata: row.metadata ? safeJson(row.metadata) : null
    }));
}

function safeJson(payload: string): Record<string, any> | null {
    try {
        return JSON.parse(payload);
    } catch {
        return null;
    }
}

export function getHistoryStats(): Array<{ channel: string; status: string; count: number }> {
    return statsStmt.all().map((row: any) => ({
        channel: row.channel,
        status: row.status,
        count: Number(row.count)
    }));
}

export function findHistoryByIdempotency(idempotencyKey: string): HistoryRecord | null {
    if (!idempotencyKey) return null;
    const row = findByIdempotencyStmt.get(idempotencyKey) as any;
    if (!row) return null;
    return {
        ...row,
        metadata: row.metadata ? safeJson(row.metadata) : null
    };
}

export function countRecentForContact(contact: string, windowSeconds: number): number {
    const clause = `-${windowSeconds} seconds`;
    const row = rateLimitStmt.get(contact, clause) as { count: number };
    return row?.count || 0;
}
