// @ts-nocheck
import i18next from 'i18next';

const SHIP_BLUEPRINTS = {
  small_cargo: { cost: { metal: 2000, crystal: 2000, deuterium: 0 }, image: 'support-cargo-freighter' },
  large_cargo: { cost: { metal: 6000, crystal: 6000, deuterium: 0 }, image: 'support-cargo-freighter' },
  light_fighter: { cost: { metal: 3000, crystal: 1000, deuterium: 0 }, image: 'fighter-interceptor' },
  heavy_fighter: { cost: { metal: 6000, crystal: 4000, deuterium: 0 }, image: 'fighter-assault' },
  cruiser: { cost: { metal: 20000, crystal: 7000, deuterium: 2000 }, image: 'cruiser-medium' },
  battleship: { cost: { metal: 45000, crystal: 15000, deuterium: 0 }, image: 'battleship-dreadnought' },
  colony_ship: { cost: { metal: 10000, crystal: 20000, deuterium: 10000 }, image: 'support-colony-ship' },
  recycler: { cost: { metal: 10000, crystal: 6000, deuterium: 2000 }, image: 'miner-industrial' },
  espionage_probe: { cost: { metal: 1000, crystal: 0, deuterium: 0 }, image: 'probe' },
  bomber: { cost: { metal: 50000, crystal: 25000, deuterium: 15000 }, image: 'bomber' },
  destroyer: { cost: { metal: 60000, crystal: 50000, deuterium: 15000 }, image: 'destroyer' },
  deathstar: { cost: { metal: 5000000, crystal: 4000000, deuterium: 1000000 }, image: 'deathstar' },
};

const DEFENSE_BLUEPRINTS = {
  rocket_launcher: { cost: { metal: 2000, crystal: 0, deuterium: 0 }, image: 'missile-battery' },
  light_laser: { cost: { metal: 1500, crystal: 500, deuterium: 0 }, image: 'defense-turret' },
  heavy_laser: { cost: { metal: 6000, crystal: 2000, deuterium: 0 }, image: 'plasma-turret' },
  gauss_cannon: { cost: { metal: 20000, crystal: 15000, deuterium: 2000 }, image: 'plasma-turret' },
  ion_cannon: { cost: { metal: 2000, crystal: 6000, deuterium: 0 }, image: 'ion-cannon' },
  plasma_turret: { cost: { metal: 50000, crystal: 50000, deuterium: 30000 }, image: 'plasma-turret' },
};

