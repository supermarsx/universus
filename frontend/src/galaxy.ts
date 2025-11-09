// @ts-nocheck

type MissionType =
  | 'attack'
  | 'transport'
  | 'deploy'
  | 'espionage'
  | 'colonize'
  | 'harvest';

// Ship labels are localized via i18n under `shipyard.ships.<key>.name`
function getShipLabel(key: string) {
  const localized = i18n.t(`shipyard.ships.${key}.name`, { defaultValue: undefined });
  if (localized && typeof localized === 'string' && localized !== `shipyard.ships.${key}.name`) return localized;
  // Fallback humanized key
  return key.replace(/_/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}


const MISSION_SHIPS: Record<MissionType, string[]> = {
  attack: ['light_fighter', 'heavy_fighter', 'cruiser', 'battleship', 'destroyer', 'bomber'],
  transport: ['small_cargo', 'large_cargo'],
  deploy: ['small_cargo', 'large_cargo', 'light_fighter', 'heavy_fighter', 'cruiser'],
  espionage: ['espionage_probe'],
  colonize: ['colony_ship'],
  harvest: ['recycler'],
};

const PHALANX_SCAN_COST = 5000;

class GalaxyCanvasRenderer {
  controller: GalaxyController;
  canvas: HTMLCanvasElement | null;
  ctx: CanvasRenderingContext2D | null;
  slots: any[] = [];
  rows = 3;
  cols = 5;
  focusPosition: number | null = null;
  starfield: Array<{ x: number; y: number; radius: number; alpha: number }> = [];
  nodePositions: Map<number, { x: number; y: number; radius: number }> = new Map();

  constructor(controller: GalaxyController) {
    this.controller = controller;
    this.canvas = document.getElementById('galaxyCanvas') as HTMLCanvasElement | null;
    this.ctx = this.canvas?.getContext('2d') || null;

    if (this.canvas && this.ctx) {
      this.generateStarfield();
      this.resize();
      window.addEventListener('resize', () => this.resize());
      this.canvas.addEventListener('click', (event) => this.handleClick(event));
    }
  }

  setSlots(slots: any[]) {
    this.slots = slots;
    this.render();
  }

  setFocusPosition(position: number | null) {
    this.focusPosition = position;
    this.render();
  }

  resize() {
    if (!this.canvas) return;
    const width = this.canvas.parentElement?.clientWidth || 900;
    this.canvas.width = width;
    this.canvas.height = Math.min(520, width * 0.55);
    this.render();
  }

  render() {
    if (!this.ctx || !this.canvas) return;
    this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    this.drawBackground();
    this.drawGrid();
    this.drawPlanets();
  }

  drawBackground() {
    if (!this.ctx || !this.canvas) return;
    const gradient = this.ctx.createRadialGradient(
      this.canvas.width / 2,
      this.canvas.height / 3,
      80,
      this.canvas.width / 2,
      this.canvas.height / 2,
      Math.max(this.canvas.width, this.canvas.height)
    );
    gradient.addColorStop(0, 'rgba(74, 158, 255, 0.08)');
    gradient.addColorStop(1, 'rgba(9, 12, 20, 0.95)');
    this.ctx.fillStyle = gradient;
    this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    this.ctx.fillStyle = 'rgba(255,255,255,0.7)';
    this.starfield.forEach((star) => {
      this.ctx.globalAlpha = star.alpha;
      this.ctx.beginPath();
      this.ctx.arc(star.x * this.canvas.width, star.y * this.canvas.height, star.radius, 0, Math.PI * 2);
      this.ctx.fill();
    });
    this.ctx.globalAlpha = 1;
  }

  drawGrid() {
    if (!this.ctx || !this.canvas) return;
    this.ctx.strokeStyle = 'rgba(255,255,255,0.08)';
    this.ctx.lineWidth = 1;
    const paddingX = 70;
    const paddingY = 50;
    const width = this.canvas.width - paddingX * 2;
    const height = this.canvas.height - paddingY * 2;
    const stepX = width / (this.cols - 1);
    const stepY = height / (this.rows - 1);

    for (let r = 0; r < this.rows; r++) {
      this.ctx.beginPath();
      this.ctx.moveTo(paddingX, paddingY + r * stepY);
      this.ctx.lineTo(paddingX + width, paddingY + r * stepY);
      this.ctx.stroke();
    }

    for (let c = 0; c < this.cols; c++) {
      this.ctx.beginPath();
      this.ctx.moveTo(paddingX + c * stepX, paddingY);
      this.ctx.lineTo(paddingX + c * stepX, paddingY + height);
      this.ctx.stroke();
    }
  }

  drawPlanets() {
    if (!this.ctx || !this.canvas) return;
    this.nodePositions.clear();
    const paddingX = 70;
    const paddingY = 50;
    const width = this.canvas.width - paddingX * 2;
    const height = this.canvas.height - paddingY * 2;
    const stepX = width / (this.cols - 1);
    const stepY = height / (this.rows - 1);

    for (let position = 1; position <= 15; position++) {
      const row = Math.floor((position - 1) / this.cols);
      const col = (position - 1) % this.cols;
      const x = paddingX + col * stepX;
      const y = paddingY + row * stepY;
      const slot = this.slots.find((s) => s.position === position);
      const hasPlanet = slot?.hasPlanet;
      const isOwn = slot?.owner?.relation === 'self';
      const isFocus = this.focusPosition === position;
      const radius = hasPlanet ? 16 : 10;

      this.ctx.beginPath();
      this.ctx.fillStyle = hasPlanet ? (isOwn ? '#22c55e' : '#4a9eff') : 'rgba(255,255,255,0.35)';
      if (isFocus) {
        this.ctx.shadowBlur = 20;
        this.ctx.shadowColor = '#fbbf24';
      }
      this.ctx.arc(x, y, radius, 0, Math.PI * 2);
      this.ctx.fill();
      this.ctx.shadowBlur = 0;

      this.ctx.fillStyle = 'rgba(255,255,255,0.75)';
      this.ctx.font = '11px Orbitron, sans-serif';
      this.ctx.fillText(position.toString().padStart(2, '0'), x + radius + 6, y + 4);

      this.nodePositions.set(position, { x, y, radius: radius + 6 });
    }
  }

  handleClick(event: MouseEvent) {
    if (!this.canvas) return;
    const rect = this.canvas.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;

    for (const [position, node] of this.nodePositions.entries()) {
      const distance = Math.hypot(x - node.x, y - node.y);
      if (distance <= node.radius) {
        this.controller.handleCanvasSelection(position);
        break;
      }
    }
  }

  private generateStarfield() {
    const starCount = 120;
    this.starfield = Array.from({ length: starCount }).map(() => ({
      x: Math.random(),
      y: Math.random(),
      radius: Math.random() * 1.5 + 0.5,
      alpha: Math.random() * 0.6 + 0.2,
    }));
  }
}

class GalaxyController {
  currentGalaxy = 1;
  currentSystem = 1;
  limits = { galaxies: 9, systems: 499, positions: 15 };

  ownedPlanets: any[] = [];
  originPlanetId: number | null = null;
  originPlanet: any | null = null;
  availableShips: Record<string, number> = {};

  planets: any[] = [];
  intel = {
    sensorRange: 1,
    espionageLevel: 0,
    originPlanetName: null as string | null,
    sensorSources: {
      espionage: 0,
      phalanx: 0,
      sensorArray: 0,
    },
  };

  isLoading = false;
  focusPosition: number | null = null;
  pendingTarget: { galaxy: number; system: number; position: number; mission: MissionType } | null = null;

  drawerState: {
    open: boolean;
    mission: MissionType;
    target: any | null;
  } = {
    open: false,
    mission: 'attack',
    target: null,
  };

  elements: Record<string, HTMLElement | null> = {};
  canvasRenderer: GalaxyCanvasRenderer | null = null;
  originMoon: any = null;
  isPhalanxLoading = false;

  constructor() {
    this.cacheDom();
    this.updatePhalanxState();
    this.canvasRenderer = new GalaxyCanvasRenderer(this);
    this.bindEvents();
    this.bootstrapFromGlobals();
    this.loadOwnedPlanets();
    this.loadResearch();
    this.scanSystem();
    this.subscribeToSocket();
  }

  cacheDom() {
    this.elements = {
      galaxyInput: document.getElementById('galaxyInput'),
      systemInput: document.getElementById('systemInput'),
      scanBtn: document.getElementById('scanBtn'),
      prevSystem: document.getElementById('prevSystem'),
      nextSystem: document.getElementById('nextSystem'),
      prevGalaxy: document.getElementById('prevGalaxy'),
      nextGalaxy: document.getElementById('nextGalaxy'),
      jumpInput: document.getElementById('coordinateJump'),
      jumpBtn: document.getElementById('jumpBtn'),
      galaxySummary: document.getElementById('galaxySystemSummary'),
      sensorSummary: document.getElementById('sensorSummary'),
      paginationSummary: document.getElementById('paginationSummary'),
      systemDisplay: document.getElementById('galaxySystemDisplay'),
      missionDrawer: document.getElementById('missionDrawer'),
      missionDrawerTitle: document.getElementById('missionDrawerTitle'),
      missionTargetSummary: document.getElementById('missionTargetSummary'),
      missionTypeSelect: document.getElementById('missionTypeSelect'),
      shipInputs: document.getElementById('shipInputs'),
      missionDrawerError: document.getElementById('missionDrawerError'),
      missionForm: document.getElementById('missionForm'),
      closeDrawer: document.getElementById('closeMissionDrawer'),
      drawerCancel: document.getElementById('drawerCancel'),
      drawerLaunch: document.getElementById('drawerLaunch'),
      toast: document.getElementById('galaxyToast'),
      planetModal: document.getElementById('planetDetailsModal'),
      planetModalContent: document.getElementById('planetDetailsContent'),
      originSelect: document.getElementById('originPlanetSelect'),
      originSummary: document.getElementById('originSummary'),
      shipSummary: document.getElementById('originShipSummary'),
      inlineTargetInput: document.getElementById('inlineTargetCoords'),
      inlineTargetMission: document.getElementById('inlineMissionSelect'),
      inlineTargetBtn: document.getElementById('inlineTargetBtn'),
      originPlanetLabel: document.getElementById('originPlanetLabel'),
      phalanxScanBtn: document.getElementById('phalanxScanBtn'),
      phalanxStatus: document.getElementById('phalanxStatus'),
      phalanxModal: document.getElementById('phalanxModal'),
      phalanxResults: document.getElementById('phalanxResults'),
      closePhalanxModal: document.getElementById('closePhalanxModal'),
    };
  }

  bindEvents() {
    this.elements.scanBtn?.addEventListener('click', () => this.scanSystem());
    this.elements.prevSystem?.addEventListener('click', () => this.shiftSystem(-1));
    this.elements.nextSystem?.addEventListener('click', () => this.shiftSystem(1));
    this.elements.prevGalaxy?.addEventListener('click', () => this.shiftGalaxy(-1));
    this.elements.nextGalaxy?.addEventListener('click', () => this.shiftGalaxy(1));
    this.elements.jumpBtn?.addEventListener('click', () => this.handleJump());
    this.elements.galaxyInput?.addEventListener('change', () => this.updateFromInputs());
    this.elements.systemInput?.addEventListener('change', () => this.updateFromInputs());

    this.elements.missionTypeSelect?.addEventListener('change', (event) => {
      this.drawerState.mission = (event.target as HTMLSelectElement).value as MissionType;
      this.renderMissionForm();
    });

    this.elements.missionForm?.addEventListener('submit', (event) => {
      event.preventDefault();
      this.dispatchMission();
    });

    this.elements.closeDrawer?.addEventListener('click', () => this.closeMissionDrawer());
    this.elements.drawerCancel?.addEventListener('click', () => this.closeMissionDrawer());

    this.elements.systemDisplay?.addEventListener('click', (event) => {
      const target = event.target as HTMLElement;
      const actionBtn = target.closest<HTMLButtonElement>('[data-action]');
      if (!actionBtn) return;

      const action = actionBtn.dataset.action as MissionType | 'details';
      const position = parseInt(actionBtn.dataset.position || '0', 10);
      const slot = this.planets.find((p) => p.position === position);
      if (!slot) return;

      if (action === 'details') {
        this.openPlanetDetails(slot);
        return;
      }

      this.openMissionDrawer(slot, action);
    });

    const closeModal = this.elements.planetModal?.querySelector('.close');
    closeModal?.addEventListener('click', () => this.hidePlanetModal());
    this.elements.planetModal?.addEventListener('click', (event) => {
      if (event.target === this.elements.planetModal) {
        this.hidePlanetModal();
      }
    });

    this.elements.originSelect?.addEventListener('change', (event) => {
      const select = event.target as HTMLSelectElement;
      this.handleOriginChange(parseInt(select.value || '0', 10));
    });

    this.elements.inlineTargetBtn?.addEventListener('click', () => this.handleInlineTargeting());
    this.elements.inlineTargetInput?.addEventListener('keydown', (event: KeyboardEvent) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        this.handleInlineTargeting();
      }
    });

    this.elements.phalanxScanBtn?.addEventListener('click', () => this.handlePhalanxScan());
    this.elements.closePhalanxModal?.addEventListener('click', () => this.togglePhalanxModal(false));
    this.elements.phalanxModal?.addEventListener('click', (event) => {
      if (event.target === this.elements.phalanxModal) {
        this.togglePhalanxModal(false);
      }
    });
  }

  bootstrapFromGlobals() {
    if (window.currentPlanet) {
      this.applyOriginPlanet(window.currentPlanet);
      this.currentGalaxy = window.currentPlanet.galaxy || 1;
      this.currentSystem = window.currentPlanet.system || 1;
    }

    if (this.elements.galaxyInput) {
      (this.elements.galaxyInput as HTMLInputElement).value = String(this.currentGalaxy);
    }
    if (this.elements.systemInput) {
      (this.elements.systemInput as HTMLInputElement).value = String(this.currentSystem);
    }
  }

  async loadOwnedPlanets() {
    try {
      const planets = await api.get('/planets');
      this.ownedPlanets = Array.isArray(planets) ? planets : [];
      this.populateOriginSelect();
    } catch (error) {
      console.warn('Failed to load owned planets', error);
    }
  }

  populateOriginSelect() {
    const select = this.elements.originSelect as HTMLSelectElement | null;
    if (!select) return;

    if (this.ownedPlanets.length === 0) {
      select.innerHTML = `<option value="">${i18n.t('galaxy.noPlanets', { defaultValue: 'No planets available' })}</option>`;
      return;
    }

    select.innerHTML =
      `${i18n.t('galaxy.selectPlanet', { defaultValue: 'Select a planet…' })}` +
      this.ownedPlanets
      .map(
        (planet) =>
          `<option value="${planet.id}">
            ${planet.name || i18n.t('galaxy.unnamed', { defaultValue: 'Unnamed' })} [${planet.galaxy}:${planet.system}:${planet.position}]
          </option>`
      )
      .join('');


    if (this.originPlanetId) {
      select.value = String(this.originPlanetId);
    } else if (this.ownedPlanets.length > 0) {
      this.applyOriginPlanet(this.ownedPlanets[0]);
    }
  }

  handleOriginChange(planetId: number) {
    if (!planetId) {
      this.originPlanetId = null;
      this.originPlanet = null;
      this.availableShips = {};
      this.renderOriginSummary();
      return;
    }

    const planet = this.ownedPlanets.find((p) => p.id === planetId);
    if (planet) {
      this.applyOriginPlanet(planet);
      return;
    }

    this.fetchAndApplyOrigin(planetId);
  }

  async fetchAndApplyOrigin(planetId: number) {
    try {
      const response = await api.get(`/planets/${planetId}`);
      if (response?.planet) {
        this.syncOwnedPlanet(response.planet);
        this.applyOriginPlanet(response.planet);
      }
    } catch (error) {
      console.warn('Failed to load origin planet', error);
      this.showToast(i18n.t('galaxy.errors.unableToLoadPlanetData', { defaultValue: 'Unable to load planet data.' }), 'error');
    }
  }

  syncOwnedPlanet(planet: any) {
    if (!planet) return;
    const index = this.ownedPlanets.findIndex((p) => p.id === planet.id);
    if (index >= 0) {
      this.ownedPlanets[index] = planet;
    } else {
      this.ownedPlanets.push(planet);
      this.populateOriginSelect();
    }
  }

  subscribeToSocket() {
    if (!window.socket) return;
    window.socket.on('galaxyUpdate', (payload) => {
      if (
        payload?.galaxy === this.currentGalaxy &&
        payload?.system === this.currentSystem
      ) {
        this.scanSystem({ silent: true });
      }
    });
  }

  async loadResearch() {
    try {
      const response = await api.get('/research');
      const technologies = response?.technologies || [];
      technologies.forEach((tech) => {
        if (tech?.type && typeof tech.level === 'number') {
          this.intel[tech.type] = tech.level;
          if (tech.type === 'espionage_technology') {
            this.intel.espionageLevel = tech.level;
          }
        }
      });
    } catch (error) {
      console.warn('Failed to load research overview', error);
    }
  }

  updateFromInputs() {
    const galaxyValue = parseInt(
      (this.elements.galaxyInput as HTMLInputElement)?.value || '1',
      10
    );
    const systemValue = parseInt(
      (this.elements.systemInput as HTMLInputElement)?.value || '1',
      10
    );

    this.currentGalaxy = this.clamp(galaxyValue, 1, this.limits.galaxies);
    this.currentSystem = this.clamp(systemValue, 1, this.limits.systems);
    this.scanSystem();
  }

  clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  shiftSystem(delta: number) {
    this.currentSystem = this.clamp(
      this.currentSystem + delta,
      1,
      this.limits.systems
    );
    this.updateInputs();
    this.scanSystem();
  }

  shiftGalaxy(delta: number) {
    this.currentGalaxy = this.clamp(
      this.currentGalaxy + delta,
      1,
      this.limits.galaxies
    );
    this.currentSystem = 1;
    this.updateInputs();
    this.scanSystem();
  }

  updateInputs() {
    if (this.elements.galaxyInput) {
      (this.elements.galaxyInput as HTMLInputElement).value = String(
        this.currentGalaxy
      );
    }
    if (this.elements.systemInput) {
      (this.elements.systemInput as HTMLInputElement).value = String(
        this.currentSystem
      );
    }
  }

  handleJump() {
    const value = (this.elements.jumpInput as HTMLInputElement)?.value || '';
    const [galaxy, system, position] = value.split(':').map((part) => parseInt(part, 10));

    if (Number.isFinite(galaxy)) {
      this.currentGalaxy = this.clamp(galaxy, 1, this.limits.galaxies);
    }
    if (Number.isFinite(system)) {
      this.currentSystem = this.clamp(system, 1, this.limits.systems);
    }
    if (Number.isFinite(position)) {
      this.focusPosition = position;
    }

    this.updateInputs();
    this.scanSystem();
  }

  async scanSystem(options: { silent?: boolean } = {}) {
    if (this.isLoading) return;
    this.isLoading = true;
    if (!options.silent) {
      this.setLoadingState(true);
    }

    try {
      const query = new URLSearchParams({
        galaxy: String(this.currentGalaxy),
        system: String(this.currentSystem),
      });
      if (this.originPlanetId) {
        query.append('originPlanetId', String(this.originPlanetId));
      }

      const response = await api.get(`/galaxy?${query.toString()}`);
      this.handleScanResponse(response);
    } catch (error) {
      console.error('Failed to scan galaxy', error);
      this.showToast(i18n.t('galaxy.errors.failedToScan', { defaultValue: 'Failed to scan system' }), 'error');
    } finally {
      this.isLoading = false;
      this.setLoadingState(false);
    }
  }

  handleScanResponse(response: any) {
    if (!response) return;

    const { coordinates, pagination, intel, planets } = response;
    this.planets = planets || [];
    if (coordinates) {
      this.currentGalaxy = coordinates.galaxy;
      this.currentSystem = coordinates.system;
    }

    if (pagination) {
      this.limits = {
        galaxies: pagination.galaxyCount || this.limits.galaxies,
        systems: pagination.systemsPerGalaxy || this.limits.systems,
        positions: pagination.positionsPerSystem || this.limits.positions,
      };
      this.updatePaginationSummary(pagination);
    }

    if (intel) {
      this.intel.sensorRange = intel.sensorRange;
      this.intel.espionageLevel = intel.espionageLevel;
      if (intel.sensorSources) {
        this.intel.sensorSources = intel.sensorSources;
      }
      if (intel.originPlanetName) {
        this.intel.originPlanetName = intel.originPlanetName;
      }
      this.updateIntelSummary();
    }

    this.renderSystem();
    this.updateInputs();
  }

  setLoadingState(state: boolean) {
    if (!this.elements.systemDisplay) return;
    if (state) {
      this.elements.systemDisplay.innerHTML = `
        <div class="loading">
          <div class="spinner"></div>
          <p>${i18n.t('galaxy.scanning', { defaultValue: 'Scanning system…' })}</p>
        </div>
      `;
    }
  }

  renderSystem() {
    if (!this.elements.systemDisplay) return;
    this.elements.systemDisplay.innerHTML = '';

    this.planets.forEach((slot) => {
      const row = this.renderSlotRow(slot);
      this.elements.systemDisplay?.appendChild(row);
    });

    if (this.planets.length === 0) {
      this.elements.systemDisplay.innerHTML = `
        <div class="empty-state card-compact">${i18n.t('galaxy.noIntel', { defaultValue: 'No intel for this system.' })}</div>
      `;
    }

    this.canvasRenderer?.setSlots(this.planets);

    if (this.focusPosition) {
      this.highlightRow(this.focusPosition);
      this.focusPosition = null;
    }

    this.consumePendingTarget();
  }

  renderOriginSummary() {
    const summary = this.elements.originSummary;
    if (!summary) return;

    if (!this.originPlanet) {
      summary.textContent = i18n.t('galaxy.selectOrigin', { defaultValue: 'Select an origin planet to enable missions.' });
      const shipSummary = this.elements.shipSummary;
      if (shipSummary) {
        shipSummary.innerHTML = `<span class="text-muted">${i18n.t('galaxy.noHangarData', { defaultValue: 'No hangar data yet.' })}</span>`;
      }
      this.updateOriginLabel();
      return;
    }

    summary.innerHTML = `
      <strong>${this.originPlanet.name || i18n.t('galaxy.unnamedWorld', { defaultValue: 'Unnamed World' })}</strong>
      <span class="coords">[${this.originPlanet.galaxy}:${this.originPlanet.system}:${this.originPlanet.position}]</span>
    `;

    this.updateShipSummary();
    this.updateOriginLabel();
  }

  updateShipSummary() {
    const container = this.elements.shipSummary;
    if (!container) return;

    const prioritizedShips = [
      'small_cargo',
      'large_cargo',
      'light_fighter',
      'heavy_fighter',
      'cruiser',
      'battleship',
      'bomber',
      'destroyer',
      'colony_ship',
      'recycler',
      'espionage_probe',
    ];

    const entries = prioritizedShips
      .map((ship) => ({
        ship,
        count: this.availableShips[ship] || 0,
      }))
      .filter((entry) => entry.count > 0)
      .slice(0, 6);

    if (entries.length === 0) {
      container.innerHTML = `<span class="text-muted">${i18n.t('galaxy.hangarEmpty', { defaultValue: 'Hangar is empty.' })}</span>`;
      return;
    }

container.innerHTML = entries
      .map(
        (entry) =>
          `<span class="ship-pill">${getShipLabel(entry.ship)}: ${this.formatNumber(entry.count)}</span>`
      )
      .join('');
  }

  updateOriginLabel() {
    const label = this.elements.originPlanetLabel;
    if (!label) return;

    if (!this.originPlanet) {
      label.textContent = i18n.t('galaxy.originNone', { defaultValue: 'Origin: —' });
      return;
    }

    label.textContent = i18n.t('galaxy.originLabel', { defaultValue: `Origin: ${this.originPlanet.name || 'Unnamed'} [${this.originPlanet.galaxy}:${this.originPlanet.system}:${this.originPlanet.position}]`, name: this.originPlanet.name || 'Unnamed', galaxy: this.originPlanet.galaxy, system: this.originPlanet.system, position: this.originPlanet.position });
  }

  renderSlotRow(slot: any) {
    const row = document.createElement('div');
    row.className = 'galaxy-row';
    row.dataset.position = String(slot.position);

    const ownerName = slot.owner?.username || (slot.hasPlanet ? i18n.t('galaxy.unknown', { defaultValue: 'Unknown' }) : i18n.t('galaxy.dash', { defaultValue: '—' }));
    const allianceTag = slot.owner?.alliance?.tag
      ? `[${slot.owner.alliance.tag}]`
      : '';
    const activityLabel = slot.owner?.activity?.label || 'unknown';
    const moonBadge = slot.moon
      ? `<span class="moon-pill" title="${i18n.t('galaxy.moonDiameterTitle', { defaultValue: 'Diameter {km} km', km: this.formatNumber(slot.moon.diameter) }).replace('{km}', this.formatNumber(slot.moon.diameter))}">${i18n.t('galaxy.moon', { defaultValue: 'Moon' })}</span>`
      : '';
    const relationBadge = this.renderRelationBadge(slot.owner?.relation);

    const actions = this.renderActionButtons(slot);
    const intelTagClass = `intel-tag intel-${slot.intelQuality}`;

    row.innerHTML = `
      <div class="slot-position">
        <strong>${slot.position}</strong>
        <span class="${intelTagClass}">${slot.intelQuality}</span>
      </div>
      <div class="slot-planet">
        ${slot.hasPlanet ? slot.planet?.name || i18n.t('galaxy.unknownWorld', { defaultValue: 'Unknown World' }) : `<span class="slot-empty">${i18n.t('galaxy.vacantOrbit', { defaultValue: 'Vacant Orbit' })}</span>`}
        <div class="coords">${this.currentGalaxy}:${this.currentSystem}:${slot.position}</div>
        ${moonBadge}
        <div class="slot-markers">${this.renderMarkers(slot)}</div>
      </div>
      <div class="slot-owner">
        <strong>${ownerName}</strong>
        ${relationBadge}
        ${slot.hasPlanet ? `<span>${i18n.t('galaxy.intelLabel', { defaultValue: 'Intel:' })} ${slot.intelQuality}</span>` : ''}
      </div>
      <div class="slot-alliance">
        ${allianceTag ? `<span class="alliance-tag">${allianceTag}</span>` : `<span>${i18n.t('galaxy.dash', { defaultValue: '—' })}</span>`}
        <span class="activity-indicator">
          <span class="activity-pill activity-${activityLabel}">${activityLabel}</span>
          ${slot.owner?.activity?.minutesSince !== null ? `${slot.owner.activity.minutesSince}${i18n.t('galaxy.minutesSuffix', { defaultValue: 'm ago' })}` : ''}
        </span>
      </div>
      <div class="slot-debris">
        ${slot.debris ? `<strong>${this.formatNumber(slot.debris.metal)} M / ${this.formatNumber(slot.debris.crystal)} C</strong>` : i18n.t('galaxy.dash', { defaultValue: '—' })}
      </div>
      <div class="slot-actions">${actions}</div>
    `;

    return row;
  }

  renderRelationBadge(relation?: 'self' | 'ally' | 'neutral') {
    if (!relation || relation === 'neutral') return '';
    const label = relation === 'self' ? i18n.t('galaxy.relation.self', { defaultValue: 'Self' }) : i18n.t('galaxy.relation.ally', { defaultValue: 'Ally' });
    return `<span class="relation-badge relation-${relation}">${label}</span>`;
  }

  renderMarkers(slot: any) {
    const badges: string[] = [];

    if (!slot.hasPlanet && slot.markers?.canColonize) {
      badges.push(`<span class="marker-badge colonize">${i18n.t('galaxy.marker.colonize', { defaultValue: 'Colonize' })}</span>`);
    }

    if (slot.debris) {
      badges.push(`<span class="marker-badge debris">${i18n.t('galaxy.marker.debris', { defaultValue: 'Debris' })}</span>`);
    }

    return badges.join('');
  }

  renderActionButtons(slot: any) {
    const buttons: string[] = [];
    const canTarget = Boolean(this.originPlanetId);

    if (slot.hasPlanet) {
      if (slot.owner?.relation === 'self') {
        buttons.push(this.buildActionButton(i18n.t('galaxy.action.deploy', { defaultValue: 'Deploy' }), 'deploy', slot.position, canTarget));
        buttons.push(this.buildActionButton(i18n.t('galaxy.action.transport', { defaultValue: 'Transport' }), 'transport', slot.position, canTarget));
      } else if (slot.owner?.relation === 'ally') {
        buttons.push(this.buildActionButton(i18n.t('galaxy.action.transport', { defaultValue: 'Transport' }), 'transport', slot.position, canTarget));
        buttons.push(
          this.buildActionButton(
            i18n.t('galaxy.action.espionage', { defaultValue: 'Espionage' }),
            'espionage',
            slot.position,
            canTarget && this.intel.espionageLevel > 0
          )
        );
      } else {
        buttons.push(this.buildActionButton(i18n.t('galaxy.action.attack', { defaultValue: 'Attack' }), 'attack', slot.position, canTarget));
        buttons.push(
          this.buildActionButton(i18n.t('galaxy.action.transport', { defaultValue: 'Transport' }), 'transport', slot.position, canTarget)
        );
        buttons.push(
          this.buildActionButton(
            i18n.t('galaxy.action.espionage', { defaultValue: 'Espionage' }),
            'espionage',
            slot.position,
            canTarget && this.intel.espionageLevel > 0
          )
        );
      }
    } else if (!slot.hasPlanet) {
      buttons.push(
        this.buildActionButton(
          i18n.t('galaxy.action.colonize', { defaultValue: 'Colonize' }),
          'colonize',
          slot.position,
          canTarget && this.availableShips.colony_ship > 0
        )
      );
    }

    if (slot.debris) {
      buttons.push(
        this.buildActionButton(
          i18n.t('galaxy.action.harvest', { defaultValue: 'Harvest' }),
          'harvest',
          slot.position,
          canTarget && this.availableShips.recycler > 0
        )
      );
    }

    buttons.push(
      `<button class="btn-tertiary" data-action="details" data-position="${slot.position}">${i18n.t('galaxy.details', { defaultValue: 'Details' })}</button>`
    );

    return buttons.join('');
  }

  buildActionButton(label: string, action: MissionType, position: number, enabled: boolean) {
    return `
      <button class="btn-small ${enabled ? 'btn-primary' : 'btn-disabled'}"
        data-action="${action}"
        data-position="${position}"
        ${enabled ? '' : 'disabled'}>
        ${label}
      </button>
    `;
  }

  async handleInlineTargeting() {
    const input = (this.elements.inlineTargetInput as HTMLInputElement | null)?.value || '';
    const mission = ((this.elements.inlineTargetMission as HTMLSelectElement | null)?.value || 'attack') as MissionType;
    const coords = this.parseCoordinates(input);

    if (!coords) {
      this.showToast(i18n.t('galaxy.errors.enterCoords', { defaultValue: 'Enter coordinates as G:S:P' }), 'error');
      return;
    }

    this.pendingTarget = {
      galaxy: coords.galaxy,
      system: coords.system,
      position: coords.position,
      mission,
    };

    if (coords.galaxy === this.currentGalaxy && coords.system === this.currentSystem) {
      this.consumePendingTarget(true);
      return;
    }

    this.currentGalaxy = this.clamp(coords.galaxy, 1, this.limits.galaxies);
    this.currentSystem = this.clamp(coords.system, 1, this.limits.systems);
    this.updateInputs();
    await this.scanSystem();
  }

  parseCoordinates(input: string) {
    if (!input) return null;
    const parts = input.split(':').map((segment) => parseInt(segment.trim(), 10));
    if (parts.length < 2 || parts.some((value) => Number.isNaN(value))) {
      return null;
    }

    return {
      galaxy: parts[0],
      system: parts[1],
      position: parts[2] && Number.isFinite(parts[2]) ? parts[2] : 1,
    };
  }

  consumePendingTarget(force = false) {
    if (!this.pendingTarget) return;
    if (
      !force &&
      (this.pendingTarget.galaxy !== this.currentGalaxy || this.pendingTarget.system !== this.currentSystem)
    ) {
      return;
    }

    const pending = this.pendingTarget;
    this.focusPosition = pending.position;
    const slot = this.planets.find((p) => p.position === pending.position);

    if (slot) {
      this.openMissionDrawer(slot, pending.mission);
    } else {
      this.showToast(i18n.t('galaxy.noIntelOrbit', { defaultValue: 'No intel for that orbit yet.' }), 'info');
    }

    this.pendingTarget = null;
  }

  handleCanvasSelection(position: number) {
    this.focusPosition = position;
    const slot = this.planets.find((p) => p.position === position);
    this.highlightRow(position);
    if (slot) {
      this.openPlanetDetails(slot);
    } else {
      this.showToast(i18n.t('galaxy.orbitEmpty', { defaultValue: `Orbit ${position} is currently empty.`, position }), 'info');
    }
  }

  highlightRow(position: number) {
    if (!this.elements.systemDisplay) return;
    this.elements.systemDisplay
      .querySelectorAll('.galaxy-row')
      .forEach((row) => row.classList.remove('highlight'));

    const targetRow = this.elements.systemDisplay.querySelector<HTMLDivElement>(
      `.galaxy-row[data-position="${position}"]`
    );

    if (targetRow) {
      targetRow.classList.add('highlight');
      targetRow.scrollIntoView({ behavior: 'smooth', block: 'center' });
      setTimeout(() => targetRow.classList.remove('highlight'), 1500);
    }

    this.canvasRenderer?.setFocusPosition(position);
  }

  updatePaginationSummary(pagination: any) {
    if (this.elements.galaxySummary) {
      this.elements.galaxySummary.textContent = i18n.t('galaxy.summary.galaxySystem', { defaultValue: `Galaxy ${this.currentGalaxy} • System ${this.currentSystem}`, galaxy: this.currentGalaxy, system: this.currentSystem });
    }
    if (this.elements.paginationSummary) {
      this.elements.paginationSummary.textContent = i18n.t('galaxy.summary.systemOf', { defaultValue: `System ${this.currentSystem} of ${pagination.systemsPerGalaxy}`, system: this.currentSystem, total: pagination.systemsPerGalaxy });
    }
  }

  updateIntelSummary() {
    if (!this.elements.sensorSummary) return;
    const phalanx = this.intel.sensorSources?.phalanx || 0;
    const sensorArray = this.intel.sensorSources?.sensorArray || 0;
    this.elements.sensorSummary.textContent = i18n.t('galaxy.intelSummary', { defaultValue: `±${this.intel.sensorRange} systems • Esp ${this.intel.espionageLevel} • Phalanx ${phalanx} • Array ${sensorArray}`, range: this.intel.sensorRange, espionage: this.intel.espionageLevel, phalanx, array: sensorArray });
    this.updateOriginLabel();
  }

  openMissionDrawer(slot: any, mission: MissionType) {
    if (
      (mission === 'colonize' && slot.hasPlanet) ||
      (mission === 'espionage' && !slot.hasPlanet)
    ) {
      return;
    }

    this.drawerState = {
      open: true,
      mission,
      target: slot,
    };

    if (this.elements.missionDrawerTitle) {
      this.elements.missionDrawerTitle.textContent = i18n.t('galaxy.modal.sendMission', { defaultValue: `Send ${mission.toUpperCase()} Mission`, mission: mission.toUpperCase() });
    }

    if (this.elements.missionTargetSummary) {
      this.elements.missionTargetSummary.textContent = i18n.t('galaxy.modal.target', { defaultValue: `Target: ${this.currentGalaxy}:${this.currentSystem}:${slot.position}`, coords: `${this.currentGalaxy}:${this.currentSystem}:${slot.position}` });
    }

    if (this.elements.missionTypeSelect) {
      (this.elements.missionTypeSelect as HTMLSelectElement).value = mission;
    }

    this.renderMissionForm();
    this.elements.missionDrawer?.classList.remove('hidden');
  }

  closeMissionDrawer() {
    this.drawerState.open = false;
    this.drawerState.target = null;
    this.elements.missionDrawer?.classList.add('hidden');
    this.hideDrawerError();
  }

  renderMissionForm() {
    if (!this.elements.shipInputs) return;
    const ships = MISSION_SHIPS[this.drawerState.mission] || [];
    this.elements.shipInputs.innerHTML = '';

    ships.forEach((ship) => {
      const available = this.availableShips[ship] || 0;
      const defaultValue = this.getDefaultShipValue(ship, available);
      const wrapper = document.createElement('div');
      wrapper.className = 'ship-input';
wrapper.innerHTML = `
        <label>${getShipLabel(ship)}</label>
        <input type="number"
          min="0"
          max="${available}"
          value="${defaultValue}"
          data-ship="${ship}">
        <span class="available">Available: ${this.formatNumber(available)}</span>
      `;
      this.elements.shipInputs?.appendChild(wrapper);
    });
  }

  getDefaultShipValue(ship: string, available: number) {
    if (available === 0) return 0;
    if (ship === 'espionage_probe') return Math.min(1, available);
    if (ship === 'colony_ship') return Math.min(1, available);
    if (ship === 'recycler') return Math.min(available, 5);
    return 0;
  }

  gatherShipsFromForm() {
    const inputs = this.elements.shipInputs?.querySelectorAll<HTMLInputElement>('input[data-ship]');
    const ships: Record<string, number> = {};
    inputs?.forEach((input) => {
      const value = parseInt(input.value || '0', 10);
      if (value > 0) {
        ships[input.dataset.ship!] = value;
      }
    });
    return ships;
  }

  async dispatchMission() {
    if (!this.originPlanetId || !this.drawerState.target) {
      this.showDrawerError(i18n.t('galaxy.errors.selectOriginBeforeLaunch', { defaultValue: 'Select an origin planet before launching.' }));
      return;
    }

    const ships = this.gatherShipsFromForm();
    if (Object.keys(ships).length === 0) {
      this.showDrawerError(i18n.t('galaxy.errors.selectAtLeastOneShip', { defaultValue: 'Select at least one ship.' }));
      return;
    }

    const payload = {
      originPlanetId: this.originPlanetId,
      targetGalaxy: this.currentGalaxy,
      targetSystem: this.currentSystem,
      targetPosition: this.drawerState.target.position,
      missionType: this.drawerState.mission,
      ships,
      cargo: { metal: 0, crystal: 0, deuterium: 0 },
    };

    try {
      await api.post('/fleet/dispatch', payload);
      this.showToast(i18n.t('galaxy.toasts.fleetDispatched', { defaultValue: 'Fleet dispatched' }), 'success');
      this.closeMissionDrawer();
    } catch (error: any) {
      const message = error?.response?.data?.error || 'Failed to dispatch fleet';
      this.showDrawerError(i18n.t('galaxy.errors.dispatchFailed', { defaultValue: message }));
    }
  }

  showDrawerError(message: string) {
    if (!this.elements.missionDrawerError) return;
    this.elements.missionDrawerError.textContent = message;
    this.elements.missionDrawerError.classList.remove('hidden');
  }

  hideDrawerError() {
    if (!this.elements.missionDrawerError) return;
    this.elements.missionDrawerError.classList.add('hidden');
  }

  openPlanetDetails(slot: any) {
    if (!this.elements.planetModal || !this.elements.planetModalContent) return;

      const owner = slot.owner?.username || i18n.t('galaxy.unknownCommander', { defaultValue: 'Unknown Commander' });
      const alliance = slot.owner?.alliance?.tag
      ? `[${slot.owner.alliance.tag}] ${slot.owner.alliance.name || ''}`
      : i18n.t('galaxy.dash', { defaultValue: '—' });
      const activity = slot.owner?.activity?.label || i18n.t('galaxy.unknown', { defaultValue: 'unknown' });
      const relation = slot.owner?.relation || i18n.t('galaxy.unknown', { defaultValue: 'unknown' });
      const lastSeen = slot.owner?.lastSeen
      ? new Date(slot.owner.lastSeen).toLocaleString()
      : i18n.t('galaxy.unknown', { defaultValue: 'Unknown' });

    this.elements.planetModalContent.innerHTML = `
      <p><strong>${i18n.t('galaxy.modal.coordinates', { defaultValue: 'Coordinates:' })}</strong> ${this.currentGalaxy}:${this.currentSystem}:${slot.position}</p>
      <p><strong>${i18n.t('galaxy.modal.planet', { defaultValue: 'Planet:' })}</strong> ${slot.planet?.name || i18n.t('galaxy.unknown', { defaultValue: 'Unknown' })}</p>
      <p><strong>${i18n.t('galaxy.modal.owner', { defaultValue: 'Owner:' })}</strong> ${owner}</p>
      <p><strong>${i18n.t('galaxy.modal.alliance', { defaultValue: 'Alliance:' })}</strong> ${alliance}</p>
      <p><strong>${i18n.t('galaxy.modal.activity', { defaultValue: 'Activity:' })}</strong> ${activity}</p>
      <p><strong>${i18n.t('galaxy.modal.relation', { defaultValue: 'Relation:' })}</strong> ${relation}</p>
      <p><strong>${i18n.t('galaxy.modal.lastSeen', { defaultValue: 'Last Seen:' })}</strong> ${lastSeen}</p>
      <p><strong>${i18n.t('galaxy.modal.intelQuality', { defaultValue: 'Intel Quality:' })}</strong> ${slot.intelQuality}</p>
      ${
        slot.debris
          ? `<p><strong>${i18n.t('galaxy.modal.debris', { defaultValue: 'Debris:' })}</strong> ${this.formatNumber(slot.debris.metal)} ${i18n.t('galaxy.modal.metal', { defaultValue: 'Metal' })} / ${this.formatNumber(
              slot.debris.crystal
            )} ${i18n.t('galaxy.modal.crystal', { defaultValue: 'Crystal' })}</p>`
          : ''
      }
    `;

    this.elements.planetModal.style.display = 'block';
  }

  hidePlanetModal() {
    if (this.elements.planetModal) {
      this.elements.planetModal.style.display = 'none';
    }
  }

  showToast(message: string, type: 'success' | 'error' | 'info' = 'success') {
    if (!this.elements.toast) return;
    this.elements.toast.textContent = message;
    this.elements.toast.className = `toast show ${type}`;
    setTimeout(() => {
      this.elements.toast?.classList.remove('show');
    }, 2800);
  }

  applyOriginPlanet(planet: any) {
    if (!planet) return;
    this.originPlanetId = planet.id;
    this.originPlanet = planet;
    this.availableShips = this.extractShips(planet);
    this.intel.originPlanetName = planet.name || null;

    const select = this.elements.originSelect as HTMLSelectElement | null;
    if (select) {
      select.value = String(planet.id);
    }

    this.renderOriginSummary();
    this.scanSystem({ silent: true });
    if (!this.originMoon || this.originMoon.planet_id !== planet.id) {
      this.loadOriginMoon(planet.id);
    }
  }

