# Backup Scripts for Universus

This directory contains scripts for automated backups of PostgreSQL and Redis databases running in Docker containers.

## Usage

1. **Edit `backup.sh`**
   - Set the correct container names and credentials at the top of the script if needed.

2. **Run the script:**
   ```sh
   ./backup.sh
   ```
   Backups will be saved in the `backups/` directory at the project root.

3. **Automate with cron:**
   - Add a cron job to run this script daily:
     ```sh
     0 2 * * * /path/to/universus/scripts/backup/backup.sh
     ```

4. **(Optional) Cloud Upload:**
   - Uncomment and configure the S3 upload lines in the script if you want to upload backups to cloud storage.

## What it does
- Dumps PostgreSQL database to a timestamped `.sql` file
- Triggers and copies Redis dump to a timestamped `.rdb` file
- Cleans up backups older than 7 days

## Requirements
- Docker
- `pg_dump` and `redis-cli` available in the containers
- S3 CLI (optional, for cloud upload)

---

**Edit the script as needed for your environment!**