export class ShipyardManager {
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
      planetOption.textContent = `${this.planet.name} ${i18next.t('shipyard.shipyardLabel')}`;
      this.locationSelect.appendChild(planetOption);
    }

    if (this.moon) {
      const moonOption = document.createElement('option');
      moonOption.value = 'moon';
      moonOption.textContent = `${this.moon.name} ${i18next.t('shipyard.shipyardLabel')}`;
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
        this.locationStatus.textContent = i18next.t('shipyard.noMoon');
      } else if ((this.moon.moon_shipyard || 0) === 0) {
        this.locationStatus.textContent = i18next.t('shipyard.buildMoonShipyard');
      } else {
        this.locationStatus.textContent = i18next.t('shipyard.moonShipyardLevel', { level: this.moon.moon_shipyard });
      }
    } else {
      const level = this.planet?.shipyard || 0;
      this.locationStatus.textContent = level
        ? i18next.t('shipyard.planetaryShipyardLevel', { level })
        : i18next.t('shipyard.buildShipyard');
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
      grid.innerHTML = `<p class="text-muted">${i18next.t('shipyard.selectProductionLocation')}</p>`;
      return;
    }

    if ((context.shipyardLevel || 0) === 0) {
      grid.innerHTML =
        this.locationType === 'moon'
          ? `<p class="text-muted">${i18next.t('shipyard.buildMoonShipyard')}</p>`
          : `<p class="text-muted">${i18next.t('shipyard.buildShipyard')}</p>`;
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
            <h3>${i18next.t(`shipyard.ships.${key}.name`)}</h3>
            <p>${i18next.t('shipyard.inHangar', { count: available })}</p>
          </div>
          <img src="/assets/ships/${blueprint.image}.png" alt="${i18next.t(`shipyard.ships.${key}.name`)}" onerror="this.src='/assets/ships/fighter-interceptor.png'">
        </div>
        <p class="ship-description">${i18next.t(`shipyard.ships.${key}.description`)}</p>
        ${this.renderCost(blueprint.cost, resources)}
        <div class="build-controls">
          <input type="number" class="quantity-input" min="1" value="1" aria-label="quantity">
          <button class="btn btn-primary" ${canAfford ? '' : 'disabled'}>
            ${canAfford ? i18next.t('shipyard.build') : i18next.t('shipyard.insufficientResources')}
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
      grid.innerHTML = `<p class="text-muted">${i18next.t('shipyard.selectProductionLocation')}</p>`;
      return;
    }

    if ((context.shipyardLevel || 0) === 0) {
      grid.innerHTML =
        this.locationType === 'moon'
          ? `<p class="text-muted">${i18next.t('shipyard.buildMoonShipyardForDefense')}</p>`
          : `<p class="text-muted">${i18next.t('shipyard.buildShipyardForDefense')}</p>`;
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
            <h3>${i18next.t(`shipyard.defense.${key}.name`)}</h3>
            <p>${i18next.t('shipyard.deployed', { count: available })}</p>
          </div>
          <img src="/assets/buildings/${blueprint.image}.png" alt="${i18next.t(`shipyard.defense.${key}.name`)}" onerror="this.src='/assets/buildings/defense-turret.png'">
        </div>
        <p class="ship-description">${i18next.t(`shipyard.defense.${key}.description`)}</p>
        ${this.renderCost(blueprint.cost, resources)}
        <div class="build-controls">
          <input type="number" class="quantity-input" min="1" value="1" aria-label="quantity">
            <button class="btn btn-primary" ${canAfford ? '' : 'disabled'}>
              ${canAfford ? i18next.t('shipyard.build') : i18next.t('shipyard.insufficientResources')}
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

      showNotification(i18next.t('shipyard.notificationTitle'), i18next.t('shipyard.productionStarted', { quantity, unit: i18next.t(`shipyard.ships.${unitType}.name`) }), 'success');
      await loadPlanetData(this.planet.id);
    } catch (error) {
      showNotification(i18next.t('shipyard.notificationTitle'), i18next.t('shipyard.failedToStartProduction'), 'error');
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
        this.locationType === 'moon' ? i18next.t('shipyard.moonProductionQueue') : i18next.t('shipyard.shipProductionQueue');
    }

    this.queue.forEach((item) => {
      const percent = Math.round((item.progress || 0) * 100);
      const element = document.createElement('div');
      element.className = 'queue-item';
      element.innerHTML = `
        <div class="queue-header">
          <div>
            <strong>${item.quantity}x ${this.formatName(item.unit_type)}</strong>
            <span class="queue-status">${i18next.t('shipyard.eta')}: <span data-queue-timer="${item.id}">${this.formatTime(item.secondsRemaining || 0)}</span></span>
          </div>
          <button class="btn btn-text" data-cancel="${item.id}">${i18next.t('shipyard.cancel')}</button>
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

    if (!confirm(i18next.t('shipyard.cancelConfirm'))) {
      return;
    }

    try {
      await api.delete(`/shipyard/queue/${queueId}`);
      showNotification(i18next.t('shipyard.notificationTitle'), i18next.t('shipyard.productionCancelled'), 'info');
      await loadPlanetData(this.planet.id);
    } catch (error) {
      showNotification(i18next.t('shipyard.notificationTitle'), i18next.t('shipyard.failedToCancelProduction'), 'error');
    }
  }

  formatName(key) {
    if (key === null || key === undefined) return String(key);

    const shipKey = `shipyard.ships.${key}.name`;
    const defenseKey = `shipyard.defense.${key}.name`;

    const shipName = i18next.t(shipKey);
    if (shipName && shipName !== shipKey) return shipName;

    const defenseName = i18next.t(defenseKey);
    if (defenseName && defenseName !== defenseKey) return defenseName;

    if (typeof key !== 'string') return String(key);

    if (!key) return '';

    return key
      .split('_')
      .map((chunk) => (chunk ? chunk.charAt(0).toUpperCase() + chunk.slice(1) : ''))
      .join(' ');
  }

  formatNumber(value) {
    const locale = this.getLocale();
    if (typeof Intl !== 'undefined' && Intl.NumberFormat) {
      return new Intl.NumberFormat(locale).format(Math.floor(value || 0));
    }
    return Math.floor(value || 0).toLocaleString();
  }

  getLocale() {
    try {
      if (i18next && i18next.language) {
        return i18next.language;
      }
    } catch (error) {
      // ignore i18next access errors
    }
    try {
      const stored = localStorage.getItem('preferredLanguage');
      if (stored) return stored;
    } catch (error) {
      // ignore storage access errors
    }
    return navigator.language || 'en-US';
  }

  formatTime(seconds) {
    // Handle non-number values
    if (typeof seconds !== 'number' || isNaN(seconds)) return '0h 0m 0s';

    // Use floor division for hours so negative seconds borrow from hours as expected
    const hrs = Math.floor(seconds / 3600);
    const remainder = seconds - hrs * 3600;
    const mins = Math.floor(remainder / 60);
    const secs = remainder % 60;

    return `${hrs}h ${mins}m ${secs}s`;
  }
}

export let shipyardManager;

export function updatePageData(data) {
    if (!shipyardManager) return;
    shipyardManager.updatePlanet(data);
}

document.addEventListener('DOMContentLoaded', () => {
  shipyardManager = new ShipyardManager();
  window.updatePageData = updatePageData;
});
