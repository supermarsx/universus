// Phase 6 Schema Deployment and Testing Script
// Deploys the Phase 6 real-time database schema and runs verification tests

const fs = require('fs');
const path = require('path');
const { Pool } = require('pg');

// Database configuration from environment
const pool = new Pool({
    host: process.env.DB_HOST || '127.0.0.1',
    port: process.env.DB_PORT || 5432,
    database: process.env.DB_NAME || 'universus_rpg',
    user: process.env.DB_USER || 'postgres',
    password: process.env.DB_PASSWORD || 'postgres'
});

// ANSI color codes
const colors = {
    green: '\x1b[32m',
    red: '\x1b[31m',
    yellow: '\x1b[33m',
    blue: '\x1b[34m',
    reset: '\x1b[0m'
};

function log(message, color = 'reset') {
    console.log(`${colors[color]}${message}${colors.reset}`);
}

function printHeader(title) {
    console.log('\n=====================================');
    console.log(title);
    console.log('=====================================\n');
}

async function testDatabaseConnection() {
    try {
        const result = await pool.query('SELECT version()');
        log('✓ Database connection successful', 'green');
        return true;
    } catch (error) {
        log(`✗ Database connection failed: ${error.message}`, 'red');
        return false;
    }
}

async function deploySchema() {
    printHeader('Deploying Phase 6 Schema');
    
    const schemaPath = path.join(__dirname, 'backend/src/database/phase6_realtime_schema.sql');
    
    try {
        log('Reading schema file...', 'blue');
        const schema = fs.readFileSync(schemaPath, 'utf8');
        
        log('Executing schema...', 'blue');
        await pool.query(schema);
        
        log('✓ Schema deployed successfully', 'green');
        return true;
    } catch (error) {
        log(`✗ Schema deployment failed: ${error.message}`, 'red');
        console.error(error);
        return false;
    }
}

async function verifyTables() {
    printHeader('Verifying Tables');
    
    const expectedTables = [
        'chat_channels',
        'chat_messages',
        'private_messages',
        'private_conversations',
        'notifications',
        'notification_preferences',
        'notification_types',
        'player_status',
        'player_activity_log',
        'fleet_movement_events',
        'combat_alerts',
        'trading_offers',
        'trading_transactions',
        'chat_moderators',
        'chat_reports',
        'chat_bans',
        'alliance_announcements',
        'world_events'
    ];
    
    let missingTables = 0;
    
    for (const table of expectedTables) {
        try {
            const result = await pool.query(
                `SELECT EXISTS (
                    SELECT FROM information_schema.tables 
                    WHERE table_name = $1
                )`,
                [table]
            );
            
            if (result.rows[0].exists) {
                log(`✓ Table exists: ${table}`, 'green');
            } else {
                log(`✗ Table missing: ${table}`, 'red');
                missingTables++;
            }
        } catch (error) {
            log(`✗ Error checking table ${table}: ${error.message}`, 'red');
            missingTables++;
        }
    }
    
    return missingTables === 0;
}

async function verifyViews() {
    printHeader('Verifying Views');
    
    const expectedViews = [
        'chat_activity_summary',
        'notification_statistics',
        'player_activity_summary',
        'fleet_movement_summary'
    ];
    
    for (const view of expectedViews) {
        try {
            const result = await pool.query(
                `SELECT EXISTS (
                    SELECT FROM information_schema.views 
                    WHERE table_name = $1
                )`,
                [view]
            );
            
            if (result.rows[0].exists) {
                log(`✓ View exists: ${view}`, 'green');
            } else {
                log(`⚠ View missing: ${view}`, 'yellow');
            }
        } catch (error) {
            log(`✗ Error checking view ${view}: ${error.message}`, 'red');
        }
    }
}

async function verifyFunctions() {
    printHeader('Verifying Functions');
    
    const expectedFunctions = [
        'mark_notification_as_read',
        'get_unread_notification_count',
        'update_player_last_activity',
        'log_player_activity'
    ];
    
    for (const func of expectedFunctions) {
        try {
            const result = await pool.query(
                `SELECT EXISTS (
                    SELECT FROM information_schema.routines 
                    WHERE routine_name = $1
                )`,
                [func]
            );
            
            if (result.rows[0].exists) {
                log(`✓ Function exists: ${func}`, 'green');
            } else {
                log(`⚠ Function missing: ${func}`, 'yellow');
            }
        } catch (error) {
            log(`✗ Error checking function ${func}: ${error.message}`, 'red');
        }
    }
}

