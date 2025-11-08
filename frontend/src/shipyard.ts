// @ts-nocheck

const SHIP_BLUEPRINTS = {
  small_cargo: { name: 'Small Cargo', description: 'Basic transporter for resources.', cost: { metal: 2000, crystal: 2000, deuterium: 0 }, image: 'support-cargo-freighter' },
  large_cargo: { name: 'Large Cargo', description: 'Heavy duty transporter with expanded capacity.', cost: { metal: 6000, crystal: 6000, deuterium: 0 }, image: 'support-cargo-freighter' },
  light_fighter: { name: 'Light Fighter', description: 'Fast attack craft ideal for raiding.', cost: { metal: 3000, crystal: 1000, deuterium: 0 }, image: 'fighter-interceptor' },
  heavy_fighter: { name: 'Heavy Fighter', description: 'Armored strike craft with higher damage.', cost: { metal: 6000, crystal: 4000, deuterium: 0 }, image: 'fighter-assault' },
  cruiser: { name: 'Cruiser', description: 'Balanced ship excelling at escort missions.', cost: { metal: 20000, crystal: 7000, deuterium: 2000 }, image: 'cruiser-medium' },
  battleship: { name: 'Battleship', description: 'Heavy flagship-class combat vessel.', cost: { metal: 45000, crystal: 15000, deuterium: 0 }, image: 'battleship-dreadnought' },
  colony_ship: { name: 'Colony Ship', description: 'Carries a full population ready to settle new worlds.', cost: { metal: 10000, crystal: 20000, deuterium: 10000 }, image: 'support-colony-ship' },
  recycler: { name: 'Recycler', description: 'Harvests debris left behind after battles.', cost: { metal: 10000, crystal: 6000, deuterium: 2000 }, image: 'miner-industrial' },
  espionage_probe: { name: 'Espionage Probe', description: 'Gather intel on enemy worlds.', cost: { metal: 1000, crystal: 0, deuterium: 0 }, image: 'probe' },
  bomber: { name: 'Bomber', description: 'Specialized at destroying defenses.', cost: { metal: 50000, crystal: 25000, deuterium: 15000 }, image: 'bomber' },
  destroyer: { name: 'Destroyer', description: 'Capital ship that counters most defenses.', cost: { metal: 60000, crystal: 50000, deuterium: 15000 }, image: 'destroyer' },
  deathstar: { name: 'Deathstar', description: 'Planet buster with massive firepower.', cost: { metal: 5000000, crystal: 4000000, deuterium: 1000000 }, image: 'deathstar' },
};

const DEFENSE_BLUEPRINTS = {
  rocket_launcher: { name: 'Rocket Launcher', description: 'Basic defensive battery.', cost: { metal: 2000, crystal: 0, deuterium: 0 }, image: 'missile-battery' },
  light_laser: { name: 'Light Laser', description: 'Standard defensive laser.', cost: { metal: 1500, crystal: 500, deuterium: 0 }, image: 'defense-turret' },
  heavy_laser: { name: 'Heavy Laser', description: 'Upgraded laser with enhanced output.', cost: { metal: 6000, crystal: 2000, deuterium: 0 }, image: 'plasma-turret' },
  gauss_cannon: { name: 'Gauss Cannon', description: 'Magnetically accelerated slug launcher.', cost: { metal: 20000, crystal: 15000, deuterium: 2000 }, image: 'plasma-turret' },
  ion_cannon: { name: 'Ion Cannon', description: 'Disrupts shields with ionized beams.', cost: { metal: 2000, crystal: 6000, deuterium: 0 }, image: 'ion-cannon' },
  plasma_turret: { name: 'Plasma Turret', description: 'End-game defense platform.', cost: { metal: 50000, crystal: 50000, deuterium: 30000 }, image: 'plasma-turret' },
};

class ShipyardManager {
  constructor() {
    this.planet = null;
    this.moon = null;
    this.activeTab = 'ships';
    this.queue = [];
    this.queueTimers = [];
    this.locationType = 'planet';
    this.locationSelect = document.getElementById('shipyardLocationSelect');
    this.locationStatus = document.getElementById('shipyardLocationStatus');
    this.locationResources = document.getElementById('shipyardLocationResources');
    this.initTabs();
    this.initLocationControls();
    this.startQueuePolling();
  }