extractShips(planet: any) {
    const ships: Record<string, number> = {};
    const allShips = [
      'small_cargo',
      'large_cargo',
      'light_fighter',
      'heavy_fighter',
      'cruiser',
      'battleship',
      'bomber',
      'destroyer',
      'colony_ship',
      'recycler',
      'espionage_probe',
    ];
    allShips.forEach((ship) => {
      ships[ship] = planet[ship] || 0;
    });
    return ships;
  }

  handlePageData(data: any) {
    if (data?.moonData) {
      this.setOriginMoon(data.moonData?.moon || null);
    }
    if (data?.planet) {
      this.syncOwnedPlanet(data.planet);
      this.applyOriginPlanet(data.planet);
    }
  }

  formatNumber(value: number) {
    return new Intl.NumberFormat().format(Math.floor(value || 0));
  }

  formatCountdown(seconds: number | null) {
    if (seconds == null) return '—';
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    if (hrs > 0) return `${hrs}h ${mins}m ${secs}s`;
    if (mins > 0) return `${mins}m ${secs}s`;
    return `${secs}s`;
  }

getMissionLabel(mission: string) {
    // fallback for display in places where i18n key might be missing
    switch (mission) {
      case 'attack':
        return 'Attack';
      case 'transport':
        return 'Transport';
      case 'deploy':
        return 'Deploy';
      case 'colonize':
        return 'Colonize';
      case 'harvest':
        return 'Harvest';
      case 'espionage':
        return 'Espionage';
      default:
        return mission;
    }
  }

  setOriginMoon(moon: any | null) {
    this.originMoon = moon;
    this.updatePhalanxState();
  }

  async loadOriginMoon(planetId: number) {
    try {
      const response = await api.get(`/moons/${planetId}`);
      const moon = response?.data?.moon || response?.data || null;
      this.setOriginMoon(moon);
    } catch (error) {
      this.setOriginMoon(null);
    }
  }

  updatePhalanxState() {
    const button = this.elements.phalanxScanBtn as HTMLButtonElement | null;
    const status = this.elements.phalanxStatus;
    if (!button || !status) return;

    if (!this.originMoon || (this.originMoon.sensor_phalanx || 0) === 0) {
      button.disabled = true;
      status.textContent = i18n.t('galaxy.phalanx.requiresMoon', { defaultValue: 'Requires a moon with Sensor Phalanx.' });
      return;
    }

    const level = this.originMoon.sensor_phalanx || 0;
    const range = Math.max(0, level * level - 1);
    button.disabled = this.isPhalanxLoading;
    status.textContent = i18n.t('galaxy.phalanx.status', { defaultValue: `Level ${level} • Range ±${range} systems • Cost ${this.formatNumber(
      PHALANX_SCAN_COST
    )} deut.`, level, range, cost: this.formatNumber(PHALANX_SCAN_COST) });
  }

  async handlePhalanxScan() {
    if (!this.originMoon || (this.originMoon.sensor_phalanx || 0) === 0) {
      this.showToast(i18n.t('galaxy.phalanx.noPhalanx', { defaultValue: 'This planet has no Sensor Phalanx.' }), 'error');
      return;
    }

    const coords =
      this.parseCoordinates(
        ((this.elements.inlineTargetInput as HTMLInputElement | null)?.value || '').trim()
      ) || {
        galaxy: this.currentGalaxy,
        system: this.currentSystem,
        position: this.focusPosition || 1,
      };

    try {
      this.setPhalanxLoading(true);
      const response = await api.post(`/moons/${this.originMoon.id}/phalanx`, {
        targetGalaxy: coords.galaxy,
        targetSystem: coords.system,
        targetPosition: coords.position,
      });
      const result = response?.data || response;
      this.renderPhalanxResults(result);
      this.togglePhalanxModal(true);
      this.showToast(i18n.t('galaxy.phalanx.scanComplete', { defaultValue: 'Sensor scan complete.' }), 'success');
    } catch (error: any) {
      const message =
        error?.response?.data?.error || error?.message || i18n.t('galaxy.phalanx.scanFailed', { defaultValue: 'Sensor Phalanx scan failed' });
      this.showToast(message, 'error');
    } finally {
      this.setPhalanxLoading(false);
    }
  }

  setPhalanxLoading(state: boolean) {
    this.isPhalanxLoading = state;
    const button = this.elements.phalanxScanBtn as HTMLButtonElement | null;
    if (button) {
      button.disabled = state || !this.originMoon || (this.originMoon.sensor_phalanx || 0) === 0;
    }
  }

  renderPhalanxResults(result: any) {
    const container = this.elements.phalanxResults;
    if (!container) return;

    const target = result?.target || {};
    const inbound = result?.fleets?.inbound || [];
    const outbound = result?.fleets?.outbound || [];
    const targetLabel = `${target.galaxy}:${target.system}:${target.position}`;

    const buildFleetList = (fleets: any[], emptyText: string) => {
      if (!fleets.length) {
        return `<p class="text-muted">${emptyText}</p>`;
      }
      return fleets.map((fleet) => this.renderPhalanxFleet(fleet)).join('');
    };

    container.innerHTML = `
      <p class="text-muted">
        Target ${targetLabel}
        ${target.planetName ? `• ${target.planetName}` : ''}
      </p>
      <div class="phalanx-section">
        <h4>Inbound Fleets</h4>
        ${buildFleetList(inbound, 'No inbound fleets detected.')}
      </div>
      <div class="phalanx-section">
        <h4>Outbound Fleets</h4>
        ${buildFleetList(outbound, 'No outbound fleets detected.')}
      </div>
    `;
  }

  renderPhalanxFleet(fleet: any) {
    const eta = this.formatCountdown(fleet.etaSeconds ?? null);
    const origin = fleet.origin
      ? `[${fleet.origin.galaxy}:${fleet.origin.system}:${fleet.origin.position}]`
      : i18n.t('galaxy.unknownOrigin', { defaultValue: 'Unknown origin' });
    const mission = i18n.t(`galaxy.action.${fleet.mission}`, { defaultValue: this.getMissionLabel(fleet.mission) });
    return `
      <div class="phalanx-fleet">
        <div>
          <strong>${fleet.owner || i18n.t('galaxy.unknownCommander', { defaultValue: 'Unknown Commander' })}</strong> · ${mission}
          <div class="text-muted">${origin}</div>
        </div>
        <div>${eta}</div>
      </div>
    `;
  }

  togglePhalanxModal(show: boolean) {
    const modal = this.elements.phalanxModal as HTMLElement | null;
    if (!modal) return;
    modal.style.display = show ? 'block' : 'none';
  }
}

let galaxyController: GalaxyController | null = null;

document.addEventListener('DOMContentLoaded', () => {
  galaxyController = new GalaxyController();
  window.updatePageData = (data) => galaxyController?.handlePageData(data);
});