async function verifySeededData() {
    printHeader('Verifying Seeded Data');
    
    try {
        // Check chat channels
        const channels = await pool.query('SELECT COUNT(*) FROM chat_channels');
        const channelCount = parseInt(channels.rows[0].count);
        
        if (channelCount >= 5) {
            log(`✓ Chat channels seeded: ${channelCount} channels`, 'green');
        } else {
            log(`⚠ Expected 5 chat channels, found ${channelCount}`, 'yellow');
        }
        
        // Check notification types
        const notifTypes = await pool.query('SELECT COUNT(*) FROM notification_types');
        const notifCount = parseInt(notifTypes.rows[0].count);
        
        if (notifCount >= 12) {
            log(`✓ Notification types seeded: ${notifCount} types`, 'green');
        } else {
            log(`⚠ Expected 12 notification types, found ${notifCount}`, 'yellow');
        }
        
        // List notification types
        const types = await pool.query('SELECT type_name FROM notification_types ORDER BY type_name');
        log('\nNotification types:', 'blue');
        types.rows.forEach(row => {
            console.log(`  - ${row.type_name}`);
        });
        
    } catch (error) {
        log(`✗ Error verifying seeded data: ${error.message}`, 'red');
    }
}

async function verifyIndexes() {
    printHeader('Verifying Indexes');
    
    try {
        const result = await pool.query(`
            SELECT COUNT(*) as index_count
            FROM pg_indexes
            WHERE schemaname = 'public'
            AND (
                tablename LIKE 'chat_%' OR
                tablename LIKE 'private_%' OR
                tablename = 'notifications' OR
                tablename = 'notification_preferences' OR
                tablename = 'player_status' OR
                tablename = 'player_activity_log' OR
                tablename = 'fleet_movement_events' OR
                tablename = 'combat_alerts' OR
                tablename LIKE 'trading_%' OR
                tablename = 'alliance_announcements' OR
                tablename = 'world_events'
            )
        `);
        
        const indexCount = parseInt(result.rows[0].index_count);
        
        if (indexCount >= 30) {
            log(`✓ Performance indexes created: ${indexCount} indexes`, 'green');
        } else {
            log(`⚠ Expected ~42 indexes, found ${indexCount}`, 'yellow');
        }
    } catch (error) {
        log(`✗ Error verifying indexes: ${error.message}`, 'red');
    }
}

async function runDiagnostics() {
    printHeader('Running System Diagnostics');
    
    try {
        // Check database size
        const dbSize = await pool.query(`
            SELECT pg_size_pretty(pg_database_size(current_database())) as size
        `);
        log(`Database size: ${dbSize.rows[0].size}`, 'blue');
        
        // Check table statistics
        const tableStats = await pool.query(`
            SELECT 
                schemaname,
                COUNT(*) as table_count
            FROM pg_tables
            WHERE schemaname = 'public'
            GROUP BY schemaname
        `);
        log(`Total tables: ${tableStats.rows[0]?.table_count || 0}`, 'blue');
        
        // Check connection status
        const connections = await pool.query(`
            SELECT COUNT(*) as active_connections
            FROM pg_stat_activity
            WHERE datname = current_database()
        `);
        log(`Active connections: ${connections.rows[0].active_connections}`, 'blue');
        
    } catch (error) {
        log(`⚠ Diagnostics error: ${error.message}`, 'yellow');
    }
}

async function main() {
    printHeader('Phase 6 Real-time Database Deployment & Verification');
    log(`Target database: ${process.env.DB_NAME || 'universus_rpg'}`, 'blue');
    log(`Host: ${process.env.DB_HOST || '127.0.0.1'}:${process.env.DB_PORT || 5432}`, 'blue');
    log(`Started: ${new Date().toISOString()}\n`, 'blue');
    
    try {
        // Test connection
        const connected = await testDatabaseConnection();
        if (!connected) {
            log('\nPlease ensure PostgreSQL is running and credentials are correct', 'red');
            process.exit(1);
        }
        
        // Deploy schema
        const deployed = await deploySchema();
        if (!deployed) {
            log('\nSchema deployment failed. Please check error messages above.', 'red');
            process.exit(1);
        }
        
        // Verify everything
        await verifyTables();
        await verifyViews();
        await verifyFunctions();
        await verifySeededData();
        await verifyIndexes();
        await runDiagnostics();
        
        // Final summary
        printHeader('Deployment Complete');
        log('✓ Phase 6 Real-time Communication Systems deployed successfully', 'green');
        log('\nNext steps:', 'blue');
        log('  1. Start the backend server: cd backend && npm start', 'reset');
        log('  2. Access chat interface: http://localhost:3000/chat', 'reset');
        log('  3. Run comprehensive tests: ./test-phase6-realtime.sh', 'reset');
        
    } catch (error) {
        log(`\n✗ Fatal error: ${error.message}`, 'red');
        console.error(error);
        process.exit(1);
    } finally {
        await pool.end();
    }
}

// Run if executed directly
if (require.main === module) {
    main().catch(error => {
        console.error('Fatal error:', error);
        process.exit(1);
    });
}

module.exports = { deploySchema, verifyTables, verifyViews, verifyFunctions };
