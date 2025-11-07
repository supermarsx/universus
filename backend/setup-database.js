const { Client } = require('pg');
const { createClient } = require('redis');
const fs = require('fs');
const path = require('path');

// Database configuration
const DB_CONFIG = {
  host: '127.0.0.1',
  port: 5432,
  user: 'postgres',
  password: 'postgres',
  database: 'postgres' // Connect to postgres first
};

const TARGET_DB = 'universus_rpg';

async function executeSQLFile(client, filePath) {
  const sql = fs.readFileSync(filePath, 'utf8');
  try {
    await client.query(sql);
    console.log(`✓ Executed: ${path.basename(filePath)}`);
    return true;
  } catch (error) {
    console.error(`✗ Error in ${path.basename(filePath)}:`, error.message);
    return false;
  }
}

async function main() {
  console.log('==========================================');
  console.log('Universus - Database Setup & Validation');
  console.log('==========================================\n');

  let client;
  let targetClient;
  
  try {
    // Step 1: Connect to PostgreSQL
    console.log('[1/10] Connecting to PostgreSQL...');
    client = new Client(DB_CONFIG);
    await client.connect();
    console.log('✓ Connected to PostgreSQL\n');

    // Step 2: Drop and create database
    console.log('[2/10] Creating fresh database...');
    await client.query(`DROP DATABASE IF EXISTS ${TARGET_DB}`);
    await client.query(`CREATE DATABASE ${TARGET_DB}`);
    console.log(`✓ Database '${TARGET_DB}' created\n`);

    await client.end();

    // Step 3: Connect to target database
    console.log('[3/10] Connecting to target database...');
    targetClient = new Client({ ...DB_CONFIG, database: TARGET_DB });
    await targetClient.connect();
    console.log('✓ Connected to target database\n');

    // Step 4: Apply base schema
    console.log('[4/10] Applying base schema...');
    const baseSchema = path.join(__dirname, 'src/database/schema.sql');
    if (fs.existsSync(baseSchema)) {
      await executeSQLFile(targetClient, baseSchema);
    } else {
      console.log('⚠ Base schema not found, skipping');
    }
    console.log('');

    // Step 5: Apply migrations
    console.log('[5/10] Applying migrations...');
    const migrations = [
      'src/database/migrations/001_update_messages_table.sql',
      'src/database/migrations/002_add_shop_tables.sql',
      'src/database/migrations/003_millisecond_precision_combat.sql',
      'src/database/migrations/004_admin_features.sql',
      'src/database/migrations/005_bot_system.sql'
    ];

    for (const migration of migrations) {
      const migrationPath = path.join(__dirname, migration);
      if (fs.existsSync(migrationPath)) {
        await executeSQLFile(targetClient, migrationPath);
      } else {
        console.log(`⚠ Migration not found: ${path.basename(migration)}`);
      }
    }
    console.log('');

    // Step 6: Apply Phase 2 schema
    console.log('[6/10] Applying Phase 2 (Admin) schema...');
    const adminSchema = path.join(__dirname, 'src/database/admin_schema.sql');
    if (fs.existsSync(adminSchema)) {
      await executeSQLFile(targetClient, adminSchema);
    } else {
      console.log('⚠ Admin schema not found, skipping');
    }
    console.log('');

    // Step 7: Apply Phase 3 schema
    console.log('[7/10] Applying Phase 3 (Debris) schema...');
    const debrisSchema = path.join(__dirname, 'src/database/debris_schema.sql');
    if (fs.existsSync(debrisSchema)) {
      await executeSQLFile(targetClient, debrisSchema);
    } else {
      console.log('⚠ Debris schema not found, skipping');
    }
    console.log('');

    // Step 8: Apply Phase 4 schema
    console.log('[8/10] Applying Phase 4 (Universe) schema...');
    const universeSchema = path.join(__dirname, 'src/database/universe_seeding_schema.sql');
    if (fs.existsSync(universeSchema)) {
      await executeSQLFile(targetClient, universeSchema);
    } else {
      console.log('⚠ Universe schema not found, skipping');
    }
    console.log('');

    // Step 9: Verify database
    console.log('[9/10] Verifying database...');
    const tableCountResult = await targetClient.query(`
      SELECT COUNT(*) as count 
      FROM information_schema.tables 
      WHERE table_schema = 'public'
    `);
    const tableCount = parseInt(tableCountResult.rows[0].count);
    console.log(`✓ Tables created: ${tableCount}`);

    if (tableCount < 30) {
      console.log(`⚠ Warning: Expected 40+ tables, found ${tableCount}`);
    }

    // Count indexes
    const indexCountResult = await targetClient.query(`
      SELECT COUNT(*) as count 
      FROM pg_indexes 
      WHERE schemaname = 'public'
    `);
    const indexCount = parseInt(indexCountResult.rows[0].count);
    console.log(`✓ Indexes created: ${indexCount}`);

    // Count views
    const viewCountResult = await targetClient.query(`
      SELECT COUNT(*) as count 
      FROM information_schema.views 
      WHERE table_schema = 'public'
    `);
    const viewCount = parseInt(viewCountResult.rows[0].count);
    console.log(`✓ Views created: ${viewCount}`);
    console.log('');

    // Step 10: Create admin user
    console.log('[10/10] Creating admin user...');
    const bcrypt = require('bcrypt');
    const adminPassword = await bcrypt.hash('admin123', 10);
    
    await targetClient.query(`
      INSERT INTO users (username, email, password_hash, dark_matter, is_admin, created_at)
      VALUES ($1, $2, $3, $4, $5, NOW())
      ON CONFLICT (email) DO NOTHING
    `, ['admin', 'admin@universus.com', adminPassword, 10000, true]);
    
    console.log('✓ Admin user created');
    console.log('  Email: admin@universus.com');
    console.log('  Password: admin123');
    console.log('');

    // Test Redis connection
    console.log('Testing Redis connection...');
    try {
      const redisClient = createClient({ url: 'redis://127.0.0.1:6379' });
      await redisClient.connect();
      await redisClient.ping();
      console.log('✓ Redis connected');
      await redisClient.disconnect();
    } catch (error) {
      console.log('⚠ Redis not available:', error.message);
    }
    console.log('');

    // Final summary
    console.log('==========================================');
    console.log('Database Setup Complete!');
    console.log('==========================================');
    console.log(`Database: ${TARGET_DB}`);
    console.log(`Tables: ${tableCount}`);
    console.log(`Indexes: ${indexCount}`);
    console.log(`Views: ${viewCount}`);
    console.log('');
    console.log('Admin Account:');
    console.log('  Email: admin@universus.com');
    console.log('  Password: admin123');
    console.log('  Dark Matter: 10,000');
    console.log('==========================================');
    console.log('');
    console.log('Next steps:');
    console.log('1. Start the application: npm start');
    console.log('2. Access at: http://localhost:3000');
    console.log('3. Run tests: npm test');
    console.log('==========================================');

  } catch (error) {
    console.error('\n✗ Fatal error:', error.message);
    console.error(error.stack);
    process.exit(1);
  } finally {
    if (targetClient) {
      await targetClient.end();
    }
  }
}

main().catch(console.error);
