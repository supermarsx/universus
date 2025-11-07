#!/usr/bin/env python3
import subprocess
import sys
import time
import os

def run_cmd(cmd, description, shell=True, check=False):
    """Run a command and return output"""
    print(f"\n{'='*70}")
    print(f"{description}")
    print(f"{'='*70}")
    print(f"Command: {cmd}\n")
    
    try:
        result = subprocess.run(
            cmd,
            shell=shell,
            capture_output=True,
            text=True,
            timeout=120
        )
        
        if result.stdout:
            print("STDOUT:")
            print(result.stdout)
        if result.stderr:
            print("STDERR:")
            print(result.stderr)
        
        print(f"Return code: {result.returncode}")
        
        if check and result.returncode != 0:
            print(f"✗ {description} FAILED")
            return False
        
        print(f"✓ {description} completed")
        return True
        
    except subprocess.TimeoutExpired:
        print(f"✗ {description} TIMED OUT")
        return False
    except Exception as e:
        print(f"✗ {description} ERROR: {e}")
        return False

def main():
    print("="*70)
    print(" UNIVERSUS - Complete Deployment & Testing")
    print("="*70)
    print(f" Started: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    print("="*70)
    
    os.chdir('/workspace/universus-rpg/backend')
    
    # Check current user
    run_cmd('whoami', '[0] Checking current user')
    run_cmd('id', '[0] Checking user ID')
    
    # Check PostgreSQL
    print("\n[1] Checking PostgreSQL...")
    pg_running = run_cmd('pg_isready -h 127.0.0.1 -p 5432', 'PostgreSQL Check')
    
    if not pg_running:
        print("\nPostgreSQL is not responding.")
        print("Attempting to start PostgreSQL...")
        
        # Try multiple start methods
        methods = [
            ('service postgresql start', 'Service start'),
            ('pg_ctlcluster 15 main start', 'Cluster start'),
            ('/etc/init.d/postgresql start', 'Init.d start'),
        ]
        
        for cmd, desc in methods:
            print(f"\nTrying: {desc}")
            result = run_cmd(cmd, desc)
            if result:
                time.sleep(3)
                pg_running = run_cmd('pg_isready -h 127.0.0.1 -p 5432', 'PostgreSQL Check')
                if pg_running:
                    break
    
    # Check process
    run_cmd('ps aux | grep postgres | grep -v grep || echo "No PostgreSQL process found"', 
            'PostgreSQL Process Check')
    
    # Check Redis
    print("\n[2] Checking Redis...")
    redis_running = run_cmd('redis-cli ping', 'Redis Check')
    
    print("\n[3] Running Database Setup...")
    if pg_running:
        db_setup = run_cmd('node setup-database.js', 'Database Setup')
    else:
        print("✗ Skipping database setup - PostgreSQL not running")
        db_setup = False
    
    print("\n[4] Installing Dependencies...")
    npm_install = run_cmd('npm install --legacy-peer-deps 2>&1 | head -50', 'NPM Install')
    
    print("\n[5] Building TypeScript...")
    ts_build = run_cmd('npm run build 2>&1 | tail -20', 'TypeScript Build')
    
    # Summary
    print("\n\n")
    print("╔" + "="*78 + "╗")
    print("║" + " DEPLOYMENT SUMMARY ".center(78) + "║")
    print("╠" + "="*78 + "╣")
    print(f"║  PostgreSQL:     {'✓ Running' if pg_running else '✗ Not Running'}".ljust(79) + "║")
    print(f"║  Redis:          {'✓ Running' if redis_running else '✗ Not Running'}".ljust(79) + "║")
    print(f"║  Database Setup: {'✓ Complete' if db_setup else '✗ Failed/Skipped'}".ljust(79) + "║")
    print(f"║  NPM Install:    {'✓ Complete' if npm_install else '✗ Failed'}".ljust(79) + "║")
    print(f"║  TS Build:       {'✓ Complete' if ts_build else '✗ Failed'}".ljust(79) + "║")
    print("╚" + "="*78 + "╝")
    
    if all([pg_running, db_setup, npm_install, ts_build]):
        print("\n✓ ALL STEPS COMPLETED SUCCESSFULLY!")
        print("\nNext: Start the server with 'npm start'")
    else:
        print("\n⚠ SOME STEPS FAILED - See details above")
    
    print(f"\nCompleted: {time.strftime('%Y-%m-%d %H:%M:%S')}\n")

if __name__ == '__main__':
    main()
