// @ts-nocheck

import i18n from './i18n';


const SHIP_STATS = {
  small_cargo: { cargo: 5000, fuel: 10 },
  large_cargo: { cargo: 25000, fuel: 50 },
  light_fighter: { cargo: 50, fuel: 20 },
  heavy_fighter: { cargo: 100, fuel: 75 },
  cruiser: { cargo: 800, fuel: 300 },
  battleship: { cargo: 1500, fuel: 500 },
  colony_ship: { cargo: 7500, fuel: 1000 },
  recycler: { cargo: 20000, fuel: 300 },
  espionage_probe: { cargo: 5, fuel: 1 },
  bomber: { cargo: 500, fuel: 1000 },
  destroyer: { cargo: 2000, fuel: 1000 },
  deathstar: { cargo: 1000000, fuel: 1 },
};

export class FleetManager {
  constructor() {
    this.planet = null;
    this.selectedShips = {};
    this.selectedMission = 'attack';
    this.activeFleets = [];
    this.combatReports = [];
    this.missionLog = [];
    this.filters = { mission: 'all', status: 'all' };
    this.currentStep = 1;
    this.selectedTab = 'dispatch';
    this.pollInterval = null;
    this.explicitTarget = false;
    this.acsGroups = [];
    this.selectedAcsGroupId = null;
    this.countdownInterval = null;
    this.init();
  }

  init() {
    this.bindTabs();
    this.bindStepButtons();
    this.bindMissionButtons();
    this.bindLaunchButton();
    this.bindFilters();
    this.bindAcsControls();
    this.startPolling();
    this.attachSocketListeners();
    this.prefillTargetFromQuery();
    this.loadAcsGroups();
    this.startCountdownTicker();
  }

