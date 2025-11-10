// @ts-nocheck

class CombatVisualizer {
  constructor() {
    this.modal = document.getElementById('combatReplayModal');
    this.canvas = document.getElementById('combatReplayCanvas') as HTMLCanvasElement | null;
    this.ctx = this.canvas?.getContext('2d') || null;
    this.summary = document.getElementById('combatReplaySummary');
    this.rounds = [];
    this.roundIndex = 0;
    this.animationFrame = null;
    this.lastFrame = 0;

    document.getElementById('closeCombatReplay')?.addEventListener('click', () => this.close());
    this.modal?.addEventListener('click', (event) => {
      if (event.target === this.modal) this.close();
    });
  }

  play(report) {
    if (!this.modal || !this.ctx) return;
    this.report = report;
    this.rounds = Array.isArray(report?.rounds) ? report.rounds : [];
    this.roundIndex = -1;
    this.lastFrame = 0;

    this.summary.innerHTML = `
      <p><strong>${i18n.t('fleet.battleLabel', { defaultValue: 'Battle:' })}</strong> ${report.attacker} vs ${report.defender || i18n.t('fleet.unknown', { defaultValue: 'Unknown' })}</p>
      <p><strong>${i18n.t('fleet.outcomeLabel', { defaultValue: 'Outcome:' })}</strong> ${report.winner ? report.winner.toUpperCase() : ''}</p>
      <p><strong>${i18n.t('fleet.lootLabel', { defaultValue: 'Loot:' })}</strong> ${this.formatLoot(report.loot)}</p>
    `;

    this.modal.style.display = 'block';

    if (!this.rounds.length) {
      this.drawStatic();
      return;
    }

    cancelAnimationFrame(this.animationFrame);
    this.animationFrame = requestAnimationFrame((ts) => this.animate(ts));
  }

  animate(timestamp) {
    if (!this.ctx || !this.canvas) return;
    if (!this.lastFrame) this.lastFrame = timestamp;
    const elapsed = timestamp - this.lastFrame;

    if (elapsed > 1100 && this.roundIndex < this.rounds.length - 1) {
      this.roundIndex++;
      this.lastFrame = timestamp;
    }

    this.drawRound();

    if (this.roundIndex < this.rounds.length - 1) {
      this.animationFrame = requestAnimationFrame((ts) => this.animate(ts));
    }
  }

  drawStatic() {
    if (!this.ctx || !this.canvas) return;
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    this.ctx.fillStyle = 'rgba(12, 17, 32, 0.9)';
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
    this.ctx.fillStyle = '#fbbf24';
    this.ctx.font = '18px Orbitron, sans-serif';
    this.ctx.fillText(i18n.t('fleet.replayUnavailable', { defaultValue: 'Replay unavailable for this report.' }), 40, this.canvas.height / 2);
  }

  drawRound() {
    if (!this.ctx || !this.canvas) return;
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

    const width = this.canvas.width;
    const height = this.canvas.height;

    this.ctx.fillStyle = '#080c1a';
    this.ctx.fillRect(0, 0, width, height);

    const round = this.rounds[this.roundIndex] || { attackerShots: 0, defenderShots: 0 };
    const attackerPower = Math.min(1, (round.attackerShots || 0) / 200);
    const defenderPower = Math.min(1, (round.defenderShots || 0) / 200);

    const attackerRadius = 40 + attackerPower * 60;
    const defenderRadius = 40 + defenderPower * 60;

    // Draw attacker node
    this.ctx.beginPath();
    this.ctx.fillStyle = 'rgba(99,102,241,0.8)';
    this.ctx.shadowColor = '#6366f1';
    this.ctx.shadowBlur = 20;
    this.ctx.arc(width * 0.25, height * 0.6, attackerRadius, 0, Math.PI * 2);
    this.ctx.fill();

    // Draw defender node
    this.ctx.beginPath();
    this.ctx.fillStyle = 'rgba(239,68,68,0.8)';
    this.ctx.shadowColor = '#ef4444';
    this.ctx.shadowBlur = 20;
    this.ctx.arc(width * 0.75, height * 0.6, defenderRadius, 0, Math.PI * 2);
    this.ctx.fill();
    this.ctx.shadowBlur = 0;

    // Draw volley indicator
    this.ctx.strokeStyle = '#fbbf24';
    this.ctx.lineWidth = 2;
    this.ctx.beginPath();
    this.ctx.moveTo(width * 0.25, height * 0.6);
    this.ctx.lineTo(width * 0.75, height * 0.6);
    this.ctx.stroke();

    // Round info
    this.ctx.fillStyle = '#e2e8f0';
    this.ctx.font = '16px Orbitron, sans-serif';
    const roundLabel = this.roundIndex >= 0 ? i18n.t('combat.roundLabel', { count: this.roundIndex + 1, defaultValue: `Round ${this.roundIndex + 1}` }) : i18n.t('combat.deploying', { defaultValue: 'Deploying Fleets' });
    this.ctx.fillText(roundLabel, 30, 40);
    this.ctx.font = '13px Inter, sans-serif';
    this.ctx.fillText(i18n.t('combat.volley', { shots: round.attackerShots || 0, defaultValue: `Volley: ${round.attackerShots || 0} shots` }), 30, 70);
    this.ctx.fillText(i18n.t('combat.counter', { shots: round.defenderShots || 0, defaultValue: `Counter: ${round.defenderShots || 0} shots` }), 30, 90);
    this.ctx.fillText(i18n.t('combat.attackerLosses', { losses: round.attackerDestroyed || 0, defaultValue: `Attacker losses: ${round.attackerDestroyed || 0}` }), width - 220, 70);
    this.ctx.fillText(i18n.t('combat.defenderLosses', { losses: round.defenderDestroyed || 0, defaultValue: `Defender losses: ${round.defenderDestroyed || 0}` }), width - 220, 90);
  }

  close() {
    if (this.modal) {
      this.modal.style.display = 'none';
    }
    cancelAnimationFrame(this.animationFrame);
  }

  formatLoot(loot = {}) {
    return `${this.formatNumber(loot.metal)} M / ${this.formatNumber(loot.crystal)} C / ${this.formatNumber(
      loot.deuterium
    )} D`;
  }

  formatNumber(num = 0) {
    return new Intl.NumberFormat().format(Math.floor(num));
  }
}

document.addEventListener('DOMContentLoaded', () => {
  window.combatVisualizer = new CombatVisualizer();
});