  initTabs() {
    document.querySelectorAll('.shipyard-tabs .btn').forEach((btn) => {
      btn.addEventListener('click', () => {
        document.querySelectorAll('.shipyard-tabs .btn').forEach((b) => b.classList.remove('active'));
        document.querySelectorAll('.tab-content').forEach((c) => c.classList.remove('active'));

        btn.classList.add('active');
        const tab = btn.dataset.tab;
        document.getElementById(`${tab}Tab`)?.classList.add('active');
        this.activeTab = tab;

        if (tab === 'ships') {
          this.renderShips();
        } else {
          this.renderDefense();
        }
      });
    });
  }

  initLocationControls() {
    if (!this.locationSelect) return;
    this.locationSelect.addEventListener('change', () => {
      this.locationType = this.locationSelect.value === 'moon' ? 'moon' : 'planet';
      this.updateLocationStatus();
      this.updateLocationResources();
      this.renderShips();
      this.renderDefense();
      this.loadQueue();
    });
  }

  refreshLocationControls() {
    if (!this.locationSelect) return;
    this.locationSelect.innerHTML = '';

    if (this.planet) {
      const planetOption = document.createElement('option');
      planetOption.value = 'planet';
      planetOption.textContent = `${this.planet.name} Shipyard`;
      this.locationSelect.appendChild(planetOption);
    }

    if (this.moon) {
      const moonOption = document.createElement('option');
      moonOption.value = 'moon';
      moonOption.textContent = `${this.moon.name} Shipyard`;
      this.locationSelect.appendChild(moonOption);
      this.locationSelect.disabled = false;
    } else {
      this.locationSelect.disabled = true;
    }

    if (this.locationType === 'moon' && !this.moon) {
      this.locationType = 'planet';
    }

    this.locationSelect.value = this.locationType;
    this.updateLocationStatus();
    this.updateLocationResources();
  }

  updateLocationStatus() {
    if (!this.locationStatus) return;

    if (this.locationType === 'moon') {
      if (!this.moon) {
        this.locationStatus.textContent = 'No moon available.';
      } else if ((this.moon.moon_shipyard || 0) === 0) {
        this.locationStatus.textContent = 'Build a Moon Shipyard to produce fleets here.';
      } else {
        this.locationStatus.textContent = `Moon shipyard level ${this.moon.moon_shipyard}.`;
      }
    } else {
      const level = this.planet?.shipyard || 0;
      this.locationStatus.textContent = level
        ? `Planetary shipyard level ${level}.`
        : 'Build a shipyard on this planet to unlock production.';
    }
  }

  updateLocationResources() {
    if (!this.locationResources) return;
    const context = this.getActiveLocation();

    if (!context || !context.source) {
      this.locationResources.classList.add('hidden');
      return;
    }

    this.locationResources.classList.remove('hidden');
    const { source } = context;
    this.locationResources.innerHTML = `
      <div class="resource-chip"><img src="/assets/ui/resource-metal.png" alt="Metal">${this.formatNumber(source.metal || 0)}</div>
      <div class="resource-chip"><img src="/assets/ui/resource-crystal.png" alt="Crystal">${this.formatNumber(source.crystal || 0)}</div>
      <div class="resource-chip"><img src="/assets/ui/resource-deuterium.png" alt="Deuterium">${this.formatNumber(source.deuterium || 0)}</div>
    `;
  }

  getActiveLocation() {
    if (this.locationType === 'moon' && this.moon) {
      return {
        type: 'moon',
        source: this.moon,
        shipyardLevel: this.moon.moon_shipyard || 0,
      };
    }

    return {
      type: 'planet',
      source: this.planet,
      shipyardLevel: this.planet?.shipyard || 0,
    };
  }

  startQueuePolling() {
    setInterval(() => {
      if (this.planet) {
        this.loadQueue();
      }
    }, 30000);
  }

  updatePlanet(data) {
    this.planet = data.planet;
    this.moon = data.moonData?.moon || null;
    this.refreshLocationControls();
    this.renderShips();
    this.renderDefense();
    this.loadQueue();
  }