  bindTabs() {
    document.querySelectorAll('.fleet-tabs .tab-button').forEach((btn) => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.fleet-tabs .tab-button').forEach((b) => b.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach((tab) => tab.classList.remove('active'));

        btn.classList.add('active');
        const tabName = btn.dataset.tab;
        document.getElementById(`${tabName}Tab`)?.classList.add('active');
        this.selectedTab = tabName;

        if (tabName === 'missions') {
          this.fetchActiveFleets();
        } else if (tabName === 'overview') {
          this.renderOverview();
        }
      });
    });
  }

  bindStepButtons() {
    document.getElementById('nextToStep2')?.addEventListener('click', () => this.goToStep(2));
    document.getElementById('backToStep1')?.addEventListener('click', () => this.goToStep(1));
    document.getElementById('nextToStep3')?.addEventListener('click', () => this.goToStep(3));
    document.getElementById('backToStep2')?.addEventListener('click', () => this.goToStep(2));
  }

  bindMissionButtons() {
    document.querySelectorAll('#missionSelection .mission-button').forEach((btn) => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('#missionSelection .mission-button').forEach((b) => b.classList.remove('selected'));
        btn.classList.add('selected');
        this.selectedMission = btn.dataset.mission;
        this.toggleCargoInputs();
        this.updateAcsPanelState();
      });
    });
  }

  bindLaunchButton() {
    document.getElementById('launchFleet')?.addEventListener('click', () => this.dispatchFleet());
    document.getElementById('resetFleet')?.addEventListener('click', () => this.resetForm());
  }

  bindFilters() {
    document.getElementById('missionFilterType')?.addEventListener('change', (e) => {
      this.filters.mission = e.target.value;
      this.renderActiveMissions();
    });

    document.getElementById('missionFilterStatus')?.addEventListener('change', (e) => {
      this.filters.status = e.target.value;
      this.renderActiveMissions();
    });

    document.getElementById('clearMissionLog')?.addEventListener('click', () => {
      this.missionLog = [];
      this.renderMissionLog();
    });

    document.getElementById('refreshMissionLog')?.addEventListener('click', () => {
      this.loadMissionHistory();
    });
  }

  bindAcsControls() {
    document.getElementById('refreshAcsGroups')?.addEventListener('click', () => this.loadAcsGroups());
    document.getElementById('openAcsModal')?.addEventListener('click', () => this.openAcsModal());
    document.getElementById('closeAcsModal')?.addEventListener('click', () => this.closeAcsModal());
    document.getElementById('cancelAcsModal')?.addEventListener('click', () => this.closeAcsModal());
    document.getElementById('acsForm')?.addEventListener('submit', (e) => {
      e.preventDefault();
      this.createAcsGroup();
    });
    document.getElementById('leaveAcsSelection')?.addEventListener('click', () => this.clearAcsSelection());
  }

  startPolling() {
    this.pollInterval = setInterval(() => {
      if (this.selectedTab === 'missions') {
        this.fetchActiveFleets();
      }
    }, 30000);
  }

  attachSocketListeners() {
    if (!window.socket) return;
    window.socket.on('fleetUpdate', (payload = {}) => {
      this.fetchActiveFleets();
      this.recordMissionLog(payload);
      if (payload.action === 'combat') {
        const outcomeKey =
          payload.report?.winner === 'attacker'
            ? payload.role === 'attacker'
              ? 'fleet.outcome.victory'
              : 'fleet.outcome.defeat'
            : payload.role === 'defender'
            ? 'fleet.outcome.victory'
            : 'fleet.outcome.defeat';
        const outcome = i18n.t(outcomeKey, { defaultValue: payload.report?.winner === 'attacker' ? (payload.role === 'attacker' ? 'Victory' : 'Defeat') : (payload.role === 'defender' ? 'Victory' : 'Defeat') });
        this.notify(i18n.t('fleet.combatAt', { result: outcome, coords: this.formatCoords(payload.report), defaultValue: `${outcome}: combat at ${this.formatCoords(payload.report)}` }), 'info');
        this.loadCombatReports();
        this.loadMissionHistory();
      }
    });
  }

  goToStep(step) {
    if (step === 2 && Object.keys(this.selectedShips).length === 0) {
      this.notify(i18n.t('fleet.selectAtLeastOneShip', { defaultValue: 'Please select at least one ship before proceeding.' }), 'error');
      return;
    }

    if (step === 3) {
      const targetGalaxy = parseInt((document.getElementById('targetGalaxy') as HTMLInputElement)?.value) || 1;
      const targetSystem = parseInt((document.getElementById('targetSystem') as HTMLInputElement)?.value) || 1;
      const targetPosition = parseInt((document.getElementById('targetPosition') as HTMLInputElement)?.value) || 1;
      if (!targetGalaxy || !targetSystem || !targetPosition) {
        this.notify(i18n.t('fleet.provideValidCoords', { defaultValue: 'Provide valid target coordinates.' }), 'error');
        return;
      }
    }

    this.currentStep = step;
    document.querySelectorAll('.fleet-step').forEach((stepEl) => stepEl.classList.add('hidden'));
    document.getElementById(`step${step}`)?.classList.remove('hidden');
  }

  updatePlanet(data) {
    this.planet = data.planet;
    if (!this.hasExplicitTarget()) {
      document.getElementById('targetGalaxy').value = this.planet.galaxy;
      document.getElementById('targetSystem').value = this.planet.system;
      document.getElementById('targetPosition').value = this.planet.position;
    }
    this.renderFleetSelection();
    this.renderOverview();
    Promise.all([this.fetchActiveFleets(), this.loadCombatReports(), this.loadMissionHistory()]);
  }

  renderFleetSelection() {
    const container = document.getElementById('fleetSelection');
    if (!container || !this.planet) return;

    container.innerHTML = '';
    const ships = this.extractPlanetShips();

    if (Object.keys(ships).length === 0) {
      container.innerHTML = `<div class="empty-state card-compact">${i18n.t('fleet.noShips', { defaultValue: 'No ships available on this planet.' })}</div>`;
      return;
    }

    Object.entries(ships).forEach(([key, amount]) => {
      if (!SHIP_STATS[key]) return;
      const card = document.createElement('div');
      card.className = 'fleet-card';
card.innerHTML = `
        <div class="fleet-card-header">
          <h3>${this.getShipLabel(key)}</h3>
          <span>${i18n.t('fleet.available', { defaultValue: 'Available:' })} ${this.formatNumber(amount)}</span>
        </div>
        <div class="fleet-card-body">
          <label>
            ${i18n.t('fleet.quantity', { defaultValue: 'Quantity' })}
            <input type="number" min="0" max="${amount}" value="${this.selectedShips[key] || 0}" data-ship="${key}">
          </label>
          <button class="btn btn-text" data-select-all="${key}">${i18n.t('fleet.fill', { defaultValue: 'Fill' })}</button>
        </div>
      `;

      card.querySelector('input')?.addEventListener('input', () => this.handleShipSelection());
      card.querySelector('button')?.addEventListener('click', () => {
        card.querySelector('input').value = amount;
        this.handleShipSelection();
      });

      container.appendChild(card);
    });
  }

  prefillTargetFromQuery() {
    if (!window?.location?.search) return;
    const params = new URLSearchParams(window.location.search);
    const target = params.get('target');
    if (!target) return;

    const [galaxy, system, position] = target.split(':').map((value) => parseInt(value, 10));
    if (![galaxy, system, position].every((val) => Number.isFinite(val))) return;

    const galaxyInput = document.getElementById('targetGalaxy') as HTMLInputElement | null;
    const systemInput = document.getElementById('targetSystem') as HTMLInputElement | null;
    const positionInput = document.getElementById('targetPosition') as HTMLInputElement | null;

    if (galaxyInput) galaxyInput.value = String(galaxy);
    if (systemInput) systemInput.value = String(system);
    if (positionInput) positionInput.value = String(position);

    this.markExplicitTarget();
  }

  markExplicitTarget() {
    this.explicitTarget = true;
  }

  async loadAcsGroups() {
    try {
      const response = await api.get('/acs');
      this.acsGroups = response?.groups || [];
      this.renderAcsGroups();
    } catch (error) {
      console.error('Failed to load ACS groups:', error);
    }
  }

  renderAcsGroups() {
    const container = document.getElementById('acsGroupList');
    if (!container) return;

    if (!this.acsGroups.length) {
      container.innerHTML = `<div class="empty-state card-compact">${i18n.t('fleet.noAcsGroups', { defaultValue: 'No ACS groups available. Create one to coordinate attacks.' })}</div>`;
      this.updateAcsSelectionBadge();
      return;
    }

    container.innerHTML = '';
    this.acsGroups.forEach((group) => {
      const card = document.createElement('div');
      card.className = 'acs-group-card';
      const coords = `${group.target_galaxy}:${group.target_system}:${group.target_position}`;
const windowLabel = group.departure_window_start
        ? `${new Date(group.departure_window_start).toLocaleTimeString()} - ${new Date(
            group.departure_window_end
          ).toLocaleTimeString()}`
        : i18n.t('fleet.flexibleWindow', { defaultValue: 'Flexible window' });
      const selected = this.selectedAcsGroupId === group.id;
card.innerHTML = `
        <div>
          <p class="acs-group-title">${group.mission_type.toUpperCase()} • ${coords}</p>
          <p class="acs-group-window">${windowLabel}</p>
          ${group.notes ? `<p class="acs-group-notes">${group.notes}</p>` : ''}
        </div>
        <button class="btn ${selected ? 'btn-secondary' : 'btn-primary'} acs-join-btn" data-group="${group.id}">
          ${selected ? i18n.t('fleet.selected', { defaultValue: 'Selected' }) : i18n.t('fleet.linkFleet', { defaultValue: 'Link Fleet' })}
        </button>
      `;
      card.querySelector('button')?.addEventListener('click', () => this.joinAcsGroup(group.id, coords));
      container.appendChild(card);
    });
    this.updateAcsSelectionBadge();
    this.updateAcsPanelState();
  }

  updateAcsSelectionBadge() {
    const badge = document.getElementById('selectedAcsBadge');
    const label = document.getElementById('selectedAcsLabel');
    if (!badge || !label) return;
    if (!this.selectedAcsGroupId) {
      badge.classList.add('hidden');
      return;
    }
    const group = this.acsGroups.find((g) => g.id === this.selectedAcsGroupId);
    label.textContent = group
      ? `${group.mission_type.toUpperCase()} @ ${group.target_galaxy}:${group.target_system}:${group.target_position}`
      : `#${this.selectedAcsGroupId}`;
    badge.classList.remove('hidden');
  }

  openAcsModal() {
if (this.selectedMission !== 'attack') {
      this.notify(i18n.t('fleet.acsOnlyAttack', { defaultValue: 'ACS groups are only available for attack missions.' }), 'info');
      return;
    }
    const modal = document.getElementById('acsModal');
    const galaxyInput = document.getElementById('acsTargetGalaxy') as HTMLInputElement | null;
    const systemInput = document.getElementById('acsTargetSystem') as HTMLInputElement | null;
    const positionInput = document.getElementById('acsTargetPosition') as HTMLInputElement | null;

    const targetGalaxy = (document.getElementById('targetGalaxy') as HTMLInputElement)?.value || this.planet?.galaxy || 1;
    galaxyInput && (galaxyInput.value = String(targetGalaxy));
    systemInput && (systemInput.value = (document.getElementById('targetSystem') as HTMLInputElement)?.value || String(this.planet?.system || 1));
    positionInput && (positionInput.value = (document.getElementById('targetPosition') as HTMLInputElement)?.value || String(this.planet?.position || 1));

    if (modal) modal.style.display = 'block';
  }

  closeAcsModal() {
    const modal = document.getElementById('acsModal');
    if (modal) modal.style.display = 'none';
  }

  async createAcsGroup() {
    try {
      const missionType = (document.getElementById('acsMissionType') as HTMLSelectElement)?.value || 'attack';
      const payload = {
        missionType,
        targetGalaxy: parseInt((document.getElementById('acsTargetGalaxy') as HTMLInputElement)?.value || '1', 10),
        targetSystem: parseInt((document.getElementById('acsTargetSystem') as HTMLInputElement)?.value || '1', 10),
        targetPosition: parseInt((document.getElementById('acsTargetPosition') as HTMLInputElement)?.value || '1', 10),
        departureWindowStart: (document.getElementById('acsWindowStart') as HTMLInputElement)?.value || undefined,
        departureWindowEnd: (document.getElementById('acsWindowEnd') as HTMLInputElement)?.value || undefined,
        notes: (document.getElementById('acsNotes') as HTMLTextAreaElement)?.value || undefined,
      };

      await api.post('/acs', payload);
      this.closeAcsModal();
      await this.loadAcsGroups();
      this.notify(i18n.t('fleet.acsCreated', { defaultValue: 'ACS group created.' }), 'success');
    } catch (error) {
      console.error('Failed to create ACS group:', error);
      this.notify(error?.response?.data?.message || i18n.t('fleet.acsCreateFailed', { defaultValue: 'Unable to create ACS group.' }), 'error');
    }
  }

  async joinAcsGroup(groupId: number, label: string) {
if (this.selectedMission !== 'attack') {
      this.notify(i18n.t('fleet.selectAttackBeforeAcs', { defaultValue: 'Select an attack mission before joining an ACS group.' }), 'info');
      return;
    }
    try {
      await api.post(`/acs/${groupId}/join`, { planetId: this.planet?.id });
      this.selectedAcsGroupId = groupId;
      this.updateAcsSelectionBadge();
      await this.loadAcsGroups();
      this.notify(i18n.t('fleet.linkedToAcs', { label, defaultValue: `Linked fleet to ACS group targeting ${label}.` }), 'success');
    } catch (error) {
      console.error('Failed to join ACS group:', error);
      this.notify(error?.response?.data?.message || i18n.t('fleet.acsJoinFailed', { defaultValue: 'Unable to join ACS group.' }), 'error');
    }
  }

  clearAcsSelection() {
    this.selectedAcsGroupId = null;
    this.updateAcsSelectionBadge();
    this.updateAcsPanelState();
  }

  updateAcsPanelState() {
    const panel = document.getElementById('acsPanel');
    if (!panel) return;
    const enabled = this.selectedMission === 'attack';
    panel.classList.toggle('acs-disabled', !enabled);
    if (!enabled && this.selectedAcsGroupId) {
      this.clearAcsSelection();
    }
  }

  hasExplicitTarget(): boolean {
    return Boolean(this.explicitTarget);
  }

  handleShipSelection() {
    this.selectedShips = {};
    document.querySelectorAll('#fleetSelection input[data-ship]').forEach((input) => {
      const shipKey = input.dataset.ship;
      const value = Math.max(0, parseInt(input.value) || 0);
      if (value > 0) {
        this.selectedShips[shipKey] = value;
      }
    });

    this.updateDispatchSummary();
  }

  updateDispatchSummary() {
    const summary = document.getElementById('selectedShipsSummary');
    const cargoLabel = document.getElementById('availableCargo');
    const fuelLabel = document.getElementById('fuelEstimate');

    if (!summary || !cargoLabel || !fuelLabel) return;

    if (Object.keys(this.selectedShips).length === 0) {
      summary.textContent = i18n.t('fleet.noShipsSelected', { defaultValue: 'No ships selected.' });
      cargoLabel.textContent = '0';
      fuelLabel.textContent = '0';
      return;
    }

    const parts = Object.entries(this.selectedShips).map(([key, value]) => `${this.getShipLabel(key)}: ${value}`);
    summary.textContent = parts.join(', ');

    const totalCargo = Object.entries(this.selectedShips).reduce((sum, [key, value]) => {
      return sum + (SHIP_STATS[key].cargo || 0) * value;
    }, 0);

    cargoLabel.textContent = this.formatNumber(totalCargo);
    fuelLabel.textContent = this.formatNumber(this.estimateFuel());
  }

  toggleCargoInputs() {
    const section = document.getElementById('cargoInputs');
    if (!section) return;
    if (this.selectedMission === 'transport' || this.selectedMission === 'deploy') {
      section.style.display = 'grid';
    } else {
      section.style.display = 'none';
    }
  }

  async dispatchFleet() {
    if (!this.planet) return;
    if (Object.keys(this.selectedShips).length === 0) {
      this.notify(i18n.t('fleet.selectShipsForFleet', { defaultValue: 'Select ships for the fleet.' }), 'error');
      return;
    }

    if (!this.selectedMission) {
      this.notify(i18n.t('fleet.chooseMissionType', { defaultValue: 'Choose a mission type.' }), 'error');
      return;
    }

    const payload = {
      originPlanetId: this.planet.id,
      targetGalaxy: parseInt(document.getElementById('targetGalaxy')?.value) || 1,
      targetSystem: parseInt(document.getElementById('targetSystem')?.value) || 1,
      targetPosition: parseInt(document.getElementById('targetPosition')?.value) || 1,
      missionType: this.selectedMission,
      ships: this.selectedShips,
      cargo: {
        metal: parseInt(document.getElementById('cargoMetal')?.value) || 0,
        crystal: parseInt(document.getElementById('cargoCrystal')?.value) || 0,
        deuterium: parseInt(document.getElementById('cargoDeuterium')?.value) || 0,
      },
      acsGroupId: this.selectedMission === 'attack' ? this.selectedAcsGroupId : null,
    };

    try {
      await api.post('/fleet/dispatch', payload);
      this.notify(i18n.t('fleet.dispatchedSuccess', { defaultValue: 'Fleet dispatched successfully!' }), 'success');
      this.resetForm();
      await loadPlanetData(this.planet.id);
      this.fetchActiveFleets();
      this.loadCombatReports();
    } catch (error) {
      this.notify(error.message || i18n.t('fleet.failedToDispatch', { defaultValue: 'Failed to dispatch fleet' }), 'error');
    }
  }

  resetForm() {
    this.selectedShips = {};
    this.selectedMission = null;
    document.querySelectorAll('#fleetSelection input[data-ship]').forEach((input) => (input.value = 0));
    document.querySelectorAll('#missionSelection .mission-button').forEach((btn) => btn.classList.remove('selected'));
    ['cargoMetal', 'cargoCrystal', 'cargoDeuterium'].forEach((id) => {
      const input = document.getElementById(id);
      if (input) input.value = '0';
    });
    this.updateDispatchSummary();
    this.goToStep(1);
  }

  async fetchActiveFleets() {
    try {
      const fleets = await api.get('/fleet');
      this.activeFleets = Array.isArray(fleets) ? fleets : [];
      this.renderActiveMissions();
    } catch (error) {
      console.error('Failed to load fleets:', error);
    }
  }

  async loadCombatReports() {
    try {
      const reports = await api.get('/fleet/reports?limit=5');
      this.combatReports = Array.isArray(reports) ? reports : [];
      this.renderCombatReports();
    } catch (error) {
      console.error('Failed to load combat reports:', error);
    }
  }

  renderOverview() {
    const container = document.getElementById('fleetInventory');
    if (!container || !this.planet) return;

    const ships = this.extractPlanetShips();
    if (Object.keys(ships).length === 0) {
      container.innerHTML = `<div class="empty-state card-compact">${i18n.t('fleet.noShipsStationed', { defaultValue: 'No ships stationed on this planet.' })}</div>`;
      return;
    }

    container.innerHTML = `
      <div class="fleet-inventory-grid">
        ${Object.entries(ships)
          .map(
            ([key, value]) => `
              <div class="fleet-inventory-card">
                <span>${SHIP_STATS[key]?.name || this.formatName(key)}</span>
                <strong>${this.formatNumber(value)}</strong>
              </div>
            `
          )
          .join('')}
      </div>
    `;
  }

  renderActiveMissions() {
    const container = document.getElementById('activeMissions');
    if (!container) return;

    const fleets = this.filterFleets();

    if (fleets.length === 0) {
      container.innerHTML = `<div class="empty-state card-compact">${i18n.t('fleet.noActiveMissions', { defaultValue: 'No active missions.' })}</div>`;
      return;
    }

    container.innerHTML = '';

    fleets.forEach((fleet) => {
      const ships = typeof fleet.ships === 'string' ? JSON.parse(fleet.ships) : fleet.ships;
      const arrivalTs =
        fleet.arrivalTimestamp ??
        (fleet.arrival_time ? new Date(fleet.arrival_time).getTime() : null);
      const returnTs =
        fleet.returnTimestamp ??
        (fleet.return_time ? new Date(fleet.return_time).getTime() : null);
const arrivalText = arrivalTs ? this.formatCountdownMs(arrivalTs - Date.now()) : i18n.t('fleet.noArrival', { defaultValue: '—' });
       const returnText = returnTs ? this.formatCountdownMs(returnTs - Date.now()) : i18n.t('fleet.pending', { defaultValue: 'Pending' });

      const card = document.createElement('div');
      card.className = 'mission-card card-enhanced';

card.innerHTML = `
        <div class="mission-header">
          <div>
            <h3>${this.getLocalizedMissionLabel(fleet.mission_type)}</h3>
            <p>${i18n.t('fleet.targetLabel', { defaultValue: 'Target:' })} ${fleet.target_galaxy}:${fleet.target_system}:${fleet.target_position}</p>
          </div>
          <div class="mission-status">${i18n.t(`fleet.status.${fleet.status}`, { defaultValue: fleet.status })}</div>
        </div>
        <div class="mission-body">
          <div><strong>${i18n.t('fleet.arrivalLabel', { defaultValue: 'Arrival:' })}</strong> <span class="countdown" data-countdown="arrival" ${
            arrivalTs ? `data-timestamp="${arrivalTs}"` : ''
          }>${arrivalText}</span></div>
          <div><strong>${i18n.t('fleet.returnLabel', { defaultValue: 'Return:' })}</strong> <span class="countdown" data-countdown="return" ${
            returnTs ? `data-timestamp="${returnTs}"` : ''
          }>${returnText}</span></div>
          <div class="mission-ships">
            ${Object.entries(ships || {})
              .map(([key, count]) => `<span>${SHIP_STATS[key]?.name || this.formatName(key)}: ${count}</span>`)
              .join('')}
          </div>
        </div>
        <div class="mission-actions">
          ${fleet.status === 'outbound' ? `<button class="btn btn-secondary" data-recall="${fleet.id}">${i18n.t('fleet.recall', { defaultValue: 'Recall' })}</button>` : ''}
        </div>
      `;

      const recallBtn = card.querySelector('[data-recall]');
      recallBtn?.addEventListener('click', () => this.recallFleet(fleet.id));

      container.appendChild(card);
    });

    this.updateCountdownNodes();
  }

  renderCombatReports() {
    const container = document.getElementById('combatReports');
    if (!container) return;

    if (!this.combatReports.length) {
      container.innerHTML = `<div class="empty-state card-compact">${i18n.t('fleet.noCombatReports', { defaultValue: 'No recent combat reports.' })}</div>`;
      return;
    }

    container.innerHTML = '';
    this.combatReports.forEach((report, index) => {
      const card = document.createElement('div');
      card.className = 'combat-report card-compact';
      card.innerHTML = `
        <div class="combat-report-header">
          <div>
            <strong>${this.formatCoords(report)}</strong>
            <span class="combat-tag ${report.winner}">${report.winner.toUpperCase()}</span>
          </div>
          <small>${new Date(report.battleTime).toLocaleString()}</small>
        </div>
        <div class="combat-report-body">
          <p><strong>${i18n.t('fleet.attackerLabel', { defaultValue: 'Attacker:' })}</strong> ${report.attacker}</p>
           ${report.attackerAllies?.length ? `<p class="combat-allies">${i18n.t('fleet.alliesLabel', { defaultValue: 'Allies:' })} ${report.attackerAllies.map((ally) => ally.username).join(', ')}</p>` : ''}
           <p><strong>${i18n.t('fleet.defenderLabel', { defaultValue: 'Defender:' })}</strong> ${report.defender || i18n.t('fleet.unknown', { defaultValue: 'Unknown' })}</p>
           <div class="combat-loot">
             <span>${i18n.t('fleet.lootLabel', { defaultValue: 'Loot:' })} ${this.formatLoot(report.loot)}</span>
           </div>
           <div class="combat-losses">
             <span>${i18n.t('fleet.attackerLossesLabel', { defaultValue: 'Attacker Losses:' })} ${this.formatLosses(report.attackerLosses)}</span>
             <span>${i18n.t('fleet.defenderLossesLabel', { defaultValue: 'Defender Losses:' })} ${this.formatLosses(report.defenderLosses)}</span>
           </div>
           <div class="combat-report-actions">
             <button class="btn btn-secondary btn-small" data-replay="${index}">${i18n.t('fleet.watchReplay', { defaultValue: 'Watch Replay' })}</button>
           </div>
        </div>
      `;
      container.appendChild(card);
    });

    container.querySelectorAll<HTMLButtonElement>('[data-replay]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const report = this.combatReports[parseInt(btn.dataset.replay, 10)];
        if (window.combatVisualizer && report) {
          window.combatVisualizer.play(report);
        } else {
          this.notify(i18n.t('fleet.replayUnavailable', { defaultValue: 'Replay unavailable for this report.' }), 'info');
        }
      });
    });
  }

  renderMissionLog() {
    const container = document.getElementById('missionLogEntries');
    if (!container) return;

    if (!this.missionLog.length) {
      container.innerHTML = `<div class="empty-state card-compact">${i18n.t('fleet.noMissionActivity', { defaultValue: 'No mission activity yet.' })}</div>`;
      return;
    }

    container.innerHTML = this.missionLog
      .map(
        (entry) => `
        <div class="mission-log-entry">
          <div>
            <strong>${entry.title}</strong>
            <p>${entry.message}</p>
          </div>
          <small>${new Date(entry.timestamp).toLocaleTimeString()}</small>
        </div>`
      )
      .join('');
  }

  async recallFleet(fleetId) {
    try {
      await api.post(`/fleet/${fleetId}/recall`);
      this.notify(i18n.t('fleet.recallSent', { defaultValue: 'Recall order sent.' }), 'info');
      this.fetchActiveFleets();
    } catch (error) {
      this.notify(error.message || i18n.t('fleet.failedToRecall', { defaultValue: 'Failed to recall fleet' }), 'error');
    }
  }

  extractPlanetShips() {
    if (!this.planet) return {};
    const result = {};
    Object.keys(SHIP_STATS).forEach((key) => {
      if (this.planet[key] > 0) {
        result[key] = this.planet[key];
      }
    });
    return result;
  }

  estimateFuel() {
    const distance = this.calculateDistance();
    return Object.entries(this.selectedShips).reduce((sum, [key, count]) => {
      return sum + (SHIP_STATS[key]?.fuel || 0) * count * (distance / 10000000);
    }, 0);
  }

  getShipLabel(key) {
    const i18nLabel = i18n.t(`shipyard.ships.${key}.name`, { defaultValue: '' });
    if (i18nLabel) return i18nLabel;
    if (SHIP_STATS[key] && SHIP_STATS[key].name) return SHIP_STATS[key].name;
    return this.formatName(key);
  }

  calculateDistance() {
    if (!this.planet) return 0;

    const from = this.planet;
    const toGalaxy = parseInt(document.getElementById('targetGalaxy')?.value) || 1;
    const toSystem = parseInt(document.getElementById('targetSystem')?.value) || 1;
    const toPosition = parseInt(document.getElementById('targetPosition')?.value) || 1;

    if (from.galaxy !== toGalaxy) {
      return 20000 * Math.abs(from.galaxy - toGalaxy) * 1000000;
    } else if (from.system !== toSystem) {
      return 2700 + 95 * Math.abs(from.system - toSystem) * 1000;
    } else {
      return 1000 + 5 * Math.abs(from.position - toPosition) * 100;
    }
  }

  notify(message, type = 'info') {
    if (typeof showNotification === 'function') {
      showNotification('Fleet', message, type);
    } else {
      console.log(`[${type}] ${message}`);
    }
  }

  formatNumber(value) {
    return new Intl.NumberFormat('en-US').format(Math.floor(value || 0));
  }

  formatCountdown(seconds) {
    if (seconds == null) return '—';
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hrs}h ${mins}m ${secs}s`;
  }

  startCountdownTicker() {
    if (this.countdownInterval) {
      clearInterval(this.countdownInterval);
    }
    this.countdownInterval = window.setInterval(() => this.updateCountdownNodes(), 200);
  }

  updateCountdownNodes() {
    const nodes = document.querySelectorAll<HTMLElement>('[data-countdown]');
    const now = Date.now();
    nodes.forEach((node) => {
      const timestamp = Number(node.dataset.timestamp);
      if (!timestamp) {
        node.textContent = '—';
        return;
      }
      node.textContent = this.formatCountdownMs(timestamp - now);
    });
  }

  formatCountdownMs(ms: number) {
    if (ms <= 0) return '0.0s';
    const hours = Math.floor(ms / 3600000);
    const minutes = Math.floor((ms % 3600000) / 60000);
    const seconds = Math.floor((ms % 60000) / 1000);
    const tenths = Math.floor((ms % 1000) / 100);
    if (hours > 0) return `${hours}h ${minutes}m ${seconds}s`;
    if (minutes > 0) return `${minutes}m ${seconds}.${tenths}s`;
    return `${seconds}.${tenths}s`;
  }

  getMissionLabel(mission) {
    const labels = {
      attack: 'Attack',
      transport: 'Transport',
      deploy: 'Deploy',
      espionage: 'Espionage',
      colonize: 'Colonize',
      harvest: 'Harvest',
    };
    return labels[mission] || this.formatName(mission);
  }

  getLocalizedMissionLabel(mission) {
    return i18n.t(`galaxy.action.${mission}`, { defaultValue: this.getMissionLabel(mission) });
  }

  formatName(key) {
    return key
      .split('_')
      .map((chunk) => chunk.charAt(0).toUpperCase() + chunk.slice(1))
      .join(' ');
  }
  formatCoords(report) {
    if (!report?.target) return i18n.t('fleet.unknownCoordinates', { defaultValue: 'Unknown coordinates' });
    const { galaxy, system, position } = report.target;
    return `${galaxy}:${system}:${position}`;
  }

  formatLoot(loot = {}) {
    const parts = [];
    if (loot.metal) parts.push(`${this.formatNumber(loot.metal)} metal`);
    if (loot.crystal) parts.push(`${this.formatNumber(loot.crystal)} crystal`);
    if (loot.deuterium) parts.push(`${this.formatNumber(loot.deuterium)} deut.`);
    return parts.length ? parts.join(', ') : 'None';
  }

  formatLosses(losses = {}) {
    const entries = Object.entries(losses);
    if (!entries.length) return 'None';
    return entries
      .map(([key, count]) => `${this.formatName(key)}-${count}`)
      .slice(0, 4)
      .join(', ');
  }
  filterFleets() {
    return this.activeFleets.filter((fleet) => {
      const missionMatch = this.filters.mission === 'all' || fleet.mission_type === this.filters.mission;
      const statusMatch = this.filters.status === 'all' || fleet.status === this.filters.status;
      return missionMatch && statusMatch;
    });
  }

  recordMissionLog(payload) {
    const now = new Date().toISOString();
    let entry = null;

    switch (payload.action) {
      case 'dispatch':
        entry = {
          title: i18n.t('fleet.log.dispatched', { defaultValue: 'Fleet Dispatched' }),
          message: this.getLocalizedMissionLabel(payload.fleet?.mission_type) + ' to ' + this.formatCoords({
            target: {
              galaxy: payload.fleet?.target_galaxy,
              system: payload.fleet?.target_system,
              position: payload.fleet?.target_position,
            },
          }),
          timestamp: now,
        };
        break;
      case 'arrival':
        entry = {
          title: i18n.t('fleet.log.arrived', { defaultValue: 'Fleet Arrived' }),
          message: i18n.t('fleet.log.arrivedMessage', { id: payload.fleetId, defaultValue: `Fleet #${payload.fleetId} reached its destination` }),
          timestamp: now,
        };
        break;
      case 'recall':
        entry = {
          title: i18n.t('fleet.log.recall', { defaultValue: 'Recall Issued' }),
          message: i18n.t('fleet.log.recallMessage', { id: payload.fleetId, defaultValue: `Fleet #${payload.fleetId} is returning` }),
          timestamp: now,
        };
        break;
      case 'combat':
        entry = {
          title: payload.role === 'attacker' ? i18n.t('fleet.log.combatReport', { defaultValue: 'Combat Report' }) : i18n.t('fleet.log.defenseReport', { defaultValue: 'Defense Report' }),
          message: i18n.t('fleet.log.combatMessage', { result: (payload.report?.winner || '').toUpperCase(), coords: this.formatCoords(payload.report), defaultValue: `${(payload.report?.winner || '').toUpperCase()} at ${this.formatCoords(payload.report)}` }),

          timestamp: now,
        };
        break;
      case 'colonize': {
        const planetCoords = payload.planet
          ? `${payload.planet.galaxy}:${payload.planet.system}:${payload.planet.position}`
          : 'target coordinates';
        entry = {
          title: payload.status === 'success' ? i18n.t('fleet.log.colonyEstablished', { defaultValue: 'Colony Established' }) : i18n.t('fleet.log.colonizationFailed', { defaultValue: 'Colonization Failed' }),
          message:
            payload.status === 'success'
              ? `New colony founded at ${planetCoords}`
              : `Colony ship could not claim ${planetCoords}`,
          timestamp: now,
        };
        break;
      }
      case 'espionage':
        entry = {
          title: i18n.t('fleet.log.espionage', { defaultValue: 'Espionage Report' }),
          message: i18n.t('fleet.log.espionageMessage', { level: payload.intelLevel || 'standard', detected: payload.detected ? i18n.t('fleet.detected', { defaultValue: 'Detected' }) : '', defaultValue: `Intel ${payload.intelLevel || 'standard'}${payload.detected ? ' • Detected' : ''}` }),

          timestamp: now,
        };
        break;
      case 'harvest': {
        const metal = payload.collected?.metal || 0;
        const crystal = payload.collected?.crystal || 0;
        entry = {
          title: payload.empty ? i18n.t('fleet.log.harvestAttempt', { defaultValue: 'Harvest Attempt' }) : i18n.t('fleet.log.harvestComplete', { defaultValue: 'Harvest Complete' }),
          message: payload.empty
            ? i18n.t('fleet.noDebris', { defaultValue: 'No debris recovered.' })
            : i18n.t('fleet.harvestRecovered', { metal: this.formatNumber(metal), crystal: this.formatNumber(crystal), defaultValue: 'Recovered ' + this.formatNumber(metal) + ' metal and ' + this.formatNumber(crystal) + ' crystal.' }),
          timestamp: now,
        };
        break;
      }
    }

    if (entry) {
      this.missionLog = [entry, ...this.missionLog].slice(0, 25);
      this.renderMissionLog();
    }
  }

  async loadMissionHistory() {
    try {
      const history = await api.get('/fleet/history?limit=25');
    this.missionLog = (history || []).map((fleet) => ({
        title: this.getLocalizedMissionLabel(fleet.mission_type) + ' (' + (fleet.status || i18n.t('fleet.unknown', { defaultValue: 'unknown' })) + ')',
        message: this.formatCoords({
          target: {
            galaxy: fleet.target_galaxy,
            system: fleet.target_system,
            position: fleet.target_position,
          },
        }) + ' • ' + i18n.t('fleet.shipsLabel', { defaultValue: 'Ships:' }) + ' ' + this.formatShipsSummary(fleet.ships),
        timestamp: fleet.departure_time || fleet.createdAt || new Date().toISOString(),
      }));
      this.renderMissionLog();
    } catch (error) {
      console.error('Failed to load mission history:', error);
    }
  }

  formatShipsSummary(ships = {}) {
    const entries = Object.entries(ships);
    if (!entries.length) return i18n.t('fleet.noShips', { defaultValue: 'No ships' });
    return entries
      .map(([key, count]) => (SHIP_STATS[key]?.name || this.formatName(key)) + ' ' + count)
      .slice(0, 4)
      .join(', ');
  }

  destroy() {
    if (this.pollInterval) {
      clearInterval(this.pollInterval);
      this.pollInterval = null;
    }
    if (this.countdownInterval) {
      clearInterval(this.countdownInterval);
      this.countdownInterval = null;
    }
  }
}

let fleetManager;

function updatePageData(data) {
  if (!fleetManager) return;
  fleetManager.updatePlanet(data);
}

document.addEventListener('DOMContentLoaded', () => {
  fleetManager = new FleetManager();
  window.updatePageData = updatePageData;
  window.addEventListener('beforeunload', () => fleetManager?.destroy());
});
