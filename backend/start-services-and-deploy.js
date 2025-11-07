#!/usr/bin/env node

const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);
const fs = require('fs');
const path = require('path');

async function runCommand(cmd, description) {
  console.log(`\n${'='.repeat(60)}`);
  console.log(`${description}`);
  console.log(`${'='.repeat(60)}`);
  console.log(`Command: ${cmd}\n`);
  
  try {
    const { stdout, stderr } = await execPromise(cmd);
    if (stdout) console.log('STDOUT:', stdout);
    if (stderr) console.log('STDERR:', stderr);
    console.log('✓ Success');
    return { success: true, stdout, stderr };
  } catch (error) {
    console.error('✗ Error:', error.message);
    if (error.stdout) console.log('STDOUT:', error.stdout);
    if (error.stderr) console.error('STDERR:', error.stderr);
    return { success: false, error: error.message };
  }
}

async function checkService(name, checkCmd) {
  console.log(`\nChecking ${name}...`);
  try {
    const { stdout } = await execPromise(checkCmd);
    console.log(`✓ ${name} is running`);
    console.log(stdout.trim());
    return true;
  } catch (error) {
    console.log(`✗ ${name} is not running: ${error.message}`);
    return false;
  }
}

async function main() {
  console.log('==========================================');
  console.log('UNIVERSUS - Complete Deployment Script');
  console.log('==========================================\n');
  console.log(`Started at: ${new Date().toISOString()}\n`);

  // Step 1: Check PostgreSQL
  console.log('\n[STEP 1] Checking PostgreSQL Service...');
  let pgRunning = await checkService('PostgreSQL', 'pg_isready -h 127.0.0.1 -p 5432');
  
  if (!pgRunning) {
    console.log('\nAttempting to start PostgreSQL...');
    await runCommand('sudo -u postgres pg_ctlcluster 15 main start', 'Starting PostgreSQL');
    await new Promise(resolve => setTimeout(resolve, 3000));
    pgRunning = await checkService('PostgreSQL', 'pg_isready -h 127.0.0.1 -p 5432');
  }

  if (!pgRunning) {
    console.error('\n✗ CRITICAL: PostgreSQL could not be started');
    console.error('Please start PostgreSQL manually:');
    console.error('  sudo service postgresql start');
    console.error('  OR');
    console.error('  sudo -u postgres pg_ctlcluster 15 main start');
    process.exit(1);
  }

  // Step 2: Check Redis
  console.log('\n[STEP 2] Checking Redis Service...');
  let redisRunning = await checkService('Redis', 'redis-cli ping');
  
  if (!redisRunning) {
    console.log('\nAttempting to start Redis...');
    await runCommand('redis-server --daemonize yes', 'Starting Redis');
    await new Promise(resolve => setTimeout(resolve, 2000));
    redisRunning = await checkService('Redis', 'redis-cli ping');
  }

  if (!redisRunning) {
    console.error('\n✗ WARNING: Redis could not be started');
    console.log('Continuing without Redis (some features may not work)');
  }

  // Step 3: Run database setup
  console.log('\n[STEP 3] Running Database Setup...');
  const setupResult = await runCommand('node setup-database.js', 'Database Setup');
  
  if (!setupResult.success) {
    console.error('\n✗ Database setup failed!');
    console.error('Check the error above for details.');
    process.exit(1);
  }

  // Step 4: Install dependencies
  console.log('\n[STEP 4] Installing Dependencies...');
  const installResult = await runCommand('npm install --legacy-peer-deps', 'NPM Install');
  
  if (!installResult.success) {
    console.error('\n✗ NPM install failed!');
    process.exit(1);
  }

  // Step 5: Build TypeScript
  console.log('\n[STEP 5] Building TypeScript...');
  const buildResult = await runCommand('npm run build', 'TypeScript Build');
  
  if (!buildResult.success) {
    console.error('\n✗ Build failed!');
    console.error('Check TypeScript errors above.');
    process.exit(1);
  }

  // Success summary
  console.log('\n\n');
  console.log('╔' + '═'.repeat(78) + '╗');
  console.log('║' + ' '.repeat(78) + '║');
  console.log('║' + '  DEPLOYMENT COMPLETE - ALL SYSTEMS READY'.padEnd(78) + '║');
  console.log('║' + ' '.repeat(78) + '║');
  console.log('╚' + '═'.repeat(78) + '╝');
  console.log('\nServices Status:');
  console.log(`  ✓ PostgreSQL: ${pgRunning ? 'Running' : 'Not Running'}`);
  console.log(`  ✓ Redis: ${redisRunning ? 'Running' : 'Not Running'}`);
  console.log('  ✓ Database: Setup Complete');
  console.log('  ✓ Dependencies: Installed');
  console.log('  ✓ Build: Complete');
  console.log('\nNext Steps:');
  console.log('  1. Start the server: npm start');
  console.log('  2. Access: http://localhost:3000');
  console.log('  3. Login: admin@universus.com / admin123');
  console.log('\nTo start the server now:');
  console.log('  cd /workspace/ogame-rpg/backend');
  console.log('  npm start');
  console.log('\n' + '='.repeat(80) + '\n');

  console.log(`\nCompleted at: ${new Date().toISOString()}\n`);
}

main().catch(error => {
  console.error('\n✗ FATAL ERROR:', error);
  process.exit(1);
});