  renderShips() {
    const grid = document.getElementById('shipsGrid');
    if (!grid) return;

    const context = this.getActiveLocation();
    if (!context || !context.source) {
      grid.innerHTML = '<p class="text-muted">Select a production location.</p>';
      return;
    }

    if ((context.shipyardLevel || 0) === 0) {
      grid.innerHTML =
        this.locationType === 'moon'
          ? '<p class="text-muted">Build a Moon Shipyard to construct fleets here.</p>'
          : '<p class="text-muted">Build a shipyard on this planet to construct ships.</p>';
      return;
    }

    const resources = context.source;
    grid.innerHTML = '';
    Object.entries(SHIP_BLUEPRINTS).forEach(([key, blueprint]) => {
      const available = resources[key] || 0;
      const canAfford = this.canAfford(blueprint.cost, resources);

      const card = document.createElement('div');
      card.className = 'ship-card card-enhanced';
      card.innerHTML = `
        <div class="ship-card-header">
          <div>
            <h3>${blueprint.name}</h3>
            <p>In hangar: ${available}</p>
          </div>
          <img src="/assets/ships/${blueprint.image}.png" alt="${blueprint.name}" onerror="this.src='/assets/ships/fighter-interceptor.png'">
        </div>
        <p class="ship-description">${blueprint.description}</p>
        ${this.renderCost(blueprint.cost, resources)}
        <div class="build-controls">
          <input type="number" class="quantity-input" min="1" value="1" aria-label="quantity">
          <button class="btn btn-primary" ${canAfford ? '' : 'disabled'}>
            ${canAfford ? 'Build' : 'Insufficient resources'}
          </button>
        </div>
      `;

      const input = card.querySelector('.quantity-input');
      const button = card.querySelector('button');
      button?.addEventListener('click', () => {
        const quantity = Math.max(1, parseInt(input.value) || 1);
        this.startProduction(key, quantity);
      });

      grid.appendChild(card);
    });
  }

  renderDefense() {
    const grid = document.getElementById('defenseGrid');
    if (!grid) return;

    const context = this.getActiveLocation();
    if (!context || !context.source) {
      grid.innerHTML = '<p class="text-muted">Select a production location.</p>';
      return;
    }

    if ((context.shipyardLevel || 0) === 0) {
      grid.innerHTML =
        this.locationType === 'moon'
          ? '<p class="text-muted">Build a Moon Shipyard to construct defenses here.</p>'
          : '<p class="text-muted">Build a shipyard on this planet to unlock defenses.</p>';
      return;
    }

    const resources = context.source;
    grid.innerHTML = '';
    Object.entries(DEFENSE_BLUEPRINTS).forEach(([key, blueprint]) => {
      const available = resources[key] || 0;
      const canAfford = this.canAfford(blueprint.cost, resources);

      const card = document.createElement('div');
      card.className = 'ship-card card-enhanced';
      card.innerHTML = `
        <div class="ship-card-header">
          <div>
            <h3>${blueprint.name}</h3>
            <p>Deployed: ${available}</p>
          </div>
          <img src="/assets/buildings/${blueprint.image}.png" alt="${blueprint.name}" onerror="this.src='/assets/buildings/defense-turret.png'">
        </div>
        <p class="ship-description">${blueprint.description}</p>
        ${this.renderCost(blueprint.cost, resources)}
        <div class="build-controls">
          <input type="number" class="quantity-input" min="1" value="1" aria-label="quantity">
          <button class="btn btn-primary" ${canAfford ? '' : 'disabled'}>
            ${canAfford ? 'Build' : 'Insufficient resources'}
          </button>
        </div>
      `;

      const input = card.querySelector('.quantity-input');
      const button = card.querySelector('button');
      button?.addEventListener('click', () => {
        const quantity = Math.max(1, parseInt(input.value) || 1);
        this.startProduction(key, quantity);
      });

      grid.appendChild(card);
    });
  }

  renderCost(cost, resources = {}) {
    return `
      <div class="building-cost">
        <div class="cost-item">
          <img src="/assets/ui/resource-metal.png" alt="Metal">
          <span class="${(resources.metal || 0) < cost.metal ? 'insufficient' : ''}">${this.formatNumber(cost.metal)}</span>
        </div>
        <div class="cost-item">
          <img src="/assets/ui/resource-crystal.png" alt="Crystal">
          <span class="${(resources.crystal || 0) < cost.crystal ? 'insufficient' : ''}">${this.formatNumber(cost.crystal)}</span>
        </div>
        <div class="cost-item">
          <img src="/assets/ui/resource-deuterium.png" alt="Deuterium">
          <span class="${(resources.deuterium || 0) < cost.deuterium ? 'insufficient' : ''}">${this.formatNumber(cost.deuterium)}</span>
        </div>
      </div>
    `;
  }

  canAfford(cost, resources = {}) {
    return (
      (resources.metal || 0) >= cost.metal &&
      (resources.crystal || 0) >= cost.crystal &&
      (resources.deuterium || 0) >= cost.deuterium
    );
  }

