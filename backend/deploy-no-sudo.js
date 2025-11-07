#!/usr/bin/env node

const { exec } = require('child_process');
const util = require('util');
const execPromise = util.promisify(exec);

async function runCommand(cmd, description) {
  console.log(`\n${'='.repeat(60)}`);
  console.log(`${description}`);
  console.log(`${'='.repeat(60)}`);
  console.log(`Command: ${cmd}\n`);
  
  try {
    const { stdout, stderr } = await execPromise(cmd, { timeout: 120000 });
    if (stdout) console.log('STDOUT:', stdout);
    if (stderr && stderr.trim()) console.log('STDERR:', stderr);
    console.log('✓ Success');
    return { success: true, stdout, stderr };
  } catch (error) {
    console.error('✗ Error:', error.message);
    if (error.stdout) console.log('STDOUT:', error.stdout);
    if (error.stderr) console.error('STDERR:', error.stderr);
    return { success: false, error: error.message, stdout: error.stdout, stderr: error.stderr };
  }
}

async function main() {
  console.log('==========================================');
  console.log('UNIVERSUS - Deployment & Build');
  console.log('==========================================\n');
  console.log(`Started at: ${new Date().toISOString()}\n`);

  // Step 1: Database setup (will fail gracefully if PostgreSQL not running)
  console.log('\n[STEP 1] Running Database Setup...');
  console.log('Note: This step requires PostgreSQL to be running on port 5432');
  const setupResult = await runCommand('node setup-database.js', 'Database Setup');
  
  if (!setupResult.success) {
    console.error('\n✗ Database setup failed!');
    console.error('PostgreSQL may not be running. To start it:');
    console.error('  1. Check if running: pg_isready');
    console.error('  2. Start manually if needed');
    console.error('\nContinuing with build steps...\n');
  }

  // Step 2: Install dependencies
  console.log('\n[STEP 2] Installing Dependencies...');
  const installResult = await runCommand('npm install --legacy-peer-deps', 'NPM Install');
  
  if (!installResult.success) {
    console.error('\n✗ NPM install failed!');
    console.error('Check the errors above.');
  }

  // Step 3: Build TypeScript
  console.log('\n[STEP 3] Building TypeScript...');
  const buildResult = await runCommand('npm run build', 'TypeScript Build');
  
  if (!buildResult.success) {
    console.error('\n✗ Build failed!');
    console.error('Check TypeScript errors above.');
  }

  // Summary
  console.log('\n\n');
  console.log('╔' + '═'.repeat(78) + '╗');
  console.log('║' + '  DEPLOYMENT SUMMARY'.padEnd(79) + '║');
  console.log('╠' + '═'.repeat(78) + '╣');
  console.log('║  Database Setup:   ' + (setupResult.success ? '✓ Complete' : '✗ Failed (check PostgreSQL)').padEnd(59) + '║');
  console.log('║  NPM Install:      ' + (installResult.success ? '✓ Complete' : '✗ Failed').padEnd(59) + '║');
  console.log('║  TypeScript Build: ' + (buildResult.success ? '✓ Complete' : '✗ Failed').padEnd(59) + '║');
  console.log('╚' + '═'.repeat(78) + '╝');
  
  if (setupResult.success && installResult.success && buildResult.success) {
    console.log('\n✓ ALL STEPS COMPLETED SUCCESSFULLY!\n');
    console.log('Next Steps:');
    console.log('  1. Start the server: npm start');
    console.log('  2. Access: http://localhost:3000');
    console.log('  3. Login: admin@universus.com / admin123');
  } else {
    console.log('\n⚠ SOME STEPS FAILED - Review errors above\n');
  }

  console.log(`\nCompleted at: ${new Date().toISOString()}\n`);
  console.log('='.repeat(80) + '\n');
}

main().catch(error => {
  console.error('\n✗ FATAL ERROR:', error);
  process.exit(1);
});
