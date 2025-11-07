// @ts-nocheck

const SHIP_STATS = {
  small_cargo: { name: 'Small Cargo', cargo: 5000, fuel: 10 },
  large_cargo: { name: 'Large Cargo', cargo: 25000, fuel: 50 },
  light_fighter: { name: 'Light Fighter', cargo: 50, fuel: 20 },
  heavy_fighter: { name: 'Heavy Fighter', cargo: 100, fuel: 75 },
  cruiser: { name: 'Cruiser', cargo: 800, fuel: 300 },
  battleship: { name: 'Battleship', cargo: 1500, fuel: 500 },
  colony_ship: { name: 'Colony Ship', cargo: 7500, fuel: 1000 },
  recycler: { name: 'Recycler', cargo: 20000, fuel: 300 },
  espionage_probe: { name: 'Espionage Probe', cargo: 5, fuel: 1 },
  bomber: { name: 'Bomber', cargo: 500, fuel: 1000 },
  destroyer: { name: 'Destroyer', cargo: 2000, fuel: 1000 },
  deathstar: { name: 'Deathstar', cargo: 1000000, fuel: 1 },
};

class FleetManager {
  constructor() {
    this.planet = null;
    this.selectedShips = {};
    this.selectedMission = null;
    this.activeFleets = [];
    this.currentStep = 1;
    this.selectedTab = 'dispatch';
    this.pollInterval = null;
    this.init();
  }

  init() {
    this.bindTabs();
    this.bindStepButtons();
    this.bindMissionButtons();
    this.bindLaunchButton();
    this.startPolling();
    this.attachSocketListeners();
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
      });
    });
  }

  bindLaunchButton() {
    document.getElementById('launchFleet')?.addEventListener('click', () => this.dispatchFleet());
    document.getElementById('resetFleet')?.addEventListener('click', () => this.resetForm());
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
    window.socket.on('fleetUpdate', () => this.fetchActiveFleets());
  }

  goToStep(step) {
    if (step === 2 && Object.keys(this.selectedShips).length === 0) {
      this.notify('Please select at least one ship before proceeding.', 'error');
      return;
    }

    if (step === 3) {
      const targetGalaxy = parseInt((document.getElementById('targetGalaxy') as HTMLInputElement)?.value) || 1;
      const targetSystem = parseInt((document.getElementById('targetSystem') as HTMLInputElement)?.value) || 1;
      const targetPosition = parseInt((document.getElementById('targetPosition') as HTMLInputElement)?.value) || 1;
      if (!targetGalaxy || !targetSystem || !targetPosition) {
        this.notify('Provide valid target coordinates.', 'error');
        return;
      }
    }

    this.currentStep = step;
    document.querySelectorAll('.fleet-step').forEach((stepEl) => stepEl.classList.add('hidden'));
    document.getElementById(`step${step}`)?.classList.remove('hidden');
  }

  updatePlanet(data) {
    this.planet = data.planet;
    if (!document.getElementById('targetGalaxy')?.value) {
      document.getElementById('targetGalaxy').value = this.planet.galaxy;
      document.getElementById('targetSystem').value = this.planet.system;
      document.getElementById('targetPosition').value = this.planet.position;
    }
    this.renderFleetSelection();
    this.renderOverview();
    this.fetchActiveFleets();
  }

  renderFleetSelection() {
    const container = document.getElementById('fleetSelection');
    if (!container || !this.planet) return;

    container.innerHTML = '';
    const ships = this.extractPlanetShips();

    if (Object.keys(ships).length === 0) {
      container.innerHTML = '<div class="empty-state card-compact">No ships available on this planet.</div>';
      return;
    }

    Object.entries(ships).forEach(([key, amount]) => {
      if (!SHIP_STATS[key]) return;
      const card = document.createElement('div');
      card.className = 'fleet-card';
      card.innerHTML = `
        <div class="fleet-card-header">
          <h3>${SHIP_STATS[key].name}</h3>
          <span>Available: ${this.formatNumber(amount)}</span>
        </div>
        <div class="fleet-card-body">
          <label>
            Quantity
            <input type="number" min="0" max="${amount}" value="${this.selectedShips[key] || 0}" data-ship="${key}">
          </label>
          <button class="btn btn-text" data-select-all="${key}">Fill</button>
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
      summary.textContent = 'No ships selected.';
      cargoLabel.textContent = '0';
      fuelLabel.textContent = '0';
      return;
    }

    const parts = Object.entries(this.selectedShips).map(([key, value]) => `${SHIP_STATS[key].name}: ${value}`);
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
      this.notify('Select ships for the fleet.', 'error');
      return;
    }

    if (!this.selectedMission) {
      this.notify('Choose a mission type.', 'error');
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
    };

    try {
      await api.post('/fleet/dispatch', payload);
      this.notify('Fleet dispatched successfully!', 'success');
      this.resetForm();
      await loadPlanetData(this.planet.id);
      this.fetchActiveFleets();
    } catch (error) {
      this.notify(error.message || 'Failed to dispatch fleet', 'error');
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

  renderOverview() {
    const container = document.getElementById('fleetInventory');
    if (!container || !this.planet) return;

    const ships = this.extractPlanetShips();
    if (Object.keys(ships).length === 0) {
      container.innerHTML = '<div class="empty-state card-compact">No ships stationed on this planet.</div>';
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

    if (this.activeFleets.length === 0) {
      container.innerHTML = '<div class="empty-state card-compact">No active missions.</div>';
      return;
    }

    container.innerHTML = '';

    this.activeFleets.forEach((fleet) => {
      const ships = typeof fleet.ships === 'string' ? JSON.parse(fleet.ships) : fleet.ships;
      const card = document.createElement('div');
      card.className = 'mission-card card-enhanced';

      card.innerHTML = `
        <div class="mission-header">
          <div>
            <h3>${this.getMissionLabel(fleet.mission_type)}</h3>
            <p>Target: ${fleet.target_galaxy}:${fleet.target_system}:${fleet.target_position}</p>
          </div>
          <div class="mission-status">${fleet.status}</div>
        </div>
        <div class="mission-body">
          <div><strong>Arrival:</strong> ${this.formatCountdown(fleet.secondsUntilArrival)}</div>
          <div><strong>Return:</strong> ${fleet.secondsUntilReturn ? this.formatCountdown(fleet.secondsUntilReturn) : 'Pending'}</div>
          <div class="mission-ships">
            ${Object.entries(ships || {})
              .map(([key, count]) => `<span>${SHIP_STATS[key]?.name || this.formatName(key)}: ${count}</span>`)
              .join('')}
          </div>
        </div>
        <div class="mission-actions">
          ${fleet.status === 'outbound' ? `<button class="btn btn-secondary" data-recall="${fleet.id}">Recall</button>` : ''}
        </div>
      `;

      const recallBtn = card.querySelector('[data-recall]');
      recallBtn?.addEventListener('click', () => this.recallFleet(fleet.id));

      container.appendChild(card);
    });
  }

  async recallFleet(fleetId) {
    try {
      await api.post(`/fleet/${fleetId}/recall`);
      this.notify('Recall order sent.', 'info');
      this.fetchActiveFleets();
    } catch (error) {
      this.notify(error.message || 'Failed to recall fleet', 'error');
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

  formatName(key) {
    return key
      .split('_')
      .map((chunk) => chunk.charAt(0).toUpperCase() + chunk.slice(1))
      .join(' ');
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
});