  async startProduction(unitType, quantity) {
    if (!this.planet) return;
    if (this.locationType === 'moon' && !this.moon) return;

    try {
      await api.post(`/shipyard/${this.planet.id}/build`, {
        unitType,
        quantity,
        locationType: this.locationType,
        moonId: this.locationType === 'moon' ? this.moon?.id : undefined,
      });

      showNotification('Shipyard', `Production started: ${quantity}x ${this.formatName(unitType)}`, 'success');
      await loadPlanetData(this.planet.id);
    } catch (error) {
      showNotification('Shipyard', error.message || 'Failed to start production', 'error');
    }
  }

  async loadQueue() {
    if (!this.planet) return;

    try {
      const params =
        this.locationType === 'moon' && this.moon
          ? `?locationType=moon&moonId=${this.moon.id}`
          : '?locationType=planet';
      const queue = await api.get(`/shipyard/${this.planet.id}/queue${params}`);
      this.queue = Array.isArray(queue) ? queue : [];
      this.renderQueue();
    } catch (error) {
      console.error('Failed to load shipyard queue:', error);
    }
  }

  renderQueue() {
    const wrapper = document.getElementById('shipProductionQueue');
    const queueContainer = document.getElementById('shipQueue');
    if (!wrapper || !queueContainer) return;

    queueContainer.innerHTML = '';
    this.queueTimers.forEach(clearInterval);
    this.queueTimers = [];

    if (!this.queue.length) {
      wrapper.style.display = 'none';
      return;
    }

    wrapper.style.display = 'block';
    const header = wrapper.querySelector('h3');
    if (header) {
      header.textContent =
        this.locationType === 'moon' ? 'Moon Production Queue' : 'Ship Production Queue';
    }

    this.queue.forEach((item) => {
      const percent = Math.round((item.progress || 0) * 100);
      const element = document.createElement('div');
      element.className = 'queue-item';
      element.innerHTML = `
        <div class="queue-header">
          <div>
            <strong>${item.quantity}x ${this.formatName(item.unit_type)}</strong>
            <span class="queue-status">ETA: <span data-queue-timer="${item.id}">${this.formatTime(item.secondsRemaining || 0)}</span></span>
          </div>
          <button class="btn btn-text" data-cancel="${item.id}">Cancel</button>
        </div>
        <div class="progress-bar">
          <div class="progress-fill" style="width: ${percent}%"></div>
        </div>
      `;

      queueContainer.appendChild(element);

      const cancelBtn = element.querySelector('[data-cancel]');
      cancelBtn?.addEventListener('click', () => this.cancelQueue(item.id));

      const timerDisplay = element.querySelector(`[data-queue-timer="${item.id}"]`);
      if (timerDisplay) {
        timerDisplay.dataset.remaining = String(item.secondsRemaining || 0);
      }

      const timerId = setInterval(() => {
        const timer = document.querySelector(`[data-queue-timer="${item.id}"]`);
        if (!timer) return;

        const remaining = Math.max(0, parseInt(timer.dataset.remaining || String(item.secondsRemaining || 0), 10) - 1);
        timer.dataset.remaining = remaining;
        timer.textContent = this.formatTime(remaining);

        if (remaining <= 0) {
          clearInterval(timerId);
          this.loadQueue();
        }
      }, 1000);

      this.queueTimers.push(timerId);
    });
  }

  async cancelQueue(queueId) {
    if (!this.planet) return;

    if (!confirm('Cancel production? Only 60% of resources are refunded.')) {
      return;
    }

    try {
      await api.delete(`/shipyard/queue/${queueId}`);
      showNotification('Shipyard', 'Production cancelled', 'info');
      await loadPlanetData(this.planet.id);
    } catch (error) {
      showNotification('Shipyard', error.message || 'Failed to cancel production', 'error');
    }
  }

  formatName(key) {
    return key
      .split('_')
      .map((chunk) => chunk.charAt(0).toUpperCase() + chunk.slice(1))
      .join(' ');
  }

  formatNumber(value) {
    return new Intl.NumberFormat('en-US').format(Math.floor(value || 0));
  }

  formatTime(seconds) {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hrs}h ${mins}m ${secs}s`;
  }
}

let shipyardManager;

function updatePageData(data) {
    if (!shipyardManager) return;
    shipyardManager.updatePlanet(data);
}

document.addEventListener('DOMContentLoaded', () => {
  shipyardManager = new ShipyardManager();
  window.updatePageData = updatePageData;
});
