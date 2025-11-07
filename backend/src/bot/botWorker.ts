import dotenv from 'dotenv';
import { pool } from '../config/database';
import { BotAIService } from './services/botAIService';
import { BotService } from './services/botService';

dotenv.config();

const POLL_INTERVAL_MS = parseInt(process.env.BOT_WORKER_INTERVAL_MS || '60000', 10);
const MAX_BOTS_PER_CYCLE = parseInt(process.env.BOT_WORKER_MAX_BOTS || '25', 10);

let shuttingDown = false;

const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

async function processBotQueue(): Promise<void> {
  const bots = await BotService.getBotsNeedingThink();

  if (!bots.length) {
    console.log('[Bot Worker] No bots require processing at this time');
    return;
  }

  console.log(`[Bot Worker] Processing ${Math.min(bots.length, MAX_BOTS_PER_CYCLE)} bot(s)`);

  const processingBatch = bots.slice(0, MAX_BOTS_PER_CYCLE);

  for (const bot of processingBatch) {
    if (shuttingDown) {
      console.log('[Bot Worker] Shutdown initiated, stopping bot processing loop');
      return;
    }

    try {
      await BotAIService.think(bot);
    } catch (error) {
      console.error(`[Bot Worker] Failed to process bot ${bot.id}`, error);
    }
  }
}

async function runWorker(): Promise<void> {
  console.log('[Bot Worker] Starting bot processing worker');

  while (!shuttingDown) {
    try {
      await processBotQueue();
    } catch (error) {
      console.error('[Bot Worker] Unexpected error while processing bots', error);
    }

    if (shuttingDown) {
      break;
    }

    await sleep(POLL_INTERVAL_MS);
  }

  console.log('[Bot Worker] Worker loop exited');
}

async function shutdown(): Promise<void> {
  if (shuttingDown) {
    return;
  }

  console.log('[Bot Worker] Received shutdown signal, cleaning up resources');
  shuttingDown = true;

  try {
    await pool.end();
  } catch (error) {
    console.error('[Bot Worker] Error while closing database pool', error);
  }
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

runWorker().catch(error => {
  console.error('[Bot Worker] Fatal error', error);
  process.exit(1);
});
